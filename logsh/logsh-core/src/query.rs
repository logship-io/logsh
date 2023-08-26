use std::collections::HashMap;

use log::trace;
use serde_json::value::RawValue;

use crate::{config, error::QueryError};

#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(bound(deserialize = "'de: 'a"))]
pub struct QueryResult<'a> {
    #[serde(alias = "Header")]
    #[serde(alias = "header")]
    pub header: Vec<String>,

    #[serde(alias = "Results")]
    #[serde(alias = "results")]
    pub results: Vec<HashMap<&'a str, &'a RawValue>>,
}

#[derive(serde::Serialize)]
pub struct QueryResultFmt {
    #[serde(alias = "Header")]
    #[serde(alias = "header")]
    pub header: Vec<String>,

    #[serde(alias = "Results")]
    #[serde(alias = "results")]
    pub results: Vec<HashMap<String, serde_json::Value>>,
}

pub fn result<'a>(result: &'a str) -> Result<QueryResult<'a>, QueryError> {
    result.try_into()
}

impl<'a> TryFrom<&'a str> for QueryResult<'a> {
    type Error = QueryError;

    fn try_from(value: &'a str) -> Result<Self, Self::Error> {
        serde_json::from_str(value).map_err(QueryError::JsonError)
    }
}

pub fn execute(query: &'_ str) -> Result<String, QueryError> {
    let connection = config::get_default_connection().map_err(QueryError::NoConnection)?;

    if query.trim().is_empty() {
        return Err(QueryError::NoInput);
    }

    let map = HashMap::from([("query", query)]);

    let client = reqwest::blocking::Client::new();
    let res = client
        .post(format!(
            "{}/search/{}/kusto",
            connection.server, &connection.default_acccount_id
        ))
        .json(&map)
        .header("Authorization", "Bearer ".to_owned() + &connection.token)
        .send()
        .map_err(QueryError::FailedToConnect)?;

    trace!("Response: {:?}", res);
    let status = res.status();
    let result_text = res.text().map_err(QueryError::FailedToParseResponse)?;

    if false == status.is_success() {
        return Err(QueryError::HttpErrorStatus(status, result_text));
    }

    Ok(result_text)
}
