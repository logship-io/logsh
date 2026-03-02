use anyhow::Error;
use clap::Parser;
use colored::Colorize;
use logsh_core::config;

use crate::OutputMode;

#[derive(Parser)]
#[command(about = "Show current user and connection information")]
pub struct WhoamiCommand {}

pub fn execute_whoami(_command: WhoamiCommand, output: Option<OutputMode>) -> Result<(), Error> {
    let cfg = config::load()?;
    let conn = cfg.get_current_context();

    match conn {
        Some(conn) => match conn.connection.who_am_i() {
            Ok(user) => {
                let sub = conn
                    .connection
                    .effective_account()
                    .map_or("None".to_string(), |s| s.to_string());
                match output {
                    Some(OutputMode::Json | OutputMode::JsonPretty) => {
                        crate::fmt::print_json(&serde_json::json!({
                            "status": "connected",
                            "connection": conn.name,
                            "user": user.user_name,
                            "account": sub,
                        }));
                    }
                    _ => {
                        println!("Status: {}", "Connected".green());
                        println!(
                            "Connection: {}  User: {}  Account: {}",
                            &conn.name.blue(),
                            &user.user_name.blue(),
                            sub.blue()
                        );
                    }
                }
                Ok(())
            }
            Err(err) => {
                crate::fmt::print_connect_error(&cfg, &err);
                Err(anyhow::anyhow!("Not logged in: {err}"))
            }
        },
        None => {
            crate::fmt::print_add_connection_help();
            Err(anyhow::anyhow!("No connections configured"))
        }
    }
}
