use std::fmt;
use thiserror::Error;

#[derive(Debug, Error)]
pub struct CliError {
    pub message: String,
    pub code: i32,
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Error {}: {}", self.code, self.message)
    }
}

impl From<std::io::Error> for CliError {
    fn from(value: std::io::Error) -> Self {
        Self {
            message: format!("IO Error: {value}"),
            code: 1,
        }
    }
}

impl From<csv::Error> for CliError {
    fn from(value: csv::Error) -> Self {
        Self {
            message: format!("CSV Error: {value}"),
            code: 1,
        }
    }
}