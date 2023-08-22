use log::debug;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::error::CliError;

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

fn get_configuration_path() -> Result<PathBuf, CliError> {
    let home_dir = home::home_dir().ok_or(CliError {
        message: "Unable to determine home directory".to_owned(),
        code: 1,
    })?;
    Ok(home_dir.join(Path::new(".logsh.json")))
}

pub fn get_configuration() -> Result<Configuration, CliError> {
    let config_path = get_configuration_path()?;
    if !config_path.exists() {
        return Ok(Configuration {
            connections: Vec::new(),
        });
    }

    debug!("Reading configuration from {:?}", config_path.to_str());
    let config_string = fs::read_to_string(&config_path).map_err(|e| CliError {
        message: format!("Unable to read configuration: {}", e),
        code: 1,
    })?;

    let config: Configuration = serde_json::from_str(&config_string).map_err(|e| CliError {
        message: format!("Unable to parse configuration: {}", e),
        code: 1,
    })?;
    Ok(config)
}

pub fn get_default_connection() -> Result<ConnectionInfo, CliError> {
    let config = get_configuration()?;
    let default_connection = config
        .connections
        .iter()
        .find(|c| c.default)
        .ok_or(CliError {
            message: "No default connection found.".to_owned(),
            code: 2,
        })?;
    Ok(default_connection.clone())
}

pub fn save_configuration(config: Configuration) -> Result<(), CliError> {
    let config_path = get_configuration_path()?;
    let serialized = serde_json::to_string(&config).map_err(|e| CliError {
        message: format!("Unable to serialize configuration: {}", e),
        code: 1,
    })?;
    fs::write(&config_path, serialized).map_err(|e| CliError {
        message: format!("Unable to write configuration: {}", e),
        code: 1,
    })?;
    debug!("Saved configuration to {:?}", config_path);
    Ok(())
}
