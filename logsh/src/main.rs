use std::str::FromStr;

use anyhow::{anyhow, Error};
use clap::{
    builder::{
        styling::{AnsiColor, Effects},
        Styles,
    },
    Parser, Subcommand, ValueEnum,
};
use clap::{CommandFactory, FromArgMatches};
use colored::Colorize;

mod account;
mod administration;
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
        logsh context add https://my.logship.server --name myctx\n  \
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
        help = "Use an account name or UUID for this command"
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

fn cli_command(full: bool) -> clap::Command {
    fn compact(mut cmd: clap::Command, full: bool, root: bool) -> clap::Command {
        cmd = cmd
            .disable_help_flag(true)
            .next_line_help(false)
            .term_width(80)
            .arg(
                clap::Arg::new("help")
                    .short('h')
                    .long("help")
                    .action(clap::ArgAction::HelpShort)
                    .help("Show help"),
            )
            .arg(
                clap::Arg::new("help-all")
                    .long("help-all")
                    .action(clap::ArgAction::HelpLong)
                    .help("Show all options"),
            )
            .mut_args(|mut a| {
                let advanced = matches!(
                    a.get_id().as_str(),
                    "verbose"
                        | "no_color"
                        | "config_path"
                        | "quiet"
                        | "oauth_flow"
                        | "password"
                        | "token"
                );
                let hidden = a.is_hide_set() || (!full && advanced);
                let id = a.get_id().as_str().to_owned();
                if matches!(
                    id.as_str(),
                    "sso"
                        | "pat"
                        | "username"
                        | "password"
                        | "password_stdin"
                        | "token"
                        | "token_stdin"
                        | "oauth_flow"
                ) {
                    a = a.help_heading("Authentication");
                }
                if matches!(
                    id.as_str(),
                    "context"
                        | "account"
                        | "output"
                        | "config_path"
                        | "verbose"
                        | "no_color"
                        | "quiet"
                ) {
                    a = a.help_heading("Global options");
                }
                if !full {
                    a = match id.as_str() {
                        "context" => a.help("Use a named context"),
                        "account" => a.help("Use an account name or UUID"),
                        "output" => {
                            a.help("Output format (table, json, json-pretty, csv, markdown)")
                        }
                        _ => a,
                    };
                }
                if a.get_action().takes_values() {
                    a = a.hide_possible_values(!full);
                }
                a.next_line_help(false)
                    .hide_env_values(true)
                    .hide_env(!full)
                    .hide(hidden)
            });
        if !full {
            cmd = cmd.long_about(None::<&str>);
        }
        if cmd.get_name() == "add" && cmd.get_arguments().any(|arg| arg.get_id() == "server") {
            cmd=cmd.before_help("Connect: logsh context add https://logship.example.com\nSSO:     logsh context add https://logship.example.com --sso\nScripts: logsh context add <server> --pat --token-stdin --account <name-or-id>");
        }
        if cmd.get_name() == "login" {
            cmd=cmd.before_help("Login: logsh context login [name]\nScripts: logsh context login --password-stdin --account <name-or-id>");
        }
        if root {
            cmd = cmd.before_help("Get started: logsh context add <server-url>\nQuery:       logsh query -q 'MyTable | take 10'\nUsers:       logsh account users list")
                .after_help("Global flags: --context <name>, --account <name-or-id>, -o <format>\nMore options: --help-all");
        } else {
            cmd = cmd.after_help("Use --help-all for advanced and global options.");
        }
        let children: Vec<_> = cmd.get_subcommands().cloned().collect();
        for child in children {
            let name = child.get_name().to_owned();
            cmd = cmd.mut_subcommand(name, |c| compact(c, full, false));
        }
        cmd
    }
    compact(Args::command(), full, true)
}

fn parse_args() -> Args {
    let args: Vec<_> = std::env::args_os().collect();
    let full = args.iter().any(|a| a == "--help-all");
    let matches = cli_command(full).get_matches_from(args);
    Args::from_arg_matches(&matches).unwrap_or_else(|e| e.exit())
}

#[derive(Subcommand)]
enum Commands {
    /// Manage contexts (server connections).
    Context {
        #[command(subcommand)]
        command: Option<crate::config::ContextCommand>,
    },

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
    let cli = parse_args();
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
            std::process::exit(error_exit_code(&err));
        }
    }
}

fn run(cli: Args) -> Result<i32, Error> {
    let output = cli.output;
    match cli.command {
        Some(Commands::Context { command }) => {
            crate::connect::execute_context(
                command.unwrap_or(crate::config::ContextCommand::Overview),
                output,
            )?;
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
            let mut cmd = cli_command(true);
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
                        let selected = conn.connection.effective_account()?;
                        let sub = selected.map_or("None".to_string(), |s| s.to_string());
                        let account_name = if selected == conn.connection.default_account {
                            conn.connection.default_account_name.as_deref()
                        } else {
                            logsh_core::config::get_account_override()
                        };
                        match output {
                            Some(OutputMode::Json | OutputMode::JsonPretty) => {
                                fmt::print_json(&serde_json::json!({
                                    "status": "connected",
                                    "connection": conn.name,
                                    "server": conn.connection.server,
                                    "user": user.user_name,
                                    "account": sub,
                                    "accountName": account_name,
                                }));
                            }
                            _ => {
                                println!("Status: {}", "Connected".green());
                                println!("Server: {}", conn.connection.server);
                                println!(
                                    "Connection: {}  User: {}  Account: {}",
                                    conn.name.blue(),
                                    user.user_name.blue(),
                                    account_name.unwrap_or(&sub).blue()
                                );
                                println!("Try: logsh query -q 'MyTable | take 10'\n     logsh schema list\n     logsh account users list\n     logsh context");
                            }
                        }
                        Ok(exit_codes::SUCCESS)
                    }
                    Err(err) => {
                        if matches!(output, Some(OutputMode::Json | OutputMode::JsonPretty)) {
                            eprintln!(
                                "{}",
                                serde_json::json!({"error":err.to_string(),"nextCommand":"logsh context login"})
                            );
                        } else {
                            fmt::print_connect_error(&cfg, &err);
                        }
                        Ok(exit_codes::AUTH_ERROR)
                    }
                },
                None => {
                    if !cfg.contexts.is_empty()
                        || logsh_core::config::get_context_override().is_some()
                    {
                        cfg.resolve_current_context()?;
                    }
                    if connect::interactive(output) {
                        connect::guided_setup(output)?;
                        Ok(exit_codes::SUCCESS)
                    } else {
                        if matches!(output, Some(OutputMode::Json | OutputMode::JsonPretty)) {
                            fmt::print_json(
                                &serde_json::json!({"status":"notConfigured", "nextCommand":"logsh context add <server> --account <name-or-id>"}),
                            );
                        } else {
                            fmt::print_add_connection_help();
                        }
                        Ok(exit_codes::NOT_CONFIGURED)
                    }
                }
            }
        }
    }
}

fn error_exit_code(err: &Error) -> i32 {
    use logsh_core::error::{AuthError, ClientError, ConfigError, ConnectError};
    for cause in err.chain() {
        if cause.downcast_ref::<AuthError>().is_some() {
            return exit_codes::AUTH_ERROR;
        }
        if matches!(
            cause.downcast_ref::<ClientError>(),
            Some(ClientError::NoToken | ClientError::Http { status: 401, .. })
        ) {
            return exit_codes::AUTH_ERROR;
        }
        if let Some(ConnectError::Network(error)) = cause.downcast_ref::<ConnectError>() {
            if error.status() == Some(logsh_core::reqwest::StatusCode::UNAUTHORIZED) {
                return exit_codes::AUTH_ERROR;
            }
        }
        if let Some(error) = cause.downcast_ref::<logsh_core::reqwest::Error>() {
            if error.status() == Some(logsh_core::reqwest::StatusCode::UNAUTHORIZED) {
                return exit_codes::AUTH_ERROR;
            }
        }
        if matches!(
            cause.downcast_ref::<ConfigError>(),
            Some(ConfigError::NoDefaultConnection | ConfigError::ContextNotFound(_))
        ) {
            return exit_codes::NOT_CONFIGURED;
        }
    }
    exit_codes::ERROR
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
