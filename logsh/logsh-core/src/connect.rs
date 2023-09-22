use oauth2::basic::{BasicClient, BasicTokenType};
use oauth2::devicecode::StandardDeviceAuthorizationResponse;
use oauth2::reqwest::http_client;
use oauth2::{
    AuthUrl, ClientId, DeviceAuthorizationUrl, EmptyExtraTokenFields, Scope, StandardTokenResponse,
    TokenUrl,
};
use reqwest::StatusCode;
use serde::Deserialize;
use std::collections::HashMap;
use thiserror::Error;
use uuid::Uuid;

use crate::config::Connection;
use crate::error::ConfigError;
use crate::{config, error::CliError};

pub type OAuthToken = StandardTokenResponse<EmptyExtraTokenFields, BasicTokenType>;

#[derive(Deserialize)]
struct TokenResponse {
    token: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OAuthConfigResponse {
    client_id: String,
    authorize_endpoint: String,
    device_endpoint: String,
    token_endpoint: String,
    scopes: Vec<String>,
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
        .for_each(|c| c.set_default(false));

    let connection = existing_config
        .connections
        .iter_mut()
        .find(|c| c.name() == &name);

    match connection {
        Some(connection) => {
            connection.set_default(true);
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

    #[error("HTTP Response Failed: {0}")]
    HttpResponseFailed(reqwest::StatusCode),

    #[error("OAuth2 not configured on this server.")]
    NoOauthConfiguration,

    #[error("JSON Error: {0}")]
    HttpError(reqwest::Error),

    #[error("Invalid OAuth Configuration: {0}")]
    InvalidConfigError(String),
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
            .for_each(|c| c.set_default(false));
    }

    let pass = password_cb().map_err(ConnectError::CallbackError)?;
    let token = fetch_token(server.clone(), user, pass).map_err(ConnectError::CliError)?;
    log::debug!("Successfully received token");

    let mut existing_connection = existing_config
        .connections
        .iter_mut()
        .find(|c| c.name() == &name);
    let server_name = server.clone();
    let token_value = token.clone();

    if let Some(c) = &existing_connection {
        if false == c.is_jwt() {
            existing_config.connections = existing_config
                .connections
                .into_iter()
                .filter(|c| c.name() != &name)
                .collect();
            existing_connection = None;
        }
    }

    match existing_connection {
        Some(Connection::Jwt {
            server,
            token,
            default,
            ..
        }) => {
            *server = server_name;
            *default = should_default;
            *token = token_value;
        }
        _ => {
            existing_config.connections.push(config::Connection::Jwt {
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

pub fn login(name: String, server: String, default: bool) -> Result<(), LoginError> {
    let client = reqwest::blocking::Client::new();
    let res = client
        .get(format!("{}/auth/oauth", server.trim_end_matches('/')))
        .send()
        .map_err(CliError::UnableToConnect)
        .map_err(LoginError::CliError)?;
    if false == res.status().is_success() {
        return Err(LoginError::HttpResponseFailed(res.status()));
    }

    if res.status() == StatusCode::NO_CONTENT {
        return Err(LoginError::NoOAuthConfiguration);
    }

    let json = res
        .json::<OAuthConfigResponse>()
        .map_err(LoginError::HttpError)?;

    let device_auth_url = DeviceAuthorizationUrl::new(json.device_endpoint).map_err(|e| {
        LoginError::InvalidConfigError(format!("Invalid Authorize Endpoint: {}", e))
    })?;
    let client = BasicClient::new(
        ClientId::new(json.client_id),
        None,
        AuthUrl::new(json.authorize_endpoint).map_err(|e| {
            LoginError::InvalidConfigError(format!("Invalid Authorize Endpoint: {}", e))
        })?,
        Some(TokenUrl::new(json.token_endpoint).map_err(|e| {
            LoginError::InvalidConfigError(format!("Invalid Token Endpoint: {}", e))
        })?),
    )
    .set_device_authorization_url(device_auth_url);

    let details: StandardDeviceAuthorizationResponse = client
        .exchange_device_code()
        .map_err(|e| {
            LoginError::InvalidConfigError(format!("Invalid config for device auth: {}", e))
        })?
        .add_scopes(json.scopes.into_iter().map(|s| Scope::new(s)))
        .request(http_client)
        .map_err(|e| {
            LoginError::InvalidConfigError(format!("Failed to get device auth code: {}", e))
        })?;

    println!(
        "Open this URL in your browser:\n{}\nand enter the code: {}",
        details.verification_uri().to_string(),
        details.user_code().secret().to_string()
    );

    let token_result = client
        .exchange_device_access_token(&details)
        .request(http_client, std::thread::sleep, None)
        .map_err(|e| {
            LoginError::InvalidConfigError(format!("Failed to get device auth access token: {}", e))
        })?;

    let connection = Connection::OAuth {
        name,
        server,
        token: token_result,
        default,
        default_acccount_id: Uuid::nil().to_string(),
    };

    let config = config::get_configuration()?;
    config.upsert_connection(connection)?;
    Ok(())
}
