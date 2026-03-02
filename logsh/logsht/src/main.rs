mod app;
mod backend;
mod event;
mod input;
mod tui;
mod ui;
mod views;

use std::sync::mpsc;
use std::time::Duration;

use app::App;
use backend::BackendRequest;
use event::AppEvent;

fn main() -> anyhow::Result<()> {
    let (req_tx, req_rx) = mpsc::channel::<BackendRequest>();
    let (resp_tx, resp_rx) = mpsc::channel();
    let backend_handle = backend::spawn_backend(req_rx, resp_tx);
    let mut terminal = tui::init()?;
    let mut app = App::new(req_tx.clone());
    app.init();

    let tick_rate = Duration::from_millis(50);
    while app.running {
        terminal.draw(|f| ui::draw(f, &mut app))?;

        if let Some(event) = event::poll_event(&resp_rx, tick_rate) {
            match event {
                AppEvent::Key(key) => input::handle_key(&mut app, key),
                AppEvent::Backend(response) => app.handle_backend_response(response),
                AppEvent::Tick => app.tick(),
                AppEvent::Resize => {}
            }
        }
    }

    let _ = req_tx.send(BackendRequest::Shutdown);
    let _ = backend_handle.join();
    tui::restore()?;

    Ok(())
}
