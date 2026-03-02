use std::sync::mpsc;
use std::thread;

use std::collections::HashMap;

use crate::app::{CellValue, QueryResults, SavedQuery};

struct ContextInfo {
    cfg: logsh_core::config::Configuration,
    name: String,
    connection: logsh_core::connect::Connection,
}

fn load_context() -> Result<ContextInfo, String> {
    let cfg = logsh_core::config::load().map_err(|e| format!("Config error: {e}"))?;
    let ctx = cfg
        .get_current_context()
        .ok_or_else(|| "No context configured.".to_string())?;
    Ok(ContextInfo {
        name: ctx.name.clone(),
        connection: ctx.connection,
        cfg,
    })
}

struct AuthenticatedContext {
    client: logsh_core::logship_client::LogshClient,
    account_id: uuid::Uuid,
}

fn authenticated_client() -> Result<AuthenticatedContext, String> {
    let ctx = load_context()?;
    let account_id = ctx
        .connection
        .effective_account()
        .ok_or_else(|| "No account selected.".to_string())?;
    let token = ctx
        .connection
        .get_token()
        .ok_or_else(|| "Not authenticated.".to_string())?;
    let client = logsh_core::logship_client::LogshClient::new(&ctx.connection.server, token);
    Ok(AuthenticatedContext { client, account_id })
}

/// Requests sent from the UI thread to the backend thread.
#[derive(Debug)]
pub enum BackendRequest {
    ExecuteQuery(String),
    ParseQuery(String),
    LoadSchemas,
    LoadAccounts,
    SelectAccount {
        account_id: uuid::Uuid,
        account_name: String,
    },
    SwitchContext(String),
    LoadSavedQueries,
    SaveQuery {
        name: String,
        query: String,
    },
    DeleteSavedQuery(uuid::Uuid),
    Shutdown,
}

/// Responses sent from the backend thread to the UI thread.
#[derive(Debug)]
pub enum BackendResponse {
    QueryResult(QueryResults),
    QueryError(String),
    ParseResult(logsh_core::query::ParseResult),
    ParseError(String),
    Schemas {
        tables: Vec<String>,
        columns: HashMap<String, Vec<String>>,
    },
    SchemaError(String),
    Accounts(Vec<logsh_core::account::AccountsModel>),
    AccountsError(String),
    AccountSelected,
    ContextSwitched,
    ContextError(String),
    SavedQueries(Vec<SavedQuery>),
    SavedQueriesError(String),
    QuerySaved,
    QuerySaveError(String),
    QueryDeleted,
    QueryDeleteError(String),
}

/// Spawns the backend thread that processes requests using logsh-core's blocking APIs.
pub fn spawn_backend(
    rx: mpsc::Receiver<BackendRequest>,
    tx: mpsc::Sender<BackendResponse>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        while let Ok(req) = rx.recv() {
            match req {
                BackendRequest::Shutdown => break,
                BackendRequest::ExecuteQuery(query) => {
                    let response = execute_query(&query);
                    let _ = tx.send(response);
                }
                BackendRequest::ParseQuery(query) => {
                    let response = parse_query(&query);
                    let _ = tx.send(response);
                }
                BackendRequest::LoadSchemas => {
                    let response = load_schemas();
                    let _ = tx.send(response);
                }
                BackendRequest::LoadAccounts => {
                    let response = load_accounts();
                    let _ = tx.send(response);
                }
                BackendRequest::SelectAccount {
                    account_id,
                    account_name,
                } => {
                    let response = select_account(account_id, &account_name);
                    let _ = tx.send(response);
                }
                BackendRequest::SwitchContext(name) => {
                    let response = switch_context(&name);
                    let _ = tx.send(response);
                }
                BackendRequest::LoadSavedQueries => {
                    let response = load_saved_queries();
                    let _ = tx.send(response);
                }
                BackendRequest::SaveQuery { name, query } => {
                    let response = save_query(&name, &query);
                    let _ = tx.send(response);
                }
                BackendRequest::DeleteSavedQuery(query_id) => {
                    let response = delete_saved_query(query_id);
                    let _ = tx.send(response);
                }
            }
        }
    })
}

fn execute_query(query: &str) -> BackendResponse {
    let ctx = match load_context() {
        Ok(c) => c,
        Err(e) => return BackendResponse::QueryError(e),
    };

    let mut cfg = ctx.cfg;
    let mut conn = ctx.connection;

    if conn.default_account().is_none() {
        match conn.accounts(conn.user_id) {
            Ok(accounts) => {
                if let Some(first) = accounts.first() {
                    conn.default_account = Some(first.account_id);
                    conn.default_account_name = Some(first.account_name.clone());
                    conn.known_accounts = accounts.iter().map(|a| a.account_name.clone()).collect();
                    cfg.contexts.insert(ctx.name.clone(), conn.clone());
                    let _ = logsh_core::config::save(cfg);
                }
            }
            Err(e) => {
                return BackendResponse::QueryError(format!("Failed to fetch accounts: {e}"));
            }
        }
    }

    match conn.query_raw(query, None) {
        Ok(json) => {
            let results = parse_query_results(&json);
            BackendResponse::QueryResult(results)
        }
        Err(e) => BackendResponse::QueryError(format!("{e}")),
    }
}

fn parse_query(query: &str) -> BackendResponse {
    let ctx = match load_context() {
        Ok(c) => c,
        Err(e) => return BackendResponse::ParseError(e),
    };

    match ctx.connection.query_parse(query) {
        Ok(json) => match serde_json::from_str::<logsh_core::query::ParseResult>(&json) {
            Ok(result) => BackendResponse::ParseResult(result),
            Err(e) => BackendResponse::ParseError(format!("Failed to parse response: {e}")),
        },
        Err(e) => BackendResponse::ParseError(format!("{e}")),
    }
}

fn load_accounts() -> BackendResponse {
    let ctx = match load_context() {
        Ok(c) => c,
        Err(e) => return BackendResponse::AccountsError(e),
    };

    match ctx.connection.accounts(ctx.connection.user_id) {
        Ok(mut accounts) => {
            accounts.sort_by(|a, b| a.account_name.cmp(&b.account_name));
            BackendResponse::Accounts(accounts)
        }
        Err(e) => BackendResponse::AccountsError(format!("Failed to fetch accounts: {e}")),
    }
}

fn select_account(account_id: uuid::Uuid, account_name: &str) -> BackendResponse {
    let ctx = match load_context() {
        Ok(c) => c,
        Err(e) => return BackendResponse::ContextError(e),
    };

    let mut cfg = ctx.cfg;
    let mut conn = ctx.connection;
    conn.default_account = Some(account_id);
    conn.default_account_name = Some(account_name.to_string());
    cfg.contexts.insert(ctx.name, conn);

    match logsh_core::config::save(cfg) {
        Ok(_) => BackendResponse::AccountSelected,
        Err(e) => BackendResponse::ContextError(format!("Failed to save config: {e}")),
    }
}

fn load_schemas() -> BackendResponse {
    match execute_query("$metadata.schema.tables.fields") {
        BackendResponse::QueryResult(results) => {
            let table_col = results
                .columns
                .iter()
                .position(|c| c == "TableName")
                .unwrap_or(0);
            let col_col = results
                .columns
                .iter()
                .position(|c| c == "ColumnName")
                .unwrap_or(1);

            let mut table_columns: HashMap<String, Vec<String>> = HashMap::new();
            for row in &results.rows {
                let table_name = row
                    .get(table_col)
                    .map(|c| c.to_string())
                    .unwrap_or_default();
                let col_name = row.get(col_col).map(|c| c.to_string()).unwrap_or_default();
                if !table_name.is_empty()
                    && table_name != "null"
                    && !col_name.is_empty()
                    && col_name != "null"
                {
                    table_columns.entry(table_name).or_default().push(col_name);
                }
            }

            let mut names: Vec<String> = table_columns.keys().cloned().collect();
            names.sort();
            if names.is_empty() {
                BackendResponse::Schemas {
                    tables: vec!["(no tables found)".to_string()],
                    columns: HashMap::new(),
                }
            } else {
                BackendResponse::Schemas {
                    tables: names,
                    columns: table_columns,
                }
            }
        }
        BackendResponse::QueryError(e) => BackendResponse::SchemaError(e),
        _ => BackendResponse::SchemaError("Unexpected response".to_string()),
    }
}

fn switch_context(name: &str) -> BackendResponse {
    let mut cfg = match logsh_core::config::load() {
        Ok(c) => c,
        Err(e) => return BackendResponse::ContextError(format!("Config error: {e}")),
    };

    if !cfg.contexts.contains_key(name) {
        return BackendResponse::ContextError(format!("Context \"{name}\" not found."));
    }

    cfg.current_context = name.to_string();
    match logsh_core::config::save(cfg) {
        Ok(_) => BackendResponse::ContextSwitched,
        Err(e) => BackendResponse::ContextError(format!("Failed to save config: {e}")),
    }
}

fn load_saved_queries() -> BackendResponse {
    let auth = match authenticated_client() {
        Ok(a) => a,
        Err(e) => return BackendResponse::SavedQueriesError(e),
    };

    let url = format!("search/{}/saved-queries", auth.account_id);

    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct SavedQueryResponse {
        query_id: uuid::Uuid,
        name: String,
        query: String,
    }

    match auth.client.get_json::<Vec<SavedQueryResponse>>(&url) {
        Ok(queries) => {
            let saved: Vec<SavedQuery> = queries
                .into_iter()
                .map(|q| SavedQuery {
                    query_id: q.query_id,
                    name: q.name,
                    query: q.query,
                })
                .collect();
            BackendResponse::SavedQueries(saved)
        }
        Err(e) => BackendResponse::SavedQueriesError(format!("Failed to load saved queries: {e}")),
    }
}

fn save_query(name: &str, query: &str) -> BackendResponse {
    let auth = match authenticated_client() {
        Ok(a) => a,
        Err(e) => return BackendResponse::QuerySaveError(e),
    };

    let url = format!("search/{}/saved-queries", auth.account_id);

    #[derive(serde::Serialize)]
    #[serde(rename_all = "camelCase")]
    struct SaveQueryRequest<'a> {
        name: &'a str,
        query: &'a str,
    }

    #[derive(serde::Deserialize)]
    struct SaveQueryResponse {}

    match auth
        .client
        .post_json::<_, SaveQueryResponse>(&url, &SaveQueryRequest { name, query })
    {
        Ok(_) => BackendResponse::QuerySaved,
        Err(e) => BackendResponse::QuerySaveError(format!("{e}")),
    }
}

fn delete_saved_query(query_id: uuid::Uuid) -> BackendResponse {
    let auth = match authenticated_client() {
        Ok(a) => a,
        Err(e) => return BackendResponse::QueryDeleteError(e),
    };

    let url = format!("search/{}/saved-queries/{query_id}", auth.account_id);

    match auth.client.delete(&url) {
        Ok(_) => BackendResponse::QueryDeleted,
        Err(e) => BackendResponse::QueryDeleteError(format!("{e}")),
    }
}

fn parse_query_results(json: &str) -> QueryResults {
    let parsed = match logsh_core::query::result(json) {
        Ok(r) => r,
        Err(_) => {
            return QueryResults {
                columns: vec!["Result".to_string()],
                rows: vec![vec![CellValue::String(json.to_string())]],
            }
        }
    };

    let columns = parsed.header;
    let rows: Vec<Vec<CellValue>> = parsed
        .results
        .iter()
        .map(|row| {
            columns
                .iter()
                .map(|col| match row.get(col.as_str()) {
                    None => CellValue::Null,
                    Some(raw) => raw_value_to_cell(raw),
                })
                .collect()
        })
        .collect();

    QueryResults { columns, rows }
}

fn raw_value_to_cell(raw: &serde_json::value::RawValue) -> CellValue {
    let s = raw.get();
    if s == "null" {
        CellValue::Null
    } else if s == "true" {
        CellValue::Bool(true)
    } else if s == "false" {
        CellValue::Bool(false)
    } else if s.starts_with('"') {
        // Strip surrounding quotes
        let inner = &s[1..s.len().saturating_sub(1)];
        CellValue::String(inner.to_string())
    } else {
        // Numeric or other
        CellValue::Number(s.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_query_results() {
        let json = r#"{"header":["name","age"],"results":[{"name":"Alice","age":30},{"name":"Bob","age":null}]}"#;
        let results = parse_query_results(json);
        assert_eq!(results.columns, vec!["name", "age"]);
        assert_eq!(results.rows.len(), 2);
        assert_eq!(results.rows[0][0].to_string(), "Alice");
        assert_eq!(results.rows[0][1].to_string(), "30");
        assert_eq!(results.rows[1][1].to_string(), "null");
    }

    #[test]
    fn test_parse_empty_json() {
        let json = r#"{"header":[],"results":[]}"#;
        let results = parse_query_results(json);
        assert!(results.columns.is_empty());
        assert!(results.rows.is_empty());
    }

    #[test]
    fn test_raw_value_to_cell() {
        let raw = serde_json::value::RawValue::from_string("null".to_string()).unwrap();
        assert_eq!(raw_value_to_cell(&raw).to_string(), "null");

        let raw = serde_json::value::RawValue::from_string("\"hi\"".to_string()).unwrap();
        assert_eq!(raw_value_to_cell(&raw).to_string(), "hi");

        let raw = serde_json::value::RawValue::from_string("42".to_string()).unwrap();
        assert_eq!(raw_value_to_cell(&raw).to_string(), "42");

        let raw = serde_json::value::RawValue::from_string("true".to_string()).unwrap();
        assert_eq!(raw_value_to_cell(&raw).to_string(), "true");
    }
}
