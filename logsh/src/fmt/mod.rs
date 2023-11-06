use std::collections::HashMap;

use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Connection {
    pub name: String,
    pub server: String,
    pub is_default: bool,
    pub username: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DataFrame {
    pub headers: Vec<String>,
    pub data: Vec<HashMap<String, serde_json::Value>>,
}
