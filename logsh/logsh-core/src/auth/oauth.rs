use std::{collections::HashSet, ops::Add};

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
            println!(
                "Open this URL in your browser: {}\nEnter the following code: {}",
                details.verification_uri(),
                details.user_code().secret(),
            );

            let token_result = c
                .exchange_device_access_token(&details)
                .request(&http_client, std::thread::sleep, None)
                .map_err(|err| AuthError::OAuth(OAuthError::TokenErrorResponse(err)))?;
            Ok(AuthData::OAuth {
                expires: Some(Utc::now().add(details.expires_in())),
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
