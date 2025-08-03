use anyhow::anyhow;
use clap::Subcommand;
use logsh_core::{
    logship_client::LogshClientHandler,
    subscription::{delete_subscription, list_subscriptions},
};
use term_table::{
    row::Row,
    table_cell::{Alignment, TableCell},
    Table,
};

use crate::query::markdown_style;

#[derive(Subcommand)]
#[clap(visible_alias = "sub", about = "Subscription management.")]
pub enum SubscriptionCommand {
    #[clap(about = "List subscriptions", visible_alias = "ls")]
    List {
        #[arg(long, help = "Include all subscriptions.")]
        include_all: bool,
    },
    #[clap(about = "Set the default subscription for the current connection.")]
    Default {
        #[arg(help = "Subscription ID to set as default.")]
        id: uuid::Uuid,
    },
    #[clap(about = "Delete a subscription")]
    Delete {
        #[arg(help = "Subscription ID to delete.")]
        id: uuid::Uuid,
    },
}

pub fn execute_subscription(command: SubscriptionCommand) -> Result<(), anyhow::Error> {
    match command {
        SubscriptionCommand::Default { id } => {
            let default_config = logsh_core::config::load()?;
            let default_connection = default_config
                .get_default_connection()
                .ok_or(anyhow!("No default connection found."))?;
            let conn_handler = LogshClientHandler::new();

            let subscriptions =
                list_subscriptions(&conn_handler, default_connection.connection.user_id, false)?;

            let subscription = subscriptions
                .iter()
                .find(|s| s.account_id == id)
                .ok_or(anyhow!("Subscription not found."))?;

            let mut config = default_config;
            config.connections.iter_mut().for_each(|c| {
                if c.0 != default_connection.name.as_str() {
                    return;
                }

                c.1.default_subscription = Some(subscription.account_id);
            });
            logsh_core::config::save(config)?;

            println!(
                "Default subscription set to {} ({})",
                subscription.account_name, subscription.account_id
            );
            Ok(())
        }
        SubscriptionCommand::Delete { id } => {
            let conn_handler = LogshClientHandler::new();
            delete_subscription(&conn_handler, id)?;
            Ok(())
        }
        SubscriptionCommand::List { include_all } => {
            let default_config = logsh_core::config::load()?;
            let default_connection = default_config
                .get_default_connection()
                .ok_or(anyhow!("No default connection found."))?;
            let conn_handler = LogshClientHandler::new();

            let subscriptions = list_subscriptions(
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

            for subscription in subscriptions {
                let is_default = default_connection
                    .connection
                    .default_subscription
                    .is_some_and(|s| s == subscription.account_id);
                table.add_row(Row::new(vec![
                    
                    TableCell::builder(&subscription.account_name).col_span(1).alignment(Alignment::Left).build(),
                    TableCell::builder(
                        &subscription.account_id.to_string()
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
