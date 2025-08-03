use anyhow::{anyhow, Error};

use colored::Colorize;
use logsh_core::{
    config,
    connect::Connection,
    error::{AuthError, BasicAuthError, ConnectError},
    query::QueryResultFmt,
};
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
    let mut cfg = config::load()?;
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
            let server = server
                .or_else(|| cfg.connections.get(&name).map(|s| s.server.to_owned()))
                .ok_or(anyhow!(
                    "Missing required argument \"server\" for new connection."
                ))?;

            let username = match username {
                Some(username) => username,
                None => {
                    println!(
                        "{} {}{}",
                        "Please enter your logship".cyan(),
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
                        return Result::<String, ConnectError>::Ok(password);
                    }

                    rpassword::prompt_password(format!(
                        "{} {}{}{} ",
                        "Please enter".cyan(),
                        username.bright_blue().bold(),
                        "'s password".cyan().bold(),
                        ":".cyan(),
                    ))
                    .map_err(BasicAuthError::IOError)
                    .map_err(AuthError::BasicAuth)
                    .map_err(ConnectError::Auth)
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

                    if default || cfg.connections.is_empty() {
                        cfg.default_connection = name.clone();
                    }

                    cfg.connections.insert(name, connection);
                    log::info!("Saving new connection.");
                    logsh_core::config::save(cfg).map_err(|err| {
                        crate::fmt::print_config_error(&err);
                        err
                    })?;
                    Ok(())
                }
                Err(err) => {
                    crate::fmt::print_connect_error(&cfg, &err);
                    Err(anyhow!("Error adding connection: {err}"))
                }
            }
        }
        ConfigConnectionCommand::Add(AddConnectionCommand::OAuth {
            name,
            server,
            default,
            flow,
        }) => {
            let mut cfg = config::load()?;
            let server = server
                .or_else(|| cfg.connections.get(&name).map(|s| s.server.to_owned()))
                .ok_or(anyhow!(
                    "Missing required argument \"server\" for new connection."
                ))?;

            let c = Connection::new(&server);
            let c = logsh_core::connect::add_connect::<
                Box<dyn FnOnce() -> Result<String, ConnectError>>,
            >(
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
            .map_err(|err| {
                crate::fmt::print_connect_error(&cfg, &err);
                err
            })?;

            if let Some(_old) = cfg.connections.insert(name.clone(), c) {
                log::info!(
                    "New OAuth connection \"{}\" replacing existing connection.",
                    name.yellow().dimmed()
                )
            }

            if default.unwrap_or(true) || cfg.connections.is_empty() {
                log::info!(
                    "Setting OAuth connection \"{}\" as default connection.",
                    name.yellow().dimmed()
                );
                cfg.default_connection = name.clone();
            }

            config::save(cfg).map_err(|err| {
                crate::fmt::print_config_error(&err);
                err
            })?;
            Ok(())
        }
        ConfigConnectionCommand::List { output } => list(std::io::stdout(), output),
        ConfigConnectionCommand::Remove { name } => {
            let mut cfg = config::load()?;
            if let Some(_conn) = cfg.connections.remove(&name) {
                log::info!("Removing connection with name: {}", name.clone().yellow());
            } else {
                log::info!(
                    "No connection with name: \"{}\".",
                    name.clone().red().blink()
                );
                return Ok(());
            }

            config::save(cfg).map_err(|err| {
                crate::fmt::print_config_error(&err);
                err
            })?;
            Ok(())
        }
        ConfigConnectionCommand::Default { name } => {
            if !cfg.connections.contains_key(&name) {
                let err = ConnectError::NoConnection(name.clone());
                crate::fmt::print_connect_error(&cfg, &err);
                return Err(anyhow!("Invalid Input: {}", err));
            }

            cfg.default_connection = name;
            config::save(cfg).map_err(|err| {
                crate::fmt::print_config_error(&err);
                err
            })?;
            Ok(())
        }
        ConfigConnectionCommand::Login { name } => {
            let cfg = logsh_core::config::load()?;
            let conn = if let Some(name) = name.as_ref() {
                cfg.connections.get(name).map(|c| config::ConnectionConfig {
                    name: name.clone(),
                    connection: c.clone(),
                })
            } else {
                cfg.get_default_connection()
            };

            match conn {
                Some(connection_config) => {
                    if connection_config.connection.is_jwt_auth() {
                        execute_connect(ConfigConnectionCommand::Add(AddConnectionCommand::Basic {
                            name: connection_config.name.to_owned(),
                            server: Some(connection_config.connection.server.to_owned()),
                            username: Some(connection_config.connection.username.to_owned()),
                            password: None,
                            default: None,
                        }))
                    } else if connection_config.connection.is_oauth_auth() {
                        return execute_connect(ConfigConnectionCommand::Add(
                            AddConnectionCommand::OAuth {
                                name: connection_config.name.to_owned(),
                                server: None,
                                default: None,
                                flow: OAuthFlow::Device,
                            },
                        ));
                    } else {
                        let err = ConnectError::InvalidConfigError(
                            "No authentication defined for this connection.".to_string(),
                        );
                        crate::fmt::print_connect_error(&cfg, &err);
                        Err(anyhow!("Invalid Auth Configuration: {}", err))
                    }
                }
                None => {
                    let err = ConnectError::NoConnection(name.unwrap_or_default().to_string());
                    crate::fmt::print_connect_error(&cfg, &err);
                    Err(anyhow!("Invalid Input: {}", err))
                }
            }
        }
    }
}

fn list<W: Write>(mut write: W, mode: Option<OutputMode>) -> Result<(), Error> {
    let config = logsh_core::config::load()?;
    let mut list: Vec<_> = Vec::from_iter(config.connections);
    list.sort_by_key(|c| c.0.to_owned());
    let list: Vec<_> = list
        .iter()
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
                TableCell::builder("Name".bright_white().bold()).col_span(1).alignment(Alignment::Left).build(),
                TableCell::builder("Server".bright_white().bold()).col_span(1).alignment(Alignment::Center).build(),
                TableCell::builder("Default".bright_white().bold()).col_span(1).alignment(Alignment::Left).build(),
                TableCell::builder(
                    "Logged in User".bright_white().bold()
                ).col_span(1).alignment(Alignment::Right).build(),
            ]));

            list.iter().for_each(|f| {
                table.add_row(Row::new(vec![
                    TableCell::builder(&f.name.white()).col_span(1).alignment(Alignment::Left).build(),
                    TableCell::builder(&f.server.blue()).col_span(1).alignment(Alignment::Center).build(),
                    TableCell::builder(
                        if f.is_default {
                            "true".green()
                        } else {
                            "false".red()
                        }
                    ).col_span(1).alignment(Alignment::Left).build(),
                    TableCell::builder(f.username.bright_black()).col_span(1).alignment(Alignment::Right).build(),
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
            logsh_core::csv::write_csv(&query, write)
                .map_err(|e| anyhow!("Failed to write csv output: {}", e))
        }
    }
}
