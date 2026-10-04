use crate::app::{AccountEntry, App, Overlay};
use logsh_core::{
    account::AccountsModel,
    connect::Connection,
    session::{LoginMethod, LoginRequest},
};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc,
};

pub enum LoginEvent {
    Device { url: String, code: String },
    Complete(Result<(Connection, Vec<AccountsModel>), String>),
}

pub struct LoginForm {
    pub server: String,
    pub name: String,
    pub username: String,
    pub secret: String,
    pub method: LoginMethod,
    pub field: usize,
    pub busy: bool,
    pub message: String,
    pub cancel: Arc<AtomicBool>,
    pub receiver: Option<mpsc::Receiver<LoginEvent>>,
    pub pending: Option<Connection>,
}

impl Default for LoginForm {
    fn default() -> Self {
        Self {
            server: String::new(),
            name: String::new(),
            username: String::new(),
            secret: String::new(),
            method: LoginMethod::Password,
            field: 0,
            busy: false,
            message: String::new(),
            cancel: Arc::new(AtomicBool::new(false)),
            receiver: None,
            pending: None,
        }
    }
}

impl App {
    pub fn open_login(&mut self, add: bool) {
        self.open_named_login(add, None);
    }

    pub fn open_named_login(&mut self, add: bool, name: Option<&str>) {
        self.cancel_login();
        self.login = LoginForm::default();
        if !add {
            if let Ok(ctx) = logsh_core::config::load().and_then(|cfg| {
                if let Some(name) = name {
                    cfg.contexts
                        .get(name)
                        .map(|connection| logsh_core::config::ContextConfig {
                            name: name.to_owned(),
                            connection: connection.clone(),
                        })
                        .ok_or_else(|| {
                            logsh_core::error::ConfigError::ContextNotFound(name.to_owned())
                        })
                } else {
                    cfg.resolve_current_context()
                }
            }) {
                let oauth = ctx.connection.is_oauth_auth();
                self.login.server = ctx.connection.server;
                self.login.name = ctx.name;
                self.login.username = ctx.connection.username;
                self.login.method = match ctx.connection.auth_method.as_deref() {
                    Some("pat") => LoginMethod::Pat,
                    Some("sso") => LoginMethod::Sso,
                    _ if oauth => LoginMethod::Sso,
                    _ => LoginMethod::Password,
                };
                self.login.field = 4;
            }
        }
        self.error_message = None;
        self.overlay = Some(Overlay::Login);
    }

    pub fn cancel_login(&mut self) {
        self.invalidate_backend();
        self.login.cancel.store(true, Ordering::Relaxed);
        self.login.secret.clear();
        self.login.pending = None;
        self.login.receiver = None;
        self.login.busy = false;
    }

    pub fn submit_login(&mut self) {
        if self.login.busy {
            return;
        }
        if self.login.server.trim().is_empty() {
            self.login.message = "Enter a server URL.".into();
            return;
        }
        self.login.name = self.login.name.trim().to_owned();
        if self.login.name.trim().is_empty() {
            match logsh_core::reqwest::Url::parse(&self.login.server) {
                Ok(url) => self.login.name = url.host_str().unwrap_or("logship").to_owned(),
                Err(_) => {
                    self.login.message = "Use an http:// or https:// server URL.".into();
                    return;
                }
            }
        }
        let previous = match logsh_core::config::load() {
            Ok(cfg) => cfg.contexts.get(&self.login.name).cloned(),
            Err(e) => {
                self.login.message = e.to_string();
                return;
            }
        };
        let request = LoginRequest {
            server: self.login.server.trim().to_owned(),
            method: self.login.method,
            username: self.login.username.trim().to_owned(),
            secret: std::mem::take(&mut self.login.secret),
        };
        let (tx, rx) = mpsc::channel();
        self.login.receiver = Some(rx);
        self.login.cancel = Arc::new(AtomicBool::new(false));
        let cancelled = self.login.cancel.clone();
        self.login.busy = true;
        self.login.message = "Authenticating... Esc cancels.".into();
        std::thread::spawn(move || {
            let result = logsh_core::session::authenticate(
                request,
                previous,
                &|event| {
                    let logsh_core::auth::AuthEvent::DeviceCode { url, code } = event;
                    let _ = tx.send(LoginEvent::Device { url, code });
                },
                &cancelled,
            )
            .and_then(|connection| {
                let accounts = connection.accounts(connection.user_id)?;
                Ok((connection, accounts))
            })
            .map_err(|e| e.to_string());
            if !cancelled.load(Ordering::Relaxed) {
                let _ = tx.send(LoginEvent::Complete(result));
            }
        });
    }

    pub fn poll_login(&mut self) {
        let events: Vec<_> = self
            .login
            .receiver
            .as_ref()
            .map(|rx| rx.try_iter().collect())
            .unwrap_or_default();
        for event in events {
            match event {
                LoginEvent::Device { url, code } => {
                    self.login.message = format!(
                        "Open {url}\nEnter code: {code}\nWaiting for authorization. Esc cancels."
                    )
                }
                LoginEvent::Complete(Err(error)) => {
                    self.login.busy = false;
                    self.login.receiver = None;
                    self.login.message =
                        format!("{error}\nEnter credentials and press Enter to retry.");
                }
                LoginEvent::Complete(Ok((connection, mut accounts))) => {
                    self.login.busy = false;
                    self.login.receiver = None;
                    accounts.sort_by(|a, b| {
                        a.account_name
                            .cmp(&b.account_name)
                            .then(a.account_id.cmp(&b.account_id))
                    });
                    let needs_choice = connection.default_account.is_none() && !accounts.is_empty();
                    self.accounts = accounts
                        .into_iter()
                        .map(|a| AccountEntry {
                            id: a.account_id,
                            name: a.account_name,
                            is_current: Some(a.account_id) == connection.default_account,
                        })
                        .collect();
                    self.login.pending = Some(connection);
                    if needs_choice {
                        self.account_selected = 0;
                        self.account_filter.clear();
                        self.overlay = Some(Overlay::AccountPicker);
                        self.status_message =
                            Some("Select an account to finish login. Esc cancels.".into());
                    } else {
                        self.finish_login();
                    }
                }
            }
        }
    }

    pub fn finish_login(&mut self) {
        let Some(connection) = self.login.pending.as_ref() else {
            return;
        };
        let account = connection.default_account_name.clone();
        let has_account = connection.default_account.is_some();
        let result = logsh_core::config::load().and_then(|mut cfg| {
            cfg.current_context = self.login.name.clone();
            cfg.contexts
                .insert(self.login.name.clone(), connection.clone());
            logsh_core::config::save(cfg)
        });
        match result {
            Ok(_) => {
                self.invalidate_backend();
                self.login.pending = None;
                self.overlay = None;
                self.load_contexts();
                self.current_account = account;
                self.results = None;
                self.schemas.clear();
                self.schema_columns.clear();
                self.saved_queries.clear();
                self.parse_result = None;
                self.parse_error = None;
                self.parse_dirty = false;
                self.error_message = None;
                self.status_message = Some(
                    if has_account {
                        "Logged in."
                    } else {
                        "Logged in; no accessible accounts."
                    }
                    .into(),
                );
                if has_account {
                    self.load_schemas();
                    self.load_saved_queries();
                }
            }
            Err(e) => {
                self.error_message = Some(format!("Could not save login: {e}"));
                self.login.message = e.to_string();
                self.overlay = Some(Overlay::Login);
            }
        }
    }
}

pub fn draw(f: &mut ratatui::Frame, app: &App) {
    use ratatui::{
        style::{Color, Style},
        text::Line,
        widgets::{Block, Borders, Clear, Paragraph, Wrap},
    };
    let area = crate::views::centered_rect(85, 85, f.area());
    f.render_widget(Clear, area);
    let method = match app.login.method {
        LoginMethod::Password => "Password",
        LoginMethod::Pat => "Personal Access Token",
        LoginMethod::Sso => "SSO device login",
    };
    let fields = [
        format!("Server: {}", app.login.server),
        format!("Context name: {} (blank uses hostname)", app.login.name),
        format!("Authentication: {method} (Left/Right changes)"),
        format!("Username: {}", app.login.username),
        format!(
            "Password/token: {}",
            "*".repeat(app.login.secret.chars().count())
        ),
    ];
    let mut lines = vec![
        Line::from("Tab/Shift+Tab: field   Enter: login   Esc: cancel"),
        Line::from(""),
    ];
    for (index, field) in fields.into_iter().enumerate() {
        lines.push(Line::styled(
            format!(
                "{} {field}",
                if index == app.login.field { ">" } else { " " }
            ),
            Style::default().fg(if index == app.login.field {
                Color::Yellow
            } else {
                Color::White
            }),
        ));
    }
    lines.push(Line::from(""));
    for line in app.login.message.lines() {
        lines.push(Line::from(line.to_owned()));
    }
    f.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: false }).block(
            Block::default()
                .title(" Logship login ")
                .borders(Borders::ALL),
        ),
        area,
    );
}

pub fn handle_key(app: &mut App, key: crossterm::event::KeyEvent) {
    use crossterm::event::KeyCode;
    if app.login.busy {
        return;
    }
    match key.code {
        KeyCode::Tab => app.login.field = (app.login.field + 1) % 5,
        KeyCode::BackTab => app.login.field = (app.login.field + 4) % 5,
        KeyCode::Left | KeyCode::Right if app.login.field == 2 => {
            app.login.method = match app.login.method {
                LoginMethod::Password => LoginMethod::Pat,
                LoginMethod::Pat => LoginMethod::Sso,
                LoginMethod::Sso => LoginMethod::Password,
            };
            app.login.secret.clear();
        }
        KeyCode::Enter => app.submit_login(),
        _ => {
            let field = match app.login.field {
                0 => &mut app.login.server,
                1 => &mut app.login.name,
                3 => &mut app.login.username,
                4 => &mut app.login.secret,
                _ => return,
            };
            match key.code {
                KeyCode::Char(c) => field.push(c),
                KeyCode::Backspace => {
                    field.pop();
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::BackendResponse;
    fn app() -> App {
        let (tx, _) = mpsc::channel();
        App::new(tx)
    }

    #[test]
    fn successful_login_requires_selection_even_with_one_account() {
        for count in [1, 2] {
            let mut app = app();
            app.open_login(true);
            let (tx, rx) = mpsc::channel();
            app.login.receiver = Some(rx);
            app.login.busy = true;
            let accounts = (0..count)
                .map(|i| AccountsModel {
                    account_id: uuid::Uuid::from_u128(i + 1),
                    account_name: format!("account-{i}"),
                    permissions: vec![],
                })
                .collect();
            tx.send(LoginEvent::Complete(Ok((
                Connection::new("https://example.invalid"),
                accounts,
            ))))
            .unwrap();
            app.poll_login();
            assert_eq!(app.overlay, Some(Overlay::AccountPicker));
            assert!(app.login.pending.is_some());
            assert_eq!(app.login.pending.as_ref().unwrap().default_account, None);
        }
    }

    #[test]
    fn cancellation_drops_pending_connection_and_stale_login_events() {
        let mut app = app();
        app.open_login(true);
        let (tx, rx) = mpsc::channel();
        app.login.receiver = Some(rx);
        app.login.pending = Some(Connection::new("https://example.invalid"));
        app.login.secret = "secret".into();
        let cancel = app.login.cancel.clone();
        app.cancel_login();
        assert!(cancel.load(Ordering::Relaxed));
        assert!(app.login.pending.is_none());
        assert!(app.login.secret.is_empty());
        assert!(tx
            .send(LoginEvent::Device {
                url: "stale".into(),
                code: "stale".into()
            })
            .is_err());
    }

    #[test]
    fn stale_backend_results_cannot_replace_current_results() {
        let mut app = app();
        app.invalidate_backend();
        app.handle_backend_response(BackendResponse::ForSession {
            generation: 0,
            response: Box::new(BackendResponse::QueryResult(Default::default())),
        });
        assert!(app.results.is_none());
    }

    #[test]
    fn login_form_masks_secrets_and_escape_cancels() {
        let mut app = app();
        app.open_login(true);
        app.login.secret = "super-secret".into();
        let backend = ratatui::backend::TestBackend::new(80, 24);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal.draw(|f| draw(f, &app)).unwrap();
        let text: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(!text.contains("super-secret"));
        assert!(text.contains("************"));
        crate::input::handle_key(
            &mut app,
            crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Esc,
                crossterm::event::KeyModifiers::NONE,
            ),
        );
        assert!(app.overlay.is_none());
        assert!(app.login.secret.is_empty());
    }

    #[test]
    fn auth_failure_keeps_form_open_for_retry() {
        let mut app = app();
        app.open_login(true);
        let (tx, rx) = mpsc::channel();
        app.login.receiver = Some(rx);
        app.login.busy = true;
        tx.send(LoginEvent::Complete(Err("HTTP 401".into())))
            .unwrap();
        app.poll_login();
        assert!(!app.login.busy);
        assert_eq!(app.overlay, Some(Overlay::Login));
        assert!(app.login.message.contains("retry"));
    }
}
