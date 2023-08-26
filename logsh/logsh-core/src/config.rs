use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::error::ConfigError;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ConnectionInfo {
    pub name: String,
    pub server: String,
    pub token: String,
    pub default: bool,
    pub default_acccount_id: String,
}

#[derive(Serialize, Deserialize)]
pub struct Configuration {
    pub connections: Vec<ConnectionInfo>,
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

pub fn get_default_connection() -> Result<ConnectionInfo, ConfigError> {
    let config = get_configuration()?;
    let default_connection = config
        .connections
        .iter()
        .find(|c| c.default)
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
