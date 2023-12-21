use std::{collections::HashSet, ops::Add};

use chrono::{DateTime, Utc};
use oauth2::{
    basic::{BasicClient, BasicTokenType},
    reqwest::http_client,
    AuthUrl, ClientId, DeviceAuthorizationUrl, EmptyExtraTokenFields, Scope,
    StandardDeviceAuthorizationResponse, StandardTokenResponse, TokenUrl,
};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

use crate::{
    connect::Connection,
    error::{AuthError, OAuthError, ConnectError},
};

use super::AuthData;

pub type OAuthToken = StandardTokenResponse<EmptyExtraTokenFields, BasicTokenType>;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum OAuthFlow {
    Device,
    Code,
    Refresh,
}

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
    // scopes.insert("profile".to_string());
    // scopes.insert("email".to_string());
    // scopes.insert("offline_access".to_string());
    match &flow {
        OAuthFlow::Device => {
            log::debug!("Initializing OAuth Device Code Flow");
            let device_endpoint = device_endpoint.ok_or_else(|| {
                AuthError::OAuth(OAuthError::MissingEndpoint("Device Authorization URL".to_string()))
            })?;
            let device_auth_url = DeviceAuthorizationUrl::new(device_endpoint.clone())
                .map_err(|err| AuthError::OAuth(OAuthError::ParseError(err)))?;
            let c = BasicClient::new(
                ClientId::new(client_id.clone()),
                None,
                AuthUrl::new(authorize_endpoint.clone()).map_err(|err| AuthError::OAuth(OAuthError::ParseError(err)))?,
                Some(TokenUrl::new(token_endpoint.clone()).map_err(|err| AuthError::OAuth(OAuthError::ParseError(err)))?),
            )
            .set_device_authorization_url(device_auth_url);

            let details: StandardDeviceAuthorizationResponse = c
                .exchange_device_code()
                .map_err(|err| AuthError::OAuth(OAuthError::ConfigurationError(err)))?
                .add_scopes(scopes.iter().map(|s| Scope::new(s.clone())))
                .request(http_client)
                .map_err(|err| AuthError::OAuth(OAuthError::DeviceTokenErrorResponse(err)))?;
            println!(
                "Open this URL in your browser: {}\nEnter the following code: {}",
                details.verification_uri().to_string(),
                details.user_code().secret().to_string(),
            );

            let token_result = c
                .exchange_device_access_token(&details)
                .request(http_client, std::thread::sleep, None)
                .map_err(|err| AuthError::OAuth(OAuthError::TokenErrorResponse(err)))?;
            Ok(AuthData::OAuth {
                expires: Some(Utc::now()),
                data: OAuthData {
                    received: Utc::now().add(details.expires_in()),
                    authorize_endpoint: authorize_endpoint.clone(),
                    client_id: client_id.clone(),
                    token_endpoint: token_endpoint.clone(),
                    device_endpoint: Some(device_endpoint),
                    scopes: scopes.clone().into_iter().collect(),
                    token: token_result,
                    flow: OAuthFlow::Device,
                },
            })
        }
        OAuthFlow::Code => {
            log::error!("not implemented");
            todo!()
        }
        OAuthFlow::Refresh => {
            log::error!("not implemented");
            todo!()
        }
    }
}
