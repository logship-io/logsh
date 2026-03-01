use serde_json::value::RawValue;
use std::collections::HashMap;

use crate::error::QueryError;

/// A query request payload sent to the search API.
#[derive(Clone, Copy, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryRequest<'a, 'b> {
    pub query: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor_position: Option<i32>,
    pub variables: &'b [QueryVariable],
}

/// A named variable binding for parameterized queries.
#[derive(Debug, Clone, serde::Serialize)]
pub struct QueryVariable {
    pub id: String,
    #[serde(rename = "type")]
    pub typ: String,
    pub value: serde_json::Value,
}

/// Schema column metadata returned by the backend.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SchemaColumn {
    pub name: String,
    #[serde(rename = "type")]
    pub column_type: String,
}

/// Render hints for visualization (e.g., chart type).
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RenderHint {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chart_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub axis: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<serde_json::Value>,
}

/// Deserialized query result containing headers, columns, optional render hints, and row data.
#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(bound(deserialize = "'de: 'a"))]
pub struct QueryResult<'a> {
    #[serde(alias = "Header")]
    #[serde(alias = "Headers")]
    #[serde(alias = "header")]
    pub header: Vec<String>,

    #[serde(alias = "Columns")]
    #[serde(alias = "columns")]
    #[serde(default)]
    pub columns: Vec<SchemaColumn>,

    #[serde(alias = "Render")]
    #[serde(alias = "render")]
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub render: Option<RenderHint>,

    #[serde(alias = "Results")]
    #[serde(alias = "results")]
    pub results: Vec<HashMap<&'a str, &'a RawValue>>,
}

/// Owned variant of [`QueryResult`] suitable for serialization without lifetime constraints.
#[derive(serde::Serialize)]
pub struct QueryResultFmt {
    #[serde(alias = "Header")]
    #[serde(alias = "header")]
    pub header: Vec<String>,

    #[serde(alias = "Results")]
    #[serde(alias = "results")]
    pub results: Vec<HashMap<String, serde_json::Value>>,
}

/// Parses a JSON string into a [`QueryResult`].
pub fn result<'a>(result: &'a str) -> Result<QueryResult<'a>, QueryError> {
    result.try_into()
}

impl<'a> TryFrom<&'a str> for QueryResult<'a> {
    type Error = QueryError;

    fn try_from(value: &'a str) -> Result<Self, Self::Error> {
        serde_json::from_str(value).map_err(QueryError::Json)
    }
}

/// Token position and type information from the parse endpoint.
#[derive(serde::Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TokenInfo {
    pub start: i32,
    pub length: i32,
    pub token_type: String,
    #[serde(default)]
    pub message: String,
}

/// A completion token indicating position in the query string.
#[derive(serde::Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CompletionToken {
    pub start: i32,
    pub length: i32,
}

/// An auto-completion suggestion from the parse endpoint.
#[derive(serde::Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AutoCompletion {
    pub token: CompletionToken,
    #[serde(default)]
    pub value: String,
    #[serde(default)]
    pub completion_type: String,
    #[serde(default)]
    pub description: String,
}

/// Auto-completion suggestions returned by the parse endpoint.
#[derive(serde::Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Suggestions {
    pub auto_completions: Vec<AutoCompletion>,
}

/// Result from the `/kusto/parse` endpoint.
#[derive(serde::Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ParseResult {
    pub parse_success: bool,
    #[serde(default)]
    pub token_info: Vec<TokenInfo>,
    #[serde(default)]
    pub suggestions: Option<Suggestions>,
    #[serde(default)]
    pub error: Option<crate::common::ApiErrorModel>,
    #[serde(default)]
    pub header: Vec<String>,
    #[serde(default)]
    pub columns: Vec<SchemaColumn>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_query_result_basic() {
        let json = r#"{"header":["name","value"],"results":[{"name":"test","value":42}]}"#;
        let result = result(json).unwrap();
        assert_eq!(result.header, vec!["name", "value"]);
        assert_eq!(result.results.len(), 1);
        assert_eq!(result.results[0]["name"].get(), r#""test""#);
        assert_eq!(result.results[0]["value"].get(), "42");
    }

    #[test]
    fn test_parse_query_result_with_capital_header() {
        let json = r#"{"Header":["col1"],"Results":[{"col1":"\"hello\""}]}"#;
        let result = result(json).unwrap();
        assert_eq!(result.header, vec!["col1"]);
        assert_eq!(result.results.len(), 1);
    }

    #[test]
    fn test_parse_query_result_with_columns() {
        let json = r#"{"header":["ts","msg"],"columns":[{"name":"ts","type":"datetime"},{"name":"msg","type":"string"}],"results":[]}"#;
        let result = result(json).unwrap();
        assert_eq!(result.columns.len(), 2);
        assert_eq!(result.columns[0].name, "ts");
        assert_eq!(result.columns[0].column_type, "datetime");
        assert_eq!(result.columns[1].name, "msg");
        assert_eq!(result.columns[1].column_type, "string");
    }

    #[test]
    fn test_parse_query_result_with_render_hints() {
        let json = r#"{"header":["x","y"],"render":{"chartType":"line"},"results":[]}"#;
        let result = result(json).unwrap();
        assert!(result.render.is_some());
        assert_eq!(result.render.unwrap().chart_type.unwrap(), "line");
    }

    #[test]
    fn test_parse_query_result_empty() {
        let json = r#"{"header":[],"results":[]}"#;
        let result = result(json).unwrap();
        assert!(result.header.is_empty());
        assert!(result.results.is_empty());
        assert!(result.columns.is_empty());
        assert!(result.render.is_none());
    }

    #[test]
    fn test_parse_query_result_missing_columns_defaults_empty() {
        let json = r#"{"header":["a"],"results":[]}"#;
        let result = result(json).unwrap();
        assert!(result.columns.is_empty());
    }

    #[test]
    fn test_parse_query_result_invalid_json() {
        let result = result("not json");
        assert!(result.is_err());
    }

    #[test]
    fn test_query_request_serialization() {
        let req = QueryRequest {
            query: "table | limit 10",
            cursor_position: Some(5),
            variables: &[],
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"query\":\"table | limit 10\""));
        assert!(json.contains("\"cursorPosition\":5"));
    }

    #[test]
    fn test_query_request_omits_null_cursor() {
        let req = QueryRequest {
            query: "test",
            cursor_position: None,
            variables: &[],
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(!json.contains("cursorPosition"));
    }

    #[test]
    fn test_query_result_multiple_rows() {
        let json = r#"{"header":["id","name"],"results":[{"id":"1","name":"\"a\""},{"id":"2","name":"\"b\""},{"id":"3","name":"\"c\""}]}"#;
        let result = result(json).unwrap();
        assert_eq!(result.results.len(), 3);
    }
}
