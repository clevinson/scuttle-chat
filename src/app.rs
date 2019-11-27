use crate::chat::{ChatMsg, ChatSender};
use crate::event::{Event, Events};
use crate::net::SsbPeer;
use crate::peer_manager::{PeerEvent, PeerManager, PeerManagerEvent};
use crate::ui::draw;
use ssb_crypto::generate_longterm_keypair;
use std::collections::HashMap;
use std::error::Error;
use std::sync::mpsc;
use std::sync::Arc;
use termion::event::Key;
use tui::backend::Backend;
use tui::style::{Color, Style};
use tui::Terminal;

pub struct App<'a> {
    pub available_peers: HashMap<String, Arc<SsbPeer>>,
    pub selected: Option<usize>,
    pub chats: HashMap<String, Vec<ChatMsg>>,
    pub debug_log: Vec<(String, &'a str)>,
    pub info_style: Style,
    pub warning_style: Style,
    pub error_style: Style,
    pub critical_style: Style,
    pub events: Events,
    pub peer_manager: PeerManager,
}

impl<'a> App<'a> {
    pub fn new() -> App<'a> {
        let (cm_tx, cm_rx) = mpsc::channel::<PeerManagerEvent>();
        let peer_manager = PeerManager::new(cm_tx);

        let event_listener = Events::new(cm_rx);

        App {
            available_peers: HashMap::new(),
            chats: HashMap::new(),
            selected: None,
            debug_log: Vec::new(),
            info_style: Style::default().fg(Color::White),
            warning_style: Style::default().fg(Color::Yellow),
            error_style: Style::default().fg(Color::Magenta),
            critical_style: Style::default().fg(Color::Red),
            events: event_listener,
            peer_manager,
        }
    }

    pub fn peer_list(&self) -> Vec<&String> {
        self.available_peers.keys().collect()
    }

    fn log(&mut self, entry: (String, &'a str)) {
        if self.debug_log.len() > 15 {
            self.debug_log.remove(0);
        }
        self.debug_log.push(entry);
    }

    pub fn run<B: Backend>(
        &mut self,
        mut terminal: &mut Terminal<B>,
    ) -> Result<(), Box<dyn Error>> {
        let (pk, sk) = generate_longterm_keypair();

        loop {
            draw(&mut terminal, &self)?;
            match self.events.next()? {
                Event::Input(input) => match input {
                    Key::Char('q') => {
                        break;
                    }
                    Key::Left => {
                        self.selected = None;
                    }
                    Key::Down => {
                        self.selected = if let Some(selected) = self.selected {
                            if selected >= self.available_peers.len() - 1 {
                                Some(0)
                            } else {
                                Some(selected + 1)
                            }
                        } else {
                            Some(0)
                        }
                    }
                    Key::Up => {
                        self.selected = if let Some(selected) = self.selected {
                            if selected > 0 {
                                Some(selected - 1)
                            } else {
                                Some(self.available_peers.len() - 1)
                            }
                        } else {
                            Some(0)
                        }
                    }
                    Key::Char('\n') => {
                        if let Some(selected) = self.selected {
                            let feed_id = self.peer_list()[selected];
                            let ssb_peer = self.available_peers.get(feed_id).unwrap();

                            self.peer_manager.init_handshake(
                                pk.clone(),
                                sk.clone(),
                                ssb_peer.clone(),
                            );
                        } else {
                            self.log((
                                "No peer selected. Cannot initialize handshake".to_string(),
                                "ERROR",
                            ));
                        }
                    }
                    _ => {}
                },
                Event::Tick => {
                    //let peer_str = "found da peer";
                    //self.advance();
                }
                Event::NewPeer(ssb_peer) => {
                    let peer_str = format!("{}", ssb_peer);
                    self.available_peers
                        .insert(ssb_peer.feed_id(), Arc::new(ssb_peer));
                    self.log((peer_str, "NEW PEER"));
                }
                Event::PeerManagerEvent(pm_event) => match pm_event.event {
                    PeerEvent::HandshakeSuccessful => {
                        let msg = vec![
                            ChatMsg {
                                sender: ChatSender::Info,
                                message: "Succeeded in handshake!".to_string(),
                            },
                            ChatMsg {
                                sender: ChatSender::Info,
                                message: format!(
                                    "Now connected to {} via encrypted BoxStream",
                                    pm_event.peer.feed_id()
                                ),
                            },
                        ];

                        match self.chats.get_mut(&pm_event.peer.feed_id()) {
                            Some(chat) => {
                                chat.extend(msg);
                            }
                            None => {
                                self.chats.insert(pm_event.peer.feed_id(), msg);
                            }
                        };
                    }
                    PeerEvent::ConnectionClosed(reason) => {
                        if let Some(chat) = self.chats.get_mut(&pm_event.peer.feed_id()) {
                            chat.push(ChatMsg {
                                sender: ChatSender::Info,
                                message: match reason {
                                    Ok(()) => "Connection Closed -- Goodbye!".to_string(),
                                    Err(e) => format!("Connection Closed –– Error ({})", e),
                                },
                            });
                        }
                    }
                    PeerEvent::MessageReceived(peer_msg) => {
                        if let Some(chat) = self.chats.get_mut(&pm_event.peer.feed_id()) {
                            chat.push(ChatMsg {
                                sender: ChatSender::Peer(pm_event.peer.feed_id()),
                                message: peer_msg,
                            });
                        }
                    }
                    //PeerEvent::ConnectionReady(_tcp_stream) => {
                    //    self.log(("Connection established".to_string(), "CONN READY"));
                    //}
                },
            }
        }
        Ok(())
    }
}
