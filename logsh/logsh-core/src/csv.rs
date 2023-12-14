use std::collections::BTreeMap;
use thiserror::Error;

use crate::query::QueryResult;

#[derive(Debug, Error)]
pub enum CsvError {
    #[error("Failed to write csv: {0}")]
    FailedWrite(csv::Error),

    #[error("Failed to flush csv: {0}")]
    FailedFlush(std::io::Error),
}

pub fn write_csv<'a, W: std::io::Write>(
    query: &QueryResult<'a>,
    to: W,
) -> Result<(), CsvError> {
    let mut wtr = csv::Writer::from_writer(to);

    // write headers
    wtr.write_record(&query.header)
        .map_err(CsvError::FailedWrite)?;

    let map = BTreeMap::<&str, usize>::from_iter(
        query
            .header
            .iter()
            .enumerate()
            .map(|tup| (tup.1.as_ref(), tup.0)),
    );

    for r in query.results.iter() {
        let mut arr = vec![String::default(); query.header.len()];
        for (k, v) in r.iter() {
            let i = map.get(k).copied().unwrap_or_else(|| {
                log::error!("Invalid query result. Field \"{k}\" not in headers");
                0
            });

            arr[i] = v.to_string();
        }
        wtr.write_record(arr).map_err(CsvError::FailedWrite)?;
    }

    wtr.flush().map_err(CsvError::FailedFlush)?;
    Ok(())
}
