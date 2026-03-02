use std::collections::HashMap;

use chrono::{DateTime, Utc};
use reqwest::blocking::Client;
use serde::Deserialize;

use crate::{connect::Connection, error::ConnectError};

use super::AuthData;

/// Extract the expiration time from a JWT token by decoding its payload.
/// Falls back to 24 hours from now if the token cannot be parsed.
fn extract_jwt_expiry(token: &str) -> Option<DateTime<Utc>> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return None;
    }

    // JWT payload is base64url-encoded (no padding)
    let payload = parts[1];
    // Add padding if needed
    let padded = match payload.len() % 4 {
        2 => format!("{payload}=="),
        3 => format!("{payload}="),
        _ => payload.to_string(),
    };
    let padded = padded.replace('-', "+").replace('_', "/");

    let decoded = base64_decode(&padded)?;

    #[derive(Deserialize)]
    struct JwtPayload {
        exp: Option<i64>,
    }

    let payload: JwtPayload = serde_json::from_slice(&decoded).ok()?;
    payload.exp.and_then(|exp| DateTime::from_timestamp(exp, 0))
}

fn base64_decode(input: &str) -> Option<Vec<u8>> {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = Vec::new();
    let mut buf: u32 = 0;
    let mut bits: u32 = 0;
    for &b in input.as_bytes() {
        if b == b'=' {
            break;
        }
        let val = TABLE.iter().position(|&c| c == b)? as u32;
        buf = (buf << 6) | val;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            output.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }
    Some(output)
}

/// Authenticates with username and password, returning a JWT token.
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
    let expires = extract_jwt_expiry(&token.token).or_else(|| {
        log::warn!("Could not parse JWT expiry, defaulting to 24 hours");
        Some(Utc::now() + chrono::Duration::hours(24))
    });
    Ok(AuthData::Jwt {
        expires,
        token: token.token,
    })
}

/// Authenticate using a Personal Access Token (PAT).
/// Sends the PAT to `/auth/pat-token` and receives a JWT in return.
pub fn fetch_pat_token(
    connection: &Connection,
    client: &Client,
    pat: String,
) -> Result<AuthData, ConnectError> {
    let res = client
        .post(format!(
            "{}/auth/pat-token",
            connection.server.trim_end_matches('/')
        ))
        .bearer_auth(&pat)
        .send()?
        .error_for_status()?;

    let token: TokenResponse = res.json()?;
    let expires = extract_jwt_expiry(&token.token).or_else(|| {
        log::warn!("Could not parse PAT JWT expiry, defaulting to 24 hours");
        Some(Utc::now() + chrono::Duration::hours(24))
    });
    Ok(AuthData::Jwt {
        expires,
        token: token.token,
    })
}

#[derive(Deserialize)]
struct TokenResponse {
    token: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_jwt_expiry_valid() {
        // JWT with exp=1893456000 (2030-01-01T00:00:00Z)
        // Header: {"alg":"HS256","typ":"JWT"}
        // Payload: {"sub":"user","exp":1893456000}
        let token = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiJ1c2VyIiwiZXhwIjoxODkzNDU2MDAwfQ.signature";
        let expiry = extract_jwt_expiry(token);
        assert!(expiry.is_some());
        let dt = expiry.unwrap();
        assert_eq!(dt.timestamp(), 1893456000);
    }

    #[test]
    fn test_extract_jwt_expiry_no_exp_field() {
        // Payload: {"sub":"user"} (no exp)
        let token = "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJ1c2VyIn0.signature";
        let expiry = extract_jwt_expiry(token);
        assert!(expiry.is_none());
    }

    #[test]
    fn test_extract_jwt_expiry_invalid_token() {
        assert!(extract_jwt_expiry("not.a.jwt.token.at.all").is_none());
        assert!(extract_jwt_expiry("").is_none());
        assert!(extract_jwt_expiry("only-one-part").is_none());
    }

    #[test]
    fn test_base64_decode() {
        // "hello" in base64
        let decoded = base64_decode("aGVsbG8=").unwrap();
        assert_eq!(decoded, b"hello");
    }

    #[test]
    fn test_base64_decode_no_padding() {
        // "hi" in base64 without padding
        let decoded = base64_decode("aGk").unwrap();
        assert_eq!(decoded, b"hi");
    }
}
