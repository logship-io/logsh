use std::{error::Error, fmt};

#[derive(Debug)]
pub struct CliError {
    pub message: String,
    pub code: i32,
}

impl Error for CliError {}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Error {}: {}", self.code, self.message)
    }
}
