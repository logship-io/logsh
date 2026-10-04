//! Shared login preparation. Callers own presentation, selection, and persistence.
use crate::{
    auth::{AuthEvent, AuthRequest},
    connect::Connection,
    error::ConnectError,
};
use std::sync::atomic::AtomicBool;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoginMethod {
    Password,
    Pat,
    Sso,
}

pub struct LoginRequest {
    pub server: String,
    pub method: LoginMethod,
    pub username: String,
    pub secret: String,
}

pub fn authenticate(
    request: LoginRequest,
    previous: Option<Connection>,
    observer: &dyn Fn(AuthEvent),
    cancelled: &AtomicBool,
) -> Result<Connection, ConnectError> {
    let url = reqwest::Url::parse(&request.server)
        .map_err(|e| ConnectError::InvalidConfigError(format!("Invalid server URL: {e}")))?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err(ConnectError::InvalidConfigError(
            "Use an http:// or https:// server URL.".into(),
        ));
    }
    let server = request.server.trim_end_matches('/');
    let mut connection = previous
        .filter(|c| c.server.trim_end_matches('/') == server)
        .unwrap_or_else(|| Connection::new(server));
    let auth: AuthRequest<Box<dyn FnOnce() -> Result<String, ConnectError>>> = match request.method
    {
        LoginMethod::Password => {
            if request.username.trim().is_empty() || request.secret.is_empty() {
                return Err(ConnectError::InvalidConfigError(
                    "Username and password are required.".into(),
                ));
            }
            AuthRequest::Jwt {
                username: request.username,
                password: Box::new(move || Ok(request.secret)),
            }
        }
        LoginMethod::Pat => {
            if request.secret.trim().is_empty() {
                return Err(ConnectError::InvalidConfigError(
                    "A Personal Access Token is required.".into(),
                ));
            }
            AuthRequest::Pat {
                token: request.secret,
            }
        }
        LoginMethod::Sso => AuthRequest::OAuth {
            client_id: String::new(),
            device_endpoint: None,
            scopes: vec![],
            authorize_endpoint: String::new(),
            token_endpoint: String::new(),
            flow: crate::auth::oauth::OAuthFlow::Device,
        },
    };
    connection.refresh_auth_with_events(Some(auth), observer, cancelled)?;
    connection.post_auth_setup()?;
    if cancelled.load(std::sync::atomic::Ordering::Relaxed) {
        return Err(ConnectError::InvalidConfigError("Login cancelled".into()));
    }
    Ok(connection)
}
