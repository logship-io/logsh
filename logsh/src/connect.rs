use anyhow::{anyhow, Error};
use clap::Subcommand;
use logsh_core::query::QueryResultFmt;
use std::{collections::HashMap, io::Write};
use term_table::{
    row::Row,
    table_cell::{Alignment, TableCell},
    Table, TableStyle,
};

use crate::{query::markdown_style, OutputMode};

#[derive(Subcommand, Debug)]
#[clap(about = "Connect to a Logship server.")]
pub enum ConnectCommand {
    #[clap(about = "Add a new connection.")]
    Add {
        #[arg(help = "Name of the connection. Just for your own reference.")]
        name: String,
        #[arg(help = "Logship server. e.g. https://logship.io")]
        server: String,
        #[arg(long, help = "Set this connection as the default connection.")]
        default: bool,

        #[arg(short, long, help = "Username to authenticate with.")]
        user: String,

        #[arg(short, long, help = "Password to authenticate with.")]
        password: Option<String>,
    },
    #[clap(about = "Use OAuth2 to login to a connection.")]
    Login {
        #[arg(help = "Name of the connection. Just for your own reference.")]
        name: String,
        #[arg(help = "Logship server. e.g. https://logship.io")]
        server: String,
        #[arg(long, help = "Set this connection as the default connection.")]
        default: bool,
    },
    #[clap(about = "List existing connections.")]
    List {
        #[arg(short, long, help = "Output result format")]
        output: Option<OutputMode>,
    },
    #[clap(about = "Modify the currently defaulted connection.")]
    Default {
        #[arg(help = "Name of the connection to set as default.")]
        name: String,
    },
}

pub fn execute_connect(command: ConnectCommand) -> Result<(), Error> {
    match command {
        ConnectCommand::Add {
            name,
            server,
            default,
            user,
            password,
        } => connect(name, server, default, user, password),
        ConnectCommand::Login {
            name,
            server,
            default,
        } => login(name, server, default),
        ConnectCommand::List { output } => list(std::io::stdout(), output),
        ConnectCommand::Default { name } => set_default(name),
    }
}

fn connect(
    name: String,
    server: String,
    default: bool,
    user: String,
    password: Option<String>,
) -> Result<(), Error> {
    logsh_core::connect::connect(name, server, default, user, || {
        if let Some(password) = password {
            return Ok(password);
        }

        rpassword::prompt_password("Enter logship password: ")
    })
    .map_err(|e| anyhow!("Failed to connect: {}", e))
}

fn login(name: String, server: String, default: bool) -> Result<(), Error> {
    logsh_core::connect::login(name, server, default).map_err(|e| anyhow!("Failed to login: {}", e))
}

fn list<W: Write>(mut write: W, mode: Option<OutputMode>) -> Result<(), Error> {
    let config = logsh_core::config::get_configuration()?;
    match mode.unwrap_or_default() {
        OutputMode::Table | OutputMode::Markdown => {
            let mut table = Table::new();
            table.style = match mode.unwrap_or_default() {
                OutputMode::Table => TableStyle::thin(),
                OutputMode::Markdown => markdown_style(),
                _ => unreachable!(),
            };
            table.add_row(Row::new(vec![
                TableCell::new_with_alignment("Name", 1, Alignment::Center),
                TableCell::new_with_alignment("Server", 1, Alignment::Center),
                TableCell::new_with_alignment("Default", 1, Alignment::Center),
            ]));

            config.connections.iter().for_each(|f| {
                table.add_row(Row::new(vec![
                    TableCell::new_with_alignment(f.name(), 1, Alignment::Center),
                    TableCell::new_with_alignment(f.server(), 1, Alignment::Center),
                    TableCell::new_with_alignment(f.is_default().to_string(), 1, Alignment::Center),
                ]));
            });

            log::trace!("Rendering output table.");
            let render = table.render();
            writeln!(write, "{}", render).map_err(|e| anyhow!("Failed to write output: {}", e))
        }
        OutputMode::Json => {
            let json = serde_json::to_string(&config)?;
            writeln!(write, "{}", json).map_err(|e| anyhow!("Failed to write json output: {}", e))
        }
        OutputMode::JsonPretty => {
            let json = serde_json::to_string_pretty(&config)?;
            writeln!(write, "{}", json)
                .map_err(|e| anyhow!("Failed to write pretty json output: {}", e))
        }
        OutputMode::Csv => {
            let results = config
                .connections
                .iter()
                .map(|c| {
                    HashMap::from([
                        (
                            "Name".to_string(),
                            serde_json::Value::String(c.name().to_string()),
                        ),
                        (
                            "Server".to_string(),
                            serde_json::Value::String(c.server().to_string()),
                        ),
                        (
                            "Default".to_string(),
                            serde_json::Value::String(c.is_default().to_string()),
                        ),
                    ])
                })
                .collect();
            let result = QueryResultFmt {
                header: vec![
                    "Name".to_string(),
                    "Server".to_string(),
                    "Default".to_string(),
                ],
                results,
            };
            let result = serde_json::to_string(&result).map_err(|e| {
                anyhow::anyhow!("Error converting connections to query response json: {}", e)
            })?;
            let query = result
                .as_str()
                .try_into()
                .map_err(|e| anyhow::anyhow!("Error converting connection json to csv: {}", e))?;
            logsh_core::csv::write_csv(&query, write)
                .map_err(|e| anyhow!("Failed to write csv output: {}", e))
        }
    }
}

fn set_default(name: String) -> Result<(), Error> {
    logsh_core::connect::set_default(name).map_err(|e| anyhow!("Failed to set default: {}", e))
}
