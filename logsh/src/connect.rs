use clap::{Subcommand, arg};
use log::debug;
use serde::Deserialize;
use std::{collections::HashMap, error::Error};
use term_table::{Table, table_cell::{TableCell, Alignment}, row::Row};

use crate::config;

#[derive(Deserialize)]
struct TokenResponse {
    token: String,
}

#[derive(Subcommand, Debug)]
pub enum ConnectCommand {
    Add{
        name: String,
        server: String,
        #[arg(long)]
        default: bool,

        #[arg(short, long)]
        user: String,

        #[arg(short, long)]
        password: String,
    },
    List{

    }
}

pub fn execute_connect(command: ConnectCommand) -> Result<(), Box<dyn Error>> {
    match command {
        ConnectCommand::Add {
            name,
            server,
            default,
            user,
            password,
        } => {
            return connect(name, server, default, user, password);
        }
        ConnectCommand::List {} => {
            return list();
        }
    }
}

fn fetch_token(server: String, user: String, password: String) -> Result<String, Box<dyn Error>> {
    let mut map = HashMap::new();
    map.insert("username", user);
    map.insert("password", password);

    let client = reqwest::blocking::Client::new();
    let res = client
        .post(server + "/auth/token")
        .json(&map)
        .send()
        .unwrap();

    let token: TokenResponse = res.json()?;
    Ok(token.token)
}

pub fn list() -> Result<(), Box<dyn Error>> {
    let existing_config = config::get_configuration()?;
    let mut table = Table::new();
    table.add_row(Row::new(
        vec![TableCell::new_with_alignment("Name", 1, Alignment::Center),
        TableCell::new_with_alignment("Server", 1, Alignment::Center),
        TableCell::new_with_alignment("Default", 1, Alignment::Center)]));
    existing_config.connections.iter()
        .for_each(|f| {
            table.add_row(Row::new(vec![
                TableCell::new_with_alignment(&f.name, 1, Alignment::Center),
                TableCell::new_with_alignment(&f.server, 1, Alignment::Center),
                TableCell::new_with_alignment(&f.default.to_string(), 1, Alignment::Center),
            ]));
        });

    println!("{}", table.render());
    Ok(())
}

pub fn connect(
    name: String,
    server: String,
    default: bool,
    user: String,
    password: String,
) -> Result<(), Box<dyn Error>> {
    debug!("Connecting to {} at {}", name, server);

    let mut existing_config = config::get_configuration()?;
    let should_default = default || existing_config.connections.is_empty();
    if should_default {
        existing_config
            .connections
            .iter_mut()
            .for_each(|c| c.default = false);
    }

    let token = fetch_token(server.clone(), user, password)?;
    debug!("Successfully received token");

    let existing_connection = existing_config
        .connections
        .iter_mut()
        .find(|c| c.name == name);
    match existing_connection {
        Some(connection) => {
            connection.server = server;
            connection.default = should_default;
            connection.token = token;
        }
        None => {
            existing_config.connections.push(config::ConnectionInfo {
                name,
                server,
                default: should_default,
                token,
            });
        }
    }

    config::save_configuration(existing_config)?;
    Ok(())
}
