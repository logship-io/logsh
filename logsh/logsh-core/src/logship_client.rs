use crate::{error::{self}, config};

pub struct LogshClient {
    pub server : String,
    pub token : String
}

pub trait LogshClientHandlerExecute<T> {
    fn execute(&self, client : &LogshClient) -> Result<T, error::ClientError>;
}

pub struct LogshClientHandler {
    override_connection_name : Option<String>
}

fn get_clean_path(path: &str) -> &str {
    let mut path_clean = path;
    if path.len() > 0 && path.chars().nth(0).unwrap() == '/' {
        path_clean = path[1..].as_ref();
    }
    path_clean
}

impl LogshClient {
    pub fn new(server: &str, token : String) -> Self {
        Self {
            server: server.trim().to_string(),
            token: token.trim().to_string()
        }
    }

    pub fn get_json<TResult :  for<'de> serde::Deserialize<'de>>(&self, path: &str) -> Result<TResult, error::ClientError> {
        let path_clean = get_clean_path(path);
        let url = format!("{}/{}", self.server, path_clean);
        log::debug!("[GET] {}", url);
        let client = reqwest::blocking::Client::new();
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("Authorization", format!("Bearer {}", self.token).parse().unwrap());
        let response = client.get(&url).headers(headers).send()?;
        let json = response.json()?;
        Ok(json)
    }

    pub fn delete(&self, path: &str) -> Result<(), error::ClientError> {
        let path_clean = get_clean_path(path);
        let url = format!("{}/{}", self.server, path_clean);
        log::debug!("[DELETE] {}", url);
        let client = reqwest::blocking::Client::new();
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("Authorization", format!("Bearer {}", self.token).parse().unwrap());
        client.delete(&url).headers(headers).send()?;
        Ok(())
    }
}

impl LogshClientHandler {
    pub fn new() -> Self {
        Self {
            override_connection_name: None
        }
    }

    pub fn execute<T>(&self, arg : &dyn LogshClientHandlerExecute<T>) -> Result<T, error::ClientError> {
        let default_config = config::load()?;

        // Grab the connection.
        let connection = match &self.override_connection_name {
            Some(name) => default_config.connections.get(name).ok_or(error::ClientError::ConnectionNotFound(name.to_string()))?.clone(),
            None => default_config.get_default_connection().ok_or(error::ConfigError::NoDefaultConnection)?.connection.clone()
        };

        let token = connection.get_token().ok_or(error::ClientError::NoToken)?;

        let client = LogshClient::new(connection.server.as_ref(), token);
        return arg.execute(&client)
    }

    pub fn execute_func<T>(&self, func: &dyn Fn(&LogshClient) -> Result<T, error::ClientError>) -> Result<T, error::ClientError> {
        let result = self.execute(&ExecuteWrapper { func })?;
        Ok(result)
    }
}

struct ExecuteWrapper<'a, T> {
    func : &'a dyn Fn(&LogshClient) -> Result<T, error::ClientError>
}

impl <'a, T> LogshClientHandlerExecute<T> for ExecuteWrapper<'a, T> {
    fn execute(&self, client : &LogshClient) -> Result<T, error::ClientError> {
        let result = (self.func)(client)?;
        Ok(result)
    }
}