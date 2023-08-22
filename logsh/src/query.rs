use std::{collections::HashMap, io::Read};

use log::{error, trace};
use serde_json::value::RawValue;
use term_table::{
    row::Row,
    table_cell::{Alignment, TableCell},
    Table,
};

use crate::{config, error::CliError};

#[derive(Debug, clap::Args)]
#[clap(about = "Execute a query against a logship server.")]
pub struct QueryCommand {
    #[arg(
        short,
        long,
        help = "Query to execute. If not provided, will read from stdin."
    )]
    query: Option<String>,

    #[arg(long, help = "Output results as JSON.")]
    json: bool,
}

#[derive(serde::Deserialize, Debug)]
#[serde(bound(deserialize = "'de: 'a"))]
struct QueryResult<'a> {
    #[serde(alias = "Header")]
    #[serde(alias = "header")]
    header: Vec<String>,

    #[serde(alias = "Results")]
    #[serde(alias = "results")]
    results: Vec<HashMap<&'a str, &'a RawValue>>,
}

pub fn execute_query(command: QueryCommand) -> Result<(), CliError> {
    let connection = config::get_default_connection()?;

    let mut query_string = std::string::String::new();
    match command.query {
        Some(query) => query_string = query,
        None => {
            std::io::stdin()
                .read_to_string(&mut query_string)
                .map_err(|e| CliError {
                    message: format!("Unable to read query from stdin: {}", e),
                    code: 1,
                })?;
        }
    };

    let mut map = HashMap::new();
    map.insert("query", query_string);

    let client = reqwest::blocking::Client::new();
    let res = client
        .post(connection.server + "/search/" + &connection.default_acccount_id + "/kusto")
        .json(&map)
        .header("Authorization", "Bearer ".to_owned() + &connection.token)
        .send()
        .map_err(|e| CliError {
            message: format!("Unable to connect to server: {}", e),
            code: 1,
        })?;

    trace!("Response: {:?}", res);
    let status = res.status();
    let result_text = res.text().map_err(|e| CliError {
        message: format!("Unable to read response: {}", e),
        code: 1,
    })?;
    if status != 200 {
        error!("Status {}: Error: {}", status, result_text);
        return Ok(());
    }

    if command.json {
        println!("{}", result_text);
        return Ok(());
    }

    trace!("Response text: {:?}", result_text);

    let result: QueryResult = serde_json::from_str(&result_text).map_err(|e| CliError {
        message: format!("Unable to parse response: {}", e),
        code: 1,
    })?;
    let mut table = Table::new();

    table.add_row(Row::new(
        result
            .header
            .iter()
            .map(|f| TableCell::new_with_alignment(f, 1, Alignment::Center)),
    ));
    result.results.iter().for_each(|f| {
        table.add_row(Row::new(result.header.iter().map(|h| {
            TableCell::new_with_alignment(f[h as &str].get(), 1, Alignment::Center)
        })));
    });

    println!("{}", table.render());
    Ok(())
}
