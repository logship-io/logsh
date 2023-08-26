use std::str::FromStr;

use anyhow::{anyhow, Error};
use clap::ValueEnum;
use logsh_core::query::QueryResponse;

#[cfg(feature = "csv")]
pub mod csv;

pub fn render<W: std::io::Write>(response: QueryResponse<'_>, format: OutputMode, write: W) {
    
    
    match format {
        OutputMode::Table => todo!(),
        OutputMode::Json => todo!(),
        OutputMode::JsonPretty => todo!(),
        OutputMode::Csv => csv::write_csv(query?, to),
    }
}

trait Formatter {
    fn format<W: std::io::Write>(&self, response: QueryResponse<'_>, write: W);
}

impl Formatter for OutputMode::Table {
    fn format<W: std::io::Write>(&self, response: QueryResponse<'_>, write: W) {
        match self {
            OutputMode::Table => OutputMode::Table.format(response, write),
            OutputMode::Json => OutputMode::Json.format(response, write),
            OutputMode::JsonPretty => OutputMode::JsonPretty.format(response, write),
            OutputMode::Csv => OutputMode::Csv.format(response, write),
        }
    }
}

#[derive(Copy, Clone, Debug, Default, ValueEnum)]
pub (crate) enum OutputMode {
    #[default]
    Table,
    Json,
    JsonPretty,
    Csv,
}

impl FromStr for OutputMode {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "json" => Ok(OutputMode::Json),
            "json-pretty" => Ok(OutputMode::JsonPretty),
            "csv" => Ok(OutputMode::Csv),
            _ => Err(anyhow!("Failed to read output format: \"{}\"", s)),
        }
    }
}
