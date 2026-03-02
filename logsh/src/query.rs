use std::{
    io::{Read, Write},
    str::FromStr,
    time::Instant,
};

use anyhow::{anyhow, Error};

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

use crate::{fmt::parse::OptionalDurationArg, OutputMode};

pub fn markdown_style() -> TableStyle {
    let mut style: TableStyle = TableStyle::simple();
    style.top_left_corner = '│';
    style.top_right_corner = '│';
    style.bottom_left_corner = '│';
    style.bottom_right_corner = '│';
    style.outer_right_vertical = '|';
    style.outer_left_vertical = '|';
    style.intersection = '|';
    style.vertical = '|';
    style
}

#[derive(Debug, clap::Args)]
#[clap(
    about = "Execute a query against a logship server.",
    long_about = "Execute a Kusto query against a Logship server.\n\n\
        Reads the query from --query, --file, or from stdin if neither is provided.\n\
        Supports multiple output formats for both human and machine consumption.\n\n\
        Examples:\n  \
        logsh query -q 'MyTable | take 10'\n  \
        logsh query -f query.kql -o json\n  \
        logsh query -q 'MyTable | count' -o json\n  \
        echo 'MyTable | take 5' | logsh query -o csv"
)]
pub struct QueryCommand {
    #[arg(
        short,
        long,
        help = "Query to execute. If not provided, reads from --file or stdin."
    )]
    query: Option<String>,

    #[arg(
        short,
        long,
        help = "Read query from a file (e.g. query.kql).",
        conflicts_with = "query"
    )]
    file: Option<String>,

    #[arg(
        short,
        long,
        help = "Query timeout (e.g. '30s', '5m'). Use 'none' to disable.",
        default_value = "60s"
    )]
    timeout: OptionalDurationArg,
}

pub fn execute_query<W: Write>(
    command: QueryCommand,
    output: Option<OutputMode>,
    mut write: W,
) -> Result<(), Error> {
    log::debug!("Entering query execution: {:?}", &command);
    let start = Instant::now();

    let query = if let Some(q) = command.query {
        q
    } else if let Some(path) = command.file {
        std::fs::read_to_string(&path)
            .map_err(|err| anyhow!("Failed to read query file \"{path}\": {err}"))?
    } else {
        let mut s = String::new();
        let _ = std::io::stdin()
            .read_to_string(&mut s)
            .map_err(|err| anyhow!("Failed to read STDIN: {err}"))?;
        s
    };

    let cfg = config::load()?;
    let connection: config::ContextConfig = cfg
        .get_current_context()
        .ok_or(ConnectError::Config(ConfigError::NoDefaultConnection))?;
    log::info!("Starting query. Timeout = {}", &command.timeout);
    let r = connection
        .connection
        .query_raw(&query, command.timeout.into())
        .inspect_err(|err| {
            crate::fmt::print_query_error(&cfg, &query, err);
        })?;

    log::debug!("Response text: {r:?}");
    let result = logsh_core::query::result(&r).inspect_err(|err| {
        crate::fmt::print_query_error(&cfg, &query, err);
    })?;
    let query_duration = start.elapsed();
    let render_start = Instant::now();
    match output.unwrap_or_default() {
        OutputMode::Table => render_table(result, TableStyle::thin(), false, write),
        OutputMode::Markdown => render_table(result, markdown_style(), true, write),
        OutputMode::Json => {
            writeln!(write, "{r}")?;
            Ok(())
        }
        OutputMode::JsonPretty => {
            serde_json::to_writer_pretty(write, &result)?;
            Ok(())
        }
        OutputMode::Csv => {
            logsh_core::csv::write_csv(&result, write)
                .map_err(|e| anyhow!("Failed to convert to CSV: {e}"))
        }
    }?;

    let render_duration = render_start.elapsed();
    let elapsed = start.elapsed();
    log::debug!(
        "Query execution in {}s [{}ms]",
        query_duration.as_secs_f64(),
        query_duration.as_millis()
    );
    log::debug!(
        "Query rendered in {}s [{}ms]",
        render_duration.as_secs_f64(),
        render_duration.as_millis()
    );
    log::info!(
        "Query executed and rendered in {}s [{}ms]",
        elapsed.as_secs_f64(),
        elapsed.as_millis()
    );
    Ok(())
}

fn render_table<W: Write>(
    result: logsh_core::query::QueryResult<'_>,
    style: TableStyle,
    is_markdown: bool,
    mut write: W,
) -> Result<(), Error> {
    let mut table = Table::new();
    table.style = style;
    table.has_bottom_boarder = !is_markdown;
    let mut header_row = Row::new(
        result
            .header
            .iter()
            .map(|s| {
                if is_markdown {
                    s.to_string()
                } else {
                    s.bright_white().bold().to_string()
                }
            })
            .map(|f| {
                TableCell::builder(f)
                    .col_span(1)
                    .alignment(Alignment::Center)
                    .build()
            }),
    );
    header_row.has_separator = !is_markdown;
    table.add_row(header_row);

    let mut is_first = true;
    for row in result.results {
        let cells = result.header.iter().map(|header| match header.as_str() {
            "json" => {
                let str = header.as_str();
                let json = row[str].get();
                if let Ok(json) =
                    serde_json::Value::from_str(json).and_then(|j| serde_json::to_string_pretty(&j))
                {
                    TableCell::builder(json)
                        .col_span(1)
                        .alignment(Alignment::Center)
                        .build()
                } else {
                    TableCell::builder(json)
                        .col_span(1)
                        .alignment(Alignment::Center)
                        .build()
                }
            }
            _ => {
                let str = header.as_str();
                let json = row[str].get();

                if let Ok(json) = serde_json::Value::from_str(json) {
                    if !is_markdown {
                        match json {
                            serde_json::Value::Null => {
                                return TableCell::builder("<null>".bright_black())
                                    .col_span(1)
                                    .alignment(Alignment::Center)
                                    .build()
                            }
                            serde_json::Value::Bool(b) => {
                                return TableCell::builder(if b {
                                    "true".green()
                                } else {
                                    "false".red()
                                })
                                .col_span(1)
                                .alignment(Alignment::Center)
                                .build()
                            }
                            serde_json::Value::Number(n) => {
                                return TableCell::builder(n)
                                    .col_span(1)
                                    .alignment(Alignment::Left)
                                    .build()
                            }
                            serde_json::Value::String(s) => {
                                return TableCell::builder(s)
                                    .col_span(1)
                                    .alignment(Alignment::Center)
                                    .build()
                            }
                            _ => { /* noop */ }
                        }
                    }

                    if let Ok(serialized) = serde_json::to_string_pretty(&json) {
                        return TableCell::builder(serialized)
                            .col_span(1)
                            .alignment(Alignment::Center)
                            .build();
                    }
                }

                TableCell::builder(json)
                    .col_span(1)
                    .alignment(Alignment::Center)
                    .build()
            }
        });

        let mut row = Row::new(cells);
        row.has_separator = !is_markdown || is_first;
        table.add_row(row);

        is_first = false;
    }

    let table = table.render();
    writeln!(write, "{table}").map_err(|e| anyhow!("Failed to write table: {e}"))
}
