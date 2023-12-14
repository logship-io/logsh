use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::OnceLock,
};

use crate::{connect::Connection, error::ConfigError};
static mut CONFIG_PATH: OnceLock<Result<PathBuf, ConfigError>> = OnceLock::new();

#[derive(Serialize, Deserialize, Clone)]
pub struct Configuration {
    pub default_connection: String,
    pub connections: HashMap<String, Connection>,
}

pub struct ConnectionConfig {
    pub name: String,
    pub connection: Connection,
}


impl Configuration {
    pub fn get_default_connection(&self) -> Option<ConnectionConfig> {
        if let Some(c) = self.connections.get(&self.default_connection) {
            return Some(ConnectionConfig { name: self.default_connection.clone(), connection: c.clone() });
        }

        
        let conn = self.connections.iter().next();
        if let Some((name, _conn)) = conn {
            log::warn!("Default connection \"{}\" does not exist. Updating to \"{}\".", &self.default_connection, name);
        }

        return Some(ConnectionConfig { name: self.default_connection.clone(), connection: conn.unwrap().1.clone() });
    }
}

impl Default for Configuration {
    fn default() -> Self {
        Self {
            default_connection: Default::default(),
            connections: Default::default(),
        }
    }
}

pub fn get_configuration_path() -> Result<PathBuf, ConfigError> {
    let path = unsafe {
        CONFIG_PATH.get_or_init(|| {
            if let Ok(path) = std::env::var("LOGSH_CONFIG_PATH") {
                if path.trim().len() > 0 {
                    log::trace!(
                        "Environment override of config path: {}={}",
                        "LOGSH_CONFIG_PATH",
                        &path
                    );

                    let path = PathBuf::from(&path);
                    if false == path.exists() {
                        return Err(ConfigError::InvalidConfigPath(format!(
                            "{} does not exist.",
                            &path.to_string_lossy()
                        )));
                    }

                    return Ok(path);
                }
            }

            let path = home::home_dir()
                .map(|mut h| {
                    h.push(Path::new(".logsh"));
                    h.push(Path::new("logsh-config.json"));
                    h
                })
                .ok_or(ConfigError::NoHome)?;
            log::trace!("Configuration path: {}", &path.display());
            if let Some(parent) = path.parent() {
                if false == parent.exists() {
                    log::debug!(
                        "Configuration parent doesn't exist. Creating: {}",
                        parent.display()
                    );
                    std::fs::create_dir_all(&parent)?;
                }
            }

            Ok(path)
        })
    };

    match path {
        Ok(p) => Ok(p.clone()),
        Err(_e) => {
            match unsafe { CONFIG_PATH.take() } {
                Some(path) => {
                    return path;
                }
                None => {
                    // wtf
                    return Err(ConfigError::InvalidConfigPath("unknown error".to_string()));
                }
            }
        }
    }
}

pub fn load() -> Result<Configuration, ConfigError> {
    let cfg = get_configuration_path()?;
    if cfg.exists() {
        let cfg = fs::read_to_string(cfg).map_err(ConfigError::FailedRead)?;
        let config = serde_json::from_str(&cfg).map_err(ConfigError::FailedDeserialize)?;
        return Ok(config);
    } else {
        return Ok(Configuration::default());
    }
}

pub fn save(config: Configuration) -> Result<Configuration, ConfigError> {
    let path = get_configuration_path()?;
    let serialized: String =
        serde_json::to_string(&config).map_err(ConfigError::FailedSerialize)?;
    fs::write(&path, serialized).map_err(ConfigError::FailedWrite)?;
    Ok(config)
}
