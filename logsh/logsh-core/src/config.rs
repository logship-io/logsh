use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::OnceLock,
};

use crate::{connect::Connection, error::ConfigError};
static CONFIG_PATH: OnceLock<Result<PathBuf, ConfigError>> = OnceLock::new();

static CONTEXT_OVERRIDE: OnceLock<String> = OnceLock::new();
static ACCOUNT_OVERRIDE: OnceLock<String> = OnceLock::new();

/// Sets the global context override (from --context/--ctx flag).
pub fn set_context_override(name: String) {
    let _ = CONTEXT_OVERRIDE.set(name);
}

/// Returns the global context override, if set.
pub fn get_context_override() -> Option<&'static str> {
    CONTEXT_OVERRIDE.get().map(|s| s.as_str())
}

/// Sets the global account override (from --account flag).
pub fn set_account_override(name: String) {
    let _ = ACCOUNT_OVERRIDE.set(name);
}

/// Returns the global account override, if set.
pub fn get_account_override() -> Option<&'static str> {
    ACCOUNT_OVERRIDE.get().map(|s| s.as_str())
}

/// Top-level configuration containing named contexts.
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct Configuration {
    pub current_context: String,
    pub contexts: HashMap<String, Connection>,
}

/// A named context paired with its connection data.
pub struct ContextConfig {
    pub name: String,
    pub connection: Connection,
}

impl Configuration {
    /// Returns the active context, respecting the global override, then falling back
    /// to the configured current context, then the first available context.
    pub fn get_current_context(&self) -> Option<ContextConfig> {
        if self.contexts.is_empty() {
            return None;
        }

        // Check global override first
        if let Some(override_name) = get_context_override() {
            if let Some(c) = self.contexts.get(override_name) {
                return Some(ContextConfig {
                    name: override_name.to_string(),
                    connection: c.clone(),
                });
            }
            log::warn!("Context override \"{override_name}\" does not exist, falling back to current context.");
        }

        if let Some(c) = self.contexts.get(&self.current_context) {
            return Some(ContextConfig {
                name: self.current_context.clone(),
                connection: c.clone(),
            });
        }

        let conn = self.contexts.iter().next();
        if let Some((name, _conn)) = conn {
            log::warn!(
                "Current context \"{}\" does not exist. Updating to \"{}\".",
                &self.current_context,
                name
            );
        }

        match conn {
            Some((name, conn)) => Some(ContextConfig {
                name: name.to_string(),
                connection: conn.clone(),
            }),
            None => {
                log::error!("No contexts found.");
                None
            }
        }
    }
}

/// Returns the path to the logsh configuration file, creating parent directories if needed.
pub fn get_configuration_path() -> Result<PathBuf, ConfigError> {
    let path = CONFIG_PATH.get_or_init(|| {
        if let Ok(path) = std::env::var("LOGSH_CONFIG_PATH") {
            if !path.trim().is_empty() {
                log::trace!(
                    "Environment override of config path: {}={}",
                    "LOGSH_CONFIG_PATH",
                    &path
                );

                let path = PathBuf::from(&path);
                if !path.exists() {
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
                h.push(Path::new("config.json"));
                h
            })
            .ok_or(ConfigError::NoHome)?;
        log::trace!("Configuration path: {}", &path.display());
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                log::debug!(
                    "Configuration parent doesn't exist. Creating: {}",
                    parent.display()
                );
                std::fs::create_dir_all(parent)?;
            }
        }

        Ok(path)
    });

    match path {
        Ok(p) => Ok(p.into()),
        Err(_) => Err(ConfigError::InvalidConfigPath(
            "Failed to get config path".to_string(),
        )),
    }
}

/// Loads the configuration from disk, returning a default if the file does not exist.
pub fn load() -> Result<Configuration, ConfigError> {
    let cfg = get_configuration_path()?;
    if cfg.exists() {
        let cfg = fs::read_to_string(cfg).map_err(ConfigError::FailedRead)?;
        let config = serde_json::from_str(&cfg).map_err(ConfigError::FailedDeserialize)?;
        Ok(config)
    } else {
        Ok(Configuration::default())
    }
}

/// Serializes and writes the configuration to disk.
pub fn save(config: Configuration) -> Result<Configuration, ConfigError> {
    let path = get_configuration_path()?;
    let serialized: String =
        serde_json::to_string(&config).map_err(ConfigError::FailedSerialize)?;
    fs::write(&path, serialized).map_err(ConfigError::FailedWrite)?;
    Ok(config)
}
