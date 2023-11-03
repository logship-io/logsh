use std::collections::HashMap;

use reqwest::blocking::Client;
use serde::Deserialize;

use crate::{connect::Connection, error::AuthError};

use super::AuthData;

type Error = AuthError;

pub fn fetch_token<F>(
    connection: &Connection,
    client: &Client,
    username: String,
    password: F,
) -> Result<AuthData, Error>
where
    F: FnOnce() -> Result<String, Error>,
{
    let mut map = HashMap::new();
    map.insert("username", username);
    map.insert("password", password()?);
    let res = client
        .post(format!(
            "{}/auth/token",
            connection.server.trim_end_matches('/')
        ))
        .json(&map)
        .send()?
        .error_for_status()?;

    let token: TokenResponse = res.json()?;
    Ok(AuthData::Jwt { token: token.token })
}

#[derive(Deserialize)]
struct TokenResponse {
    token: String,
}
