use std::collections::HashMap;

use annotate_snippets::{Level, Renderer, Snippet};

use colored::Colorize;
use logsh_core::reqwest::StatusCode;
use logsh_core::{
    config::Configuration,
    error::{ConfigError, ConnectError},
};
use serde::Serialize;

pub mod parse;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Connection {
    pub name: String,
    pub server: String,
    pub is_current_context: bool,
    pub username: String,
    pub current_account: String,
    pub accounts: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DataFrame {
    pub headers: Vec<String>,
    pub data: Vec<HashMap<String, serde_json::Value>>,
}

/// Write a compact JSON line to stdout.
pub fn print_json<T: Serialize>(value: &T) {
    if let Ok(json) = serde_json::to_string(value) {
        println!("{json}");
    }
}

/// Write a pretty-printed JSON value to stdout.
pub fn print_json_pretty<T: Serialize>(value: &T) {
    if let Ok(json) = serde_json::to_string_pretty(value) {
        println!("{json}");
    }
}

pub fn print_connect_error(cfg: &Configuration, err: &ConnectError) {
    match err {
        ConnectError::Config(err) => print_config_error(err),
        ConnectError::NoConnection(name) => {
            eprintln!("{} No context named \"{}\".", "Error:".red(), name.yellow());
            eprintln!(
                "Run {} to see available contexts.",
                "logsh context list".blue()
            );
        }
        ConnectError::Network(err) => print_reqwest_error(cfg, err),
        err => {
            eprintln!("{} {}", "Error:".red(), err.to_string().bright_red());
        }
    }
}

fn print_reqwest_error(_cfg: &Configuration, err: &logsh_core::reqwest::Error) {
    match err.status() {
        Some(StatusCode::UNAUTHORIZED) => {
            eprintln!("{} {}", "Error:".red(), "User Unauthorized".yellow());
            eprintln!(
                "Run {} to re-authenticate.",
                "logsh context login".magenta().bold()
            );
        }
        Some(code) => {
            eprintln!(
                "{} {} ({})",
                "Error:".red(),
                "Request failed".red(),
                code.as_str().yellow()
            );
        }
        None => {
            eprintln!(
                "{} {}",
                "Error:".red(),
                "Unable to connect — check your server URL.".red()
            );
        }
    }
}

pub fn print_add_connection_help() {
    eprintln!("Run {} to get started.", "logsh context add --help".blue(),);
}

pub(crate) fn print_config_error(err: &ConfigError) {
    eprintln!("{} {}", "Error:".red(), err.to_string().red());
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

            let mut snippet = Snippet::source(&extended_source).line_start(1).fold(true);

            for e in bad_request.errors.iter() {
                for t in e.tokens.iter() {
                    if let Some(label) = &e.message {
                        snippet = snippet.annotation(
                            Level::Error
                                .span(t.start as usize..t.end as usize)
                                .label(label.as_str()),
                        );
                    }
                }
            }

            let message = Level::Error.title(&bad_request.message).snippet(snippet);

            let renderer = Renderer::styled();
            eprintln!("{}", renderer.render(message));
        }
        logsh_core::error::QueryError::Connection(err) => print_connect_error(cfg, err),
        err => {
            eprintln!("{} {}", "Error:".red(), err.to_string().red());
        }
    }
}
