use anyhow::Error;
use clap::Parser;
use colored::Colorize;
use logsh_core::config;

#[derive(Parser)]
#[command(about = "Show current user and connection information")]
pub struct WhoamiCommand {}

pub fn execute_whoami(_command: WhoamiCommand) -> Result<(), Error> {
    let cfg = config::load()?;
    let conn = cfg.get_default_connection();
    
    match conn {
        Some(conn) => match conn.connection.who_am_i() {
            Ok(user) => {
                let sub = conn
                    .connection
                    .default_subscription()
                    .map_or("None".to_string(), |s| s.to_string());
                println!("Status: {}", "Connected".green());
                println!(
                    "Logged into connection {} as user {} with subscription: {}",
                    &conn.name.blue(),
                    &user.user_name.blue(),
                    sub.blue()
                );
                Ok(())
            }
            Err(err) => {
                println!("Status: {}", "Not Connected".red());
                crate::fmt::print_connect_error(&cfg, &err);
                Err(anyhow::anyhow!("Not logged in: {err}"))
            }
        },
        None => {
            println!(
                "Status: {} {}",
                "No connections configured.".red(),
                "Configuration Required.".red()
            );
            crate::fmt::print_add_connection_help();
            Err(anyhow::anyhow!("No connections configured"))
        }
    }
}