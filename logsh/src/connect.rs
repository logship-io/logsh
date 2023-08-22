use clap::{arg, Subcommand};
use log::debug;
use serde::Deserialize;
use std::collections::HashMap;
use term_table::{
    row::Row,
    table_cell::{Alignment, TableCell},
    Table,
};
use uuid::Uuid;

use crate::{config, error::CliError};

#[derive(Deserialize)]
struct TokenResponse {
    token: String,
}

#[derive(Subcommand, Debug)]
#[clap(about = "Connect to a Logship server.")]
pub enum ConnectCommand {
    #[clap(about = "Add a new connection.")]
    Add {
        #[arg(help = "Name of the connection. Just for your own reference.")]
        name: String,
        #[arg(help = "Logship server. e.g. https://logship.io")]
        server: String,
        #[arg(long, help = "Set this connection as the default connection.")]
        default: bool,

        #[arg(short, long, help = "Username to authenticate with.")]
        user: String,

        #[arg(short, long, help = "Password to authenticate with.")]
        password: String,
    },
    #[clap(about = "List existing connections.")]
    List {},
    #[clap(about = "Modify the currently defaulted connection.")]
    Default {
        #[arg(help = "Name of the connection to set as default.")]
        name: String,
    },
}

pub fn execute_connect(command: ConnectCommand) -> Result<(), CliError> {
    match command {
        ConnectCommand::Add {
            name,
            server,
            default,
            user,
            password,
        } => connect(name, server, default, user, password),
        ConnectCommand::List {} => list(),
        ConnectCommand::Default { name } => set_default(name),
    }
}

fn fetch_token(server: String, user: String, password: String) -> Result<String, CliError> {
    let mut map = HashMap::new();
    map.insert("username", user);
    map.insert("password", password);

    let client = reqwest::blocking::Client::new();
    let res = client
        .post(server + "/auth/token")
        .json(&map)
        .send()
        .map_err(|e| CliError {
            message: format!("Unable to connect to server: {}", e),
            code: 1,
        })?;

    let token: TokenResponse = res.json().map_err(|e| CliError {
        message: format!("Unable to parse token response: {}", e),
        code: 1,
    })?;
    Ok(token.token)
}

pub fn set_default(name: String) -> Result<(), CliError> {
    let mut existing_config = config::get_configuration().map_err(|e| CliError {
        message: format!("Unable to read configuration: {}", e),
        code: 1,
    })?;
    existing_config
        .connections
        .iter_mut()
        .for_each(|c| c.default = false);

    let connection = existing_config
        .connections
        .iter_mut()
        .find(|c| c.name == name);
    match connection {
        Some(connection) => {
            connection.default = true;
        }
        None => {
            return Err(CliError {
                message: format!("No connection found with name {}", name),
                code: 1,
            });
        }
    }

    config::save_configuration(existing_config).map_err(|e| CliError {
        message: format!("Unable to save configuration: {}", e),
        code: 1,
    })
}

pub fn list() -> Result<(), CliError> {
    let existing_config = config::get_configuration().map_err(|e| CliError {
        message: format!("Unable to read configuration: {}", e),
        code: 1,
    })?;
    let mut table = Table::new();
    table.add_row(Row::new(vec![
        TableCell::new_with_alignment("Name", 1, Alignment::Center),
        TableCell::new_with_alignment("Server", 1, Alignment::Center),
        TableCell::new_with_alignment("Default", 1, Alignment::Center),
    ]));
    existing_config.connections.iter().for_each(|f| {
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
) -> Result<(), CliError> {
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
                default_acccount_id: Uuid::nil().to_string(),
            });
        }
    }

    config::save_configuration(existing_config)?;
    Ok(())
}
