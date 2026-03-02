use ratatui::{
    style::{Color, Style},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

use crate::app::App;

pub fn draw(f: &mut Frame, app: &App) {
    let area = super::centered_rect(80, 70, f.area());
    f.render_widget(Clear, area);

    let (col_name, cell) = match app.focused_cell() {
        Some(v) => v,
        None => return,
    };

    let row_idx = app.results_cursor + 1;
    let title = format!(" {col_name}  (row {row_idx}, Esc to close) ");

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let value_str = cell.to_string();

    // Try to pretty-print JSON values
    let display_text = if value_str.starts_with('{') || value_str.starts_with('[') {
        serde_json::from_str::<serde_json::Value>(&value_str)
            .ok()
            .and_then(|v| serde_json::to_string_pretty(&v).ok())
            .unwrap_or(value_str)
    } else {
        value_str
    };

    let style = match cell {
        crate::app::CellValue::Null => Style::default().fg(Color::DarkGray),
        crate::app::CellValue::Number(_) => Style::default().fg(Color::Cyan),
        crate::app::CellValue::Bool(_) => Style::default().fg(Color::Magenta),
        crate::app::CellValue::String(_) => Style::default().fg(Color::White),
    };

    let paragraph = Paragraph::new(display_text)
        .style(style)
        .block(block)
        .wrap(Wrap { trim: false });

    f.render_widget(paragraph, area);
}
