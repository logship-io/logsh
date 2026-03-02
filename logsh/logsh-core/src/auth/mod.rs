use chrono::{DateTime, Utc};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

use crate::{connect::Connection, error::ConnectError};

use self::oauth::{OAuthData, OAuthFlow};

pub mod jwt;
pub mod oauth;

/// Authentication data stored with a connection, either JWT or OAuth tokens.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum AuthData {
    Jwt {
        expires: Option<DateTime<Utc>>,
        token: String,
    },
    OAuth {
        expires: Option<DateTime<Utc>>,
        data: Box<OAuthData>,
    },
}

/// An authentication request specifying the method (JWT, PAT, or OAuth) and credentials.
pub enum AuthRequest<F>
where
    F: FnOnce() -> Result<String, ConnectError>,
{
    Jwt {
        username: String,
        password: F,
    },
    /// Personal Access Token authentication for CI/automation.
    Pat {
        token: String,
    },
    OAuth {
        client_id: String,
        device_endpoint: Option<String>,
        scopes: Vec<String>,
        authorize_endpoint: String,
        token_endpoint: String,
        flow: OAuthFlow,
    },
}

impl<F> AuthRequest<F>
where
    F: FnOnce() -> Result<String, ConnectError>,
{
    /// Performs authentication using the configured method and returns the resulting [`AuthData`].
    pub fn authenticate(
        self,
        client: Client,
        connection: &Connection,
    ) -> Result<AuthData, ConnectError> {
        match self {
            AuthRequest::Jwt { username, password } => {
                jwt::fetch_token(connection, &client, username, password)
            }
            AuthRequest::Pat { token } => jwt::fetch_pat_token(connection, &client, token),
            AuthRequest::OAuth {
                client_id,
                flow,
                device_endpoint,
                scopes: _,
                authorize_endpoint,
                token_endpoint,
            } => {
                let mut client_id = client_id;
                let mut authorize_endpoint = authorize_endpoint;
                let mut token_endpoint = token_endpoint;
                let mut device_endpoint = device_endpoint;
                let mut scopes = vec![];
                if client_id.trim().is_empty() {
                    let oauth = connection.refresh_oauth()?;
                    client_id = oauth.client_id;
                    authorize_endpoint = oauth.authorize_endpoint;
                    token_endpoint = oauth.token_endpoint;
                    device_endpoint = Some(oauth.device_endpoint);
                    scopes = oauth.scopes;
                }

                let never = || -> Result<String, ConnectError> { Ok(String::new()) };
                oauth::authenticate(
                    connection,
                    &client,
                    None,
                    Some(never),
                    client_id,
                    authorize_endpoint,
                    token_endpoint,
                    scopes,
                    device_endpoint,
                    flow,
                )
            }
        }
    }
}
