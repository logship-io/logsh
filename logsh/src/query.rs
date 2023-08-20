use std::{collections::HashMap, error::Error, io::Read};

use log::{error, trace};
use serde_json::value::RawValue;
use term_table::{
    row::Row,
    table_cell::{Alignment, TableCell},
    Table,
};

use crate::config;

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

pub fn execute_query(command: QueryCommand) -> Result<(), Box<dyn Error>> {
    let connection = config::get_default_connection()
        .expect("No default connection info found. Please try adding a connection");

    let mut query_string = std::string::String::new();
    match command.query {
        Some(query) => query_string = query,
        None => {
            std::io::stdin().read_to_string(&mut query_string)?;
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
        .unwrap();

    trace!("Response: {:?}", res);

    if res.status() != 200 {
        error!("Status {}: Error: {}", res.status(), res.text()?);
        return Ok(());
    }

    let full_text = res.text()?;
    if command.json {
        println!("{}", full_text);
        return Ok(());
    }

    trace!("Response text: {:?}", full_text);

    let result: QueryResult = serde_json::from_str(&full_text)?;
    println!("{:?}", result);

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
