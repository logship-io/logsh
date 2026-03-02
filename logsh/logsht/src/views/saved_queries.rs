use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
    Frame,
};

use crate::app::App;

pub fn draw(f: &mut Frame, app: &App) {
    let area = super::centered_rect(60, 60, f.area());
    f.render_widget(Clear, area);

    let block = Block::default()
        .title(" Saved Queries (Enter to load, Ctrl+D delete, Esc close) ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta));

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(1)])
        .split(area);

    let filtered = app.filtered_saved_queries();

    if filtered.is_empty() {
        let msg = if app.saved_queries_loading {
            "Loading..."
        } else if !app.saved_query_filter.is_empty() {
            "(no matches)"
        } else {
            "(no saved queries — use :save <name> to create one)"
        };
        let p = Paragraph::new(msg)
            .style(Style::default().fg(Color::DarkGray))
            .block(block);
        f.render_widget(p, chunks[0]);
    } else {
        let items: Vec<ListItem> = filtered
            .iter()
            .map(|(_, sq)| {
                let preview: String = sq.query.chars().take(40).collect();
                let line = Line::from(vec![
                    Span::styled(
                        format!("{:<20}", sq.name),
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(preview, Style::default().fg(Color::DarkGray)),
                ]);
                ListItem::new(line)
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
            app.saved_query_selected
                .min(filtered.len().saturating_sub(1)),
        ));
        f.render_stateful_widget(list, chunks[0], &mut state);
    }

    // Filter bar
    let filter_line = if app.saved_query_filter.is_empty() {
        Line::from(Span::styled(
            " type to filter...",
            Style::default().fg(Color::DarkGray),
        ))
    } else {
        Line::from(vec![
            Span::styled(" /", Style::default().fg(Color::Cyan)),
            Span::raw(&app.saved_query_filter),
        ])
    };
    f.render_widget(Paragraph::new(filter_line), chunks[1]);
}
