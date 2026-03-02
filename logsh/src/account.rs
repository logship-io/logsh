use anyhow::anyhow;
use clap::Subcommand;
use logsh_core::{
    account::{delete_account, list_accounts},
    logship_client::LogshClientHandler,
};
use serde::Serialize;
use term_table::{
    row::Row,
    table_cell::{Alignment, TableCell},
    Table,
};

use crate::{query::markdown_style, OutputMode};

#[derive(Subcommand)]
#[clap(visible_alias = "acc", about = "Account management.")]
pub enum AccountCommand {
    #[clap(about = "List accounts", visible_alias = "ls")]
    List {
        #[arg(long, help = "Include all accounts.")]
        include_all: bool,
    },
    #[clap(
        visible_alias = "switch",
        about = "Set the current account for this context."
    )]
    Use {
        #[arg(help = "Account name to switch to.")]
        name: String,
    },
    #[clap(about = "Show the current account")]
    Current,
    #[clap(about = "Delete an account")]
    Delete {
        #[arg(help = "Account ID to delete.")]
        id: uuid::Uuid,
    },
}

pub fn execute_account(
    command: AccountCommand,
    output: Option<OutputMode>,
) -> Result<(), anyhow::Error> {
    match command {
        AccountCommand::Use { name } => {
            let default_config = logsh_core::config::load()?;
            let default_connection = default_config
                .get_current_context()
                .ok_or(anyhow!("No current context found."))?;
            let conn_handler = LogshClientHandler::new();

            let accounts =
                list_accounts(&conn_handler, default_connection.connection.user_id, false)?;

            let account = accounts
                .iter()
                .find(|s| s.account_name.eq_ignore_ascii_case(&name))
                .ok_or(anyhow!(
                    "Account \"{}\" not found. Run 'logsh account list' to see available accounts.",
                    &name
                ))?;

            let mut config = default_config;
            config.contexts.iter_mut().for_each(|c| {
                if c.0 != default_connection.name.as_str() {
                    return;
                }

                c.1.default_account = Some(account.account_id);
                c.1.default_account_name = Some(account.account_name.clone());
                c.1.known_accounts = accounts.iter().map(|a| a.account_name.clone()).collect();
            });
            logsh_core::config::save(config)?;

            match output {
                Some(OutputMode::Json | OutputMode::JsonPretty) => {
                    crate::fmt::print_json(&serde_json::json!({
                        "status": "ok",
                        "message": format!("Current account set to {} ({})", account.account_name, account.account_id)
                    }));
                }
                _ => {
                    println!(
                        "Current account set to {} ({})",
                        account.account_name, account.account_id
                    );
                }
            }
            Ok(())
        }
        AccountCommand::Current => {
            let default_config = logsh_core::config::load()?;
            let ctx = default_config
                .get_current_context()
                .ok_or(anyhow!("No current context found."))?;

            match ctx.connection.default_account() {
                Some(account_id) => {
                    let conn_handler = LogshClientHandler::new();
                    let accounts = list_accounts(&conn_handler, ctx.connection.user_id, false)?;
                    match accounts.iter().find(|a| a.account_id == account_id) {
                        Some(account) => match output {
                            Some(OutputMode::Json | OutputMode::JsonPretty) => {
                                crate::fmt::print_json(&serde_json::json!({
                                    "accountName": account.account_name,
                                    "accountId": account.account_id.to_string(),
                                }));
                            }
                            _ => {
                                println!("{} ({})", account.account_name, account.account_id);
                            }
                        },
                        None => match output {
                            Some(OutputMode::Json | OutputMode::JsonPretty) => {
                                crate::fmt::print_json(&serde_json::json!({
                                    "accountId": account_id.to_string(),
                                }));
                            }
                            _ => println!("{account_id}"),
                        },
                    }
                }
                None => match output {
                    Some(OutputMode::Json | OutputMode::JsonPretty) => {
                        crate::fmt::print_json(&serde_json::json!({
                            "accountName": null,
                            "accountId": null,
                        }));
                    }
                    _ => println!("No current account set."),
                },
            }
            Ok(())
        }
        AccountCommand::Delete { id } => {
            let conn_handler = LogshClientHandler::new();
            delete_account(&conn_handler, id)?;
            if let Some(OutputMode::Json | OutputMode::JsonPretty) = output {
                crate::fmt::print_json(&serde_json::json!({
                    "status": "ok",
                    "message": format!("Account \"{id}\" deleted")
                }));
            }
            Ok(())
        }
        AccountCommand::List { include_all } => {
            let mut config = logsh_core::config::load()?;
            let default_connection = config
                .get_current_context()
                .ok_or(anyhow!("No current context found."))?;
            let conn_handler = LogshClientHandler::new();

            let accounts = list_accounts(
                &conn_handler,
                default_connection.connection.user_id,
                include_all,
            )?;

            // Refresh cached account names for shell completions
            if let Some(ctx) = config.contexts.get_mut(&default_connection.name) {
                ctx.known_accounts = accounts.iter().map(|a| a.account_name.clone()).collect();
                let _ = logsh_core::config::save(config.clone());
            }

            let effective = default_connection.connection.effective_account();

            match output {
                Some(OutputMode::Json) => {
                    #[derive(Serialize)]
                    #[serde(rename_all = "camelCase")]
                    struct AccountEntry {
                        name: String,
                        id: String,
                        is_current: bool,
                    }
                    let entries: Vec<AccountEntry> = accounts
                        .iter()
                        .map(|a| AccountEntry {
                            name: a.account_name.clone(),
                            id: a.account_id.to_string(),
                            is_current: effective.is_some_and(|s| s == a.account_id),
                        })
                        .collect();
                    crate::fmt::print_json(&entries);
                }
                Some(OutputMode::JsonPretty) => {
                    #[derive(Serialize)]
                    #[serde(rename_all = "camelCase")]
                    struct AccountEntry {
                        name: String,
                        id: String,
                        is_current: bool,
                    }
                    let entries: Vec<AccountEntry> = accounts
                        .iter()
                        .map(|a| AccountEntry {
                            name: a.account_name.clone(),
                            id: a.account_id.to_string(),
                            is_current: effective.is_some_and(|s| s == a.account_id),
                        })
                        .collect();
                    crate::fmt::print_json_pretty(&entries);
                }
                _ => {
                    let mut table = Table::new();
                    table.style = markdown_style();
                    table.add_row(Row::new(vec![
                        TableCell::builder("Name")
                            .col_span(1)
                            .alignment(Alignment::Left)
                            .build(),
                        TableCell::builder("ID")
                            .col_span(1)
                            .alignment(Alignment::Left)
                            .build(),
                        TableCell::builder("Current")
                            .col_span(1)
                            .alignment(Alignment::Left)
                            .build(),
                    ]));

                    for account in accounts {
                        let is_current = effective.is_some_and(|s| s == account.account_id);
                        table.add_row(Row::new(vec![
                            TableCell::builder(&account.account_name)
                                .col_span(1)
                                .alignment(Alignment::Left)
                                .build(),
                            TableCell::builder(account.account_id.to_string())
                                .col_span(1)
                                .alignment(Alignment::Left)
                                .build(),
                            TableCell::builder(if is_current { "Yes" } else { "no" })
                                .col_span(1)
                                .alignment(Alignment::Left)
                                .build(),
                        ]));
                    }

                    println!("{}", table.render());
                }
            }
            Ok(())
        }
    }
}
