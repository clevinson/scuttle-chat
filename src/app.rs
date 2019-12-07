use crate::chat::{ChatMsg, ChatSender, FeedId, PeerChat};
use crate::discovery::PeerAddr;
use crate::event::{Event, Events};
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
    pub available_peers: HashMap<FeedId, Arc<PeerAddr>>,
    pub selected: Option<usize>,
    pub peer_chats: HashMap<FeedId, PeerChat>,
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
        let (pm_tx, pm_rx) = mpsc::channel::<PeerManagerEvent>();
        let (pk, sk) = generate_longterm_keypair();
        let peer_manager = PeerManager::new(pk.clone(), sk.clone(), pm_tx);

        let event_listener = Events::new(pk, pm_rx);

        App {
            available_peers: HashMap::new(),
            peer_chats: HashMap::new(),
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

    pub fn selected_chat(&self) -> Option<&PeerChat> {
        match self.selected {
            Some(selected_idx) => {
                let feed_id = self.peer_list()[selected_idx].clone();

                self.peer_chats.get(&feed_id)
            }
            None => None,
        }
    }

    fn selected_chat_mut(&mut self) -> Option<&mut PeerChat> {
        match self.selected {
            Some(selected_idx) => {
                let feed_id = self.peer_list()[selected_idx].clone();

                self.peer_chats.get_mut(&feed_id)
            }
            None => None,
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
        self.peer_manager.start_listener()?;

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
                        } else if !self.available_peers.is_empty() {
                            Some(0)
                        } else {
                            None
                        }
                    }
                    Key::Up => {
                        self.selected = if let Some(selected) = self.selected {
                            if selected > 0 {
                                Some(selected - 1)
                            } else {
                                Some(self.available_peers.len() - 1)
                            }
                        } else if !self.available_peers.is_empty() {
                            Some(0)
                        } else {
                            None
                        }
                    }
                    Key::Char('\n') => {
                        if let Some(selected) = self.selected {
                            let feed_id = self.peer_list()[selected].clone();
                            let ssb_peer = self.available_peers.get(&feed_id).unwrap();

                            match self.peer_chats.get_mut(&feed_id) {
                                Some(peer_chat) => match &peer_chat.peer_tx {
                                    Some(tx) => {
                                        tx.send(peer_chat.input.clone())?;
                                        peer_chat.messages.push(ChatMsg {
                                            sender: ChatSender::_You,
                                            message: peer_chat.input.clone(),
                                        });
                                        peer_chat.input = "".to_string();
                                    }
                                    None => {
                                        peer_chat.messages.push(ChatMsg {
                                            sender: ChatSender::Info,
                                            message: "Cannot send message, broken connetion?"
                                                .to_string(),
                                        });
                                    }
                                },
                                None => {
                                    // No peer_chat initiated, so we should handshake,
                                    // which on "success" will initialiae a peer_chat
                                    // struct
                                    self.peer_manager.init_connection(**ssb_peer);
                                }
                            };

                        // implement something later to poll errors from join handles
                        // this is the only way we'll be able to handle TCP timeouts
                        // and similar errors from handshakes
                        } else {
                            self.log((
                                "No peer selected. Cannot initialize handshake".to_string(),
                                "ERROR",
                            ));
                        }
                    }
                    Key::Char(c) => {
                        if let Some(chat) = self.selected_chat_mut() {
                            chat.input.push(c);
                        }
                    }
                    Key::Backspace => {
                        if let Some(chat) = self.selected_chat_mut() {
                            chat.input.pop();
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
                    self.log((peer_str, "ANN"));
                }
                Event::PeerManagerEvent(pm_event) => match pm_event.event {
                    PeerEvent::HandshakeSuccessful(peer_connection) => {
                        let msgs = vec![
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

                        let box_writer = peer_connection.box_writer_tx.clone();

                        match self.peer_chats.get_mut(&pm_event.peer.feed_id()) {
                            // should check if peer_tx is already set, and handle
                            // gracefully (fail to set new handshake connection, or
                            // check prior peer_tx to see if it still is valid)
                            Some(chat) => {
                                chat.messages.extend(msgs);
                                chat.peer_tx = Some(box_writer);
                            }
                            None => {
                                self.peer_chats.insert(
                                    pm_event.peer.feed_id(),
                                    PeerChat {
                                        messages: msgs,
                                        input: "".to_string(),
                                        peer_tx: Some(box_writer),
                                    },
                                );
                            }
                        };
                        self.peer_manager.connections.push(peer_connection);
                    }
                    PeerEvent::ConnectionClosed(reason) => {
                        if let Err(e) = &reason {
                            self.log((format!("Connection Closed –– Error ({})", e), "ERROR"));
                        }
                        if let Some(chat) = self.peer_chats.get_mut(&pm_event.peer.feed_id()) {
                            chat.messages.push(ChatMsg {
                                sender: ChatSender::Info,
                                message: match reason {
                                    Ok(()) => "Connection Closed -- Goodbye!".to_string(),
                                    Err(e) => format!("Connection Closed –– Error ({})", e),
                                },
                            });
                            chat.peer_tx = None;
                        }
                    }
                    PeerEvent::MessageReceived(peer_msg) => {
                        if let Some(chat) = self.peer_chats.get_mut(&pm_event.peer.feed_id()) {
                            chat.messages.push(ChatMsg {
                                sender: ChatSender::Peer(pm_event.peer.feed_id()),
                                message: peer_msg,
                            });
                        }
                    }
                },
            }
        }
        Ok(())
    }
}
