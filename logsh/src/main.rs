use std::str::FromStr;

use anyhow::{anyhow, Error};
use clap::{
    arg,
    builder::{
        styling::{AnsiColor, Effects},
        Styles,
    },
    command, Parser, Subcommand, ValueEnum,
};
use colored::Colorize;

mod config;
mod connect;
mod fmt;
mod query;
mod upload;
mod version;

#[derive(Parser)]
#[clap(name = "logsh", author = "logship.llc", styles = styles())]
#[command(arg_required_else_help = false)]
struct Args {
    #[command(subcommand)]
    command: Option<Commands>,

    #[arg(short = 'v', action = clap::ArgAction::Count, global = true, help = "Set command verbosity. The more 'v's, the more verbose. -vvvv is the most verbose.")]
    verbose: u8,

    #[arg(long, global = true, help = "Disable global color output.")]
    no_color: bool,
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
    #[clap(subcommand)]
    Connection(crate::config::ConfigConnectionCommand),
    #[command(subcommand)]
    Config(crate::config::ConfigCommand),
    Query(crate::query::QueryCommand),
    Upload(crate::upload::UploadCommand),
    Version(crate::version::VersionCommand),
}

fn main() -> Result<(), Error> {
    let cli = Args::parse();
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

    match cli.command {
        Some(Commands::Connection(command)) => crate::connect::execute_connect(command),
        Some(Commands::Query(command)) => crate::query::execute_query(command, std::io::stdout()),
        Some(Commands::Upload(command)) => crate::upload::execute_upload(command),
        Some(Commands::Version(command)) => {
            crate::version::version(std::io::stdout(), command, cli.verbose)
        }
        Some(Commands::Config(command)) => crate::config::execute_config(command),
        None => {
            log::debug!("No arguments provided. Output status.");
            let cfg = logsh_core::config::load()?;
            let conn = cfg.get_default_connection();
            match conn {
                Some(conn) => {
                    match conn.connection.who_am_i() {
                        Ok(user) => {
                            println!("Status: {}", "Connected".green());
                            println!(
                                "Logged into connection {} as user {} with subscription: {}",
                                &conn.name.blue(),
                                &user.user_name.blue(),
                                conn.connection.default_subscription().to_string().blue()
                            );
                        }
                        Err(err) => {
                            fmt::print_connect_error(&cfg, &conn.name, &conn.connection, err)
                        }
                    };
                }
                None => {
                    println!(
                        "Status: {} {}",
                        "Missing default connection.".red(),
                        "Configuration Required.".red()
                    );
                    fmt::print_add_connection_help();
                }
            }

            println!(
                "{} {} {}",
                "# Execute".bright_black(),
                "logsh --help".blue(),
                "to view available commands.".bright_black()
            );
            Ok(())
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
            _ => Err(anyhow!("Failed to read output format: \"{}\"", s)),
        }
    }
}
