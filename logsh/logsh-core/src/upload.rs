use std::{
    collections::HashMap,
    fs::File,
    io::{prelude::*, BufReader},
    path::Path,
    thread,
    time::Duration,
};

use crate::{
    config::{self, Connection},
    error::{CommonError, UploadError},
};
use chrono;
use log::{debug, info, trace, warn};
use reqwest::blocking::Client;
use serde_json::{Number, Value};

use flate2::write::GzEncoder;
use flate2::Compression;

#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde()]
pub struct Record {
    #[serde(rename = "Schema")]
    pub schema: String,

    #[serde(rename = "Timestamp")]
    pub timestamp: String,

    #[serde(rename = "Data")]
    pub data: HashMap<String, Value>,
}

trait FileReader {
    fn read(&mut self) -> Result<Record, UploadError>;
    fn progress(&mut self) -> f32;
}

struct TsvFileReader {
    reader: BufReader<File>,
    header: Vec<String>,
    schema: String,
    now: String,

    first: bool,
    converters: Vec<fn(&str) -> Value>,
}

impl TsvFileReader {
    pub fn new<'b>(path: String, schema: String) -> Self {
        trace!("Opening file: {}", path);
        let file = File::open(path).unwrap();
        let mut reader = BufReader::new(file);
        let mut buffer = String::new();
        let len = reader.read_line(&mut buffer).unwrap();
        debug!("Read header: {}", buffer[..len].trim());
        let header = buffer[..len]
            .split("\t")
            .map(|s| s.trim().to_string())
            .collect();

        let now = chrono::Utc::now();

        let result: TsvFileReader = TsvFileReader {
            reader,
            header,
            schema,
            now: format!("{:?}", now),
            first: true,
            converters: Vec::new(),
        };
        return result;
    }
}

impl FileReader for TsvFileReader {
    fn read(&mut self) -> Result<Record, UploadError> {
        let mut buffer = String::new();
        match self.reader.read_line(&mut buffer) {
            Ok(size) => {
                if size == 0 {
                    return Err(UploadError::Common(CommonError::EndOfFile()));
                }
                trace!("Read line: {}", buffer[..size].trim());
                let mut data = HashMap::new();

                if self.first {
                    self.first = false;
                    buffer[..size].trim().split("\t").for_each(|f| {
                        if f.parse::<bool>().is_ok() {
                            self.converters.push(|s| {
                                s.parse::<bool>()
                                    .and_then(|op| Ok(Value::Bool(op)))
                                    .unwrap_or_else(|_| Value::Null)
                            });
                        } else if f.parse::<i64>().is_ok() {
                            self.converters.push(|s| {
                                s.parse::<i64>()
                                    .and_then(|op| Ok(Value::Number(op.into())))
                                    .unwrap_or_else(|_| Value::Null)
                            });
                        } else if f.parse::<f64>().is_ok() {
                            self.converters.push(|s| {
                                s.parse::<f64>()
                                    .and_then(|op| Ok(Value::Number(Number::from_f64(op).unwrap())))
                                    .unwrap_or_else(|_| Value::Null)
                            });
                        } else {
                            self.converters.push(|s| Value::String(s.to_string()));
                        }
                    });
                }

                for item in self.header.iter().zip(
                    self.converters
                        .iter()
                        .zip(buffer[..size].trim().split("\t")),
                ) {
                    trace!("Adding item: {:?}", item);
                    data.insert(item.0.to_owned(), item.1 .0(item.1 .1));
                }

                let record = Record {
                    schema: self.schema.clone(),
                    timestamp: self.now.clone(),
                    data,
                };
                Ok(record)
            }
            Err(e) => return Err(UploadError::FailedToReadFile(e)),
        }
    }

    fn progress(&mut self) -> f32 {
        let offset = self.reader.stream_position().unwrap();
        let length = self.reader.get_ref().metadata().unwrap().len();
        return (offset as f64 / length as f64) as f32;
    }
}

fn create_file_reader(path: &Path, schema: String) -> Result<Box<dyn FileReader>, UploadError> {
    let extension = path.extension().unwrap().to_str().unwrap();
    debug!("Resolved file extension: {}", extension);

    match extension {
        "tsv" => {
            return Ok(Box::new(TsvFileReader::new(
                path.to_str().unwrap().to_string(),
                schema,
            )));
        }
        _ => {
            return Err(UploadError::UnsupportedFileExtension(extension.to_string()));
        }
    }
}

fn push_records(
    client: &Client,
    connection: &Connection,
    records: &Vec<Record>,
    attempts: i32,
) -> Result<(), UploadError> {
    let mut i = 0;
    loop {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
        serde_json::to_writer(&mut encoder, &records).unwrap();
        let result = encoder.finish().unwrap();
        debug!("GZIP length: {}", result.len());
        match client
            .post(format!(
                "{}/inflow/{}",
                connection.server(),
                connection.default_acccount_id()
            ))
            .body(result)
            .header(
                "Authorization",
                format!("Bearer {}", connection.bearer_token()),
            )
            .header("content-type", "application/json")
            .header("content-encoding", "gzip")
            .send()
        {
            Ok(res) => {
                if res.status() != 200 {
                    return Err(UploadError::UploadFailureStatus(
                        res.status().as_u16() as i32,
                        res.text().unwrap(),
                    ));
                }

                debug!(
                    "Uploaded {} records with result {:?}",
                    records.len(),
                    res.status()
                );
                return Ok(());
            }
            Err(e) => {
                i += 1;
                if i >= attempts {
                    return Err(UploadError::UploadFailure(e));
                }

                warn!(
                    "Failed to upload records: {} Retrying attempt {} out of {}",
                    e, i, attempts
                );
                thread::sleep(Duration::from_secs(1));
                continue;
            }
        }
    }
}

pub fn execute<'a>(schema_str: &'a str, path_str: &'a str) -> Result<(), UploadError> {
    let connection = config::get_default_connection().map_err(UploadError::Config)?;
    if path_str.trim().is_empty() {
        debug!("Uploading file: {:?}", path_str);
        return Err(UploadError::Common(CommonError::EmptyArgument(
            "path".to_string(),
        )));
    }

    let path = Path::new(path_str);
    if !path.exists() {
        return Err(UploadError::Common(CommonError::FileNotFound(
            path_str.to_string(),
        )));
    }

    if path.extension().is_none() {
        return Err(UploadError::UnsupportedFileExtension("".to_string()));
    }

    let client = reqwest::blocking::Client::new();

    let mut previous_update = chrono::Utc::now();
    let mut upload_set: Vec<Record> = Vec::new();
    let mut reader = create_file_reader(path, schema_str.to_string())?;
    loop {
        match reader.read() {
            Ok(record) => {
                upload_set.push(record);
            }
            Err(e) => match e {
                UploadError::Common(CommonError::EndOfFile()) => {
                    break;
                }
                _ => {
                    return Err(e);
                }
            },
        }

        if upload_set.len() >= 20000 {
            push_records(&client, &connection, &upload_set, 3)?;
            upload_set.clear();
        }

        let now = chrono::Utc::now();
        if (now - previous_update).num_seconds() > 5 {
            previous_update = now;
            info!("Progress: {:.2}%", reader.progress() * 100.0);
        }
    }

    if upload_set.len() > 0 {
        push_records(&client, &connection, &upload_set, 3)?;
    }

    Ok(())
}
