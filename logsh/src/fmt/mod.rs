use std::collections::HashMap;

use colored::Colorize;
use logsh_core::{
    config::Configuration,
    error::ConnectError,
};
use reqwest::StatusCode;
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Connection {
    pub name: String,
    pub server: String,
    pub is_default: bool,
    pub username: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DataFrame {
    pub headers: Vec<String>,
    pub data: Vec<HashMap<String, serde_json::Value>>,
}

pub fn print_connect_error(
    cfg: &Configuration,
    _name: &str,
    _conn: &logsh_core::connect::Connection,
    err: ConnectError,
) {
    match err {
        // ConnectError::Config(err) => format_config_error(err),
        ConnectError::NoConnection(str) => {
            println!(
                "Error: {}{}\" exists.",
                "No connection with name \"".red(),
                str.yellow().dimmed()
            );
            println!("{}   ", "# Execute logsh".bright_black())
        }
        ConnectError::Network(err) => match err.status() {
            Some(StatusCode::UNAUTHORIZED) => {
                println!("Status: {}", "Unauthorized".yellow());
                println!("Login with {}.", "logsh conn login".magenta().bold());
                if cfg.connections.len() > 1 {
                    println!(
                        "{} {} {}",
                        "# Execute".bright_black(),
                        "logsh conn ls".blue(),
                        "to view available connections.".bright_black()
                    );
                }

                print_add_connection_help();
            }
            Some(code) => {
                println!("Status: {} {}", format!("HTTP {}", code.as_u16()).yellow(), code.as_str().yellow());
                print_add_connection_help();
            },
            None => {
                println!("Status: {}", "Unable to connect".red());
                print_add_connection_help();
            },
        },
        err => {
            println!("Status: {} {}", "Error".red(), err.to_string().bright_red());
            print_add_connection_help();
        }
    }
}


pub fn print_add_connection_help() {
    println!(
        "{} {} {}",
        "# Execute".bright_black(),
        "logsh conn add --help".blue(),
        "for help with adding connections.".bright_black()
    );
}
