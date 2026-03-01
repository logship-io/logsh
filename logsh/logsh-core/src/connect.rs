use chrono::{DateTime, Utc};
use oauth2::TokenResponse;
use reqwest::blocking::RequestBuilder;
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use std::fmt;

use crate::account::AccountsModel;
use crate::auth::{AuthData, AuthRequest};
use crate::common::ApiErrorModel;
use crate::config;
use crate::error::{AuthError, ConfigError, ConnectError, OAuthError, QueryError};
use crate::query::QueryRequest;

/// Represents an authenticated connection to a logship server.
#[derive(Serialize, Deserialize, Clone)]
pub struct Connection {
    pub server: String,
    pub user_id: uuid::Uuid,
    pub username: String,
    pub default_account: Option<uuid::Uuid>,
    /// Cached name of the default account for display without API calls.
    #[serde(default)]
    pub default_account_name: Option<String>,
    /// Cached list of known account names for this context (for completions/display).
    #[serde(default)]
    pub known_accounts: Vec<String>,
    auth: Option<AuthData>,
}

/// Current status of a connection to a logship server.
#[derive(Serialize, Deserialize, Clone, Copy)]
pub enum ConnectionStatus {
    Connected,
    AuthRequired,
    NotConfigured,
}

impl fmt::Display for ConnectionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                ConnectionStatus::Connected => "Connected",
                ConnectionStatus::AuthRequired => "Authentication Required",
                ConnectionStatus::NotConfigured => "Configuration Required",
            }
        )
    }
}

impl fmt::Display for Connection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Server: {}; User: {}; Default Account: {:?}",
            self.server,
            self.user_id,
            self.default_account()
        )
    }
}

/// Response model from the `/whoami` endpoint.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserModel {
    pub user_id: uuid::Uuid,
    pub user_name: String,
}

fn get_token_if_not_expired<T>(expiration: &Option<DateTime<Utc>>, token: T) -> Option<T> {
    if let Some(expiration) = expiration {
        if Utc::now() > *expiration {
            log::debug!("Token is expired.");
            return None;
        }
    }

    Some(token)
}

impl Connection {
    /// Creates a new connection targeting the given server URL.
    pub fn new(server: &str) -> Self {
        Self {
            server: server.trim().to_string(),
            user_id: uuid::Uuid::default(),
            username: String::default(),
            default_account: None,
            default_account_name: None,
            known_accounts: Vec::new(),
            auth: None,
        }
    }

    /// Returns the effective default account UUID, respecting the global account override.
    /// If an override is set, fetches accounts and resolves the name to a UUID.
    pub fn effective_account(&self) -> Option<uuid::Uuid> {
        if let Some(override_name) = crate::config::get_account_override() {
            match self.accounts(self.user_id) {
                Ok(accounts) => {
                    if let Some(account) = accounts
                        .iter()
                        .find(|a| a.account_name.eq_ignore_ascii_case(override_name))
                    {
                        return Some(account.account_id);
                    }
                    log::warn!(
                        "Account override \"{override_name}\" not found, falling back to default."
                    );
                }
                Err(err) => {
                    log::warn!("Failed to resolve account override \"{override_name}\": {err}");
                }
            }
        }
        self.default_account
    }

    /// Returns the default account UUID, if set.
    pub fn default_account(&self) -> Option<uuid::Uuid> {
        self.default_account
    }

    /// Returns `true` if the connection uses JWT authentication.
    pub fn is_jwt_auth(&self) -> bool {
        matches!(self.auth, Some(AuthData::Jwt { .. }))
    }

    /// Returns the current bearer token if it has not expired.
    pub fn get_token(&self) -> Option<String> {
        match &self.auth {
            Some(AuthData::Jwt {
                expires: expiration,
                token,
            }) => get_token_if_not_expired(expiration, token.to_owned()),
            Some(AuthData::OAuth {
                expires: expiration,
                data,
            }) => {
                get_token_if_not_expired(expiration, data.token.access_token().secret().to_string())
            }
            None => None,
        }
    }

    /// Returns `true` if the connection uses OAuth authentication.
    pub fn is_oauth_auth(&self) -> bool {
        matches!(self.auth, Some(AuthData::OAuth { .. }))
    }

    /// Attaches the stored authentication token to an outgoing HTTP request.
    pub fn authenticate_request(&self, builder: RequestBuilder) -> RequestBuilder {
        match &self.auth {
            Some(AuthData::Jwt { expires: _, token }) => builder.bearer_auth(token),
            Some(AuthData::OAuth { expires: _, data }) => {
                builder.bearer_auth(data.token.access_token().secret())
            }
            None => builder,
        }
    }

    pub(crate) fn refresh_oauth(&self) -> Result<OAuthConfigResponse, ConnectError> {
        log::trace!("Requesting OAuth config for connection.");
        let client = client_builder().build()?;
        let res = client
            .get(format!("{}/auth/oauth", self.server.trim_end_matches('/')))
            .send()?
            .error_for_status()?;
        if res.status() == StatusCode::NO_CONTENT {
            return Err(AuthError::OAuth(OAuthError::ConfigurationError(
                oauth2::ConfigurationError::MissingUrl("oauth is not configured for this server"),
            )))?;
        }

        let json = res.json::<OAuthConfigResponse>()?;
        Ok(json)
    }

    /// Queries the server's `/whoami` endpoint and returns the authenticated user.
    pub fn who_am_i(&self) -> Result<UserModel, ConnectError> {
        log::debug!("Executing who am I query");
        let client = client_builder().build()?;
        let response: UserModel = self
            .authenticate_request(
                client.get(format!("{}/whoami", &self.server.trim_end_matches('/'))),
            )
            .send()?
            .error_for_status()?
            .json()?;
        Ok(response)
    }

    /// Fetches the list of accounts accessible by the given user.
    pub fn accounts(&self, user: uuid::Uuid) -> Result<Vec<AccountsModel>, ConnectError> {
        log::debug!("Executing accounts query");
        let client = client_builder().build()?;
        let response: Vec<AccountsModel> = self
            .authenticate_request(client.get(format!(
                "{}/users/{}/accounts",
                &self.server.trim_end_matches('/'),
                user
            )))
            .send()?
            .error_for_status()?
            .json()?;
        Ok(response)
    }

    /// Refreshes the connection's authentication, re-authenticating if necessary.
    pub fn refresh_auth<F>(&mut self, auth: Option<AuthRequest<F>>) -> Result<(), ConnectError>
    where
        F: FnOnce() -> Result<String, ConnectError>,
    {
        log::debug!("Refreshing authentication for {self}");
        let client = client_builder().build()?;
        match (&self.auth, auth) {
            (None, None) => Err(ConnectError::NoAuthentication),
            (Some(a), None) => match a {
                AuthData::Jwt {
                    expires: _,
                    token: _,
                } => Err(ConnectError::Auth(AuthError::Expired)),
                AuthData::OAuth { expires: _, data } => {
                    if let Some(expires_in) = data.token.expires_in() {
                        let expiry = data
                            .received
                            .checked_add_signed(chrono::Duration::seconds(
                                expires_in.as_secs() as i64
                            ))
                            .ok_or(ConnectError::Auth(AuthError::Expired))?;
                        if Utc::now() > expiry {
                            log::warn!("OAuth token is expired.");
                            return Err(ConnectError::Auth(AuthError::Expired));
                        }
                    }

                    Ok(())
                }
            },
            (_, Some(a)) => {
                let auth = a.authenticate(client, self)?;
                self.auth = Some(auth);
                Ok(())
            }
        }
    }

    /// Sends a query to the parse endpoint and returns the raw JSON response.
    pub fn query_parse(&self, query: &str) -> Result<String, QueryError> {
        if query.trim().is_empty() {
            return Err(QueryError::NoInput);
        }

        log::trace!("Executing parse query.");
        let req = QueryRequest {
            query,
            cursor_position: Some(query.len() as i32),
            variables: &[],
        };

        let sub = &self
            .effective_account()
            .ok_or(QueryError::Config(ConfigError::NoDefaultAccount))?;
        let client = client_builder().build()?;
        let req = self
            .authenticate_request(client.post(format!(
                "{}/search/{}/kusto/parse",
                &self.server.trim_end_matches('/'),
                sub
            )))
            .json(&req)
            .build()?;

        let response = client.execute(req)?;

        log::debug!(
            "Parse response status: {}, content length: {}",
            response.status(),
            response.content_length().unwrap_or(0)
        );
        if response.status().is_success() {
            Ok(response.text()?)
        } else if response.status() == StatusCode::BAD_REQUEST {
            let error_text = response.text()?;
            Err(QueryError::Common(crate::error::CommonError::ApiError(
                error_text.as_str().try_into()?,
            )))
        } else {
            response.error_for_status()?;
            Err(QueryError::Common(crate::error::CommonError::ApiError(
                ApiErrorModel {
                    message: "Unknown error".to_string(),
                    stack_trace: None,
                    errors: vec![],
                },
            )))
        }
    }

    /// Sends a raw query string to the server and returns the response body as text.
    pub fn query_raw(
        &self,
        query: &str,
        timeout: Option<std::time::Duration>,
    ) -> Result<String, QueryError> {
        if query.trim().is_empty() {
            return Err(QueryError::NoInput);
        }

        log::trace!("Executing query.");
        let req = QueryRequest {
            query,
            cursor_position: None,
            variables: &[],
        };

        let sub = &self
            .effective_account()
            .ok_or(QueryError::Config(ConfigError::NoDefaultAccount))?;
        let client = client_builder().timeout(timeout).build()?;
        let req = self
            .authenticate_request(client.post(format!(
                "{}/search/{}/kusto",
                &self.server.trim_end_matches('/'),
                sub
            )))
            .json(&req)
            .build()?;

        let response = client.execute(req)?;

        log::debug!(
            "Response status: {}, content length: {}",
            response.status(),
            response.content_length().unwrap_or(0)
        );
        if response.status().is_success() {
            Ok(response.text()?)
        } else if response.status() == StatusCode::BAD_REQUEST {
            let error_text = response.text()?;
            return Err(QueryError::Common(crate::error::CommonError::ApiError(
                error_text.as_str().try_into()?,
            )));
        } else {
            response.error_for_status()?;
            return Err(QueryError::Common(crate::error::CommonError::ApiError(
                ApiErrorModel {
                    message: "Unknown error".to_string(),
                    stack_trace: None,
                    errors: vec![],
                },
            )));
        }
    }
}

/// OAuth configuration returned by the server's `/auth/oauth` endpoint.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OAuthConfigResponse {
    pub client_id: String,
    pub authorize_endpoint: String,
    pub device_endpoint: String,
    pub token_endpoint: String,
    pub scopes: Vec<String>,
}

static USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"),);

pub(crate) fn client_builder() -> reqwest::blocking::ClientBuilder {
    reqwest::blocking::Client::builder()
        .user_agent(USER_AGENT)
        .default_headers({
            let mut h = HeaderMap::new();
            let host = gethostname::gethostname().to_string_lossy().to_string();
            if let Ok(host) = HeaderValue::from_str(&host) {
                h.insert("x-ls-hostname", host);
            }
            h
        })
}

/// Adds or updates a named connection in the configuration, authenticating and persisting it.
pub fn add_connect<F>(
    name: String,
    mut connection: Option<Connection>,
    auth: Option<AuthRequest<F>>,
) -> Result<Connection, ConnectError>
where
    F: FnOnce() -> Result<String, ConnectError>,
{
    let connection: Connection = {
        let mut cfg = config::load()?;
        let conn_entry = cfg.contexts.entry(name.clone());
        let c = if let Some(c) = connection.as_mut() {
            c.refresh_auth(auth)?;
            let user = c.who_am_i()?;
            let mut subs = c.accounts(user.user_id)?;
            subs.sort_by(|a, b| a.account_name.cmp(&b.account_name));
            c.user_id = user.user_id;
            c.username = user.user_name;
            c.known_accounts = subs.iter().map(|s| s.account_name.clone()).collect();

            if c.default_account.is_none() {
                c.default_account = subs.first().map(|s| s.account_id);
            }
            c.default_account_name = c.default_account.and_then(|id| {
                subs.iter()
                    .find(|s| s.account_id == id)
                    .map(|s| s.account_name.clone())
            });

            Ok(c.clone())
        } else {
            match conn_entry {
                std::collections::hash_map::Entry::Occupied(mut o) => {
                    let c = o.get_mut();
                    c.refresh_auth(auth)?;
                    let user = c.who_am_i()?;
                    let mut subs = c.accounts(user.user_id)?;
                    subs.sort_by(|a, b| a.account_name.cmp(&b.account_name));
                    c.user_id = user.user_id;
                    c.username = user.user_name;
                    c.known_accounts = subs.iter().map(|s| s.account_name.clone()).collect();

                    if c.default_account.is_none() {
                        c.default_account = subs.first().map(|s| s.account_id);
                    }
                    c.default_account_name = c.default_account.and_then(|id| {
                        subs.iter()
                            .find(|s| s.account_id == id)
                            .map(|s| s.account_name.clone())
                    });

                    Ok(c.clone())
                }
                std::collections::hash_map::Entry::Vacant(_) => {
                    Err(ConnectError::NoConnection(name))
                }
            }
        }?;

        let _cfg = config::save(cfg)?;
        c
    };

    Ok(connection)
}
