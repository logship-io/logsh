use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
    Frame,
};

use crate::app::App;

pub fn draw(f: &mut Frame, app: &App) {
    let area = centered_rect(50, 50, f.area());
    f.render_widget(Clear, area);

    let block = Block::default()
        .title(" Select Account (Enter to choose, Esc to close) ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green));

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(1)])
        .split(area);

    let filtered = app.filtered_accounts();

    let items: Vec<ListItem> = filtered
        .iter()
        .map(|(_, acct)| {
            let marker = if acct.is_current { "● " } else { "  " };
            let style = if acct.is_current {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            ListItem::new(format!("{marker}{}", acct.name)).style(style)
        })
        .collect();

    let list = List::new(items)
        .block(block)
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▸ ");

    let mut state = ListState::default();
    state.select(Some(
        app.account_selected.min(filtered.len().saturating_sub(1)),
    ));
    f.render_stateful_widget(list, chunks[0], &mut state);

    // Filter bar
    let filter_line = if app.account_filter.is_empty() {
        Line::from(Span::styled(
            " type to filter...",
            Style::default().fg(Color::DarkGray),
        ))
    } else {
        Line::from(vec![
            Span::styled(" /", Style::default().fg(Color::Cyan)),
            Span::raw(&app.account_filter),
        ])
    };
    f.render_widget(Paragraph::new(filter_line), chunks[1]);
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
