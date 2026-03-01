use ratatui::Frame;

use crate::app::{App, Overlay};
use crate::views;

/// Root draw function — renders the main layout, then any active overlay on top.
pub fn draw(f: &mut Frame, app: &mut App) {
    views::query::draw(f, app, f.area());

    match &app.overlay {
        Some(Overlay::ContextSwitcher) => views::context::draw(f, app),
        Some(Overlay::AccountPicker) => views::account::draw(f, app),
        Some(Overlay::SavedQueries) => views::saved_queries::draw(f, app),
        Some(Overlay::CellDetail) => views::cell_detail::draw(f, app),
        Some(Overlay::RowDetail) => views::row_detail::draw(f, app),
        Some(Overlay::Help) => views::help::draw(f),
        None => {}
    }
}
