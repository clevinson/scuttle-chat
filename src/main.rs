use crate::event::{Event, Events};
use crate::peer_manager::{PeerEvent, PeerManagerEvent};
use net::SsbPeer;
use ssb_crypto::{generate_longterm_keypair, PublicKey};
use std::io;
use std::sync::{mpsc, Arc};
use termion::event::Key;
use termion::input::MouseTerminal;
use termion::raw::IntoRawMode;
use termion::screen::AlternateScreen;
use tui::backend::TermionBackend;
use tui::layout::{Constraint, Corner, Direction, Layout};
use tui::style::{Color, Modifier, Style};
use tui::widgets::{Block, Borders, List, SelectableList, Text, Widget};
use tui::Terminal;

#[macro_use]
extern crate quick_error;

use peer_manager::PeerManager;
use std::collections::HashMap;

mod event;
mod net;
mod peer_manager;
mod box_stream;

struct App<'a> {
    available_peers: HashMap<String, Arc<SsbPeer>>,
    selected: Option<usize>,
    log_book: Vec<(String, &'a str)>,
    info_style: Style,
    warning_style: Style,
    error_style: Style,
    critical_style: Style,
}

impl<'a> App<'a> {
    fn new() -> App<'a> {
        App {
            available_peers: HashMap::new(),
            selected: None,
            log_book: Vec::new(),
            info_style: Style::default().fg(Color::White),
            warning_style: Style::default().fg(Color::Yellow),
            error_style: Style::default().fg(Color::Magenta),
            critical_style: Style::default().fg(Color::Red),
        }
    }

    fn peer_list(&self) -> Vec<&String> {
        self.available_peers.keys().collect()
    }

    fn log(&mut self, entry: (String, &'a str)) {
        if self.log_book.len() > 40 {
            self.log_book.pop().unwrap();
        }
        self.log_book.insert(0, entry);
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (pk, sk) = generate_longterm_keypair();
    // Terminal initialization
    let stdout = io::stdout().into_raw_mode()?;
    let stdout = MouseTerminal::from(stdout);
    let stdout = AlternateScreen::from(stdout);
    let backend = TermionBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.hide_cursor()?;

    let (cm_tx, cm_rx) = mpsc::channel::<PeerManagerEvent>();
    let mut peer_manager = PeerManager::new(cm_tx);

    let events = Events::new(cm_rx);

    // App
    let mut app = App::new();

    loop {
        terminal.draw(|mut f| {
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
                .split(f.size());

            let style = Style::default().fg(Color::Black).bg(Color::White);
            SelectableList::default()
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Available Peers"),
                )
                .items(&app.peer_list())
                .select(app.selected)
                .style(style)
                .highlight_style(style.fg(Color::LightGreen).modifier(Modifier::BOLD))
                .highlight_symbol(">")
                .render(&mut f, chunks[0]);
            {
                let log_book = app.log_book.iter().map(|(evt, level)| {
                    Text::styled(
                        format!("{}: {}", level, evt),
                        match *level {
                            "NEW PEER" => app.error_style,
                            "ERROR" => app.critical_style,
                            "DEBUG" => app.warning_style,
                            _ => app.info_style,
                        },
                    )
                });
                List::new(log_book)
                    .block(Block::default().borders(Borders::ALL).title("Debug Log"))
                    .start_corner(Corner::BottomLeft)
                    .render(&mut f, chunks[1]);
            }
        })?;

        match events.next()? {
            Event::Input(input) => match input {
                Key::Char('q') => {
                    break;
                }
                Key::Left => {
                    app.selected = None;
                }
                Key::Down => {
                    app.selected = if let Some(selected) = app.selected {
                        if selected >= app.available_peers.len() - 1 {
                            Some(0)
                        } else {
                            Some(selected + 1)
                        }
                    } else {
                        Some(0)
                    }
                }
                Key::Up => {
                    app.selected = if let Some(selected) = app.selected {
                        if selected > 0 {
                            Some(selected - 1)
                        } else {
                            Some(app.available_peers.len() - 1)
                        }
                    } else {
                        Some(0)
                    }
                }
                Key::Char('\n') => {
                    if let Some(selected) = app.selected {
                        let feed_id = app.peer_list()[selected];
                        let ssb_peer = app.available_peers.get(feed_id).unwrap();

                        peer_manager.init_handshake(pk.clone(), sk.clone(), ssb_peer.clone());
                    } else {
                        app.log((
                            "No peer selected. Cannot initialize handshake".to_string(),
                            "ERROR",
                        ));
                    }
                }
                _ => {}
            },
            Event::Tick => {
                //let peer_str = "found da peer";
                //app.advance();
            }
            Event::NewPeer(ssb_peer) => {
                let peer_str = format!("{}", ssb_peer);
                app.available_peers
                    .insert(ssb_peer.feed_id(), Arc::new(ssb_peer));
                app.log((peer_str, "NEW PEER"));
            }
            Event::PeerManagerEvent(pm_event) => match pm_event.event {
                PeerEvent::HandshakeResult(Ok(hs_keys)) => {
                    app.log(("Succeeded in handshake".to_string(), "DEBUG"));
                    let read_key = base64::encode(&hs_keys.read_key[..]);
                    let write_key = base64::encode(&hs_keys.write_key[..]);
                    app.log((format!("Peer {:?}", pm_event.peer.feed_id()), "DEBUG"));
                    app.log((format!("Read key {:?}", read_key), "DEBUG"));
                    app.log((format!("Write key {:?}", write_key), "DEBUG"))
                }
                PeerEvent::HandshakeResult(Err(e)) => {
                    let desc = format!("{}", e);
                    app.log((desc, "ERROR"));
                }
                PeerEvent::ConnectionClosed(reason) => match reason {
                    Ok(()) => app.log(("Quietly Terminated".to_string(), "CONN CLOSED")),
                    Err(e) => app.log((format!("Error ({})", e), "CONN CLOSED")),
                },
                PeerEvent::MessageReceived(peer_msg) => {
                    app.log((peer_msg, "PEER MSG"));
                }
                PeerEvent::ConnectionReady(tcp_stream) => {
                    app.log(("ya".to_string(), "CONN READY"));
                }
            },
        }
    }
    Ok(())
}
