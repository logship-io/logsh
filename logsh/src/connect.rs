use anyhow::{anyhow, Error};

use colored::Colorize;
use logsh_core::{config, connect::Connection, error::AuthError, query::QueryResultFmt};
use std::{collections::HashMap, io::Write};
use term_table::{
    row::Row,
    table_cell::{Alignment, TableCell},
    Table, TableStyle,
};

use crate::{
    config::{AddConnectionCommand, ConfigConnectionCommand, OAuthFlow},
    query::markdown_style,
    OutputMode,
};

pub fn execute_connect(command: ConfigConnectionCommand) -> Result<(), Error> {
    match command {
        ConfigConnectionCommand::Add(AddConnectionCommand::Basic {
            name,
            server,
            default,
            username,
            password,
        }) => {
            log::trace!("Entering {}.", "add user connection".bright_black().bold());
            let default = default.unwrap_or(true);
            let mut cfg = logsh_core::config::load()?;
            let username = match username {
                Some(username) => username,
                None => {
                    println!(
                        "{}{}{}{}",
                        "Please enter your ".cyan(),
                        "logship ".blue(),
                        "username".cyan().bold(),
                        ":".cyan(),
                    );
                    let mut username = String::new();
                    let _ = std::io::stdin().read_line(&mut username)?;
                    username.trim().to_string()
                }
            };

            log::debug!(
                "Authenticating with username: {}",
                username.clone().yellow()
            );

            let connection = Connection::new(&server);
            let auth = Some(logsh_core::auth::AuthRequest::Jwt {
                username: username.clone(),
                password: || {
                    if let Some(password) = password {
                        return Ok(password);
                    }

                    return rpassword::prompt_password(format!(
                        "{}{}{}{} ",
                        "Please enter your ".cyan(),
                        "logship user".blue(),
                        "password".cyan().bold(),
                        ":".cyan(),
                    ))
                    .map_err(logsh_core::error::BasicAuthError::IOError)
                    .map_err(logsh_core::error::AuthError::BasicAuth);
                },
            });

            let c = logsh_core::connect::add_connect(name.clone(), Some(connection), auth);
            match c {
                Ok(connection) => {
                    log::debug!(
                        "User {} added as default: {}",
                        username.yellow(),
                        default.to_string().blue()
                    );
                    if default {
                        cfg.default_connection = name.clone();
                    }

                    cfg.connections.insert(name, connection);
                    log::info!("Saving new connection.");
                    logsh_core::config::save(cfg)?;
                }
                Err(err) => {
                    return Err(anyhow!("Error adding connection: {err}"));
                }
            };

            Ok(())
        }
        ConfigConnectionCommand::Add(AddConnectionCommand::OAuth {
            name,
            server,
            default,
            flow,
        }) => {
            let c = Connection::new(&server);
            let c = logsh_core::connect::add_connect::<Box<dyn FnOnce() -> Result<String, AuthError>>>(
                name.clone(),
                Some(c),
                Some(logsh_core::auth::AuthRequest::OAuth {
                    client_id: String::default(),
                    device_endpoint: None,
                    scopes: vec![],
                    authorize_endpoint: String::default(),
                    token_endpoint: String::default(),
                    flow: match flow {
                        OAuthFlow::Device => logsh_core::auth::oauth::OAuthFlow::Device,
                        // OAuthFlow::Browser => logsh_core::auth::oauth::OAuthFlow::Code,
                    },
                }),
            )
            .map_err(|err| anyhow!("Failed to connect with OAuth: {err}"))?;

            let mut cfg = config::load()?;
            if let Some(_old) = cfg.connections.insert(name.clone(), c) {
                log::info!(
                    "New OAuth connection \"{}\" replacing existing connection.",
                    name.yellow().dimmed()
                )
            }

            if default.unwrap_or(true) {
                log::info!(
                    "Setting OAuth connection \"{}\" as default connection.",
                    name.yellow().dimmed()
                );
                cfg.default_connection = name.clone();
            }
            config::save(cfg)?;
            Ok(())
        }
        ConfigConnectionCommand::List { output } => list(std::io::stdout(), false, output),
        ConfigConnectionCommand::Remove { name } => {
            let mut cfg = config::load()?;
            if let Some(_conn) = cfg.connections.remove(&name) {
                log::info!("Removing connection with name: {}", name.clone().yellow());
            } else {
                log::info!(
                    "No connection with name: \"{}\".",
                    name.clone().red().blink()
                );
            }

            config::save(cfg)?;
            Ok(())
        },
        ConfigConnectionCommand::Default { name } => {
            let mut cfg = config::load()?;
            if false == cfg.connections.contains_key(&name) {
                return Err(anyhow!("Connection \"{name}\" does not exist in configuration."));
            }

            cfg.default_connection = name;
            config::save(cfg)?;
            Ok(())
        },
        
    }
}

fn list<W: Write>(mut write: W, color: bool, mode: Option<OutputMode>) -> Result<(), Error> {
    let config = logsh_core::config::load()?;
    let list: Vec<_> = config
        .connections
        .into_iter()
        .map(|c| crate::fmt::Connection {
            name: c.0.to_string(),
            server: c.1.server.to_string(),
            is_default: c.0 == config.default_connection,
            username: c.1.username.to_string(),
        })
        .collect();

    match mode.unwrap_or_default() {
        OutputMode::Table | OutputMode::Markdown => {
            let mut table = Table::new();
            table.style = match mode.unwrap_or_default() {
                OutputMode::Table => TableStyle::thin(),
                OutputMode::Markdown => markdown_style(),
                _ => unreachable!(),
            };
            table.add_row(Row::new(vec![
                TableCell::new_with_alignment("Name".bright_white().bold(), 1, Alignment::Left),
                TableCell::new_with_alignment("Server".bright_white().bold(), 1, Alignment::Center),
                TableCell::new_with_alignment("Default".bright_white().bold(), 1, Alignment::Left),
                TableCell::new_with_alignment(
                    "Logged in User".bright_white().bold(),
                    1,
                    Alignment::Right,
                ),
            ]));

            list.iter().for_each(|f| {
                table.add_row(Row::new(vec![
                    TableCell::new_with_alignment(&f.name.white(), 1, Alignment::Left),
                    TableCell::new_with_alignment(&f.server.blue(), 1, Alignment::Center),
                    TableCell::new_with_alignment(
                        if f.is_default {
                            "true".green()
                        } else {
                            "false".red()
                        },
                        1,
                        Alignment::Left,
                    ),
                    TableCell::new_with_alignment(f.username.bright_black(), 1, Alignment::Right),
                ]));
            });

            log::trace!("Rendering output table.");
            let render = table.render();
            writeln!(write, "{}", render).map_err(|e| anyhow!("Failed to write output: {}", e))
        }
        OutputMode::Json => {
            let json = serde_json::to_string(&list)?;
            writeln!(write, "{}", json).map_err(|e| anyhow!("Failed to write json output: {}", e))
        }
        OutputMode::JsonPretty => {
            let json = serde_json::to_string_pretty(&list)?;
            writeln!(write, "{}", json)
                .map_err(|e| anyhow!("Failed to write pretty json output: {}", e))
        }
        OutputMode::Csv => {
            let results = list
                .iter()
                .map(|c| {
                    HashMap::from([
                        (
                            "Name".to_string(),
                            serde_json::Value::String(c.name.to_string()),
                        ),
                        (
                            "Server".to_string(),
                            serde_json::Value::String(c.server.to_string()),
                        ),
                        (
                            "Default".to_string(),
                            serde_json::Value::String(c.is_default.to_string()),
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
            logsh_core::csv::write_csv(&query, color, write)
                .map_err(|e| anyhow!("Failed to write csv output: {}", e))
        }
    }
}
