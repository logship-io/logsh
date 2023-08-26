use serde::Deserialize;
use std::collections::HashMap;
use thiserror::Error;
use uuid::Uuid;

use crate::{config, error::CliError};

#[derive(Deserialize)]
struct TokenResponse {
    token: String,
}

fn fetch_token(server: String, user: String, password: String) -> Result<String, CliError> {
    let mut map = HashMap::new();
    map.insert("username", user);
    map.insert("password", password);

    let client = reqwest::blocking::Client::new();
    let res = client
        .post(format!("{}/auth/token", server.trim_end_matches('/')))
        .json(&map)
        .send()
        .map_err(CliError::UnableToConnect)?;

    let token: TokenResponse = res.json().map_err(CliError::UnableToParseJwtToken)?;
    Ok(token.token)
}

pub fn set_default(name: String) -> Result<(), CliError> {
    let mut existing_config = config::get_configuration().map_err(CliError::UnableToReadConfig)?;
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
            return Err(CliError::NoNamedConnection(name));
        }
    }

    config::save_configuration(existing_config).map_err(CliError::UnableToReadConfig)
}

#[derive(Debug, Error)]
pub enum ConnectError<T: std::error::Error> {
    #[error("Client error: {0}")]
    CliError(crate::error::CliError),

    #[error("Callback error: {0}")]
    CallbackError(T),
}

pub fn connect<'a, E, F>(
    name: String,
    server: String,
    default: bool,
    user: String,
    password_cb: F,
) -> Result<(), ConnectError<E>>
where
    F: FnOnce() -> Result<String, E>,
    E: std::error::Error,
{
    log::debug!("Connecting to {} at {}", name, server);
    let mut existing_config = config::get_configuration()
        .map_err(CliError::UnableToReadConfig)
        .map_err(ConnectError::CliError)?;

    let should_default = default || existing_config.connections.is_empty();
    if should_default {
        existing_config
            .connections
            .iter_mut()
            .for_each(|c| c.default = false);
    }

    let pass = password_cb().map_err(ConnectError::CallbackError)?;
    let token = fetch_token(server.clone(), user, pass).map_err(ConnectError::CliError)?;
    log::debug!("Successfully received token");

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

    config::save_configuration(existing_config)
        .map_err(CliError::UnableToWriteConfig)
        .map_err(ConnectError::CliError)
}
