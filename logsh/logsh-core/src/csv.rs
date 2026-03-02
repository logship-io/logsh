use std::collections::BTreeMap;
use thiserror::Error;

use crate::query::QueryResult;

/// Errors that can occur when writing CSV output.
#[derive(Debug, Error)]
pub enum CsvError {
    #[error("Failed to write csv: {0}")]
    FailedWrite(csv::Error),

    #[error("Failed to flush csv: {0}")]
    FailedFlush(std::io::Error),
}

/// Writes a [`QueryResult`] as CSV to the given writer.
pub fn write_csv<'a, W: std::io::Write>(query: &QueryResult<'a>, to: W) -> Result<(), CsvError> {
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

            let raw = v.get();
            // Strip JSON string quotes for CSV output
            arr[i] = if raw.starts_with('"') && raw.ends_with('"') {
                raw[1..raw.len() - 1].to_string()
            } else {
                raw.to_string()
            };
        }
        wtr.write_record(arr).map_err(CsvError::FailedWrite)?;
    }

    wtr.flush().map_err(CsvError::FailedFlush)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_result(json: &str) -> QueryResult<'_> {
        crate::query::result(json).unwrap()
    }

    #[test]
    fn test_csv_basic() {
        let json = r#"{"header":["name","age"],"results":[{"name":"alice","age":30},{"name":"bob","age":25}]}"#;
        let result = make_result(json);
        let mut buf = Vec::new();
        write_csv(&result, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        let lines: Vec<&str> = output.trim().lines().collect();
        assert_eq!(lines[0], "name,age");
        assert_eq!(lines[1], "alice,30");
        assert_eq!(lines[2], "bob,25");
    }

    #[test]
    fn test_csv_empty_results() {
        let json = r#"{"header":["a","b"],"results":[]}"#;
        let result = make_result(json);
        let mut buf = Vec::new();
        write_csv(&result, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        let lines: Vec<&str> = output.trim().lines().collect();
        assert_eq!(lines.len(), 1); // headers only
        assert_eq!(lines[0], "a,b");
    }

    #[test]
    fn test_csv_single_column() {
        let json = r#"{"header":["val"],"results":[{"val":1},{"val":2}]}"#;
        let result = make_result(json);
        let mut buf = Vec::new();
        write_csv(&result, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("val\n") || output.contains("val\r\n"));
        assert!(output.contains("1\n") || output.contains("1\r\n"));
    }
}
