use std::sync::mpsc;
use std::time::Duration;

use crossterm::event::{self, Event, KeyEvent, KeyEventKind};

use crate::backend::BackendResponse;

/// Unified event type for the main loop.
pub enum AppEvent {
    Key(KeyEvent),
    Backend(BackendResponse),
    Resize,
    Tick,
}

/// Polls for crossterm events and backend responses, yielding AppEvents.
pub fn poll_event(
    backend_rx: &mpsc::Receiver<BackendResponse>,
    tick_rate: Duration,
) -> Option<AppEvent> {
    // Check for backend responses first (non-blocking)
    if let Ok(response) = backend_rx.try_recv() {
        return Some(AppEvent::Backend(response));
    }

    // Poll for terminal events
    if event::poll(tick_rate).ok()? {
        match event::read().ok()? {
            Event::Key(key) if key.kind == KeyEventKind::Press => Some(AppEvent::Key(key)),
            Event::Resize(_, _) => Some(AppEvent::Resize),
            _ => Some(AppEvent::Tick),
        }
    } else {
        Some(AppEvent::Tick)
    }
}
