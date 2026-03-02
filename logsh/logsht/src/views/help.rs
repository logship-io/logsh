use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::app::COMMANDS;

const KEYBINDINGS: &[(&str, &str)] = &[
    ("Tab / Shift+Tab", "Cycle focus: Schemas → Editor → Results"),
    ("Alt+Enter / Ctrl+R", "Execute query"),
    ("Ctrl+K", "Open context switcher"),
    ("Ctrl+S", "Open saved queries"),
    ("Alt+Up / Alt+Down", "Navigate query history"),
    (":", "Open command bar"),
    ("? / Ctrl+H", "Show this help"),
    ("Ctrl+Q / Ctrl+C", "Quit"),
    ("", ""),
    ("── Schemas ──", ""),
    ("j / k / ↑ / ↓", "Navigate tables"),
    ("Enter", "Select table → editor"),
    ("r", "Refresh schemas"),
    ("Type", "Filter tables by name"),
    ("", ""),
    ("── Editor ──", ""),
    ("Type", "Enter query text (full tui-textarea)"),
    ("Esc", "Move focus to schemas"),
    ("", ""),
    ("── Results ──", ""),
    ("j / k / ↑ / ↓", "Move cursor up/down rows"),
    ("h / l / ← / →", "Navigate columns"),
    ("PgUp / PgDn", "Scroll 20 rows"),
    ("g / G", "Jump to top / bottom"),
    ("", ""),
    ("── Overlays ──", ""),
    ("Type", "Filter items in any overlay"),
    ("Backspace", "Clear filter character"),
    ("Ctrl+D", "Delete saved query (in saved overlay)"),
];

pub fn draw(f: &mut Frame) {
    let area = centered_rect(60, 70, f.area());
    f.render_widget(Clear, area);

    let block = Block::default()
        .title(" Keybindings (Esc to close) ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let mut lines: Vec<Line> = KEYBINDINGS
        .iter()
        .map(|(key, desc)| {
            if key.is_empty() {
                Line::default()
            } else if desc.is_empty() {
                // Section header
                Line::from(Span::styled(
                    *key,
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ))
            } else {
                Line::from(vec![
                    Span::styled(
                        format!("{key:<22}"),
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(*desc, Style::default().fg(Color::White)),
                ])
            }
        })
        .collect();

    // Dynamic commands section from COMMANDS constant
    lines.push(Line::default());
    lines.push(Line::from(Span::styled(
        "── Commands ──",
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    )));
    for cmd in COMMANDS {
        lines.push(Line::from(vec![
            Span::styled(
                format!(":{:<21}", cmd.name),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(cmd.description, Style::default().fg(Color::White)),
        ]));
    }

    let p = Paragraph::new(lines).block(block);
    f.render_widget(p, area);
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(area);

    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(vertical[1])[1]
}
