use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, List, ListItem, ListState, Paragraph, Row, Table},
    Frame,
};

use crate::app::{App, Focus};

/// Draw the main application layout:
/// ┌────────────┬───────────────────────────────────────┐
/// │  Schemas   │  Query Editor                         │
/// │  (left)    ├───────────────────────────────────────┤
/// │            │  Results Table                        │
/// ├────────────┴───────────────────────────────────────┤
/// │  :command bar  (only when active)                  │
/// └───────────────────────────────────────────────────-┘
/// Status line / key hints at the very bottom.
pub fn draw(f: &mut Frame, app: &mut App, area: Rect) {
    // Bottom status bar always present (1 line)
    // Command bar: 1 line when active
    let command_bar_height = if app.focus == Focus::CommandBar { 1 } else { 0 };
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(4),                     // main panes
            Constraint::Length(command_bar_height), // command bar
            Constraint::Length(1),                  // status/key hints
        ])
        .split(area);

    // Horizontal split: left schema nav + right content
    if app.schema_nav_visible {
        let h_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(28), Constraint::Min(40)])
            .split(main_chunks[0]);

        draw_schema_nav(f, app, h_chunks[0]);
        draw_right_panes(f, app, h_chunks[1]);
    } else {
        draw_right_panes(f, app, main_chunks[0]);
    }

    // Command bar
    if app.focus == Focus::CommandBar {
        draw_command_bar(f, app, main_chunks[1]);
    }

    // Status line
    draw_status_bar(f, app, main_chunks[2]);
}

fn draw_schema_nav(f: &mut Frame, app: &App, area: Rect) {
    let focused = app.focus == Focus::Schemas;
    let border_color = if focused {
        Color::Cyan
    } else {
        Color::DarkGray
    };

    let title = if app.schemas_loading {
        " Schemas ⟳ "
    } else {
        " Schemas "
    };

    // Split: schema list + optional filter bar at bottom
    let filter_height = if focused { 1 } else { 0 };
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(2), Constraint::Length(filter_height)])
        .split(area);

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    let filtered = app.filtered_schemas();

    if filtered.is_empty() {
        let msg = if app.schemas_loading {
            "Loading..."
        } else if !app.schema_filter.is_empty() {
            "(no matches)"
        } else {
            "(none)"
        };
        let p = Paragraph::new(msg)
            .style(Style::default().fg(Color::DarkGray))
            .block(block);
        f.render_widget(p, chunks[0]);
    } else {
        let items: Vec<ListItem> = filtered
            .iter()
            .map(|(_, s)| ListItem::new(s.as_str()).style(Style::default().fg(Color::White)))
            .collect();

        let list = List::new(items)
            .block(block)
            .highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("▸ ");

        let mut state = ListState::default();
        state.select(Some(
            app.schema_selected.min(filtered.len().saturating_sub(1)),
        ));
        f.render_stateful_widget(list, chunks[0], &mut state);
    }

    // Filter bar
    if filter_height > 0 {
        let filter_text = if app.schema_filter.is_empty() {
            Line::from(Span::styled(
                " type to filter...",
                Style::default().fg(Color::DarkGray),
            ))
        } else {
            Line::from(vec![
                Span::styled(" /", Style::default().fg(Color::Cyan)),
                Span::raw(&app.schema_filter),
            ])
        };
        f.render_widget(Paragraph::new(filter_text), chunks[1]);
    }
}

fn draw_right_panes(f: &mut Frame, app: &mut App, area: Rect) {
    // Dynamic editor height: min 5, grows with content
    let editor_lines = app.editor.lines().len() as u16;
    let editor_height = editor_lines.clamp(3, 12) + 2; // +2 for borders

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(editor_height), // editor
            Constraint::Length(1),             // parse status line
            Constraint::Min(4),                // results
        ])
        .split(area);

    draw_editor(f, app, chunks[0]);
    draw_parse_status(f, app, chunks[1]);
    draw_results(f, app, chunks[2]);
}

fn draw_editor(f: &mut Frame, app: &App, area: Rect) {
    let focused = app.focus == Focus::Editor;
    let border_color = if focused {
        Color::Cyan
    } else {
        Color::DarkGray
    };

    let title = if app.query_running {
        " Query ⟳ "
    } else {
        " Query "
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    // Clone and configure the textarea widget for rendering
    let mut editor = app.editor.clone();
    editor.set_block(block);
    editor.set_style(Style::default().fg(Color::White));
    if focused {
        editor.set_cursor_style(Style::default().bg(Color::White).fg(Color::Black));
    } else {
        editor.set_cursor_style(Style::default());
    }

    f.render_widget(&editor, area);
}

fn draw_parse_status(f: &mut Frame, app: &App, area: Rect) {
    let text = app.editor_text();
    if text.trim().is_empty() {
        // Nothing to parse — show empty line
        f.render_widget(Paragraph::new(""), area);
        return;
    }

    let line = if app.parse_dirty || app.parse_in_flight {
        Line::from(Span::styled(" ...", Style::default().fg(Color::DarkGray)))
    } else if let Some(ref err) = app.parse_error {
        Line::from(Span::styled(
            format!(" Parse error: {err}"),
            Style::default().fg(Color::Red),
        ))
    } else if let Some(ref result) = app.parse_result {
        if result.parse_success {
            Line::from(Span::styled(" OK", Style::default().fg(Color::Green)))
        } else {
            // Show first error message if available
            let msg = result
                .error
                .as_ref()
                .and_then(|e| {
                    e.errors
                        .first()
                        .and_then(|em| em.message.clone())
                        .or_else(|| Some(e.message.clone()))
                })
                .unwrap_or_else(|| "Parse failed".to_string());
            Line::from(Span::styled(
                format!(" Error: {msg}"),
                Style::default().fg(Color::Red),
            ))
        }
    } else {
        Line::from(Span::styled(" ...", Style::default().fg(Color::DarkGray)))
    };

    f.render_widget(Paragraph::new(line), area);
}

fn draw_results(f: &mut Frame, app: &mut App, area: Rect) {
    let focused = app.focus == Focus::Results;
    let border_color = if focused {
        Color::Cyan
    } else {
        Color::DarkGray
    };

    // Show error if present
    if let Some(err) = &app.error_message {
        let block = Block::default()
            .title(" Results ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Red));
        let msg = Paragraph::new(err.as_str())
            .style(Style::default().fg(Color::Red))
            .block(block);
        f.render_widget(msg, area);
        return;
    }

    let results = match &app.results {
        Some(r) => r,
        None => {
            let block = Block::default()
                .title(" Results ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color));
            let text = if let Some(ref status) = app.status_message {
                status.as_str()
            } else {
                "Run a query (Ctrl+R or Alt+Enter)"
            };
            let msg = Paragraph::new(text)
                .style(Style::default().fg(Color::DarkGray))
                .block(block);
            f.render_widget(msg, area);
            return;
        }
    };

    if results.columns.is_empty() {
        let block = Block::default()
            .title(" Results ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color));
        f.render_widget(
            Paragraph::new("(no columns)")
                .style(Style::default().fg(Color::DarkGray))
                .block(block),
            area,
        );
        return;
    }

    // Split: results table + 1-line position indicator at bottom
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(1)])
        .split(area);

    let col_count = results.columns.len();
    let focused_col = app.results_col.min(col_count.saturating_sub(1));
    let row_count = results.rows.len();
    let cursor = app.results_cursor.min(row_count.saturating_sub(1));

    // Compute viewport: keep cursor visible within the table area
    // Inner height = table area height - 2 (borders) - 1 (header)
    let viewport_height = chunks[0].height.saturating_sub(3) as usize;
    if viewport_height > 0 {
        if cursor < app.results_scroll {
            app.results_scroll = cursor;
        } else if cursor >= app.results_scroll + viewport_height {
            app.results_scroll = cursor.saturating_sub(viewport_height - 1);
        }
    }
    let scroll = app.results_scroll;

    // Row position display (1-indexed)
    let row_display = if row_count == 0 {
        "0/0".to_string()
    } else {
        format!("{}/{}", cursor + 1, row_count)
    };
    let col_name = results
        .columns
        .get(focused_col)
        .map(|s| s.as_str())
        .unwrap_or("");
    let pos_line = Line::from(vec![
        Span::styled(
            format!(" Row {row_display}"),
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled(
            format!("  Col {}/{col_count} ", focused_col + 1),
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled(
            col_name.to_string(),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("  :cell :row :copy", Style::default().fg(Color::DarkGray)),
    ]);
    f.render_widget(Paragraph::new(pos_line), chunks[1]);

    // Build column widths: all columns equal, focused column gets up to 50%
    let available = chunks[0].width.saturating_sub(2) as usize; // minus borders
    let base_width = if col_count > 0 {
        available / col_count
    } else {
        10
    };
    let max_focused = available / 2;
    let focused_width = base_width.max(20).min(max_focused) as u16;
    let other_width = if col_count > 1 {
        let remaining = available.saturating_sub(focused_width as usize);
        (remaining / (col_count - 1)).max(6) as u16
    } else {
        base_width as u16
    };

    let widths: Vec<Constraint> = (0..col_count)
        .map(|i| {
            if i == focused_col {
                Constraint::Length(focused_width)
            } else {
                Constraint::Length(other_width)
            }
        })
        .collect();

    let row_count_label = format!("{} rows", results.rows.len());
    let title = format!(" Results ({row_count_label}) ");
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    let header_cells: Vec<Cell> = results
        .columns
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let style = if i == focused_col {
                Style::default()
                    .fg(Color::Yellow)
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
            } else {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            };
            Cell::from(c.as_str()).style(style)
        })
        .collect();
    let header = Row::new(header_cells).bottom_margin(0);

    let rows: Vec<Row> = results
        .rows
        .iter()
        .enumerate()
        .skip(scroll)
        .take(viewport_height.max(1))
        .map(|(abs_idx, row)| {
            let is_focused_row = abs_idx == cursor && focused;
            let cells: Vec<Cell> = row
                .iter()
                .enumerate()
                .map(|(i, cell)| {
                    let base_style = match cell {
                        crate::app::CellValue::Null => Style::default().fg(Color::DarkGray),
                        crate::app::CellValue::Number(_) => Style::default().fg(Color::Cyan),
                        crate::app::CellValue::Bool(_) => Style::default().fg(Color::Magenta),
                        crate::app::CellValue::String(_) => Style::default().fg(Color::White),
                    };
                    let style = if is_focused_row && i == focused_col {
                        // Focused cell: bright highlight
                        base_style.bg(Color::Rgb(50, 50, 80))
                    } else if is_focused_row {
                        // Focused row: subtle row highlight
                        base_style.bg(Color::Rgb(30, 30, 40))
                    } else if i == focused_col {
                        // Focused column: subtle column tint
                        base_style.bg(Color::Rgb(20, 20, 30))
                    } else {
                        base_style
                    };
                    Cell::from(cell.to_string()).style(style)
                })
                .collect();
            Row::new(cells)
        })
        .collect();

    let table = Table::new(rows, &widths).header(header).block(block);

    f.render_widget(table, chunks[0]);
}

fn draw_command_bar(f: &mut Frame, app: &App, area: Rect) {
    let mut spans = vec![
        Span::styled(
            ":",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(&app.command_input),
    ];

    // Show matching command hints
    let matches = app.matching_commands();
    if !matches.is_empty() && !app.command_input.is_empty() {
        let hint = matches
            .iter()
            .map(|c| c.name)
            .collect::<Vec<_>>()
            .join(" | ");
        spans.push(Span::styled(
            format!("  [{hint}]"),
            Style::default().fg(Color::DarkGray),
        ));
    }

    let line = Line::from(spans);
    let p = Paragraph::new(line);
    f.render_widget(p, area);

    // Show cursor in command bar
    let cx = (1 + app.command_cursor) as u16; // 1 for the ':'
    f.set_cursor_position((area.x + cx, area.y));
}

fn draw_status_bar(f: &mut Frame, app: &App, area: Rect) {
    let ctx_name = app.current_context.as_deref().unwrap_or("(no context)");

    let acct_name = app.current_account.as_deref().unwrap_or("(no account)");

    let focus_label = match app.focus {
        Focus::Schemas => "schemas",
        Focus::Editor => "editor",
        Focus::Results => "results",
        Focus::CommandBar => "command",
    };

    let hints = " :cmd  Tab ↹  ^R run  ^K ctx  ^S saved  ? help  ^Q quit";

    let left = Span::styled(
        format!(" {ctx_name} "),
        Style::default().fg(Color::Black).bg(Color::Cyan),
    );
    let acct = Span::styled(
        format!(" {acct_name} "),
        Style::default().fg(Color::Black).bg(Color::Green),
    );
    let mid = Span::styled(
        format!(" {focus_label} "),
        Style::default().fg(Color::Black).bg(Color::DarkGray),
    );
    let right = Span::styled(hints, Style::default().fg(Color::DarkGray));

    let line = Line::from(vec![
        left,
        Span::raw(" "),
        acct,
        Span::raw(" "),
        mid,
        Span::raw(" "),
        right,
    ]);
    f.render_widget(Paragraph::new(line), area);
}
