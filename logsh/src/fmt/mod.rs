use std::collections::HashMap;

use annotate_snippets::{Annotation, AnnotationType, Renderer, Slice, Snippet, SourceAnnotation};

use colored::Colorize;
use logsh_core::{
    config::Configuration,
    error::{ConfigError, ConnectError},
    query::{ErrorMessage, ErrorToken},
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
        logsh_core::error::QueryError::BadRequest(bad_request) => {
            let mut annotations = Vec::new();
            for e in bad_request.errors.iter() {
                for t in e.tokens.iter() {
                    let annotation = to_source_annotation(e, t);
                    annotations.push(annotation);
                }
            }

            // This is stupid, but the library we're using is stupid.
            // You can't highlight an error which goes all the way tot he end of the line.
            // So add a tiny space to the end of the line.
            let extended_source = query.to_string() + " ";
            let snippy = Snippet {
                title: Some(Annotation {
                    label: Some(bad_request.message.as_str()),
                    id: None,
                    annotation_type: AnnotationType::Error,
                }),
                footer: vec![],
                slices: vec![Slice {
                    source: extended_source.as_str(),
                    line_start: 0,
                    origin: None,
                    fold: true,
                    annotations,
                }],
            };

            let renderer = Renderer::styled();
            println!("{}", renderer.render(snippy));
        }
        logsh_core::error::QueryError::Connection(err) => print_connect_error(cfg, err),
        err => {
            println!("{} {}", "Error:".red(), err.to_string().red(),);
        }
    }
}

fn to_source_annotation<'a>(msg: &'a ErrorMessage, e: &'a ErrorToken) -> SourceAnnotation<'a> {
    SourceAnnotation {
        label: msg.message.as_ref().unwrap().as_str(),
        annotation_type: AnnotationType::Error,
        range: (e.start as usize + 1, e.end as usize + 1),
    }
}
