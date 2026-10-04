use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
    Frame,
};

use crate::app::App;

pub fn draw(f: &mut Frame, app: &App) {
    let area = super::centered_rect(85, 60, f.area());
    f.render_widget(Clear, area);

    let block = Block::default()
        .title(" Choose account: Enter select, Esc cancel ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green));

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(1)])
        .split(area);

    let filtered = app.filtered_accounts();

    let mut items: Vec<ListItem> = filtered
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
            ListItem::new(format!("{marker}{} ({})", acct.name, acct.id)).style(style)
        })
        .collect();
    if items.is_empty() {
        items.push(ListItem::new(
            if app.status_message.as_deref() == Some("Loading accounts...") {
                "Loading accounts..."
            } else {
                "No accounts match."
            },
        ));
    }

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
