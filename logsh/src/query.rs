use std::{
    io::{Read, Write},
    str::FromStr,
    time::Instant,
};

use anyhow::{anyhow, Error};
use clap::arg;
use term_table::{
    row::Row,
    table_cell::{Alignment, TableCell},
    Table, TableStyle,
};

use crate::OutputMode;

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
#[clap(about = "Execute a query against a logship server.")]
pub struct QueryCommand {
    #[arg(
        short,
        long,
        help = "Query to execute. If not provided, will read from stdin."
    )]
    query: Option<String>,

    #[arg(short, long, help = "Output result format")]
    output: Option<OutputMode>,
}

pub fn execute_query<W: Write>(command: QueryCommand, mut write: W) -> Result<(), Error> {
    log::debug!("Entering query execution: {:?}", &command);
    let start = Instant::now();

    let query = if let Some(q) = command.query {
        log::trace!("Provided query: {}", &q);
        q
    } else {
        log::debug!("Reading query from STDIN");
        let mut s = String::new();
        let _ = std::io::stdin()
            .read_to_string(&mut s)
            .map_err(|err| anyhow!("Failed to read STDIN: {}", err))?;
        s
    };

    let r = logsh_core::query::execute(&query)?;
    log::debug!("Response text: {:?}", r);
    let result = logsh_core::query::result(&r)?;
    let query_duration = start.elapsed();
    let render_start = Instant::now();
    log::trace!("Finished query execution.");
    log::trace!("Processing result.");
    match command.output.unwrap_or_default() {
        OutputMode::Table => {
            log::trace!("Outputting table");
            render_table(result, TableStyle::thin(), false, write)
        }
        OutputMode::Markdown => {
            log::trace!("Outputting markdown table");
            render_table(result, markdown_style(), true, write)
        }
        OutputMode::Json => {
            log::trace!("Outputting unformatted JSON");
            writeln!(write, "{}", r)?;
            Ok(())
        }
        OutputMode::JsonPretty => {
            log::trace!("Outputting pretty JSON");
            serde_json::to_writer_pretty(write, &result)?;
            Ok(())
        }
        OutputMode::Csv => {
            log::trace!("Outputting CSV");
            logsh_core::csv::write_csv(&result, write)
                .map_err(|e| anyhow!("Failed to convert to CSV: {}", e))
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
            .map(|f| TableCell::new_with_alignment(f, 1, Alignment::Center)),
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
                    TableCell::new_with_alignment(json, 1, Alignment::Center)
                } else {
                    TableCell::new_with_alignment(json, 1, Alignment::Center)
                }
            }
            _ => TableCell::new_with_alignment(row[header.as_str()].get(), 1, Alignment::Center),
        });

        let mut row = Row::new(cells);
        row.has_separator = !is_markdown || is_first;
        table.add_row(row);

        is_first = false;
    }

    log::trace!("Render table.");

    let table = table.render();
    writeln!(write, "{}", table).map_err(|e| anyhow!("Failed to write table: {}", e))
}
