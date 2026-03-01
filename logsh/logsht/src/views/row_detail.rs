use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

use crate::app::App;

pub fn draw(f: &mut Frame, app: &App) {
    let area = centered_rect(80, 80, f.area());
    f.render_widget(Clear, area);

    let pairs = match app.focused_row() {
        Some(v) => v,
        None => return,
    };

    let row_idx = app.results_cursor + 1;
    let title = format!(" Row {row_idx}  (Esc to close) ");

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let focused_col = app.results_col;

    let lines: Vec<Line> = pairs
        .iter()
        .enumerate()
        .map(|(i, (col_name, cell))| {
            let is_focused = i == focused_col;
            let key_style = if is_focused {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Cyan)
            };
            let val_style = match cell {
                crate::app::CellValue::Null => Style::default().fg(Color::DarkGray),
                crate::app::CellValue::Number(_) => Style::default().fg(Color::White),
                crate::app::CellValue::Bool(_) => Style::default().fg(Color::Magenta),
                crate::app::CellValue::String(_) => Style::default().fg(Color::White),
            };
            Line::from(vec![
                Span::styled(format!("{:<24}", col_name), key_style),
                Span::styled(cell.to_string(), val_style),
            ])
        })
        .collect();

    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false });

    f.render_widget(paragraph, area);
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
