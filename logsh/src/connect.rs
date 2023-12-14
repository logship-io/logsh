use anyhow::{anyhow, Error};

use colored::Colorize;
use logsh_core::{
    config,
    connect::Connection,
    error::{AuthError, ConfigError},
    query::QueryResultFmt,
};
use std::{collections::HashMap, io::Write};
use term_table::{
    row::Row,
    table_cell::{Alignment, TableCell},
    Table, TableStyle,
};

use crate::{
    config::{AddConnectionCommand, ConfigConnectionCommand, ConfigSubscriptionCommand, OAuthFlow},
    query::markdown_style,
    OutputMode,
};

pub fn execute_subscription(command: ConfigSubscriptionCommand) -> Result<(), Error> {
    match command {
        ConfigSubscriptionCommand::List { output } => list_subscriptions(std::io::stdout(), output),
        ConfigSubscriptionCommand::Default { name } => {
            let mut cfg = logsh_core::config::load()?;
            let conn = cfg
                .get_default_connection()
                .ok_or(ConfigError::NoDefaultConnection)?;
            let mut new: Connection = conn.connection.clone();
            match conn.connection.subscriptions.get(&name) {
                Some(_id) => {
                    new.default_subscription = Some(name);
                    cfg.connections.insert(conn.name.to_owned(), new);
                    logsh_core::config::save(cfg)?;
                    Ok(())
                }
                None => {
                    Err(anyhow!("Subscription {} does not exist.", name))
                }
            }
        }
    }
}

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
                        return Result::<String, AuthError>::Ok(password);
                    }

                    rpassword::prompt_password(format!(
                        "{} {}{}{} ",
                        "Please enter".cyan(),
                        username.bright_blue().bold(),
                        "'s password".cyan().bold(),
                        ":".cyan(),
                    ))
                    .map_err(logsh_core::error::BasicAuthError::IOError)
                    .map_err(logsh_core::error::AuthError::BasicAuth)
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
                    Ok(())
                },
                Err(err) => {
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
            let c =
                logsh_core::connect::add_connect::<Box<dyn FnOnce() -> Result<String, AuthError>>>(
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

            config::save(cfg)?;
            Ok(())
        }
        ConfigConnectionCommand::Default { name } => {
            let mut cfg = config::load()?;
            if !cfg.connections.contains_key(&name) {
                return Err(anyhow!(
                    "Connection \"{name}\" does not exist in configuration."
                ));
            }

            cfg.default_connection = name;
            config::save(cfg)?;
            Ok(())
        }
        ConfigConnectionCommand::Login { name } => {
            let cfg = logsh_core::config::load()?;
            let conn = if let Some(name) = name.as_ref() {
                cfg.connections.get(name).map(|c| config::ConnectionConfig{name: name.clone(), connection: c.clone()})
            } else {
                cfg.get_default_connection()
            };

            match conn {
                Some(connection_config) => {
                    if connection_config.connection.is_jwt_auth() {
                        execute_connect(ConfigConnectionCommand::Add(AddConnectionCommand::Basic { name: connection_config.name.to_owned(), server: Some(connection_config.connection.server.to_owned()), username: Some(connection_config.connection.username.to_owned()), password: None, default: None }))
                    } else if connection_config.connection.is_oauth_auth() {
                        return execute_connect(ConfigConnectionCommand::Add(AddConnectionCommand::OAuth { name: connection_config.name.to_owned(), server: None, default: None, flow: OAuthFlow::Device }))
                    } else {
                        return Err(anyhow!("No authentication scheme defined for this connection."));
                    }
                },
                None => Err(anyhow!("No connection exists with name: {:?}", name)),
            }
        }
    }
}

fn list<W: Write>(mut write: W, mode: Option<OutputMode>) -> Result<(), Error> {
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
            logsh_core::csv::write_csv(&query, write)
                .map_err(|e| anyhow!("Failed to write csv output: {}", e))
        }
    }
}

fn list_subscriptions<W: Write>(mut write: W, mode: Option<OutputMode>) -> Result<(), Error> {
    let config = logsh_core::config::load()?;
    let conn = config
        .get_default_connection()
        .ok_or(ConfigError::NoDefaultConnection)?;
    let default_sub = conn.connection.default_subscription();

    match mode.unwrap_or_default() {
        OutputMode::Table | OutputMode::Markdown => {
            let mut table = Table::new();
            table.style = match mode.unwrap_or_default() {
                OutputMode::Table => TableStyle::thin(),
                OutputMode::Markdown => markdown_style(),
                _ => unreachable!(),
            };
            table.add_row(Row::new(vec![
                TableCell::new_with_alignment("Id".bright_white().bold(), 1, Alignment::Left),
                TableCell::new_with_alignment("Name".bright_white().bold(), 1, Alignment::Right),
                TableCell::new_with_alignment("Default".bright_white().bold(), 1, Alignment::Left),
            ]));

            conn.connection.subscriptions.iter().for_each(|f| {
                table.add_row(Row::new(vec![
                    TableCell::new_with_alignment(&f.1.to_string().white(), 1, Alignment::Left),
                    TableCell::new_with_alignment(&f.0.white(), 1, Alignment::Right),
                    TableCell::new_with_alignment(
                        if default_sub == *f.1 {
                            "true".green()
                        } else {
                            "false".red()
                        },
                        1,
                        Alignment::Left,
                    ),
                ]));
            });

            log::trace!("Rendering output table.");
            let render = table.render();
            writeln!(write, "{}", render).map_err(|e| anyhow!("Failed to write output: {}", e))
        }
        OutputMode::Json => {
            let json = serde_json::to_string(&conn.connection.subscriptions)?;
            writeln!(write, "{}", json).map_err(|e| anyhow!("Failed to write json output: {}", e))
        }
        OutputMode::JsonPretty => {
            let json = serde_json::to_string_pretty(&conn.connection.subscriptions)?;
            writeln!(write, "{}", json)
                .map_err(|e| anyhow!("Failed to write pretty json output: {}", e))
        }
        OutputMode::Csv => {
            let results = conn
                .connection
                .subscriptions
                .iter()
                .map(|c| {
                    HashMap::from([
                        (
                            "Name".to_string(),
                            serde_json::Value::String(c.1.to_string()),
                        ),
                        ("Id".to_string(), serde_json::Value::String(c.0.to_string())),
                        (
                            "Default".to_string(),
                            serde_json::Value::String(if *c.1 == default_sub {
                                "true".to_owned()
                            } else {
                                "false".to_owned()
                            }),
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
