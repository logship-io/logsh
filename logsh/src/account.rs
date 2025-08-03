use anyhow::anyhow;
use clap::Subcommand;
use logsh_core::{
    logship_client::LogshClientHandler,
    account::{delete_account, list_accounts},
};
use term_table::{
    row::Row,
    table_cell::{Alignment, TableCell},
    Table,
};

use crate::query::markdown_style;

#[derive(Subcommand)]
#[clap(visible_alias = "acc", about = "Account management.")]
pub enum AccountCommand {
    #[clap(about = "List accounts", visible_alias = "ls")]
    List {
        #[arg(long, help = "Include all accounts.")]
        include_all: bool,
    },
    #[clap(about = "Set the default account for the current connection.")]
    Default {
        #[arg(help = "Account ID to set as default.")]
        id: uuid::Uuid,
    },
    #[clap(about = "Delete an account")]
    Delete {
        #[arg(help = "Account ID to delete.")]
        id: uuid::Uuid,
    },
}

pub fn execute_account(command: AccountCommand) -> Result<(), anyhow::Error> {
    match command {
        AccountCommand::Default { id } => {
            let default_config = logsh_core::config::load()?;
            let default_connection = default_config
                .get_default_connection()
                .ok_or(anyhow!("No default connection found."))?;
            let conn_handler = LogshClientHandler::new();

            let accounts =
                list_accounts(&conn_handler, default_connection.connection.user_id, false)?;

            let account = accounts
                .iter()
                .find(|s| s.account_id == id)
                .ok_or(anyhow!("Account not found."))?;

            let mut config = default_config;
            config.connections.iter_mut().for_each(|c| {
                if c.0 != default_connection.name.as_str() {
                    return;
                }

                c.1.default_account = Some(account.account_id);
            });
            logsh_core::config::save(config)?;

            println!(
                "Default account set to {} ({})",
                account.account_name, account.account_id
            );
            Ok(())
        }
        AccountCommand::Delete { id } => {
            let conn_handler = LogshClientHandler::new();
            delete_account(&conn_handler, id)?;
            Ok(())
        }
        AccountCommand::List { include_all } => {
            let default_config = logsh_core::config::load()?;
            let default_connection = default_config
                .get_default_connection()
                .ok_or(anyhow!("No default connection found."))?;
            let conn_handler = LogshClientHandler::new();

            let accounts = list_accounts(
                &conn_handler,
                default_connection.connection.user_id,
                include_all,
            )?;

            let mut table = Table::new();
            table.style = markdown_style();
            table.add_row(Row::new(vec![
                TableCell::builder("Name").col_span(1).alignment(Alignment::Left).build(),
                TableCell::builder("ID").col_span(1).alignment(Alignment::Left).build(),
                TableCell::builder("Default").col_span(1).alignment(Alignment::Left).build(),
            ]));

            for account in accounts {
                let is_default = default_connection
                    .connection
                    .default_account
                    .is_some_and(|s| s == account.account_id);
                table.add_row(Row::new(vec![
                    
                    TableCell::builder(&account.account_name).col_span(1).alignment(Alignment::Left).build(),
                    TableCell::builder(
                        &account.account_id.to_string()
                    ).col_span(1).alignment(Alignment::Left).build(),
                    TableCell::builder(
                        if is_default { "Yes" } else { "no" }
                    ).col_span(1).alignment(Alignment::Left).build(),
                ]));
            }

            println!("{}", table.render());
            Ok(())
        }
    }
}
