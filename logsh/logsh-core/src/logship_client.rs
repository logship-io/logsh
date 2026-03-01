use crate::{
    common::ApiErrorModel,
    config,
    connect::Connection,
    error::{self},
};

/// Low-level HTTP client for making authenticated requests to a logship server.
pub struct LogshClient {
    pub server: String,
    pub token: String,
}

/// Trait for types that can be executed against a [`LogshClient`].
pub trait LogshClientHandlerExecute<T> {
    /// Executes the operation using the provided client.
    fn execute(&self, client: &LogshClient) -> Result<T, error::ClientError>;
}

/// High-level handler that resolves a connection and delegates work to a [`LogshClient`].
pub struct LogshClientHandler {
    override_connection_name: Option<String>,
}

fn get_clean_path(path: &str) -> &str {
    path.strip_prefix('/').unwrap_or(path)
}

fn map_api_error(response: reqwest::blocking::Response) -> error::ClientError {
    let error = response.json::<ApiErrorModel>().unwrap_or(ApiErrorModel {
        message: "Unknown".to_string(),
        stack_trace: None,
        errors: vec![],
    });
    error::ClientError::Common(error::CommonError::ApiError(error))
}

impl LogshClient {
    /// Creates a new client targeting the given server with the provided bearer token.
    pub fn new(server: &str, token: String) -> Self {
        Self {
            server: server.trim().to_string(),
            token: token.trim().to_string(),
        }
    }

    /// Sends a GET request and deserializes the JSON response.
    pub fn get_json<TResult: for<'de> serde::Deserialize<'de>>(
        &self,
        path: &str,
    ) -> Result<TResult, error::ClientError> {
        let path_clean = get_clean_path(path);
        let url = format!("{}/{}", self.server, path_clean);
        log::debug!("[GET] {url}");
        let client = reqwest::blocking::Client::new();
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            "Authorization",
            format!("Bearer {}", self.token).parse().unwrap(),
        );
        let response = client.get(&url).headers(headers).send()?;
        if !response.status().is_success() {
            return Err(map_api_error(response));
        }
        let json = response.json()?;
        Ok(json)
    }

    /// Sends a POST request with a JSON body and deserializes the JSON response.
    pub fn post_json<TRequest: serde::Serialize, TResult: for<'de> serde::Deserialize<'de>>(
        &self,
        path: &str,
        request: &TRequest,
    ) -> Result<TResult, error::ClientError> {
        let path_clean = get_clean_path(path);
        let url = format!("{}/{}", self.server, path_clean);
        log::debug!("[POST] {url}");
        let client = reqwest::blocking::Client::new();
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            "Authorization",
            format!("Bearer {}", self.token).parse().unwrap(),
        );
        let response = client.post(&url).headers(headers).json(request).send()?;
        if !response.status().is_success() {
            return Err(map_api_error(response));
        }
        let json = response.json()?;
        Ok(json)
    }

    /// Sends a PUT request with a body and deserializes the JSON response.
    pub fn put<
        TRequest: Into<reqwest::blocking::Body>,
        TResult: for<'de> serde::Deserialize<'de>,
    >(
        &self,
        path: &str,
        request: TRequest,
    ) -> Result<TResult, error::ClientError> {
        let path_clean = get_clean_path(path);
        let url = format!("{}/{}", self.server, path_clean);
        log::debug!("[PUT] {url}");
        let client = reqwest::blocking::Client::new();
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            "Authorization",
            format!("Bearer {}", self.token).parse().unwrap(),
        );
        let response = client.put(&url).headers(headers).body(request).send()?;
        if !response.status().is_success() {
            return Err(map_api_error(response));
        }
        let json = response.json()?;
        Ok(json)
    }

    /// Sends a DELETE request to the given path.
    pub fn delete(&self, path: &str) -> Result<(), error::ClientError> {
        let path_clean = get_clean_path(path);
        let url = format!("{}/{}", self.server, path_clean);
        log::debug!("[DELETE] {url}");
        let client = reqwest::blocking::Client::new();
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            "Authorization",
            format!("Bearer {}", self.token).parse().unwrap(),
        );
        let response = client.delete(&url).headers(headers).send()?;
        if !response.status().is_success() {
            return Err(map_api_error(response));
        }
        Ok(())
    }
}

impl Default for LogshClientHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl LogshClientHandler {
    /// Creates a new handler using the default connection from configuration.
    pub fn new() -> Self {
        Self {
            override_connection_name: None,
        }
    }

    /// Resolves the active connection from configuration, optionally using an override name.
    pub fn get_connection(&self) -> Result<Connection, error::ClientError> {
        let default_config = config::load()?;
        let connection = match &self.override_connection_name {
            Some(name) => default_config
                .contexts
                .get(name)
                .ok_or(error::ClientError::ConnectionNotFound(name.to_string()))?
                .clone(),
            None => default_config
                .get_current_context()
                .ok_or(error::ConfigError::NoDefaultConnection)?
                .connection
                .clone(),
        };
        Ok(connection)
    }

    /// Executes a [`LogshClientHandlerExecute`] implementation against the resolved connection.
    pub fn execute<T>(
        &self,
        arg: &dyn LogshClientHandlerExecute<T>,
    ) -> Result<T, error::ClientError> {
        let connection = self.get_connection()?;
        let token = connection.get_token().ok_or(error::ClientError::NoToken)?;

        let client = LogshClient::new(connection.server.as_ref(), token);
        arg.execute(&client)
    }

    /// Convenience method that executes a closure against the resolved connection's client.
    pub fn execute_func<T>(
        &self,
        func: &dyn Fn(&LogshClient) -> Result<T, error::ClientError>,
    ) -> Result<T, error::ClientError> {
        let result = self.execute(&ExecuteWrapper { func })?;
        Ok(result)
    }
}

struct ExecuteWrapper<'a, T> {
    func: &'a dyn Fn(&LogshClient) -> Result<T, error::ClientError>,
}

impl<'a, T> LogshClientHandlerExecute<T> for ExecuteWrapper<'a, T> {
    fn execute(&self, client: &LogshClient) -> Result<T, error::ClientError> {
        let result = (self.func)(client)?;
        Ok(result)
    }
}
