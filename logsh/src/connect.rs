use anyhow::{anyhow, Error};

use colored::Colorize;
use logsh_core::{config, error::ConnectError, query::QueryResultFmt};
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
                let account = logsh_core::account::resolve_account(&accounts, &account_name)?;
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
        ContextCommand::Login(args) => {
            let ctx = if let Some(name) = args.name {
                config::ContextConfig {
                    connection: cfg
                        .contexts
                        .get(&name)
                        .ok_or_else(|| anyhow!("Context '{name}' not found"))?
                        .clone(),
                    name,
                }
            } else {
                cfg.resolve_current_context()?
            };
            let method = ctx.connection.auth_method.as_deref().unwrap_or(
                if ctx.connection.is_oauth_auth() {
                    "sso"
                } else {
                    "password"
                },
            );
            execute_add_context(
                &mut cfg,
                AddContextArgs {
                    server: ctx.connection.server.clone(),
                    name: Some(ctx.name),
                    sso: args.sso || (!args.pat && args.username.is_none() && method == "sso"),
                    pat: args.pat || (!args.sso && args.username.is_none() && method == "pat"),
                    username: args.username.or(Some(ctx.connection.username)),
                    password: args.password,
                    password_stdin: args.password_stdin,
                    token: args.token,
                    token_stdin: args.token_stdin,
                    oauth_flow: OAuthFlow::Device,
                    no_default: true,
                },
                output,
            )
        }
        ContextCommand::Overview => {
            list(std::io::stdout(), output)?;
            if output.is_none() || matches!(output, Some(OutputMode::Table)) {
                println!("\nAdd: logsh context add <server>\nLogin: logsh context login [name]\nSwitch: logsh context use <name>\nAccount: logsh account use <name-or-id>");
            }
            Ok(())
        }
        ContextCommand::Current => {
            let conn = cfg.resolve_current_context()?;
            let account_display = conn
                .connection
                .default_account_name
                .as_deref()
                .unwrap_or("(none)");
            let value = serde_json::json!({"name":conn.name,"server":conn.connection.server,"account":account_display,"accountId":conn.connection.default_account});
            match output {
                Some(OutputMode::Json) => crate::fmt::print_json(&value),
                Some(OutputMode::JsonPretty) => crate::fmt::print_json_pretty(&value),
                _ => println!("{} (account: {})", conn.name, account_display),
            }
            Ok(())
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
    if let Ok(url) = logsh_core::reqwest::Url::parse(server) {
        if let Some(host) = url.host_str() {
            return host.to_owned();
        }
    }
    let s = server
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches('/');
    // Strip port if present
    s.split(':').next().unwrap_or(s).to_string()
}

pub fn interactive(output: Option<OutputMode>) -> bool {
    std::io::stdin().is_terminal()
        && std::io::stderr().is_terminal()
        && (output.is_none() || matches!(output, Some(OutputMode::Table)))
}
fn prompt(label: &str) -> Result<String, Error> {
    eprint!("{label}: ");
    std::io::stderr().flush()?;
    let value = read_stdin_line()?;
    if value.is_empty() {
        return Err(anyhow!("Setup cancelled; no configuration changed."));
    }
    Ok(value)
}
pub fn guided_setup(output: Option<OutputMode>) -> Result<(), Error> {
    eprintln!("Welcome to logsh. Connect to your Logship server to get started.\nLeave a required field empty to cancel.");
    let server = prompt("Server URL (https://...)")?;
    eprint!("Context name [{}]: ", derive_context_name(&server));
    std::io::stderr().flush()?;
    let name = read_stdin_line()?;
    let method = prompt("Authentication: 1 password, 2 Personal Access Token, 3 SSO")?;
    if !matches!(method.as_str(), "1" | "2" | "3") {
        return Err(anyhow!("Choose 1, 2, or 3."));
    }
    execute_add_context(
        &mut config::load()?,
        AddContextArgs {
            server,
            name: if name.is_empty() { None } else { Some(name) },
            sso: method == "3",
            pat: method == "2",
            username: None,
            password: None,
            password_stdin: false,
            token: None,
            token_stdin: false,
            oauth_flow: OAuthFlow::Device,
            no_default: false,
        },
        output,
    )
}
fn execute_add_context(
    cfg: &mut config::Configuration,
    args: AddContextArgs,
    output: Option<OutputMode>,
) -> Result<(), Error> {
    use logsh_core::session::{LoginMethod, LoginRequest};
    let name = args
        .name
        .unwrap_or_else(|| derive_context_name(&args.server));
    let name = name.trim().to_owned();
    if name.is_empty() {
        return Err(anyhow!("Context name must not be empty."));
    }
    let can_prompt = interactive(output);
    let method = if args.sso {
        LoginMethod::Sso
    } else if args.pat {
        LoginMethod::Pat
    } else {
        LoginMethod::Password
    };
    let username = if method == LoginMethod::Password {
        match args.username {
            Some(u) => u,
            None if can_prompt => prompt("Username")?,
            None => {
                return Err(anyhow!(
                    "Provide --username and --password-stdin (or --password)."
                ))
            }
        }
    } else {
        String::new()
    };
    let secret = match method {
        LoginMethod::Password => {
            if args.password_stdin {
                read_stdin_line()?
            } else if let Some(p) = args.password {
                p
            } else if can_prompt {
                rpassword::prompt_password("Password: ")?
            } else {
                return Err(anyhow!("Provide --password-stdin or --password."));
            }
        }
        LoginMethod::Pat => {
            if args.token_stdin {
                read_stdin_line()?
            } else if let Some(t) = args.token {
                t
            } else if can_prompt {
                rpassword::prompt_password("Personal Access Token: ")?
            } else {
                return Err(anyhow!(
                    "Provide --token, --token-stdin, or LOGSH_PAT_TOKEN."
                ));
            }
        }
        LoginMethod::Sso => {
            if !can_prompt {
                return Err(anyhow!(
                    "SSO device login requires a terminal. Use PAT authentication for scripts."
                ));
            }
            String::new()
        }
    };
    let mut connection = logsh_core::session::authenticate(
        LoginRequest {
            server: args.server,
            method,
            username,
            secret,
        },
        cfg.contexts.get(&name).cloned(),
        &|event| {
            let logsh_core::auth::AuthEvent::DeviceCode { url, code } = event;
            eprintln!("Open {url} in your browser and enter code {code}.");
        },
        &std::sync::atomic::AtomicBool::new(false),
    )?;
    let accounts = connection.accounts(connection.user_id)?;
    if let Some(selector) = config::get_account_override() {
        let account = logsh_core::account::resolve_account(&accounts, selector)?;
        connection.default_account = Some(account.account_id);
        connection.default_account_name = Some(account.account_name.clone());
    } else if connection.default_account.is_none() && !accounts.is_empty() {
        if !can_prompt {
            return Err(anyhow!(
                "Choose an account with --account <name-or-id>; no configuration changed."
            ));
        }
        let mut accounts = accounts;
        accounts.sort_by(|a, b| {
            a.account_name
                .cmp(&b.account_name)
                .then(a.account_id.cmp(&b.account_id))
        });
        eprintln!("Select the account to use:");
        for (i, a) in accounts.iter().enumerate() {
            eprintln!("  {}. {} ({})", i + 1, a.account_name, a.account_id);
        }
        let choice = prompt("Account number, name, or UUID")?;
        let account = if let Ok(index) = choice.parse::<usize>() {
            accounts
                .get(index.wrapping_sub(1))
                .ok_or_else(|| anyhow!("Invalid account number"))?
        } else {
            logsh_core::account::resolve_account(&accounts, &choice)?
        };
        connection.default_account = Some(account.account_id);
        connection.default_account_name = Some(account.account_name.clone());
    }
    let account = connection.default_account_name.clone();
    let mut updated = cfg.clone();
    if !args.no_default || updated.contexts.is_empty() {
        updated.current_context = name.clone();
    }
    updated.contexts.insert(name.clone(), connection);
    config::save(updated.clone())?;
    *cfg = updated;
    match output {
        Some(OutputMode::Json) => crate::fmt::print_json(
            &serde_json::json!({"status":"ok", "context":name, "account":account}),
        ),
        Some(OutputMode::JsonPretty) => crate::fmt::print_json_pretty(
            &serde_json::json!({"status":"ok", "context":name, "account":account}),
        ),
        _ => println!(
            "Connected to {name}. Account: {}\nTry: logsh schema list",
            account.as_deref().unwrap_or("no accessible accounts")
        ),
    }
    Ok(())
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
