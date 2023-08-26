use thiserror::Error;

#[derive(Debug, Error)]
pub enum CliError {
    #[error("No command provided.")]
    NoCommandProvided,

    #[error("Password error {0}")]
    PasswordError(Box<dyn std::error::Error>),

    #[error("No connection found with name {0}")]
    NoNamedConnection(String),

    #[error("Unable to connect to server: {0}")]
    UnableToConnect(reqwest::Error),

    #[error("Unable to parse token response: {0}")]
    UnableToParseJwtToken(reqwest::Error),

    #[error("Unable to read configuration: {0}")]
    UnableToReadConfig(ConfigError),

    #[error("Unable to write configuration: {0}")]
    UnableToWriteConfig(ConfigError),

    #[error("FailedToParseQueryResult: {0}")]
    FailedToParseQueryResult(serde_json::Error),

    #[error("Query error: {0}")]
    QueryError(QueryError),
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Unable to determine home directory")]
    NoHome,

    #[error("Unable to read configuration: {0}")]
    FailedRead(std::io::Error),

    #[error("Unable to save configuration: {0}")]
    FailedWrite(std::io::Error),

    #[error("Unable to parse configuration: {0}")]
    FailedSerialize(serde_json::Error),

    #[error("No default connection found.")]
    NoDefaultConnection,
}

#[derive(Debug, Error)]
pub enum QueryError {
    #[error("No connection. {0}")]
    NoConnection(ConfigError),

    #[error("Query string was empty.")]
    NoInput,

    #[error("Unable to connect to server: {0}")]
    FailedToConnect(reqwest::Error),

    #[error("Failed to read from STDIN")]
    FailedRead(std::io::Error),

    #[error("Failed to write to STDOUT")]
    FailedWrite(std::io::Error),

    #[error("Failed to parse query response: {0}")]
    FailedToParseResponse(reqwest::Error),

    #[error("JSON Error: {0}")]
    JsonError(serde_json::Error),

    #[error("HTTP {0} | {1}")]
    HttpErrorStatus(reqwest::StatusCode, String),
}
