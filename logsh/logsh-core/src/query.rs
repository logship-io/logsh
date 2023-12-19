
use serde_json::value::RawValue;
use std::collections::HashMap;

use crate::error::QueryError;

#[derive(serde::Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ErrorToken {
    pub start: i32,
    pub end: i32,
}


#[derive(serde::Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ErrorMessage {
    pub message: Option<String>,
    pub tokens: Vec<ErrorToken>,
}

#[derive(serde::Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ApiErrorModel {
    pub message: String,
    pub stack_trace: Option<String>,
    pub errors : Vec<ErrorMessage>
}

#[derive(Clone, Copy, Debug, serde::Serialize)]
pub struct QueryRequest<'a, 'b> {
    pub query: &'a str,
    pub variables: &'b [QueryVariable],
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct QueryVariable {
    pub id: String,
    #[serde(rename = "type")]
    pub typ: String,
    pub value: serde_json::Value,
}

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
        serde_json::from_str(value).map_err(QueryError::Json)
    }
}

impl TryFrom<String> for ApiErrorModel {
    type Error = QueryError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        serde_json::from_str(value.as_str()).map_err(QueryError::Json)
    }
}
