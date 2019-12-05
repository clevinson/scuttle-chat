use std::io;
use termion::input::MouseTerminal;
use termion::raw::IntoRawMode;
use termion::screen::AlternateScreen;
use tui::backend::TermionBackend;
use tui::Terminal;

#[macro_use]
extern crate quick_error;

mod app;
mod box_stream;
mod chat;
mod event;
mod discovery;
mod peer_manager;
mod ui;

use app::App;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Terminal initialization
    let stdout = io::stdout().into_raw_mode()?;
    let stdout = MouseTerminal::from(stdout);
    let stdout = AlternateScreen::from(stdout);
    let backend = TermionBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.hide_cursor()?;

    // App
    let mut app = App::new();
    app.run(&mut terminal)?;

    Ok(())
}
