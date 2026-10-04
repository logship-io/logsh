use crate::app::App;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
    Frame,
};

pub fn draw(f: &mut Frame, app: &App) {
    let area = super::centered_rect(85, 85, f.area());
    f.render_widget(Clear, area);
    let rows = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(3),
        Constraint::Length(1),
    ])
    .split(area);
    let filter = if app.command_input.is_empty() {
        "Search commands by name or description...".to_owned()
    } else {
        format!("Search: {}", app.command_input)
    };
    f.render_widget(
        Paragraph::new(filter).style(Style::default().fg(Color::Cyan)),
        rows[0],
    );
    let matches = app.palette_commands();
    let items: Vec<_> = if matches.is_empty() {
        vec![ListItem::new(
            "No matching commands. Backspace to change the search.",
        )]
    } else {
        matches
            .iter()
            .map(|command| ListItem::new(format!("{:<14} {}", command.name, command.description)))
            .collect()
    };
    let list = List::new(items)
        .block(Block::default().title(" Commands ").borders(Borders::ALL))
        .highlight_symbol("> ")
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );
    let mut state = ListState::default();
    if !matches.is_empty() {
        state.select(Some(app.command_selected.min(matches.len() - 1)));
    }
    f.render_stateful_widget(list, rows[1], &mut state);
    f.render_widget(
        Paragraph::new("Type to search | Up/Down select | Enter run | Esc close"),
        rows[2],
    );
}

pub fn handle_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Up => app.command_selected = app.command_selected.saturating_sub(1),
        KeyCode::Down => {
            app.command_selected =
                (app.command_selected + 1).min(app.palette_commands().len().saturating_sub(1))
        }
        KeyCode::Enter => app.run_selected_command(),
        KeyCode::Backspace => {
            app.command_input.pop();
            app.command_selected = 0;
        }
        KeyCode::Char(c)
            if !key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
        {
            app.command_input.push(c);
            app.command_selected = 0;
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        app::{Focus, Overlay},
        backend::BackendRequest,
        input,
    };
    fn app() -> (App, std::sync::mpsc::Receiver<BackendRequest>) {
        let (tx, rx) = std::sync::mpsc::channel();
        (App::new(tx), rx)
    }
    fn key(app: &mut App, code: KeyCode) {
        input::handle_key(app, KeyEvent::new(code, KeyModifiers::NONE));
    }

    #[test]
    fn account_shortcut_works_in_editor_without_changing_query() {
        let (mut app, rx) = app();
        app.focus = Focus::Editor;
        app.set_editor_text("Logs | take 10");
        key(&mut app, KeyCode::F(2));
        assert_eq!(app.overlay, Some(Overlay::AccountPicker));
        assert_eq!(app.editor_text(), "Logs | take 10");
        assert!(
            matches!(rx.try_recv().unwrap(),BackendRequest::ForSession{request,..} if matches!(*request,BackendRequest::LoadAccounts))
        );
    }

    #[test]
    fn palette_shows_all_commands_then_searches_and_runs_account() {
        let (mut app, rx) = app();
        app.focus = Focus::Editor;
        key(&mut app, KeyCode::F(1));
        assert_eq!(app.overlay, Some(Overlay::Commands));
        assert!(app.palette_commands().iter().any(|c| c.name == "account"));
        for c in "picker".chars() {
            key(&mut app, KeyCode::Char(c));
        }
        assert_eq!(app.palette_commands()[0].name, "account");
        key(&mut app, KeyCode::Enter);
        assert_eq!(app.overlay, Some(Overlay::AccountPicker));
        assert_eq!(app.focus, Focus::Editor);
        assert!(rx.try_recv().is_ok());
    }

    #[test]
    fn escape_and_empty_search_are_safe_and_save_prompts_for_name() {
        let (mut app, _) = app();
        app.focus = Focus::Editor;
        key(&mut app, KeyCode::F(1));
        app.command_input = "no-matching-command".into();
        key(&mut app, KeyCode::Enter);
        assert_eq!(app.overlay, Some(Overlay::Commands));
        key(&mut app, KeyCode::Esc);
        assert!(app.overlay.is_none());
        assert_eq!(app.focus, Focus::Editor);
        key(&mut app, KeyCode::F(1));
        app.command_input = "save".into();
        app.command_selected = app
            .palette_commands()
            .iter()
            .position(|c| c.name == "save")
            .unwrap();
        key(&mut app, KeyCode::Enter);
        assert_eq!(app.focus, Focus::CommandBar);
        assert_eq!(app.command_input, "save ");
        assert_eq!(app.pre_command_focus, Focus::Editor);
    }

    #[test]
    fn long_context_names_cannot_hide_controls() {
        let (mut app, _) = app();
        app.current_context = Some("very-long-context-name".repeat(5));
        app.current_account = Some("long-account-name".repeat(5));
        let backend = ratatui::backend::TestBackend::new(80, 24);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal
            .draw(|f| crate::views::query::draw(f, &mut app, f.area()))
            .unwrap();
        let text: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        for hint in ["F1 Commands", "F2 Account", "F3 Context", "F4 Login"] {
            assert!(text.contains(hint), "Missing {hint}");
        }
    }
}
