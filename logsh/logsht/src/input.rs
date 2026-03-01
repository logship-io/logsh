use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{App, Focus, Overlay};

/// Process a key event and update app state.
pub fn handle_key(app: &mut App, key: KeyEvent) {
    // Global quit
    if matches!(key.code, KeyCode::Char('c') | KeyCode::Char('q'))
        && key.modifiers.contains(KeyModifiers::CONTROL)
    {
        app.running = false;
        return;
    }

    // Dismiss overlays with Esc
    if key.code == KeyCode::Esc && app.overlay.is_some() {
        app.overlay = None;
        return;
    }

    // Dispatch to overlay first
    if let Some(overlay) = &app.overlay.clone() {
        match overlay {
            Overlay::ContextSwitcher => handle_context_key(app, key),
            Overlay::AccountPicker => handle_account_key(app, key),
            Overlay::SavedQueries => handle_saved_query_key(app, key),
            Overlay::Help => {
                app.overlay = None;
            }
            Overlay::CellDetail => {
                if matches!(key.code, KeyCode::Esc) {
                    app.overlay = None;
                }
            }
            Overlay::RowDetail => {
                if matches!(key.code, KeyCode::Esc) {
                    app.overlay = None;
                }
            }
        }
        return;
    }

    // Command bar mode
    if app.focus == Focus::CommandBar {
        handle_command_key(app, key);
        return;
    }

    // Global keybindings (when not in command bar)
    match key.code {
        KeyCode::Char(':') if app.focus != Focus::Editor => {
            app.open_command_bar();
            return;
        }
        // Alt+Enter to execute query
        KeyCode::Enter if key.modifiers.contains(KeyModifiers::ALT) => {
            app.execute_query();
            return;
        }
        // Ctrl+R to run query
        KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.execute_query();
            return;
        }
        KeyCode::Char('?') if app.focus != Focus::Editor => {
            app.overlay = Some(Overlay::Help);
            return;
        }
        // Ctrl+H for help
        KeyCode::Char('h') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.overlay = Some(Overlay::Help);
            return;
        }
        // Ctrl+K for context switcher
        KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.load_contexts();
            app.context_filter.clear();
            app.overlay = Some(Overlay::ContextSwitcher);
            return;
        }
        // Ctrl+S for saved queries
        KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.saved_query_filter.clear();
            app.load_saved_queries();
            app.overlay = Some(Overlay::SavedQueries);
            return;
        }
        KeyCode::Tab => {
            app.focus = match app.focus {
                Focus::Schemas => Focus::Editor,
                Focus::Editor => Focus::Results,
                Focus::Results if app.schema_nav_visible => Focus::Schemas,
                Focus::Results => Focus::Editor,
                Focus::CommandBar => Focus::Editor,
            };
            return;
        }
        KeyCode::BackTab => {
            app.focus = match app.focus {
                Focus::Editor if app.schema_nav_visible => Focus::Schemas,
                Focus::Editor => Focus::Results,
                Focus::Results => Focus::Editor,
                Focus::Schemas => Focus::Results,
                Focus::CommandBar => Focus::Editor,
            };
            return;
        }
        KeyCode::Esc if app.focus == Focus::Editor => {
            app.focus = Focus::Schemas;
            return;
        }
        _ => {}
    }

    match app.focus {
        Focus::Schemas => handle_schema_key(app, key),
        Focus::Editor => handle_editor_key(app, key),
        Focus::Results => handle_results_key(app, key),
        Focus::CommandBar => handle_command_key(app, key),
    }
}

fn handle_editor_key(app: &mut App, key: KeyEvent) {
    // Special editor keybindings before passing to textarea
    match key.code {
        KeyCode::Char(':') if app.editor_text().is_empty() => {
            app.open_command_bar();
            return;
        }
        // Alt+Up for history previous
        KeyCode::Up if key.modifiers.contains(KeyModifiers::ALT) => {
            app.history_prev();
            return;
        }
        // Alt+Down for history next
        KeyCode::Down if key.modifiers.contains(KeyModifiers::ALT) => {
            app.history_next();
            return;
        }
        _ => {}
    }

    // Pass all other keys to tui-textarea
    let input = tui_textarea::Input::from(crossterm::event::Event::Key(key));
    let changed = app.editor.input(input);
    if changed {
        app.mark_editor_dirty();
    }
}

fn handle_results_key(app: &mut App, key: KeyEvent) {
    let row_count = app.results.as_ref().map(|r| r.rows.len()).unwrap_or(0);
    let col_count = app.results.as_ref().map(|r| r.columns.len()).unwrap_or(0);
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            app.results_cursor = app.results_cursor.saturating_sub(1);
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if app.results_cursor + 1 < row_count {
                app.results_cursor += 1;
            }
        }
        KeyCode::Left | KeyCode::Char('h') => {
            app.results_col = app.results_col.saturating_sub(1);
        }
        KeyCode::Right | KeyCode::Char('l') => {
            if col_count > 0 && app.results_col + 1 < col_count {
                app.results_col += 1;
            }
        }
        KeyCode::PageUp => {
            app.results_cursor = app.results_cursor.saturating_sub(20);
        }
        KeyCode::PageDown => {
            app.results_cursor = (app.results_cursor + 20).min(row_count.saturating_sub(1));
        }
        KeyCode::Home | KeyCode::Char('g') => {
            app.results_cursor = 0;
            app.results_col = 0;
        }
        KeyCode::End | KeyCode::Char('G') => {
            if row_count > 0 {
                app.results_cursor = row_count - 1;
            }
        }
        _ => {}
    }
}

fn handle_schema_key(app: &mut App, key: KeyEvent) {
    let filtered = app.filtered_schemas();
    let filtered_len = filtered.len();

    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            app.schema_selected = app.schema_selected.saturating_sub(1);
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if app.schema_selected + 1 < filtered_len {
                app.schema_selected += 1;
            }
        }
        KeyCode::Enter => {
            let filtered = app.filtered_schemas();
            if let Some(&(_, table)) = filtered.get(app.schema_selected) {
                let table = table.clone();
                if table != "(no tables found)" {
                    let query = if let Some(cols) = app.schema_columns.get(&table) {
                        let project_list = cols.join(", ");
                        format!("{table}\n| take 100\n| project {project_list}")
                    } else {
                        format!("{table}\n| take 100")
                    };
                    app.set_editor_text(&query);
                    app.focus = Focus::Editor;
                    app.mark_editor_dirty();
                }
            }
        }
        KeyCode::Char('r') if app.schema_filter.is_empty() => app.load_schemas(),
        KeyCode::Char(':') if app.schema_filter.is_empty() => app.open_command_bar(),
        // Type-to-filter
        KeyCode::Char(c) => {
            app.schema_filter.push(c);
            app.schema_selected = 0;
        }
        KeyCode::Backspace => {
            app.schema_filter.pop();
            app.schema_selected = 0;
        }
        KeyCode::Esc if !app.schema_filter.is_empty() => {
            app.schema_filter.clear();
            app.schema_selected = 0;
        }
        _ => {}
    }
}

fn handle_context_key(app: &mut App, key: KeyEvent) {
    let filtered = app.filtered_contexts();
    let filtered_len = filtered.len();

    match key.code {
        KeyCode::Esc if !app.context_filter.is_empty() => {
            app.context_filter.clear();
            app.context_selected = 0;
        }
        KeyCode::Esc => app.overlay = None,
        KeyCode::Up | KeyCode::Char('k')
            if key.modifiers == KeyModifiers::NONE || key.code == KeyCode::Up =>
        {
            app.context_selected = app.context_selected.saturating_sub(1);
        }
        KeyCode::Down | KeyCode::Char('j')
            if key.modifiers == KeyModifiers::NONE || key.code == KeyCode::Down =>
        {
            if app.context_selected + 1 < filtered_len {
                app.context_selected += 1;
            }
        }
        KeyCode::Enter => {
            let filtered = app.filtered_contexts();
            if let Some(&(_, ctx)) = filtered.get(app.context_selected) {
                let name = ctx.name.clone();
                app.switch_context(&name);
            }
        }
        KeyCode::Char(c) => {
            app.context_filter.push(c);
            app.context_selected = 0;
        }
        KeyCode::Backspace => {
            app.context_filter.pop();
            app.context_selected = 0;
        }
        _ => {}
    }
}

fn handle_account_key(app: &mut App, key: KeyEvent) {
    let filtered = app.filtered_accounts();
    let filtered_len = filtered.len();

    match key.code {
        KeyCode::Esc if !app.account_filter.is_empty() => {
            app.account_filter.clear();
            app.account_selected = 0;
        }
        KeyCode::Esc => app.overlay = None,
        KeyCode::Up | KeyCode::Char('k')
            if key.modifiers == KeyModifiers::NONE || key.code == KeyCode::Up =>
        {
            app.account_selected = app.account_selected.saturating_sub(1);
        }
        KeyCode::Down | KeyCode::Char('j')
            if key.modifiers == KeyModifiers::NONE || key.code == KeyCode::Down =>
        {
            if app.account_selected + 1 < filtered_len {
                app.account_selected += 1;
            }
        }
        KeyCode::Enter => {
            let filtered = app.filtered_accounts();
            if let Some(&(_, acct)) = filtered.get(app.account_selected) {
                let acct = acct.clone();
                app.select_account(acct.id, &acct.name);
            }
        }
        KeyCode::Char(c) => {
            app.account_filter.push(c);
            app.account_selected = 0;
        }
        KeyCode::Backspace => {
            app.account_filter.pop();
            app.account_selected = 0;
        }
        _ => {}
    }
}

fn handle_saved_query_key(app: &mut App, key: KeyEvent) {
    let filtered = app.filtered_saved_queries();
    let filtered_len = filtered.len();

    match key.code {
        KeyCode::Esc if !app.saved_query_filter.is_empty() => {
            app.saved_query_filter.clear();
            app.saved_query_selected = 0;
        }
        KeyCode::Esc => app.overlay = None,
        KeyCode::Up | KeyCode::Char('k')
            if key.modifiers == KeyModifiers::NONE || key.code == KeyCode::Up =>
        {
            app.saved_query_selected = app.saved_query_selected.saturating_sub(1);
        }
        KeyCode::Down | KeyCode::Char('j')
            if key.modifiers == KeyModifiers::NONE || key.code == KeyCode::Down =>
        {
            if app.saved_query_selected + 1 < filtered_len {
                app.saved_query_selected += 1;
            }
        }
        KeyCode::Enter => {
            let filtered = app.filtered_saved_queries();
            if let Some(&(_, sq)) = filtered.get(app.saved_query_selected) {
                let query = sq.query.clone();
                app.set_editor_text(&query);
                app.overlay = None;
                app.focus = Focus::Editor;
                app.mark_editor_dirty();
            }
        }
        // Delete with 'd'
        KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            let filtered = app.filtered_saved_queries();
            if let Some(&(_, sq)) = filtered.get(app.saved_query_selected) {
                let id = sq.query_id;
                app.delete_saved_query(id);
            }
        }
        KeyCode::Char(c) => {
            app.saved_query_filter.push(c);
            app.saved_query_selected = 0;
        }
        KeyCode::Backspace => {
            app.saved_query_filter.pop();
            app.saved_query_selected = 0;
        }
        _ => {}
    }
}

fn handle_command_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            app.command_input.clear();
            app.command_cursor = 0;
            app.focus = app.pre_command_focus;
        }
        KeyCode::Enter => app.execute_command(),
        KeyCode::Char(c) => {
            app.command_input.insert(app.command_cursor, c);
            app.command_cursor += c.len_utf8();
        }
        KeyCode::Backspace => {
            if app.command_cursor > 0 {
                let prev = app.command_input[..app.command_cursor]
                    .char_indices()
                    .next_back()
                    .map(|(i, _)| i)
                    .unwrap_or(0);
                app.command_input.drain(prev..app.command_cursor);
                app.command_cursor = prev;
            } else {
                app.focus = Focus::Editor;
            }
        }
        KeyCode::Left => {
            if app.command_cursor > 0 {
                app.command_cursor = app.command_input[..app.command_cursor]
                    .char_indices()
                    .next_back()
                    .map(|(i, _)| i)
                    .unwrap_or(0);
            }
        }
        KeyCode::Right => {
            if app.command_cursor < app.command_input.len() {
                app.command_cursor = app.command_input[app.command_cursor..]
                    .char_indices()
                    .nth(1)
                    .map(|(i, _)| app.command_cursor + i)
                    .unwrap_or(app.command_input.len());
            }
        }
        _ => {}
    }
}
