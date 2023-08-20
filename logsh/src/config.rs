use log::debug;
use serde::{Deserialize, Serialize};
use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
};

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

fn get_configuration_path() -> Result<PathBuf, Box<dyn Error>> {
    let home_dir = home::home_dir().ok_or("Unable to find home directory")?;
    Ok(home_dir.join(Path::new(".logsh.json")))
}

pub fn get_configuration() -> Result<Configuration, Box<dyn Error>> {
    let config_path = get_configuration_path()?;
    if !config_path.exists() {
        return Ok(Configuration {
            connections: Vec::new(),
        });
    }

    debug!("Reading configuration from {:?}", config_path.to_str());
    let config_string = fs::read_to_string(&config_path)?;

    let config: Configuration = serde_json::from_str(&config_string)?;
    Ok(config)
}

pub fn get_default_connection() -> Result<ConnectionInfo, Box<dyn Error>> {
    let config = get_configuration()?;
    let default_connection = config
        .connections
        .iter()
        .find(|c| c.default)
        .ok_or("No default connection found")?;
    Ok(default_connection.clone())
}

pub fn save_configuration(config: Configuration) -> Result<(), Box<dyn Error>> {
    let config_path = get_configuration_path()?;
    let serialized = serde_json::to_string(&config)?;
    fs::write(&config_path, serialized).expect("Unable to write file");
    debug!("Saved configuration to {:?}", config_path);
    Ok(())
}
