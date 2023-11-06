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

mod config;
mod connect;
mod fmt;
mod query;
mod upload;
mod version;

#[derive(Parser)]
#[clap(name = "logsh", author = "logship.llc", styles = styles())]
#[command(arg_required_else_help = true)]
struct Args {
    #[command(subcommand)]
    command: Option<Commands>,

    #[arg(short = 'v', action = clap::ArgAction::Count, global = true, help = "Set command verbosity. The more 'v's, the more verbose. -vvvv is the most verbose.")]
    verbose: u8,
}

fn styles() -> Styles {
    if std::env::var("NO_COLOR").unwrap_or_default().trim().len() > 0 {
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

    let no_color = std::env::var("NO_COLOR").unwrap_or_default().trim().len() > 0;
    if no_color {
        colored::control::set_override(false);
    }
    
    pretty_env_logger::formatted_builder()
        .filter_level(log_level)
        .init();

    let result = match cli.command {
        Some(Commands::Connection(command)) => crate::connect::execute_connect(command),
        Some(Commands::Query(command)) => crate::query::execute_query(command, std::io::stdout()),
        Some(Commands::Upload(command)) => crate::upload::execute_upload(command),
        Some(Commands::Version(command)) => {
            crate::version::version(std::io::stdout(), command, cli.verbose)
        }
        Some(Commands::Config(command)) => crate::config::execute_config(command),
        None => Err(anyhow::anyhow!("No command provided.")),
    };

    if let Err(err) = result {
        return Err(anyhow!("Command failed: {}", err));
    }

    Ok(())
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
