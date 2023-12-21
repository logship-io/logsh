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
    timeout: Option<std::time::Duration>,
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

    let sub = &connection.default_subscription()
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
    let _response = connection
        .authenticate_request(req)
        .body(file)
        .header("content-type", "application/oxtet-stream")
        .send()?
        .error_for_status()?;
    return Ok(());
}
