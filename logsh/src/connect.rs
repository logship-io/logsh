use anyhow::{anyhow, Error};

use colored::Colorize;
use logsh_core::{
    config,
    connect::Connection,
    error::{AuthError, BasicAuthError, ConnectError},
    query::QueryResultFmt,
};
use std::io::IsTerminal;
use std::{collections::HashMap, io::Write};
use term_table::{
    row::Row,
    table_cell::{Alignment, TableCell},
    Table, TableStyle,
};

use crate::{
    config::{AddContextArgs, ContextCommand, OAuthFlow},
    query::markdown_style,
    OutputMode,
};

pub fn execute_context(command: ContextCommand, output: Option<OutputMode>) -> Result<(), Error> {
    let mut cfg = config::load()?;
    match command {
        ContextCommand::Add(args) => execute_add_context(&mut cfg, args, output),
        ContextCommand::List => list(std::io::stdout(), output),
        ContextCommand::Remove { name } => {
            let mut cfg = config::load()?;
            if let Some(_conn) = cfg.contexts.remove(&name) {
                log::info!("Removing context: {}", name.clone().yellow());
            } else {
                log::info!("No context named \"{}\".", name.clone().red().blink());
                return Ok(());
            }

            config::save(cfg).inspect_err(|err| {
                crate::fmt::print_config_error(err);
            })?;
            if let Some(OutputMode::Json | OutputMode::JsonPretty) = output {
                crate::fmt::print_json(&serde_json::json!({
                    "status": "ok",
                    "message": format!("Context \"{name}\" removed")
                }));
            }
            Ok(())
        }
        ContextCommand::Use { name, account } => {
            if !cfg.contexts.contains_key(&name) {
                let err = ConnectError::NoConnection(name.clone());
                crate::fmt::print_connect_error(&cfg, &err);
                return Err(anyhow!("Invalid Input: {err}"));
            }

            cfg.current_context = name.clone();

            if let Some(account_name) = account {
                let conn = cfg
                    .contexts
                    .get(&name)
                    .ok_or(anyhow!("Context not found"))?;
                let accounts = conn.accounts(conn.user_id).map_err(|e| anyhow!("{e}"))?;
                let account = accounts
                    .iter()
                    .find(|a| a.account_name.eq_ignore_ascii_case(&account_name))
                    .ok_or(anyhow!(
                        "Account \"{}\" not found. Run 'logsh account list' to see available accounts.",
                        &account_name
                    ))?;
                let conn_mut = cfg.contexts.get_mut(&name).unwrap();
                conn_mut.default_account = Some(account.account_id);
                conn_mut.default_account_name = Some(account.account_name.clone());
                conn_mut.known_accounts = accounts.iter().map(|a| a.account_name.clone()).collect();
            }

            config::save(cfg).inspect_err(|err| {
                crate::fmt::print_config_error(err);
            })?;
            if let Some(OutputMode::Json | OutputMode::JsonPretty) = output {
                crate::fmt::print_json(&serde_json::json!({
                    "status": "ok",
                    "message": format!("Switched to context \"{name}\"")
                }));
            }
            Ok(())
        }
        ContextCommand::Login { name } => {
            let cfg = logsh_core::config::load()?;
            let conn = if let Some(name) = name.as_ref() {
                cfg.contexts.get(name).map(|c| config::ContextConfig {
                    name: name.clone(),
                    connection: c.clone(),
                })
            } else {
                cfg.get_current_context()
            };

            match conn {
                Some(connection_config) => {
                    let server = connection_config.connection.server.clone();
                    let ctx_name = connection_config.name.clone();
                    if connection_config.connection.is_jwt_auth() {
                        let mut cfg = config::load()?;
                        execute_add_context(
                            &mut cfg,
                            AddContextArgs {
                                server,
                                name: Some(ctx_name),
                                sso: false,
                                pat: false,
                                username: Some(connection_config.connection.username.clone()),
                                password: None,
                                password_stdin: false,
                                token: None,
                                token_stdin: false,
                                oauth_flow: OAuthFlow::Device,
                                no_default: true,
                            },
                            output,
                        )
                    } else if connection_config.connection.is_oauth_auth() {
                        let mut cfg = config::load()?;
                        execute_add_context(
                            &mut cfg,
                            AddContextArgs {
                                server,
                                name: Some(ctx_name),
                                sso: true,
                                pat: false,
                                username: None,
                                password: None,
                                password_stdin: false,
                                token: None,
                                token_stdin: false,
                                oauth_flow: OAuthFlow::Device,
                                no_default: true,
                            },
                            output,
                        )
                    } else {
                        let err = ConnectError::InvalidConfigError(
                            "No authentication defined for this context.".to_string(),
                        );
                        crate::fmt::print_connect_error(&cfg, &err);
                        Err(anyhow!("Invalid Auth Configuration: {err}"))
                    }
                }
                None => {
                    let err = ConnectError::NoConnection(name.unwrap_or_default().to_string());
                    crate::fmt::print_connect_error(&cfg, &err);
                    Err(anyhow!("Invalid Input: {err}"))
                }
            }
        }
        ContextCommand::Current => {
            let cfg = config::load()?;
            match cfg.get_current_context() {
                Some(conn) => {
                    let account_display = conn
                        .connection
                        .default_account_name
                        .as_deref()
                        .unwrap_or("(none)");
                    match output {
                        Some(OutputMode::Json | OutputMode::JsonPretty) => {
                            crate::fmt::print_json(&serde_json::json!({
                                "name": conn.name,
                                "account": account_display,
                            }));
                        }
                        _ => println!("{} (account: {})", conn.name, account_display),
                    }
                    Ok(())
                }
                None => {
                    crate::fmt::print_add_connection_help();
                    Err(anyhow!("No contexts configured"))
                }
            }
        }
    }
}

/// Reads a single trimmed line from stdin.
fn read_stdin_line() -> Result<String, Error> {
    let mut line = String::new();
    std::io::stdin().read_line(&mut line)?;
    Ok(line.trim().to_string())
}

fn derive_context_name(server: &str) -> String {
    let s = server
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches('/');
    // Strip port if present
    s.split(':').next().unwrap_or(s).to_string()
}

fn execute_add_context(
    cfg: &mut config::Configuration,
    args: AddContextArgs,
    output: Option<OutputMode>,
) -> Result<(), Error> {
    let name = args
        .name
        .unwrap_or_else(|| derive_context_name(&args.server));
    let set_default = !args.no_default;

    match (args.sso, args.pat) {
        (false, false) => {
            log::trace!("Adding basic auth context \"{name}\".");
            let username = match args.username {
                Some(u) => u,
                None => {
                    println!(
                        "{} {}{}",
                        "Please enter your logship".cyan(),
                        "username".cyan().bold(),
                        ":".cyan(),
                    );
                    let mut username = String::new();
                    std::io::stdin().read_line(&mut username)?;
                    username.trim().to_string()
                }
            };

            let password = if args.password_stdin {
                Some(read_stdin_line()?)
            } else {
                args.password
            };
            let username_for_prompt = username.clone();
            let connection = Connection::new(&args.server);
            let auth = Some(logsh_core::auth::AuthRequest::Jwt {
                username: username.clone(),
                password: move || {
                    if let Some(password) = password {
                        return Result::<String, ConnectError>::Ok(password);
                    }
                    rpassword::prompt_password(format!(
                        "{} {}{}{} ",
                        "Please enter".cyan(),
                        username_for_prompt.bright_blue().bold(),
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
                    if set_default || cfg.contexts.is_empty() {
                        cfg.current_context = name.clone();
                    }
                    cfg.contexts.insert(name.clone(), connection);
                    config::save(cfg.clone()).inspect_err(|err| {
                        crate::fmt::print_config_error(err);
                    })?;
                    if let Some(OutputMode::Json | OutputMode::JsonPretty) = output {
                        crate::fmt::print_json(&serde_json::json!({
                            "status": "ok",
                            "message": format!("Context \"{name}\" added")
                        }));
                    }
                    Ok(())
                }
                Err(err) => {
                    crate::fmt::print_connect_error(cfg, &err);
                    Err(anyhow!("Error adding context: {err}"))
                }
            }
        }
        (true, false) => {
            let connection = Connection::new(&args.server);
            let c = logsh_core::connect::add_connect::<
                Box<dyn FnOnce() -> Result<String, ConnectError>>,
            >(
                name.clone(),
                Some(connection),
                Some(logsh_core::auth::AuthRequest::OAuth {
                    client_id: String::default(),
                    device_endpoint: None,
                    scopes: vec![],
                    authorize_endpoint: String::default(),
                    token_endpoint: String::default(),
                    flow: match args.oauth_flow {
                        OAuthFlow::Device => logsh_core::auth::oauth::OAuthFlow::Device,
                    },
                }),
            )
            .inspect_err(|err| {
                crate::fmt::print_connect_error(cfg, err);
            })?;

            if set_default || cfg.contexts.is_empty() {
                cfg.current_context = name.clone();
            }
            cfg.contexts.insert(name.clone(), c);
            config::save(cfg.clone()).inspect_err(|err| {
                crate::fmt::print_config_error(err);
            })?;
            if let Some(OutputMode::Json | OutputMode::JsonPretty) = output {
                crate::fmt::print_json(&serde_json::json!({
                    "status": "ok",
                    "message": format!("Context \"{name}\" added")
                }));
            }
            Ok(())
        }
        (false, true) => {
            let token = if args.token_stdin {
                Some(read_stdin_line()?)
            } else {
                args.token
            };
            let token = match token {
                Some(t) => t,
                None if std::io::stdin().is_terminal() => rpassword::prompt_password(format!(
                    "{} {}",
                    "Enter Personal Access Token:".cyan(),
                    "".bright_black(),
                ))
                .map_err(|e| anyhow!("Failed to read token: {e}"))?,
                None => {
                    return Err(anyhow!(
                        "Provide --token, --token-stdin, or set LOGSH_PAT_TOKEN env var."
                    ));
                }
            };
            let connection = Connection::new(&args.server);
            let c = logsh_core::connect::add_connect::<
                Box<dyn FnOnce() -> Result<String, ConnectError>>,
            >(
                name.clone(),
                Some(connection),
                Some(logsh_core::auth::AuthRequest::Pat { token }),
            )
            .inspect_err(|err| {
                crate::fmt::print_connect_error(cfg, err);
            })?;

            if set_default || cfg.contexts.is_empty() {
                cfg.current_context = name.clone();
            }
            cfg.contexts.insert(name.clone(), c);
            config::save(cfg.clone()).inspect_err(|err| {
                crate::fmt::print_config_error(err);
            })?;
            if let Some(OutputMode::Json | OutputMode::JsonPretty) = output {
                crate::fmt::print_json(&serde_json::json!({
                    "status": "ok",
                    "message": format!("Context \"{name}\" added")
                }));
            }
            Ok(())
        }
        (true, true) => unreachable!("clap arg group prevents --sso and --pat together"),
    }
}

fn list<W: Write>(mut write: W, mode: Option<OutputMode>) -> Result<(), Error> {
    let config = logsh_core::config::load()?;
    let mut list: Vec<_> = Vec::from_iter(config.contexts);
    list.sort_by_key(|c| c.0.to_owned());
    let list: Vec<_> = list
        .iter()
        .map(|c| {
            let current_account = c.1.default_account_name.clone().unwrap_or_default();
            crate::fmt::Connection {
                name: c.0.to_string(),
                server: c.1.server.to_string(),
                is_current_context: c.0 == config.current_context,
                username: c.1.username.to_string(),
                current_account,
                accounts: c.1.known_accounts.clone(),
            }
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
                TableCell::builder("Context".bright_white().bold())
                    .col_span(1)
                    .alignment(Alignment::Left)
                    .build(),
                TableCell::builder("Server".bright_white().bold())
                    .col_span(1)
                    .alignment(Alignment::Center)
                    .build(),
                TableCell::builder("User".bright_white().bold())
                    .col_span(1)
                    .alignment(Alignment::Left)
                    .build(),
                TableCell::builder("Account".bright_white().bold())
                    .col_span(1)
                    .alignment(Alignment::Left)
                    .build(),
                TableCell::builder("Current".bright_white().bold())
                    .col_span(1)
                    .alignment(Alignment::Center)
                    .build(),
            ]));

            for ctx in &list {
                if ctx.accounts.is_empty() {
                    // Context with no known accounts — show single row
                    let marker = if ctx.is_current_context { "* " } else { "  " };
                    table.add_row(Row::new(vec![
                        TableCell::builder(format!("{marker}{}", ctx.name).white())
                            .col_span(1)
                            .alignment(Alignment::Left)
                            .build(),
                        TableCell::builder(ctx.server.blue())
                            .col_span(1)
                            .alignment(Alignment::Center)
                            .build(),
                        TableCell::builder(ctx.username.bright_black())
                            .col_span(1)
                            .alignment(Alignment::Left)
                            .build(),
                        TableCell::builder("(none)".bright_black())
                            .col_span(1)
                            .alignment(Alignment::Left)
                            .build(),
                        TableCell::builder(if ctx.is_current_context {
                            "◉".green()
                        } else {
                            " ".normal()
                        })
                        .col_span(1)
                        .alignment(Alignment::Center)
                        .build(),
                    ]));
                } else {
                    for account_name in &ctx.accounts {
                        let is_active_account =
                            ctx.current_account.eq_ignore_ascii_case(account_name);
                        let is_active_row = ctx.is_current_context && is_active_account;
                        let marker = if is_active_row { "* " } else { "  " };
                        table.add_row(Row::new(vec![
                            TableCell::builder(format!("{marker}{}", ctx.name).white())
                                .col_span(1)
                                .alignment(Alignment::Left)
                                .build(),
                            TableCell::builder(ctx.server.blue())
                                .col_span(1)
                                .alignment(Alignment::Center)
                                .build(),
                            TableCell::builder(ctx.username.bright_black())
                                .col_span(1)
                                .alignment(Alignment::Left)
                                .build(),
                            TableCell::builder(if is_active_account {
                                account_name.clone().bright_white().bold()
                            } else {
                                account_name.clone().bright_black()
                            })
                            .col_span(1)
                            .alignment(Alignment::Left)
                            .build(),
                            TableCell::builder(if is_active_row {
                                "◉".green()
                            } else {
                                " ".normal()
                            })
                            .col_span(1)
                            .alignment(Alignment::Center)
                            .build(),
                        ]));
                    }
                }
            }

            let render = table.render();
            writeln!(write, "{render}").map_err(|e| anyhow!("Failed to write output: {e}"))
        }
        OutputMode::Json => {
            let json = serde_json::to_string(&list)?;
            writeln!(write, "{json}").map_err(|e| anyhow!("Failed to write json output: {e}"))
        }
        OutputMode::JsonPretty => {
            let json = serde_json::to_string_pretty(&list)?;
            writeln!(write, "{json}")
                .map_err(|e| anyhow!("Failed to write pretty json output: {e}"))
        }
        OutputMode::Csv => {
            let mut results = vec![];
            for ctx in &list {
                if ctx.accounts.is_empty() {
                    results.push(HashMap::from([
                        (
                            "Context".to_string(),
                            serde_json::Value::String(ctx.name.clone()),
                        ),
                        (
                            "Server".to_string(),
                            serde_json::Value::String(ctx.server.clone()),
                        ),
                        (
                            "User".to_string(),
                            serde_json::Value::String(ctx.username.clone()),
                        ),
                        (
                            "Account".to_string(),
                            serde_json::Value::String(String::new()),
                        ),
                        (
                            "Current".to_string(),
                            serde_json::Value::String(ctx.is_current_context.to_string()),
                        ),
                    ]));
                } else {
                    for account_name in &ctx.accounts {
                        let is_active = ctx.is_current_context
                            && ctx.current_account.eq_ignore_ascii_case(account_name);
                        results.push(HashMap::from([
                            (
                                "Context".to_string(),
                                serde_json::Value::String(ctx.name.clone()),
                            ),
                            (
                                "Server".to_string(),
                                serde_json::Value::String(ctx.server.clone()),
                            ),
                            (
                                "User".to_string(),
                                serde_json::Value::String(ctx.username.clone()),
                            ),
                            (
                                "Account".to_string(),
                                serde_json::Value::String(account_name.clone()),
                            ),
                            (
                                "Current".to_string(),
                                serde_json::Value::String(is_active.to_string()),
                            ),
                        ]));
                    }
                }
            }
            let result = QueryResultFmt {
                header: vec![
                    "Context".to_string(),
                    "Server".to_string(),
                    "User".to_string(),
                    "Account".to_string(),
                    "Current".to_string(),
                ],
                results,
            };
            let result = serde_json::to_string(&result).map_err(|e| {
                anyhow::anyhow!("Error converting connections to query response json: {e}")
            })?;
            let query = result
                .as_str()
                .try_into()
                .map_err(|e| anyhow::anyhow!("Error converting connection json to csv: {e}"))?;
            logsh_core::csv::write_csv(&query, write)
                .map_err(|e| anyhow!("Failed to write csv output: {e}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_derive_name_https() {
        assert_eq!(
            derive_context_name("https://my.logship.ai"),
            "my.logship.ai"
        );
    }

    #[test]
    fn test_derive_name_http() {
        assert_eq!(derive_context_name("http://localhost:8080"), "localhost");
    }

    #[test]
    fn test_derive_name_trailing_slash() {
        assert_eq!(derive_context_name("https://example.com/"), "example.com");
    }

    #[test]
    fn test_derive_name_bare() {
        assert_eq!(derive_context_name("myserver"), "myserver");
    }

    #[test]
    fn test_derive_name_with_port() {
        assert_eq!(
            derive_context_name("https://prod.logship.ai:443"),
            "prod.logship.ai"
        );
    }
}
