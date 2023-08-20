use std::str::FromStr;

use anyhow::{anyhow, Error};
use clap::{arg, Parser, Subcommand, ValueEnum};
mod connect;
mod query;
mod version;

#[derive(Parser, Debug)]
#[clap(name = "logsh", author = "logship.llc")]
struct Args {
    #[command(subcommand)]
    command: Option<Commands>,

    #[arg(short = 'v', action = clap::ArgAction::Count, global = true, help = "Set command verbosity. The more 'v's, the more verbose. -vvvv is the most verbose.")]
    verbose: u8,

    #[arg(long, help = "logsh version information.")]
    version: bool,
}

#[derive(Subcommand, Debug)]
enum Commands {
    #[command(subcommand)]
    Connection(crate::connect::ConnectCommand),
    Query(crate::query::QueryCommand),
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

    pretty_env_logger::formatted_builder()
        .filter_level(log_level)
        .init();

    if cli.version {
        return version::version(std::io::stdout(), log_level);
    }

    let result = match cli.command {
        Some(Commands::Connection(command)) => crate::connect::execute_connect(command),
        Some(Commands::Query(command)) => crate::query::execute_query(command, std::io::stdout()),
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
}

impl FromStr for OutputMode {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "json" => Ok(OutputMode::Json),
            "json-pretty" => Ok(OutputMode::JsonPretty),
            "csv" => Ok(OutputMode::Csv),
            _ => Err(anyhow!("Failed to read output format: \"{}\"", s)),
        }
    }
}