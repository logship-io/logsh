use std::{
    fs::File,
    io::{Read, Write},
    path::Path,
};

use crate::{
    connect::Connection,
    error::{CommonError, UploadError, ClientError}, logship_client::LogshClientHandler,
};
use reqwest::blocking::Body;
use serde_json::Value;

struct ProgressReader<R: Read> {
    inner: R,
    sent: u64,
    total: u64,
    last_percent: u64,
    pretty: bool,
}

impl<R: Read> ProgressReader<R> {
    fn new(inner: R, total: u64, pretty: bool) -> Self {
        Self {
            inner,
            sent: 0,
            total,
            last_percent: 0,
            pretty,
        }
    }

    fn print_progress(&mut self) {
        if !self.pretty || self.total == 0 {
            return;
        }

        let percent = ((self.sent * 100) / self.total).min(100);
        if percent != self.last_percent {
            print!("\rUploading: {:>3}%", percent);
            let _ = std::io::stdout().flush();
            self.last_percent = percent;
        }
    }
}

impl<R: Read> Read for ProgressReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let bytes_read = self.inner.read(buf)?;
        self.sent = self.sent.saturating_add(bytes_read as u64);
        if bytes_read == 0 {
            self.sent = self.total;
        }
        self.print_progress();
        Ok(bytes_read)
    }
}

fn map_upload_error(response: reqwest::blocking::Response) -> UploadError {
    let status = response.status();
    let message = response
        .text()
        .ok()
        .and_then(|body| {
            serde_json::from_str::<Value>(&body)
                .ok()
                .and_then(|v| v.get("message").and_then(|m| m.as_str().map(|s| s.to_string())))
                .or_else(|| Some(body))
        })
        .unwrap_or_else(|| "Unknown error".to_string());

    UploadError::UploadFailureStatus(status.as_u16() as i32, message)
}

pub fn execute<'a>(
    schema_str: &'a str,
    path_str: &'a str,
    connection: &Connection,
    timeout: Option<std::time::Duration>,
    pretty_progress: bool,
) -> Result<(), UploadError> {
    if path_str.trim().is_empty() {
        log::debug!("Uploading file: {:?}", path_str);
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

    let ext = path.extension()
        .ok_or(UploadError::UnsupportedFileExtension("".to_string()))
        .map(|e| e.to_string_lossy())?;

    let sub = &connection.default_account()
        .ok_or(UploadError::Config(crate::error::ConfigError::NoDefaultConnection))?;

    let client = crate::connect::client_builder()
        .timeout(timeout)
        .build()?;
    let req = client.post(format!(
        "{}/inflow/{}/{}/{}",
        &connection.server.trim_end_matches("/"),
        sub,
        schema_str,
        ext,
    ));

    let file = File::open(path)?;
    let file_size = file.metadata().map(|m| m.len()).unwrap_or(0);
    let body: Body = if pretty_progress {
        Body::new(ProgressReader::new(file, file_size, true))
    } else {
        Body::new(file)
    };

    let response = connection
        .authenticate_request(req)
        .body(body)
        .header("content-type", "application/occoptet-stream")
        .send()?;

    if !response.status().is_success() {
        return Err(map_upload_error(response));
    }

    if pretty_progress {
        println!();
    }

    Ok(())
}

pub fn execute_upload<'a>(
    client: &LogshClientHandler,
    schema_str: &'a str,
    path_str: &'a str,
) -> Result<(), UploadError> {
    if path_str.trim().is_empty() {
        log::debug!("Uploading file: {:?}", path_str);
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

    let ext = path.extension()
        .ok_or(UploadError::UnsupportedFileExtension("".to_string()))
        .map(|e| e.to_string_lossy())?;

    let connection = client.get_connection()?;
    if connection.default_account.is_none() {
        return Err(UploadError::Config(crate::error::ConfigError::NoDefaultAccount));
    }

    let query_url = format!(
        "inflow/{}/{}/{}",
        connection.default_account.unwrap(),
        schema_str,
        ext,
    );

    client.execute_func(&|client| -> Result<(), ClientError> {
        let file = File::open(path).map_err(|err| { ClientError::Common(CommonError::IOError(err))})?;
        let _result: () = client.put(&query_url, file)?;
        Ok(())
    })?;

    Ok(())
}
