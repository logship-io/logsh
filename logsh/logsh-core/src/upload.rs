use std::{
    fs::File,
    path::Path,
};

use crate::{
    connect::Connection,
    error::{CommonError, UploadError},
};

pub fn execute<'a>(
    schema_str: &'a str,
    path_str: &'a str,
    connection: &Connection,
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

    if path.extension().is_none() {
        return Err(UploadError::UnsupportedFileExtension("".to_string()));
    }

    let sub = &connection.default_subscription()
            .ok_or(UploadError::Config(crate::error::ConfigError::NoDefaultConnection))?;

    let client = reqwest::blocking::Client::new();
    let file = File::open(path).unwrap();

    match connection
            .authenticate_request(client.post(format!(
                "{}/inflow/{}/{}/{}",
                &connection.server,
                sub,
                schema_str,
                path.extension().unwrap().to_str().unwrap(),
            )))
            .body(file)
            .header("content-type", "application/oxtet-stream")
            .send()
        {
            Ok(res) => {
                if res.status() != 200 {
                    return Err(UploadError::UploadFailureStatus(
                        res.status().as_u16() as i32,
                        res.text().unwrap(),
                    ));
                }
                return Ok(());
            }
            Err(e) => {
                return Err(UploadError::UploadFailure(e));
            }
        }
}
