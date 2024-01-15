use std::fmt::Display;

use crate::error::CommonError;

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

impl Display for ApiErrorModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut message = self.message.clone();
        if let Some(stack_trace) = &self.stack_trace {
            message.push_str("\n");
            message.push_str(stack_trace);
        }
        write!(f, "{}", message)
    }
}

impl TryFrom<&str> for ApiErrorModel {
    type Error = CommonError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        serde_json::from_str(value).map_err(CommonError::Json)
    }
}