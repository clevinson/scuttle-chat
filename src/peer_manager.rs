use crate::discovery::{PeerAddr, PEER_DISCOVERY_PORT};
use crate::peer_connection::{Handshaker, PeerConnection};
use ssb_crypto::{NetworkKey, PublicKey, SecretKey};
use std::io;
use std::net::TcpListener;
use std::sync::mpsc;
use std::thread;

const HANDSHAKE_NETWORK_KEY: NetworkKey = NetworkKey::SSB_MAIN_NET;
pub const HANDSHAKE_LISTENER_PORT: u16 = PEER_DISCOVERY_PORT;

pub struct PeerManager {
    event_bus: mpsc::Sender<PeerManagerEvent>,
    handshake_listener: Option<thread::JoinHandle<io::Result<()>>>,
    handshaker: Handshaker,
    pub connections: Vec<PeerConnection>,
}

type PeerMsg = String;

pub struct PeerManagerEvent {
    pub peer: PeerAddr,
    pub event: PeerEvent,
}

pub enum PeerEvent {
    HandshakeSuccessful(PeerConnection),
    MessageReceived(PeerMsg),

    // need to implement again when the
    // ConnectionClosed event gets called
    ConnectionClosed(io::Result<()>),
}

impl PeerManager {
    pub fn new(
        ssb_public_key: PublicKey,
        ssb_secret_key: SecretKey,
        event_bus: mpsc::Sender<PeerManagerEvent>,
    ) -> PeerManager {
        let handshaker = Handshaker::new(
            event_bus.clone(),
            ssb_public_key,
            ssb_secret_key,
            HANDSHAKE_NETWORK_KEY,
        );

        PeerManager {
            event_bus,
            handshake_listener: None,
            handshaker,
            connections: Vec::new(),
        }
    }

    pub fn start_listener(&mut self) -> io::Result<()> {
        let hs_listener_socket_addr = format!("0.0.0.0:{}", HANDSHAKE_LISTENER_PORT);
        let listener = TcpListener::bind(hs_listener_socket_addr)?;

        let hs = self.handshaker.clone();
        let event_bus = self.event_bus.clone();

        let listener_handle = thread::spawn(move || -> io::Result<()> {
            for stream in listener.incoming() {
                let peer_connection = hs.server_handshake(stream?)?;

                event_bus.send(PeerManagerEvent {
                    peer: peer_connection.peer,
                    event: PeerEvent::HandshakeSuccessful(peer_connection),
                });
            }
            Ok(())
        });

        self.handshake_listener = Some(listener_handle);

        Ok(())
    }

    pub fn init_connection(&self, peer: PeerAddr) -> thread::JoinHandle<io::Result<()>> {
        let hs = self.handshaker.clone();
        let event_bus = self.event_bus.clone();

        thread::spawn(move || -> io::Result<()> {
            let peer_connection = hs.client_handshake(peer)?;

            event_bus.send(PeerManagerEvent {
                peer: peer_connection.peer,
                event: PeerEvent::HandshakeSuccessful(peer_connection),
            });
            Ok(())
        })
    }
}
