use std::collections::HashMap;

use annotate_snippets::{Level, Renderer, Snippet};

use colored::Colorize;
use logsh_core::{
    config::Configuration,
    error::{ConfigError, ConnectError},
};
use reqwest::StatusCode;
use serde::Serialize;

pub mod parse;

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

pub fn print_connect_error(cfg: &Configuration, err: &ConnectError) {
    match err {
        ConnectError::Config(err) => print_config_error(err),
        ConnectError::NoConnection(str) => {
            println!(
                "Error: {}{}\" exists.",
                "No connection with name \"".red(),
                str.yellow().dimmed()
            );
            println!("{}   ", "# Execute logsh".bright_black())
        }
        ConnectError::Network(err) => print_reqwest_error(cfg, err),
        err => {
            println!("{} {}", "Error:".red(), err.to_string().bright_red());
            print_add_connection_help();
        }
    }
}

fn print_reqwest_error(cfg: &Configuration, err: &reqwest::Error) {
    match err.status() {
        Some(StatusCode::UNAUTHORIZED) => {
            println!("{} {}", "Error:".red(), "User Unauthorized".yellow());
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
            println!("{} {}", "Error:".red(), code.as_str().yellow());
            print_add_connection_help();
        }
        None => {
            println!("{} {}", "Error:".red(), "Unable to connect".red());
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

pub(crate) fn print_config_error(err: &ConfigError) {
    println!("{} {}", "Error:".red(), err.to_string().red(),);
}

pub(crate) fn print_query_error(
    cfg: &Configuration,
    query: &str,
    err: &logsh_core::error::QueryError,
) {
    match err {
        logsh_core::error::QueryError::Config(err) => print_config_error(err),
        logsh_core::error::QueryError::Request(err) => print_reqwest_error(cfg, err),
        logsh_core::error::QueryError::Common(logsh_core::error::CommonError::ApiError(
            bad_request,
        )) => {
            // This is stupid, but the library we're using is stupid.
            // You can't highlight an error which goes all the way tot he end of the line.
            // So add a tiny space to the end of the line.
            let extended_source = query.to_string() + " ";
            
            let mut snippet = Snippet::source(&extended_source)
                .line_start(1)
                .fold(true);

            for e in bad_request.errors.iter() {
                for t in e.tokens.iter() {
                    if let Some(label) = &e.message {
                        snippet = snippet.annotation(
                            Level::Error
                                .span(t.start as usize..t.end as usize)
                                .label(label.as_str())
                        );
                    }
                }
            }

            let message = Level::Error
                .title(&bad_request.message)
                .snippet(snippet);

            let renderer = Renderer::styled();
            println!("{}", renderer.render(message));
        }
        logsh_core::error::QueryError::Connection(err) => print_connect_error(cfg, err),
        err => {
            println!("{} {}", "Error:".red(), err.to_string().red(),);
        }
    }
}

