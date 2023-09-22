use oauth2::TokenResponse;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{connect::OAuthToken, error::ConfigError};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Connection {
    Jwt {
        name: String,
        server: String,
        token: String,
        default: bool,
        default_acccount_id: String,
    },
    OAuth {
        name: String,
        server: String,
        token: OAuthToken,
        default: bool,
        default_acccount_id: String,
    },
}

impl Connection {
    pub fn is_default(&self) -> bool {
        match self {
            Connection::Jwt { default, .. } => *default,
            Connection::OAuth { default, .. } => *default,
        }
    }

    pub fn set_default(&mut self, value: bool) {
        match self {
            Connection::Jwt { default, .. } => *default = value,
            Connection::OAuth { default, .. } => *default = value,
        };
    }

    pub fn name(&self) -> &String {
        match self {
            Connection::Jwt { name, .. } => name,
            Connection::OAuth { name, .. } => name,
        }
    }

    pub fn server(&self) -> &String {
        match self {
            Connection::Jwt { server, .. } => server,
            Connection::OAuth { server, .. } => server,
        }
    }

    pub fn default_acccount_id(&self) -> &String {
        match self {
            Connection::Jwt {
                default_acccount_id,
                ..
            } => default_acccount_id,
            Connection::OAuth {
                default_acccount_id,
                ..
            } => default_acccount_id,
        }
    }

    pub fn bearer_token(&self) -> &String {
        match self {
            Connection::Jwt { token, .. } => token,
            Connection::OAuth { token, .. } => token.access_token().secret(),
        }
    }

    /// Returns `true` if the connection is [`Jwt`].
    ///
    /// [`Jwt`]: Connection::Jwt
    #[must_use]
    pub fn is_jwt(&self) -> bool {
        matches!(self, Self::Jwt { .. })
    }
}

#[derive(Serialize, Deserialize)]
pub struct Configuration {
    pub connections: Vec<Connection>,
}

impl Configuration {
    pub fn upsert_connection(mut self, new: Connection) -> Result<(), ConfigError> {
        self.connections = self
            .connections
            .into_iter()
            .filter(|c| c.name() == new.name())
            .collect();
        self.connections.push(new);
        if self.connections.len() == 1 {
            self.connections[0].set_default(true);
        }

        save_configuration(self)
    }
}

fn get_configuration_path() -> Result<PathBuf, ConfigError> {
    let home_dir = home::home_dir().ok_or(ConfigError::NoHome)?;
    Ok(home_dir.join(Path::new(".logsh.json")))
}

pub fn get_configuration() -> Result<Configuration, ConfigError> {
    let config_path = get_configuration_path()?;
    if !config_path.exists() {
        return Ok(Configuration {
            connections: Vec::new(),
        });
    }

    log::debug!("Reading configuration from {:?}", config_path.to_str());
    let config_string = fs::read_to_string(&config_path).map_err(ConfigError::FailedRead)?;
    serde_json::from_str(&config_string).map_err(ConfigError::FailedSerialize)
}

pub fn get_default_connection() -> Result<Connection, ConfigError> {
    let config = get_configuration()?;
    let default_connection = config
        .connections
        .iter()
        .find(|c| c.is_default())
        .ok_or(ConfigError::NoDefaultConnection)?;
    Ok(default_connection.clone())
}

pub fn save_configuration(config: Configuration) -> Result<(), ConfigError> {
    let config_path = get_configuration_path()?;
    let serialized = serde_json::to_string(&config).map_err(ConfigError::FailedSerialize)?;
    fs::write(&config_path, serialized).map_err(ConfigError::FailedWrite)?;
    log::debug!("Saved configuration to {:?}", config_path);
    Ok(())
}
