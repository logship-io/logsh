use std::{collections::HashMap, ops::Add};

use chrono::{Utc, Duration};
use reqwest::blocking::Client;
use serde::Deserialize;

use crate::{connect::Connection, error::ConnectError};

use super::AuthData;

pub fn fetch_token<F>(
    connection: &Connection,
    client: &Client,
    username: String,
    password: F,
) -> Result<AuthData, ConnectError>
where
    F: FnOnce() -> Result<String, ConnectError>,
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
    Ok(AuthData::Jwt {
        expires: Some(Utc::now().add(Duration::hours(24))),
        token: token.token
    })
}

#[derive(Deserialize)]
struct TokenResponse {
    token: String,
}
