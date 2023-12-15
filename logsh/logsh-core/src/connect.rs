use chrono::{Duration, Utc};
use oauth2::TokenResponse;
use reqwest::StatusCode;
use reqwest::blocking::RequestBuilder;
use reqwest::header::{HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

use crate::auth::{AuthData, AuthRequest};
use crate::error::{AuthError, ConnectError, OAuthError, QueryError, ConfigError};
use crate::config;
use crate::query::QueryRequest;

#[derive(Serialize, Deserialize, Clone)]
pub struct Connection {
    pub server: String,
    pub user: Option<uuid::Uuid>,
    pub username: String,
    pub default_subscription: Option<String>,
    pub subscriptions: HashMap<String, uuid::Uuid>,
    auth: Option<AuthData>,
}

#[derive(Serialize, Deserialize, Clone, Copy)]
pub enum ConnectionStatus {
    Connected,
    AuthRequired,
    NotConfigured,
}

impl fmt::Display for ConnectionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", match self {
            ConnectionStatus::Connected => "Connected",
            ConnectionStatus::AuthRequired => "Authentication Required",
            ConnectionStatus::NotConfigured => "Configuration Required",
        })
    }
}

impl fmt::Display for Connection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Server: {}; User: {}; Default Subscription: {:?}",
            self.server,
            self.user.unwrap_or_default(),
            self.default_subscription()
        )
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserModel {
    pub user_id: uuid::Uuid,
    pub user_name: String,
    /*
     * first_name: String,
     * last_name: String,
     * nick_name: String,
     * contact: Vec<ContactModel>,
     */
}

// #[derive(Deserialize)]
// #[serde(rename_all = "camelCase")]
// pub struct ContactModel {
//     #[serde(rename = "type")]
//     typ: String,
//     value: String,
// }

impl Connection {
    pub fn new(server: &str) -> Self {
        Self {
            server: server.trim().to_string(),
            user: None,
            username: String::default(),
            default_subscription: None,
            subscriptions: HashMap::default(),
            auth: None,
        }
    }

    pub fn default_subscription(&self) -> Option<uuid::Uuid> {
        if let Some(d) = self.default_subscription.as_ref() {
            if let Some(sub) = self.subscriptions.get(d) {
                return Some(sub.to_owned());
            }
        }

        let mut subs = Vec::from_iter(self.subscriptions.iter());
        subs.sort_by_key(|s| s.0);
        if subs.len() > 0 {
            return Some(subs[0].1.to_owned());
        }

        None
    }

    pub fn is_jwt_auth(&self) -> bool {
        match self.auth {
            Some(AuthData::Jwt { expires: _, token: _ }) => true,
            _ => false,
        }
    }

    pub fn is_oauth_auth(&self) -> bool {
        match self.auth {
            Some(AuthData::OAuth { expires: _, data: _ }) => true,
            _ => false,
        }
    }

    pub fn authenticate_request(&self, builder: RequestBuilder) -> RequestBuilder {
        match &self.auth {
            Some(AuthData::Jwt { expires: _, token }) => builder.bearer_auth(token),
            Some(AuthData::OAuth { expires: _, data }) => {
                builder.bearer_auth(data.token.access_token().secret())
            }
            None => builder,
        }
    }

    pub(crate) fn refresh_oauth(&self) -> Result<OAuthConfigResponse, AuthError> {
        log::trace!("Requesting OAuth config for connection.");
        let client = client_builder().build().unwrap();
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

    pub fn subscriptions(&self, user: uuid::Uuid) -> Result<Vec<SubscriptionsModel>, ConnectError> {
        log::debug!("Executing accounts query");
        let client = client_builder().build()?;
        let response: Vec<SubscriptionsModel> = self
            .authenticate_request(
                client.get(format!("{}/users/{}/accounts", &self.server.trim_end_matches('/'), user)),
            )
            .send()?
            .error_for_status()?
            .json()?;
        Ok(response)
    }

    pub fn refresh_auth<F>(&mut self, auth: Option<AuthRequest<F>>) -> Result<(), ConnectError>
    where
        F: FnOnce() -> Result<String, AuthError>,
    {
        log::debug!("Refreshing authentication for {self}");
        let client = client_builder().build()?;
        match (&self.auth, auth) {
            (None, None) => {
                return Err(ConnectError::NoAuthentication);
            }
            (Some(a), None) => match a {
                AuthData::Jwt { expires: _, token: _ } => return Err(ConnectError::Auth(AuthError::Expired)),
                AuthData::OAuth { expires: _, data } => {
                    if let Some(expires_in) = data.token.expires_in() {
                        let expiry = data.received
                            .checked_add_signed(Duration::seconds(expires_in.as_secs() as i64))
                            .ok_or(ConnectError::Auth(AuthError::Expired))?;
                        if Utc::now() > expiry {
                            log::warn!("OAuth token is expired.");
                            return Err(ConnectError::Auth(AuthError::Expired));
                        }
                    }

                    return Ok(());
                }
            },
            (_, Some(a)) => {
                let auth = a.authenticate(client, self).map_err(ConnectError::Auth)?;
                self.auth = Some(auth);
                Ok(())
            }
        }
    }

    pub fn query_raw(&self, query: &str) -> Result<String, QueryError> {
        if query.trim().is_empty() {
            return Err(QueryError::NoInput);
        }
        
        log::trace!("Executing query.");
        let req = QueryRequest {
            query,
            variables: &[],
        };

        let sub = &self.default_subscription()
            .ok_or(QueryError::Config(ConfigError::NoDefaultSubscription))?;
        let client = client_builder().build()?;
        let response = self
            .authenticate_request(client.post(format!(
                "{}/search/{}/kusto",
                &self.server.trim_end_matches('/'),
                sub
            )))
            .json(&req)
            .send()?
            .error_for_status()?
            .text()?;
        Ok(response)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OAuthConfigResponse {
    pub client_id: String,
    pub authorize_endpoint: String,
    pub device_endpoint: String,
    pub token_endpoint: String,
    pub scopes: Vec<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubscriptionsModel {
    pub permissions: Vec<String>,
    pub account_id: uuid::Uuid,
    pub account_name: String,
}


static USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"),);

fn client_builder() -> reqwest::blocking::ClientBuilder {
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

pub fn add_connect<'a, F>(
    name: String,
    mut connection: Option<Connection>,
    auth: Option<AuthRequest<F>>,
) -> Result<Connection, ConnectError>
where
    F: FnOnce() -> Result<String, AuthError>,
{
    let connection: Connection = {
        let mut cfg = config::load()?;
        let conn_entry = cfg.connections.entry(name.clone());
        let c = if let Some(c) = connection.as_mut() {
            c.refresh_auth(auth)?;
            let user = c.who_am_i()?;
            let mut subs = c.subscriptions(user.user_id)?;
            subs.sort_by(|a, b| a.account_name.cmp(&b.account_name));
            c.user = Some(user.user_id);
            c.username = user.user_name;
            c.subscriptions = subs.into_iter()
                .map(|s| (s.account_name, s.account_id))
                .collect();
            Ok(c.clone())
        } else {
            match conn_entry {
                std::collections::hash_map::Entry::Occupied(mut o) => {
                    let c = o.get_mut();
                    c.refresh_auth(auth)?;
                    let user = c.who_am_i()?;
                    let mut subs = c.subscriptions(user.user_id)?;
                    subs.sort_by(|a, b| a.account_name.cmp(&b.account_name));
                    c.user = Some(user.user_id);
                    c.username = user.user_name;
                    c.subscriptions = subs.into_iter()
                        .map(|s| (s.account_name, s.account_id))
                        .collect();
                    Ok(c.clone())
                },
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