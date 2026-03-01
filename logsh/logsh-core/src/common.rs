use std::fmt::Display;

use crate::error::CommonError;

/// Token position range within a query string, used for error highlighting.
#[derive(serde::Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ErrorToken {
    pub start: i32,
    pub end: i32,
}

/// A single error entry with a message and associated token positions.
#[derive(serde::Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ErrorMessage {
    pub message: Option<String>,
    pub tokens: Vec<ErrorToken>,
}

/// Structured error model returned by the logship API.
#[derive(serde::Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ApiErrorModel {
    pub message: String,
    pub stack_trace: Option<String>,
    pub errors: Vec<ErrorMessage>,
}

impl Display for ApiErrorModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut message = self.message.clone();
        if let Some(stack_trace) = &self.stack_trace {
            message.push('\n');
            message.push_str(stack_trace);
        }
        write!(f, "{message}")
    }
}

impl TryFrom<&str> for ApiErrorModel {
    type Error = CommonError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        serde_json::from_str(value).map_err(CommonError::Json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_api_error() {
        let json = r#"{"message":"Bad query","stackTrace":null,"errors":[{"message":"Syntax error","tokens":[{"start":0,"end":5}]}]}"#;
        let err: ApiErrorModel = json.try_into().unwrap();
        assert_eq!(err.message, "Bad query");
        assert!(err.stack_trace.is_none());
        assert_eq!(err.errors.len(), 1);
        assert_eq!(err.errors[0].tokens[0].start, 0);
        assert_eq!(err.errors[0].tokens[0].end, 5);
    }

    #[test]
    fn test_parse_api_error_with_stack_trace() {
        let json = r#"{"message":"Error","stackTrace":"at Main()","errors":[]}"#;
        let err: ApiErrorModel = json.try_into().unwrap();
        assert_eq!(err.stack_trace, Some("at Main()".to_string()));
    }

    #[test]
    fn test_display_without_stack_trace() {
        let err = ApiErrorModel {
            message: "Test error".to_string(),
            stack_trace: None,
            errors: vec![],
        };
        assert_eq!(format!("{err}"), "Test error");
    }

    #[test]
    fn test_display_with_stack_trace() {
        let err = ApiErrorModel {
            message: "Test error".to_string(),
            stack_trace: Some("stack".to_string()),
            errors: vec![],
        };
        assert_eq!(format!("{err}"), "Test error\nstack");
    }

    #[test]
    fn test_parse_invalid_json() {
        let result: Result<ApiErrorModel, _> = "not json".try_into();
        assert!(result.is_err());
    }
}
