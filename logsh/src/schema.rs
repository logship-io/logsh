use std::io::Write;

use anyhow::{anyhow, Error};
use clap::Subcommand;
use colored::Colorize;
use logsh_core::config;
use term_table::{
    row::Row,
    table_cell::{Alignment, TableCell},
    Table, TableStyle,
};

use crate::OutputMode;

#[derive(Debug, Subcommand)]
#[clap(about = "Inspect and manage schemas and tables on the connected server.")]
pub enum SchemaCommand {
    /// List all tables in the current account.
    #[clap(visible_alias = "ls")]
    List,

    /// Create a table or merge declared columns; other columns remain inferred.
    #[clap(visible_alias = "create")]
    Define {
        table: String,
        /// Column declaration NAME:TYPE; repeat for multiple columns.
        #[arg(long = "column", required = true, value_parser = parse_column)]
        columns: Vec<ColumnDefinition>,
    },

    /// Add a declared column (creates the table if it does not exist).
    AddColumn {
        table: String,
        column: String,
        column_type: String,
    },

    /// Remove a column from the catalog; ingestion may infer it again.
    #[clap(visible_alias = "remove-column")]
    DropColumn { table: String, column: String },

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
        command => {
            let query = management_query(command)?;
            crate::query::execute_query_text(
                &query,
                Some(std::time::Duration::from_secs(60)),
                output,
                write,
            )
        }
    }
}

fn list_tables<W: Write>(output: Option<OutputMode>, mut write: W) -> Result<(), Error> {
    let cfg = config::load()?;
    let ctx = cfg.resolve_current_context()?;

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
    let ctx = cfg.resolve_current_context()?;

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

// Backend identifiers cannot contain quotes or command delimiters.
fn identifier(value: &str) -> Result<&str, Error> {
    if value.is_empty()
        || value.encode_utf16().count() > 255
        || !value
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '.'))
    {
        return Err(anyhow!(
            "Names must contain 1–255 letters, digits, dots, underscores, or hyphens"
        ));
    }
    Ok(value)
}

#[derive(Debug, Clone)]
pub struct ColumnDefinition {
    name: String,
    column_type: String,
}

fn parse_column(value: &str) -> Result<ColumnDefinition, String> {
    let (name, typ) = value.split_once(':').ok_or("Expected NAME:TYPE")?;
    identifier(name).map_err(|e| e.to_string())?;
    if typ.is_empty() || !typ.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err("Expected a column type name".into());
    }
    Ok(ColumnDefinition {
        name: name.to_owned(),
        column_type: typ.to_owned(),
    })
}

fn management_query(command: SchemaCommand) -> Result<String, Error> {
    match command {
        SchemaCommand::Define { table, columns } => {
            identifier(&table)?;
            let columns = columns
                .iter()
                .map(|c| format!("'{}':{}", c.name, c.column_type))
                .collect::<Vec<_>>();
            if columns.is_empty() {
                return Err(anyhow!("At least one column is required"));
            }
            Ok(format!(
                ".create-merge table '{table}' ({})",
                columns.join(", ")
            ))
        }
        SchemaCommand::AddColumn {
            table,
            column,
            column_type,
        } => management_query(SchemaCommand::Define {
            table,
            columns: vec![parse_column(&format!("{column}:{column_type}")).map_err(|e| anyhow!(e))?],
        }),
        SchemaCommand::DropColumn { table, column } => {
            identifier(&table)?;
            identifier(&column)?;
            Ok(format!(".drop column '{table}'.'{column}'"))
        }
        _ => Err(anyhow!("Expected a schema management command")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn management_names_cannot_inject_commands() {
        for name in ["", "logs'; .drop table 'other", "logs\nother", "logs;other"] {
            assert!(management_query(SchemaCommand::DropColumn {
                table: name.into(),
                column: "field".into()
            })
            .is_err());
        }
        for column in [
            "message:string); .drop table 'logs'",
            "field:",
            "field",
            ":string",
        ] {
            assert!(parse_column(column).is_err());
        }
    }
}
