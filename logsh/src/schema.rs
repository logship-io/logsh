use std::io::Write;

use anyhow::{anyhow, Error};
use clap::Subcommand;
use colored::Colorize;
use logsh_core::{
    config,
    error::{ConfigError, ConnectError},
};
use term_table::{
    row::Row,
    table_cell::{Alignment, TableCell},
    Table, TableStyle,
};

use crate::OutputMode;

#[derive(Debug, Subcommand)]
#[clap(about = "Inspect schemas and tables on the connected server.")]
pub enum SchemaCommand {
    /// List all tables in the current account.
    #[clap(visible_alias = "ls")]
    List,

    /// Describe columns in a specific table.
    Describe {
        /// Table name to describe.
        table: String,
    },
}

pub fn execute_schema<W: Write>(
    command: SchemaCommand,
    output: Option<OutputMode>,
    write: W,
) -> Result<(), Error> {
    match command {
        SchemaCommand::List => list_tables(output, write),
        SchemaCommand::Describe { table } => describe_table(&table, output, write),
    }
}

fn list_tables<W: Write>(output: Option<OutputMode>, mut write: W) -> Result<(), Error> {
    let cfg = config::load()?;
    let ctx = cfg
        .get_current_context()
        .ok_or(ConnectError::Config(ConfigError::NoDefaultConnection))?;

    let r = ctx
        .connection
        .query_raw("$metadata.schema.tables.fields", None)
        .map_err(|e| anyhow!("Failed to query schema: {e}"))?;

    let result =
        logsh_core::query::result(&r).map_err(|e| anyhow!("Failed to parse schema result: {e}"))?;

    // Extract unique table names
    let table_col = result
        .header
        .iter()
        .position(|h| h == "TableName")
        .ok_or_else(|| anyhow!("Schema response missing TableName column"))?;

    let mut tables: Vec<String> = result
        .results
        .iter()
        .filter_map(|row| {
            let raw = row.get(result.header[table_col].as_str())?;
            let s = raw.get().trim_matches('"').to_string();
            if s.is_empty() || s == "null" {
                None
            } else {
                Some(s)
            }
        })
        .collect();
    tables.sort();
    tables.dedup();

    match output.unwrap_or_default() {
        OutputMode::Json => {
            serde_json::to_writer(&mut write, &tables)?;
            writeln!(write)?;
        }
        OutputMode::JsonPretty => {
            serde_json::to_writer_pretty(&mut write, &tables)?;
            writeln!(write)?;
        }
        OutputMode::Csv => {
            for t in &tables {
                writeln!(write, "{t}")?;
            }
        }
        _ => {
            let mut table = Table::new();
            table.style = TableStyle::thin();
            table.add_row(Row::new(vec![TableCell::builder(
                "Table Name".bright_white().bold().to_string(),
            )
            .alignment(Alignment::Left)
            .build()]));
            for t in &tables {
                table.add_row(Row::new(vec![TableCell::builder(t)
                    .alignment(Alignment::Left)
                    .build()]));
            }
            writeln!(write, "{}", table.render())?;
        }
    }
    Ok(())
}

fn describe_table<W: Write>(
    table_name: &str,
    output: Option<OutputMode>,
    mut write: W,
) -> Result<(), Error> {
    let cfg = config::load()?;
    let ctx = cfg
        .get_current_context()
        .ok_or(ConnectError::Config(ConfigError::NoDefaultConnection))?;

    let r = ctx
        .connection
        .query_raw("$metadata.schema.tables.fields", None)
        .map_err(|e| anyhow!("Failed to query schema: {e}"))?;

    let result =
        logsh_core::query::result(&r).map_err(|e| anyhow!("Failed to parse schema result: {e}"))?;

    let table_col = result
        .header
        .iter()
        .position(|h| h == "TableName")
        .ok_or_else(|| anyhow!("Schema response missing TableName column"))?;
    let col_col = result
        .header
        .iter()
        .position(|h| h == "ColumnName")
        .ok_or_else(|| anyhow!("Schema response missing ColumnName column"))?;
    let type_col = result.header.iter().position(|h| h == "ColumnType");

    #[derive(serde::Serialize)]
    struct ColumnInfo {
        name: String,
        #[serde(rename = "type")]
        column_type: String,
    }

    let mut columns: Vec<ColumnInfo> = result
        .results
        .iter()
        .filter_map(|row| {
            let tn = row
                .get(result.header[table_col].as_str())?
                .get()
                .trim_matches('"');
            if !tn.eq_ignore_ascii_case(table_name) {
                return None;
            }
            let cn = row
                .get(result.header[col_col].as_str())?
                .get()
                .trim_matches('"')
                .to_string();
            let ct = type_col
                .and_then(|i| row.get(result.header[i].as_str()))
                .map(|v| v.get().trim_matches('"').to_string())
                .unwrap_or_default();
            if cn.is_empty() || cn == "null" {
                None
            } else {
                Some(ColumnInfo {
                    name: cn,
                    column_type: ct,
                })
            }
        })
        .collect();
    columns.sort_by(|a, b| a.name.cmp(&b.name));

    if columns.is_empty() {
        return Err(anyhow!("Table \"{table_name}\" not found."));
    }

    match output.unwrap_or_default() {
        OutputMode::Json => {
            serde_json::to_writer(&mut write, &columns)?;
            writeln!(write)?;
        }
        OutputMode::JsonPretty => {
            serde_json::to_writer_pretty(&mut write, &columns)?;
            writeln!(write)?;
        }
        OutputMode::Csv => {
            writeln!(write, "name,type")?;
            for c in &columns {
                writeln!(write, "{},{}", c.name, c.column_type)?;
            }
        }
        _ => {
            let mut table = Table::new();
            table.style = TableStyle::thin();
            table.add_row(Row::new(vec![
                TableCell::builder("Column".bright_white().bold().to_string())
                    .alignment(Alignment::Left)
                    .build(),
                TableCell::builder("Type".bright_white().bold().to_string())
                    .alignment(Alignment::Left)
                    .build(),
            ]));
            for c in &columns {
                table.add_row(Row::new(vec![
                    TableCell::builder(&c.name)
                        .alignment(Alignment::Left)
                        .build(),
                    TableCell::builder(&c.column_type)
                        .alignment(Alignment::Left)
                        .build(),
                ]));
            }
            writeln!(
                write,
                "{}",
                format!("Table: {table_name}").bright_white().bold()
            )?;
            writeln!(write, "{}", table.render())?;
        }
    }
    Ok(())
}
