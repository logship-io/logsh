use std::collections::HashSet;

use chrono::{DateTime, Utc};
use oauth2::{
    basic::{BasicClient, BasicTokenType},
    AuthUrl, ClientId, DeviceAuthorizationUrl, EmptyExtraTokenFields, Scope,
    StandardDeviceAuthorizationResponse, StandardTokenResponse, TokenUrl,
};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

use crate::{
    connect::Connection,
    error::{AuthError, ConnectError, OAuthError},
};

use super::AuthData;

/// Type alias for the standard OAuth2 token response used by logship.
pub type OAuthToken = StandardTokenResponse<EmptyExtraTokenFields, BasicTokenType>;

/// The OAuth authentication flow variant.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum OAuthFlow {
    Device,
    Code,
    Refresh,
}

/// Persisted OAuth token data including endpoints and scopes for token refresh.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct OAuthData {
    pub received: DateTime<Utc>,
    pub client_id: String,
    pub authorize_endpoint: String,
    pub token_endpoint: String,
    pub device_endpoint: Option<String>,
    pub scopes: Vec<String>,
    pub token: OAuthToken,
    pub flow: OAuthFlow,
}

#[allow(clippy::too_many_arguments)]
pub fn authenticate<F>(
    _connection: &Connection,
    _client: &Client,
    _username: Option<String>,
    _password: Option<F>,
    client_id: String,
    authorize_endpoint: String,
    token_endpoint: String,
    scopes: Vec<String>,
    device_endpoint: Option<String>,
    flow: OAuthFlow,
    observer: &dyn Fn(super::AuthEvent),
    cancelled: &std::sync::atomic::AtomicBool,
) -> Result<AuthData, ConnectError>
where
    F: FnOnce() -> Result<String, ConnectError>,
{
    let scopes: HashSet<String> = scopes.into_iter().collect();
    match &flow {
        OAuthFlow::Device => {
            log::debug!("Initializing OAuth Device Code Flow");

            // Use oauth2's re-exported reqwest to avoid version mismatch
            let http_client = oauth2::reqwest::blocking::Client::builder()
                .redirect(oauth2::reqwest::redirect::Policy::none())
                .timeout(std::time::Duration::from_secs(20))
                .build()
                .map_err(|_| {
                    AuthError::OAuth(OAuthError::MissingEndpoint(
                        "Failed to create HTTP client".to_string(),
                    ))
                })?;

            let device_endpoint = device_endpoint.ok_or_else(|| {
                AuthError::OAuth(OAuthError::MissingEndpoint(
                    "Device Authorization URL".to_string(),
                ))
            })?;
            let device_auth_url = DeviceAuthorizationUrl::new(device_endpoint.clone())
                .map_err(|err| AuthError::OAuth(OAuthError::ParseError(err)))?;
            let c = BasicClient::new(ClientId::new(client_id.clone()))
                .set_auth_uri(
                    AuthUrl::new(authorize_endpoint.clone())
                        .map_err(|err| AuthError::OAuth(OAuthError::ParseError(err)))?,
                )
                .set_token_uri(
                    TokenUrl::new(token_endpoint.clone())
                        .map_err(|err| AuthError::OAuth(OAuthError::ParseError(err)))?,
                )
                .set_device_authorization_url(device_auth_url);

            let details: StandardDeviceAuthorizationResponse = c
                .exchange_device_code()
                .add_scopes(scopes.iter().map(|s| Scope::new(s.clone())))
                .request(&http_client)
                .map_err(|err| AuthError::OAuth(OAuthError::DeviceTokenErrorResponse(err)))?;
            observer(super::AuthEvent::DeviceCode {
                url: details.verification_uri().to_string(),
                code: details.user_code().secret().to_string(),
            });
            let deadline = std::time::Instant::now() + details.expires_in();
            let mut interval = details.interval();
            let token_result = loop {
                if cancelled.load(std::sync::atomic::Ordering::Relaxed) {
                    return Err(ConnectError::InvalidConfigError("Login cancelled".into()));
                }
                if std::time::Instant::now() >= deadline {
                    return Err(ConnectError::InvalidConfigError(
                        "Device code expired. Retry login.".into(),
                    ));
                }
                // A single poll lets the frontend cancel between HTTP requests.
                let response = http_client
                    .post(&token_endpoint)
                    .form(&[
                        ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                        ("device_code", details.device_code().secret()),
                        ("client_id", &client_id),
                    ])
                    .send()
                    .map_err(|e| {
                        ConnectError::InvalidConfigError(format!("OAuth request failed: {e}"))
                    })?;
                if response.status().is_success() {
                    let body = response.text().map_err(|e| {
                        ConnectError::InvalidConfigError(format!("OAuth response failed: {e}"))
                    })?;
                    break serde_json::from_str::<OAuthToken>(&body).map_err(|e| {
                        ConnectError::InvalidConfigError(format!("Invalid OAuth token: {e}"))
                    })?;
                }
                let body = response.text().map_err(|e| {
                    ConnectError::InvalidConfigError(format!("OAuth response failed: {e}"))
                })?;
                let error: serde_json::Value = serde_json::from_str(&body).map_err(|e| {
                    ConnectError::InvalidConfigError(format!("Invalid OAuth response: {e}"))
                })?;
                match error["error"].as_str() {
                    Some("authorization_pending") => {}
                    Some("slow_down") => interval += std::time::Duration::from_secs(5),
                    _ => {
                        return Err(ConnectError::InvalidConfigError(format!(
                            "OAuth login failed: {}",
                            error["error"]
                        )))
                    }
                }
                let until = std::time::Instant::now() + interval;
                while std::time::Instant::now() < until
                    && !cancelled.load(std::sync::atomic::Ordering::Relaxed)
                {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
            };
            use oauth2::TokenResponse;
            let expires = token_result
                .expires_in()
                .map(|d| Utc::now() + chrono::Duration::seconds(d.as_secs() as i64));
            Ok(AuthData::OAuth {
                expires,
                data: Box::new(OAuthData {
                    received: Utc::now(),
                    authorize_endpoint: authorize_endpoint.clone(),
                    client_id: client_id.clone(),
                    token_endpoint: token_endpoint.clone(),
                    device_endpoint: Some(device_endpoint),
                    scopes: scopes.clone().into_iter().collect(),
                    token: token_result,
                    flow: OAuthFlow::Device,
                }),
            })
        }
        OAuthFlow::Code => Err(ConnectError::InvalidConfigError(
            "OAuth Code flow is not yet supported. Use Device flow instead.".to_string(),
        )),
        OAuthFlow::Refresh => Err(ConnectError::InvalidConfigError(
            "OAuth Refresh flow is not yet supported. Use Device flow to re-authenticate."
                .to_string(),
        )),
    }
}
