use thiserror::Error;

#[derive(Debug, Error)]
pub enum CliError {
    #[error("No command provided.")]
    NoCommandProvided,

    #[error("Config Error: {0}")]
    Config(#[from] ConfigError),

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
pub enum CommonError {
    #[error("File not found: {0}")]
    FileNotFound(std::string::String),

    #[error("Argument {0} is empty")]
    EmptyArgument(std::string::String),

    #[error("End of file")]
    EndOfFile(),
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Unable to determine home directory")]
    NoHome,

    #[error("IO Error: {0}")]
    IOError(#[from] std::io::Error),

    #[error("Unable to use specified configuration path: {0}")]
    InvalidConfigPath(String),

    #[error("Unable to read configuration: {0}")]
    FailedRead(std::io::Error),

    #[error("Unable to save configuration: {0}")]
    FailedWrite(std::io::Error),

    #[error("Unable to serialize configuration: {0}")]
    FailedSerialize(serde_json::Error),

    #[error("Unable to deserialize configuration: {0}")]
    FailedDeserialize(serde_json::Error),

    #[error("No default connection found.")]
    NoDefaultConnection,
}

#[derive(Debug, Error)]
pub enum QueryError {
    #[error("No connection. {0}")]
    Config(#[from] ConfigError),

    #[error("Query string was empty.")]
    NoInput,

    #[error("Failed to read from STDIN")]
    FailedRead(std::io::Error),

    #[error("Failed to write to STDOUT")]
    FailedWrite(std::io::Error),

    #[error("Request Error: {0}")]
    Request(#[from] reqwest::Error),

    #[error("JSON Error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Bad request: {0}")]
    BadRequest(String),
}

#[derive(Debug, Error)]
pub enum UploadError {
    #[error("{0}")]
    Common(CommonError),
    #[error("{0}")]
    Config(ConfigError),
    #[error("Unsupported file extension: {0}")]
    UnsupportedFileExtension(String),
    #[error("Failed to read file: {0}")]
    FailedToReadFile(std::io::Error),
    #[error("Failed to read file: {0}")]
    FailedToReadFileContent(serde_json::Error),

    #[error("Failed to upload: {0}")]
    UploadFailure(reqwest::Error),
    #[error("Failed to upload, status: {0}, message: {1}")]
    UploadFailureStatus(i32, String),
}

#[derive(Debug, Error)]
pub enum ConnectError {
    #[error("Configuration Error: {0}")]
    Config(#[from] ConfigError),

    #[error("No connection exists with name \"{0}\".")]
    NoConnection(String),

    #[error("Network Error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("Auth Error: {0}")]
    Auth(#[from] AuthError),

    #[error("Client error: {0}")]
    CliError(crate::error::CliError),

    #[error("HTTP Response Failed: {0}")]
    HttpResponseFailed(reqwest::StatusCode),

    #[error("Authentication is not configured for this connection.")]
    NoAuthentication,

    #[error("JSON Error: {0}")]
    HttpError(reqwest::Error),

    #[error("Invalid OAuth Configuration: {0}")]
    InvalidConfigError(String),
}

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("An error occurred with the request: {0}")]
    Network(#[from] reqwest::Error),

    #[error("The specified authentication has timed out and cannot be automatically refreshed.")]
    Expired,

    #[error("Basic Auth Error: {0}")]
    BasicAuth(#[from] BasicAuthError),

    #[error("OAuth Error: {0}")]
    OAuth(#[from] OAuthError),
}

#[derive(Debug, Error)]
pub enum BasicAuthError {
    #[error("IO Error: {0}")]
    IOError(#[from] std::io::Error),
}

#[derive(Debug, Error)]
pub enum OAuthError {
    #[error("URL Parse Error: {0}")]
    ParseError(#[from] oauth2::url::ParseError),

    #[error("Configuration Error: {0}")]
    ConfigurationError(#[from] oauth2::ConfigurationError),

    #[error("Request Token Error: {0}")]
    DeviceTokenErrorResponse(
        #[from]
        oauth2::RequestTokenError<
            oauth2::reqwest::Error<reqwest::Error>,
            oauth2::StandardErrorResponse<oauth2::basic::BasicErrorResponseType>,
        >,
    ),

    #[error("Request Token Error: {0}")]
    TokenErrorResponse(
        #[from]
        oauth2::RequestTokenError<
            oauth2::reqwest::Error<reqwest::Error>,
            oauth2::StandardErrorResponse<oauth2::DeviceCodeErrorResponseType>,
        >,
    ),

    #[error("Missing or empty endpoint: {0}")]
    MissingEndpoint(String),
}

#[derive(Debug, Error)]
pub enum LoginError {
    #[error("Client error: {0}")]
    CliError(crate::error::CliError),

    #[error("Configuration error during login: {0}")]
    ConfigError(#[from] ConfigError),

    #[error("HTTP Response Failed: {0}")]
    HttpResponseFailed(reqwest::StatusCode),

    #[error("OAuth2 not configured on this server.")]
    NoOAuthConfiguration,

    #[error("JSON Error: {0}")]
    HttpError(reqwest::Error),

    #[error("Invalid OAuth Configuration: {0}")]
    InvalidConfigError(String),

    #[error("OAuth Failed. No tokens in response.")]
    TokenResponseError,
}
