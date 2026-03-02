use std::str::FromStr;

use anyhow::{anyhow, Error};
use clap::{
    builder::{
        styling::{AnsiColor, Effects},
        Styles,
    },
    Parser, Subcommand, ValueEnum,
};
use colored::Colorize;

mod account;
mod config;
mod connect;
mod fmt;
mod query;
mod schema;
mod upload;
mod version;
mod whoami;

/// Exit codes for structured error handling (useful for scripts and CI).
pub mod exit_codes {
    /// Successful execution.
    pub const SUCCESS: i32 = 0;
    /// General error.
    pub const ERROR: i32 = 1;
    /// Authentication failure.
    pub const AUTH_ERROR: i32 = 2;
    /// No connection configured.
    pub const NOT_CONFIGURED: i32 = 3;
}

#[derive(Parser)]
#[clap(
    name = "logsh",
    author = "logship.llc",
    about = "The Logship CLI — query, upload, and manage your Logship services.",
    long_about = "logsh is the official CLI for Logship.\n\n\
        Use it to query log data, upload CSV/TSV files, manage connections,\n\
        and administer accounts. Supports multiple output formats (table, JSON,\n\
        CSV, markdown) for both human and machine consumption.\n\n\
        Get started:\n  \
        logsh context add basic myctx https://my.logship.server\n  \
        logsh query -q 'MyTable | take 10'\n\n\
        Environment variables:\n  \
        LOGSH_CONFIG_PATH  Override config file location\n  \
        LOGSH_PAT_TOKEN    Personal Access Token for CI/automation\n  \
        NO_COLOR           Disable color output",
    styles = styles(),
)]
#[command(arg_required_else_help = false)]
struct Args {
    #[command(subcommand)]
    command: Option<Commands>,

    #[arg(short = 'v', action = clap::ArgAction::Count, global = true, help = "Increase verbosity (-v warn, -vv info, -vvv debug, -vvvv trace)")]
    verbose: u8,

    #[arg(long, global = true, help = "Disable colored output")]
    no_color: bool,

    #[arg(
        long,
        visible_alias = "ctx",
        global = true,
        help = "Use a specific named context instead of the default"
    )]
    context: Option<String>,

    #[arg(
        long,
        global = true,
        env = "LOGSH_CONFIG_PATH",
        help = "Override the config file path"
    )]
    config_path: Option<String>,

    #[arg(
        long,
        global = true,
        help = "Override the account for this command (by name)"
    )]
    account: Option<String>,

    #[arg(long, global = true, help = "Suppress non-essential output")]
    quiet: bool,

    #[arg(short, long, global = true, value_enum, help = "Output format")]
    output: Option<OutputMode>,
}

fn styles() -> Styles {
    if !std::env::var("NO_COLOR")
        .unwrap_or_default()
        .trim()
        .is_empty()
    {
        return Styles::default();
    }

    Styles::styled()
        .header(AnsiColor::BrightBlack.on_default() | Effects::BOLD)
        .usage(AnsiColor::White.on_default())
        .literal(AnsiColor::BrightWhite.on_default() | Effects::BOLD)
        .invalid(AnsiColor::Red.on_default() | Effects::BOLD)
        .valid(AnsiColor::BrightBlue.on_default())
        .placeholder(AnsiColor::Green.on_default())
}

#[derive(Subcommand)]
enum Commands {
    /// Manage contexts (server connections).
    #[clap(subcommand)]
    Context(crate::config::ContextCommand),

    /// View and manage logsh configuration.
    #[command(subcommand)]
    Config(crate::config::ConfigCommand),

    /// Manage Logship accounts (list, set default, delete).
    #[command(subcommand)]
    Account(crate::account::AccountCommand),

    /// Execute a Kusto query against a Logship server.
    Query(crate::query::QueryCommand),

    /// Inspect schemas and tables on the connected server.
    #[command(subcommand)]
    Schema(crate::schema::SchemaCommand),

    /// Upload CSV/TSV data files to a Logship schema.
    Upload(crate::upload::UploadCommand),

    /// Show version info and manage self-updates.
    Version(crate::version::VersionCommand),

    /// Show current user and connection status.
    Whoami(crate::whoami::WhoamiCommand),

    /// Generate shell completion scripts.
    #[clap(about = "Generate shell completion scripts for bash, zsh, fish, or powershell")]
    Completions {
        /// The shell to generate completions for
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },

    /// List context names (for shell completion).
    #[clap(name = "__complete-contexts", hide = true)]
    CompleteContexts,

    /// List account names for the current context (for shell completion).
    #[clap(name = "__complete-accounts", hide = true)]
    CompleteAccounts,
}

fn main() {
    // Pre-parse: if args are exactly ["context"|"ctx"] with no subcommand, inject "current"
    let args: Vec<String> = std::env::args().collect();
    let effective_args = if args.len() == 2 && matches!(args[1].as_str(), "context" | "ctx") {
        vec![args[0].clone(), args[1].clone(), "current".to_string()]
    } else {
        args
    };

    let cli = Args::parse_from(effective_args);
    let log_level = match cli.verbose {
        0 => log::LevelFilter::Error,
        1 => log::LevelFilter::Warn,
        2 => log::LevelFilter::Info,
        3 => log::LevelFilter::Debug,
        _ => log::LevelFilter::Trace,
    };

    let no_color = !std::env::var("NO_COLOR")
        .unwrap_or_default()
        .trim()
        .is_empty();
    if no_color || cli.no_color {
        colored::control::set_override(false);
    }

    pretty_env_logger::formatted_builder()
        .filter_level(log_level)
        .init();

    if let Some(ref path) = cli.config_path {
        std::env::set_var("LOGSH_CONFIG_PATH", path);
    }

    if let Some(ref ctx) = cli.context {
        logsh_core::config::set_context_override(ctx.clone());
    }

    if let Some(ref account) = cli.account {
        logsh_core::config::set_account_override(account.clone());
    }

    let output = cli.output;
    let result = run(cli);
    match result {
        Ok(code) => std::process::exit(code),
        Err(err) => {
            match output {
                Some(OutputMode::Json | OutputMode::JsonPretty) => {
                    eprintln!("{}", serde_json::json!({"error": err.to_string()}));
                }
                _ => {
                    eprintln!("{}: {err}", "error".red().bold());
                }
            }
            std::process::exit(exit_codes::ERROR);
        }
    }
}

fn run(cli: Args) -> Result<i32, Error> {
    let output = cli.output;
    match cli.command {
        Some(Commands::Context(command)) => {
            crate::connect::execute_context(command, output)?;
            Ok(exit_codes::SUCCESS)
        }
        Some(Commands::Query(command)) => {
            crate::query::execute_query(command, output, std::io::stdout())?;
            Ok(exit_codes::SUCCESS)
        }
        Some(Commands::Schema(command)) => {
            crate::schema::execute_schema(command, output, std::io::stdout())?;
            Ok(exit_codes::SUCCESS)
        }
        Some(Commands::Upload(command)) => {
            crate::upload::execute_upload(command, output)?;
            Ok(exit_codes::SUCCESS)
        }
        Some(Commands::Version(command)) => {
            crate::version::version(std::io::stdout(), command, cli.verbose, output)?;
            Ok(exit_codes::SUCCESS)
        }
        Some(Commands::Account(command)) => {
            crate::account::execute_account(command, output)?;
            Ok(exit_codes::SUCCESS)
        }
        Some(Commands::Config(command)) => {
            crate::config::execute_config(command, output)?;
            Ok(exit_codes::SUCCESS)
        }
        Some(Commands::Whoami(command)) => {
            crate::whoami::execute_whoami(command, output)?;
            Ok(exit_codes::SUCCESS)
        }
        Some(Commands::Completions { shell }) => {
            let mut cmd = <Args as clap::CommandFactory>::command();
            clap_complete::generate(shell, &mut cmd, "logsh", &mut std::io::stdout());
            Ok(exit_codes::SUCCESS)
        }
        Some(Commands::CompleteContexts) => {
            if let Ok(cfg) = logsh_core::config::load() {
                let mut names: Vec<_> = cfg.contexts.keys().cloned().collect();
                names.sort();
                for name in names {
                    println!("{name}");
                }
            }
            Ok(exit_codes::SUCCESS)
        }
        Some(Commands::CompleteAccounts) => {
            if let Ok(cfg) = logsh_core::config::load() {
                if let Some(ctx) = cfg.get_current_context() {
                    for name in &ctx.connection.known_accounts {
                        println!("{name}");
                    }
                }
            }
            Ok(exit_codes::SUCCESS)
        }
        None => {
            let cfg = logsh_core::config::load()?;
            let conn = cfg.get_current_context();
            match conn {
                Some(conn) => match conn.connection.who_am_i() {
                    Ok(user) => {
                        let sub = conn
                            .connection
                            .effective_account()
                            .map_or("None".to_string(), |s| s.to_string());
                        match output {
                            Some(OutputMode::Json | OutputMode::JsonPretty) => {
                                fmt::print_json(&serde_json::json!({
                                    "status": "connected",
                                    "connection": conn.name,
                                    "user": user.user_name,
                                    "account": sub,
                                }));
                            }
                            _ => {
                                println!("Status: {}", "Connected".green());
                                println!(
                                    "Connection: {}  User: {}  Account: {}",
                                    &conn.name.blue(),
                                    &user.user_name.blue(),
                                    sub.blue()
                                );
                            }
                        }
                        Ok(exit_codes::SUCCESS)
                    }
                    Err(err) => {
                        fmt::print_connect_error(&cfg, &err);
                        Ok(exit_codes::AUTH_ERROR)
                    }
                },
                None => {
                    fmt::print_add_connection_help();
                    Ok(exit_codes::NOT_CONFIGURED)
                }
            }
        }
    }
}

#[derive(Copy, Clone, Debug, Default, ValueEnum)]
pub enum OutputMode {
    #[default]
    Table,
    Json,
    JsonPretty,
    Csv,
    Markdown,
}

impl FromStr for OutputMode {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "json" => Ok(OutputMode::Json),
            "json-pretty" => Ok(OutputMode::JsonPretty),
            "csv" => Ok(OutputMode::Csv),
            "markdown" => Ok(OutputMode::Markdown),
            _ => Err(anyhow!("Failed to read output format: \"{s}\"")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_output_mode_json() {
        let mode: OutputMode = "json".parse().unwrap();
        assert!(matches!(mode, OutputMode::Json));
    }

    #[test]
    fn test_output_mode_json_pretty() {
        let mode: OutputMode = "json-pretty".parse().unwrap();
        assert!(matches!(mode, OutputMode::JsonPretty));
    }

    #[test]
    fn test_output_mode_csv() {
        let mode: OutputMode = "csv".parse().unwrap();
        assert!(matches!(mode, OutputMode::Csv));
    }

    #[test]
    fn test_output_mode_markdown() {
        let mode: OutputMode = "markdown".parse().unwrap();
        assert!(matches!(mode, OutputMode::Markdown));
    }

    #[test]
    fn test_output_mode_invalid() {
        let result: Result<OutputMode, _> = "xml".parse();
        assert!(result.is_err());
    }

    #[test]
    fn test_output_mode_default() {
        let mode = OutputMode::default();
        assert!(matches!(mode, OutputMode::Table));
    }
}
