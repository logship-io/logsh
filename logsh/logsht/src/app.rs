use std::collections::HashMap;
use std::sync::mpsc;

use tui_textarea::TextArea;

use crate::backend::{BackendRequest, BackendResponse};

/// Which modal overlay is active, if any.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Overlay {
    ContextSwitcher,
    AccountPicker,
    SavedQueries,
    Help,
    CellDetail,
    RowDetail,
}

/// Which pane has focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Schemas,
    Editor,
    Results,
    CommandBar,
}

/// A single cell value extracted from query results.
#[derive(Debug, Clone)]
pub enum CellValue {
    Null,
    String(String),
    Number(String),
    Bool(bool),
}

impl std::fmt::Display for CellValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CellValue::Null => write!(f, "null"),
            CellValue::String(s) => write!(f, "{s}"),
            CellValue::Number(n) => write!(f, "{n}"),
            CellValue::Bool(b) => write!(f, "{b}"),
        }
    }
}

/// Owned query results suitable for holding across frames.
#[derive(Debug, Clone, Default)]
pub struct QueryResults {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<CellValue>>,
}

/// A saved query loaded from the server.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SavedQuery {
    pub query_id: uuid::Uuid,
    pub name: String,
    pub query: String,
}

/// A previously executed query stored in local history.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HistoryEntry {
    pub query: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// A command that can be run from the command bar.
#[derive(Debug, Clone)]
pub struct Command {
    pub name: &'static str,
    pub description: &'static str,
}

pub const COMMANDS: &[Command] = &[
    Command {
        name: "quit",
        description: "Exit logsht",
    },
    Command {
        name: "refresh",
        description: "Reload schemas from server",
    },
    Command {
        name: "clear",
        description: "Clear query and results",
    },
    Command {
        name: "ctx",
        description: "Open context switcher",
    },
    Command {
        name: "help",
        description: "Show keybindings",
    },
    Command {
        name: "account",
        description: "Open account picker",
    },
    Command {
        name: "saved",
        description: "Open saved queries",
    },
    Command {
        name: "save",
        description: "Save current query",
    },
    Command {
        name: "cell",
        description: "Fullscreen focused cell value",
    },
    Command {
        name: "row",
        description: "Expand focused row as key-value pairs",
    },
    Command {
        name: "copy cell",
        description: "Copy focused cell to clipboard",
    },
    Command {
        name: "copy row",
        description: "Copy focused row as JSON",
    },
    Command {
        name: "copy json",
        description: "Copy all results as JSON",
    },
];

/// Top-level application state.
pub struct App {
    pub running: bool,
    pub overlay: Option<Overlay>,
    pub focus: Focus,

    pub editor: TextArea<'static>,

    pub results: Option<QueryResults>,
    pub results_cursor: usize,
    pub results_scroll: usize,
    pub results_col: usize,
    pub query_running: bool,
    pub error_message: Option<String>,

    pub contexts: Vec<ContextEntry>,
    pub context_selected: usize,
    pub current_context: Option<String>,
    pub context_filter: String,

    pub schemas: Vec<String>,
    pub schema_columns: HashMap<String, Vec<String>>,
    pub schema_selected: usize,
    pub schemas_loading: bool,
    pub schema_nav_visible: bool,
    pub schema_filter: String,

    pub accounts: Vec<AccountEntry>,
    pub account_selected: usize,
    pub current_account: Option<String>,
    pub account_filter: String,

    pub saved_queries: Vec<SavedQuery>,
    pub saved_query_selected: usize,
    pub saved_queries_loading: bool,
    pub saved_query_filter: String,
    pub _save_query_name: Option<String>,

    pub history: Vec<HistoryEntry>,
    pub history_pos: Option<usize>,

    pub command_input: String,
    pub command_cursor: usize,
    pub pre_command_focus: Focus,

    pub parse_result: Option<logsh_core::query::ParseResult>,
    pub parse_error: Option<String>,
    pub parse_dirty: bool,
    pub parse_in_flight: bool,
    pub last_edit_tick: u64,
    pub tick_count: u64,

    pub status_message: Option<String>,
    pub backend_tx: mpsc::Sender<BackendRequest>,
}

#[derive(Debug, Clone)]
pub struct ContextEntry {
    pub name: String,
    pub server: String,
    pub is_current: bool,
}

#[derive(Debug, Clone)]
pub struct AccountEntry {
    pub id: uuid::Uuid,
    pub name: String,
    pub is_current: bool,
}

impl App {
    pub fn new(backend_tx: mpsc::Sender<BackendRequest>) -> Self {
        let editor = TextArea::default();
        Self {
            running: true,
            overlay: None,
            focus: Focus::Schemas,
            editor,
            results: None,
            results_cursor: 0,
            results_scroll: 0,
            results_col: 0,
            query_running: false,
            error_message: None,
            contexts: Vec::new(),
            context_selected: 0,
            current_context: None,
            context_filter: String::new(),
            schemas: Vec::new(),
            schema_columns: HashMap::new(),
            schema_selected: 0,
            schemas_loading: false,
            schema_nav_visible: true,
            schema_filter: String::new(),
            accounts: Vec::new(),
            account_selected: 0,
            current_account: None,
            account_filter: String::new(),
            saved_queries: Vec::new(),
            saved_query_selected: 0,
            saved_queries_loading: false,
            saved_query_filter: String::new(),
            _save_query_name: None,
            history: Vec::new(),
            history_pos: None,
            command_input: String::new(),
            command_cursor: 0,
            pre_command_focus: Focus::Editor,
            parse_result: None,
            parse_error: None,
            parse_dirty: false,
            parse_in_flight: false,
            last_edit_tick: 0,
            tick_count: 0,
            status_message: None,
            backend_tx,
        }
    }

    /// Called on startup: loads contexts, history, and if none exist or no account is set,
    /// shows the appropriate picker overlay.
    pub fn init(&mut self) {
        self.load_contexts();
        self.load_history();

        if self.current_context.is_none() {
            if self.contexts.is_empty() {
                self.error_message =
                    Some("No contexts configured. Run `logsh ctx add <server>` first.".to_string());
            } else {
                self.status_message =
                    Some("Welcome to logsht. Select a context to begin.".to_string());
                self.overlay = Some(Overlay::ContextSwitcher);
            }
            return;
        }

        if let Ok(cfg) = logsh_core::config::load() {
            if let Some(ctx) = cfg.get_current_context() {
                self.current_account = ctx.connection.default_account_name.clone();
                if ctx.connection.default_account().is_none() {
                    self.status_message = Some("Select an account to continue.".to_string());
                    // Need to fetch accounts and let user pick
                    let _ = self.backend_tx.send(BackendRequest::LoadAccounts);
                } else {
                    self.load_schemas();
                }
            }
        }
    }

    /// Called on each tick (~50ms) to handle debounced parse requests.
    pub fn tick(&mut self) {
        self.tick_count += 1;

        let query_text = self.editor_text();

        // Debounce: fire parse request if dirty and enough ticks have passed (~750ms = 15 ticks)
        if self.parse_dirty
            && !self.parse_in_flight
            && self.tick_count.saturating_sub(self.last_edit_tick) > 15
            && !query_text.trim().is_empty()
        {
            self.parse_dirty = false;
            self.parse_in_flight = true;
            let _ = self.backend_tx.send(BackendRequest::ParseQuery(query_text));
        }
    }

    /// Get editor content as a single string.
    pub fn editor_text(&self) -> String {
        self.editor.lines().join("\n")
    }

    /// Get the focused cell's column name and value (for cell detail overlay).
    pub fn focused_cell(&self) -> Option<(&str, &CellValue)> {
        let results = self.results.as_ref()?;
        let col = self
            .results_col
            .min(results.columns.len().saturating_sub(1));
        let col_name = results.columns.get(col)?;
        let row = results.rows.get(self.results_cursor)?;
        let cell = row.get(col)?;
        Some((col_name.as_str(), cell))
    }

    /// Get the focused row as column-value pairs (for row detail overlay).
    pub fn focused_row(&self) -> Option<Vec<(&str, &CellValue)>> {
        let results = self.results.as_ref()?;
        let row = results.rows.get(self.results_cursor)?;
        Some(
            results
                .columns
                .iter()
                .zip(row.iter())
                .map(|(c, v)| (c.as_str(), v))
                .collect(),
        )
    }

    /// Get the focused row as a JSON object string.
    pub fn focused_row_json(&self) -> Option<String> {
        let results = self.results.as_ref()?;
        let row = results.rows.get(self.results_cursor)?;
        let mut map = serde_json::Map::new();
        for (col, val) in results.columns.iter().zip(row.iter()) {
            let jval = match val {
                CellValue::Null => serde_json::Value::Null,
                CellValue::String(s) => serde_json::Value::String(s.clone()),
                CellValue::Number(n) => n
                    .parse::<f64>()
                    .map(|f| serde_json::json!(f))
                    .unwrap_or(serde_json::Value::String(n.clone())),
                CellValue::Bool(b) => serde_json::Value::Bool(*b),
            };
            map.insert(col.clone(), jval);
        }
        serde_json::to_string_pretty(&serde_json::Value::Object(map)).ok()
    }

    /// Get all results as a JSON array string.
    pub fn results_json(&self) -> Option<String> {
        let results = self.results.as_ref()?;
        let arr: Vec<serde_json::Value> = results
            .rows
            .iter()
            .map(|row| {
                let mut map = serde_json::Map::new();
                for (col, val) in results.columns.iter().zip(row.iter()) {
                    let jval = match val {
                        CellValue::Null => serde_json::Value::Null,
                        CellValue::String(s) => serde_json::Value::String(s.clone()),
                        CellValue::Number(n) => n
                            .parse::<f64>()
                            .map(|f| serde_json::json!(f))
                            .unwrap_or(serde_json::Value::String(n.clone())),
                        CellValue::Bool(b) => serde_json::Value::Bool(*b),
                    };
                    map.insert(col.clone(), jval);
                }
                serde_json::Value::Object(map)
            })
            .collect();
        serde_json::to_string_pretty(&arr).ok()
    }

    /// Set editor content from a string.
    pub fn set_editor_text(&mut self, text: &str) {
        let lines: Vec<String> = text.lines().map(|l| l.to_string()).collect();
        let lines = if lines.is_empty() {
            vec![String::new()]
        } else {
            lines
        };
        self.editor = TextArea::new(lines);
        // Move cursor to end
        self.editor.move_cursor(tui_textarea::CursorMove::Bottom);
        self.editor.move_cursor(tui_textarea::CursorMove::End);
    }

    /// Mark the editor content as changed, resetting the debounce timer.
    pub fn mark_editor_dirty(&mut self) {
        self.parse_dirty = true;
        self.last_edit_tick = self.tick_count;
        let text = self.editor_text();
        if text.trim().is_empty() {
            self.parse_result = None;
            self.parse_error = None;
            self.parse_dirty = false;
        }
    }

    /// Copy text to clipboard using OSC 52 escape sequence (works in most terminals).
    pub fn copy_to_clipboard(&mut self, text: &str) {
        use std::io::Write;
        let b64 = base64_encode(text.as_bytes());
        // OSC 52: \x1b]52;c;<base64>\x07
        let seq = format!("\x1b]52;c;{b64}\x07");
        let _ = std::io::stdout().write_all(seq.as_bytes());
        let _ = std::io::stdout().flush();
        self.error_message = None;
        self.status_message = Some("Copied to clipboard".to_string());
    }

    pub fn select_account(&mut self, id: uuid::Uuid, name: &str) {
        let _ = self.backend_tx.send(BackendRequest::SelectAccount {
            account_id: id,
            account_name: name.to_string(),
        });
    }

    pub fn load_contexts(&mut self) {
        if let Ok(cfg) = logsh_core::config::load() {
            self.current_context = if cfg.current_context.is_empty() {
                None
            } else {
                Some(cfg.current_context.clone())
            };
            let mut entries: Vec<_> = cfg
                .contexts
                .iter()
                .map(|(name, conn)| ContextEntry {
                    name: name.clone(),
                    server: conn.server.clone(),
                    is_current: *name == cfg.current_context,
                })
                .collect();
            entries.sort_by(|a, b| a.name.cmp(&b.name));
            self.contexts = entries;
        }
    }

    pub fn execute_query(&mut self) {
        let query = self.editor_text().trim().to_string();
        if query.is_empty() {
            return;
        }

        self.add_to_history(&query);
        self.history_pos = None;

        self.query_running = true;
        self.error_message = None;
        let _ = self.backend_tx.send(BackendRequest::ExecuteQuery(query));
    }

    pub fn load_schemas(&mut self) {
        self.schemas_loading = true;
        self.schemas.clear();
        let _ = self.backend_tx.send(BackendRequest::LoadSchemas);
    }

    pub fn load_saved_queries(&mut self) {
        self.saved_queries_loading = true;
        let _ = self.backend_tx.send(BackendRequest::LoadSavedQueries);
    }

    pub fn save_current_query(&mut self, name: &str) {
        let query = self.editor_text().trim().to_string();
        if query.is_empty() {
            self.error_message = Some("Cannot save empty query.".to_string());
            return;
        }
        let _ = self.backend_tx.send(BackendRequest::SaveQuery {
            name: name.to_string(),
            query,
        });
    }

    pub fn delete_saved_query(&mut self, query_id: uuid::Uuid) {
        let _ = self
            .backend_tx
            .send(BackendRequest::DeleteSavedQuery(query_id));
    }

    pub fn switch_context(&mut self, name: &str) {
        let _ = self
            .backend_tx
            .send(BackendRequest::SwitchContext(name.to_string()));
    }

    pub fn open_command_bar(&mut self) {
        self.command_input.clear();
        self.command_cursor = 0;
        self.pre_command_focus = self.focus;
        self.focus = Focus::CommandBar;
    }

    pub fn execute_command(&mut self) {
        let cmd = self.command_input.trim().to_lowercase();
        self.focus = self.pre_command_focus;
        match cmd.as_str() {
            "q" | "quit" | "exit" => self.running = false,
            "refresh" | "r" => self.load_schemas(),
            "clear" | "c" => {
                self.set_editor_text("");
                self.results = None;
                self.error_message = None;
            }
            "ctx" | "contexts" => {
                self.load_contexts();
                self.context_filter.clear();
                self.overlay = Some(Overlay::ContextSwitcher);
            }
            "help" | "h" | "?" => {
                self.overlay = Some(Overlay::Help);
            }
            "account" | "acct" | "a" => {
                self.account_filter.clear();
                let _ = self.backend_tx.send(BackendRequest::LoadAccounts);
            }
            "schemas" | "s" => {
                self.schema_nav_visible = !self.schema_nav_visible;
            }
            "saved" | "sq" => {
                self.saved_query_filter.clear();
                self.load_saved_queries();
                self.overlay = Some(Overlay::SavedQueries);
            }
            s if s.starts_with("save ") => {
                let name = s.strip_prefix("save ").unwrap_or("").trim();
                if !name.is_empty() {
                    self.save_current_query(name);
                } else {
                    self.error_message = Some("Usage: :save <name>".to_string());
                }
            }
            "save" => {
                self.error_message = Some("Usage: :save <name>".to_string());
            }
            "cell" => {
                if self.focused_cell().is_some() {
                    self.overlay = Some(Overlay::CellDetail);
                } else {
                    self.error_message = Some("No results to inspect".to_string());
                }
            }
            "row" | "expand" => {
                if self.focused_row().is_some() {
                    self.overlay = Some(Overlay::RowDetail);
                } else {
                    self.error_message = Some("No results to inspect".to_string());
                }
            }
            "copy cell" | "yy" => {
                if let Some((_, cell)) = self.focused_cell() {
                    let text = cell.to_string();
                    self.copy_to_clipboard(&text);
                } else {
                    self.error_message = Some("No cell to copy".to_string());
                }
            }
            "copy row" => {
                if let Some(json) = self.focused_row_json() {
                    self.copy_to_clipboard(&json);
                } else {
                    self.error_message = Some("No row to copy".to_string());
                }
            }
            "copy json" | "copy results" => {
                if let Some(json) = self.results_json() {
                    self.copy_to_clipboard(&json);
                } else {
                    self.error_message = Some("No results to copy".to_string());
                }
            }
            _ => {
                self.error_message = Some(format!("Unknown command: {cmd}"));
            }
        }
        self.command_input.clear();
        self.command_cursor = 0;
    }

    /// Returns filtered commands matching current command bar input.
    pub fn matching_commands(&self) -> Vec<&Command> {
        if self.command_input.is_empty() {
            return COMMANDS.iter().collect();
        }
        let input = self.command_input.to_lowercase();
        COMMANDS
            .iter()
            .filter(|c| c.name.starts_with(&input))
            .collect()
    }

    /// Filter schemas by the current filter string.
    pub fn filtered_schemas(&self) -> Vec<(usize, &String)> {
        if self.schema_filter.is_empty() {
            self.schemas.iter().enumerate().collect()
        } else {
            let filter = self.schema_filter.to_lowercase();
            self.schemas
                .iter()
                .enumerate()
                .filter(|(_, s)| s.to_lowercase().contains(&filter))
                .collect()
        }
    }

    /// Filter contexts by the current filter string.
    pub fn filtered_contexts(&self) -> Vec<(usize, &ContextEntry)> {
        if self.context_filter.is_empty() {
            self.contexts.iter().enumerate().collect()
        } else {
            let filter = self.context_filter.to_lowercase();
            self.contexts
                .iter()
                .enumerate()
                .filter(|(_, c)| {
                    c.name.to_lowercase().contains(&filter)
                        || c.server.to_lowercase().contains(&filter)
                })
                .collect()
        }
    }

    /// Filter accounts by the current filter string.
    pub fn filtered_accounts(&self) -> Vec<(usize, &AccountEntry)> {
        if self.account_filter.is_empty() {
            self.accounts.iter().enumerate().collect()
        } else {
            let filter = self.account_filter.to_lowercase();
            self.accounts
                .iter()
                .enumerate()
                .filter(|(_, a)| a.name.to_lowercase().contains(&filter))
                .collect()
        }
    }

    /// Filter saved queries by the current filter string.
    pub fn filtered_saved_queries(&self) -> Vec<(usize, &SavedQuery)> {
        if self.saved_query_filter.is_empty() {
            self.saved_queries.iter().enumerate().collect()
        } else {
            let filter = self.saved_query_filter.to_lowercase();
            self.saved_queries
                .iter()
                .enumerate()
                .filter(|(_, q)| {
                    q.name.to_lowercase().contains(&filter)
                        || q.query.to_lowercase().contains(&filter)
                })
                .collect()
        }
    }

    // Query history management
    fn history_path() -> Option<std::path::PathBuf> {
        home::home_dir().map(|mut h| {
            h.push(".logsh");
            h.push("query-history.json");
            h
        })
    }

    fn load_history(&mut self) {
        if let Some(path) = Self::history_path() {
            if path.exists() {
                if let Ok(data) = std::fs::read_to_string(&path) {
                    if let Ok(entries) = serde_json::from_str::<Vec<HistoryEntry>>(&data) {
                        self.history = entries;
                    }
                }
            }
        }
    }

    fn save_history(&self) {
        if let Some(path) = Self::history_path() {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            // Keep last 500 entries
            let entries: Vec<&HistoryEntry> = self.history.iter().rev().take(500).collect();
            let entries: Vec<&HistoryEntry> = entries.into_iter().rev().collect();
            if let Ok(json) = serde_json::to_string_pretty(&entries) {
                let _ = std::fs::write(&path, json);
            }
        }
    }

    pub fn add_to_history(&mut self, query: &str) {
        // Don't add duplicates of the last entry
        if self.history.last().is_some_and(|h| h.query == query) {
            return;
        }
        self.history.push(HistoryEntry {
            query: query.to_string(),
            timestamp: chrono::Utc::now(),
        });
        self.save_history();
    }

    pub fn history_prev(&mut self) {
        if self.history.is_empty() {
            return;
        }
        let pos = match self.history_pos {
            None => self.history.len().saturating_sub(1),
            Some(p) => p.saturating_sub(1),
        };
        self.history_pos = Some(pos);
        let query = self.history.get(pos).map(|e| e.query.clone());
        if let Some(q) = query {
            self.set_editor_text(&q);
        }
    }

    pub fn history_next(&mut self) {
        if let Some(pos) = self.history_pos {
            let next = pos + 1;
            if next < self.history.len() {
                self.history_pos = Some(next);
                let query = self.history.get(next).map(|e| e.query.clone());
                if let Some(q) = query {
                    self.set_editor_text(&q);
                }
            } else {
                self.history_pos = None;
                self.set_editor_text("");
            }
        }
    }

    pub fn handle_backend_response(&mut self, response: BackendResponse) {
        match response {
            BackendResponse::QueryResult(results) => {
                self.query_running = false;
                self.results = Some(results);
                self.results_cursor = 0;
                self.results_scroll = 0;
                self.results_col = 0;
                self.error_message = None;
            }
            BackendResponse::QueryError(msg) => {
                self.query_running = false;
                self.error_message = Some(msg);
            }
            BackendResponse::ParseResult(result) => {
                self.parse_in_flight = false;
                self.parse_result = Some(result);
                self.parse_error = None;
            }
            BackendResponse::ParseError(msg) => {
                self.parse_in_flight = false;
                self.parse_error = Some(msg);
            }
            BackendResponse::Schemas { tables, columns } => {
                self.schemas_loading = false;
                self.schemas = tables;
                self.schema_columns = columns;
                // Clear the "Connected to..." message once schemas load
                self.status_message = None;
            }
            BackendResponse::SchemaError(msg) => {
                self.schemas_loading = false;
                self.error_message = Some(msg);
            }
            BackendResponse::ContextSwitched => {
                self.load_contexts();
                self.overlay = None;
                self.status_message = None;
                self.context_filter.clear();
                // Clear parse state for new context
                self.parse_result = None;
                self.parse_error = None;
                self.parse_dirty = false;
                // Re-check account for the new context
                if let Ok(cfg) = logsh_core::config::load() {
                    if let Some(ctx) = cfg.get_current_context() {
                        self.current_account = ctx.connection.default_account_name.clone();
                        if ctx.connection.default_account().is_none() {
                            self.status_message =
                                Some("Select an account to continue.".to_string());
                            let _ = self.backend_tx.send(BackendRequest::LoadAccounts);
                            return;
                        }
                    }
                }
                self.load_schemas();
            }
            BackendResponse::ContextError(msg) => {
                let ctx_name = self.current_context.as_deref().unwrap_or("unknown");
                if msg.contains("expired") || msg.contains("Expired") || msg.contains("401") {
                    self.error_message = Some(format!(
                        "Authentication expired for {ctx_name}. Re-run `logsh ctx add {ctx_name}` to refresh."
                    ));
                } else {
                    self.error_message = Some(msg);
                }
            }
            BackendResponse::Accounts(accounts) => {
                let current_id = logsh_core::config::load()
                    .ok()
                    .and_then(|cfg| cfg.get_current_context())
                    .and_then(|ctx| ctx.connection.default_account());

                self.accounts = accounts
                    .iter()
                    .map(|a| AccountEntry {
                        id: a.account_id,
                        name: a.account_name.clone(),
                        is_current: Some(a.account_id) == current_id,
                    })
                    .collect();
                self.account_selected = 0;

                if self.accounts.len() == 1 {
                    // Auto-select the only account
                    let a = &self.accounts[0];
                    self.select_account(a.id, &a.name.clone());
                } else if self.accounts.is_empty() {
                    self.error_message = Some("No accounts available.".to_string());
                } else {
                    self.account_filter.clear();
                    self.overlay = Some(Overlay::AccountPicker);
                }
            }
            BackendResponse::AccountsError(msg) => {
                self.error_message = Some(msg);
            }
            BackendResponse::AccountSelected => {
                self.overlay = None;
                self.status_message = None;
                self.account_filter.clear();
                // Refresh account name display
                if let Ok(cfg) = logsh_core::config::load() {
                    if let Some(ctx) = cfg.get_current_context() {
                        self.current_account = ctx.connection.default_account_name.clone();
                        let server = &ctx.connection.server;
                        let acct = self.current_account.as_deref().unwrap_or("unknown");
                        self.status_message = Some(format!("Connected to {server} as {acct}"));
                    }
                }
                self.load_schemas();
            }
            BackendResponse::SavedQueries(queries) => {
                self.saved_queries_loading = false;
                self.saved_queries = queries;
                self.saved_query_selected = 0;
            }
            BackendResponse::SavedQueriesError(msg) => {
                self.saved_queries_loading = false;
                self.error_message = Some(msg);
            }
            BackendResponse::QuerySaved => {
                self.status_message = Some("Query saved.".to_string());
                // Refresh saved queries if the overlay is open
                if self.overlay == Some(Overlay::SavedQueries) {
                    self.load_saved_queries();
                }
            }
            BackendResponse::QuerySaveError(msg) => {
                self.error_message = Some(format!("Failed to save query: {msg}"));
            }
            BackendResponse::QueryDeleted => {
                self.status_message = Some("Query deleted.".to_string());
                self.load_saved_queries();
            }
            BackendResponse::QueryDeleteError(msg) => {
                self.error_message = Some(format!("Failed to delete query: {msg}"));
            }
        }
    }
}

/// Simple base64 encode (no external dep needed for this).
fn base64_encode(input: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        result.push(CHARS[((triple >> 18) & 0x3F) as usize] as char);
        result.push(CHARS[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            result.push(CHARS[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
        if chunk.len() > 2 {
            result.push(CHARS[(triple & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
    }
    result
}
