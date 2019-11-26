use crate::net::SsbPeer;
use hex;
use ssb_crypto::handshake::HandshakeKeys;
use ssb_crypto::{generate_longterm_keypair, NetworkKey, PublicKey, SecretKey};
use ssb_handshake::HandshakeError;
use std::collections::HashMap;
use std::io;
use std::io::Read;
use std::net::TcpStream;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{mpsc, Arc};
use std::thread;

use crate::box_stream::BoxReader;

pub struct PeerManager {
    event_bus: mpsc::Sender<PeerManagerEvent>,
    peer_connections: HashMap<String, mpsc::Sender<PeerMsg>>,
}

type HandshakeResult = Result<HandshakeKeys, HandshakeError>;
type PeerMsg = String;

pub struct PeerManagerEvent {
    pub peer: Arc<SsbPeer>,
    pub event: PeerEvent,
}

pub enum PeerEvent {
    HandshakeResult(HandshakeResult),
    ConnectionClosed(io::Result<()>),
    MessageReceived(PeerMsg),
    ConnectionReady(TcpStream),
}

impl PeerManager {
    pub fn new(event_bus: mpsc::Sender<PeerManagerEvent>) -> PeerManager {
        let mut peer_connections = HashMap::new();
        PeerManager {
            event_bus,
            peer_connections,
        }
    }

    pub fn init_handshake(
        &mut self,
        pk: PublicKey,
        sk: SecretKey,
        peer: Arc<SsbPeer>,
    ) -> thread::JoinHandle<()> {
        let event_bus = self.event_bus.clone();

        let (peer_tx, peer_rx) = mpsc::channel::<PeerMsg>();

        self.peer_connections.insert(peer.feed_id(), peer_tx);

        thread::spawn(move || {
            let init_connection = || {
                let net_key = NetworkKey::SSB_MAIN_NET;
                let server_pk = peer.public_key;

                let mut tcp_stream = TcpStream::connect_timeout(
                    &peer.socket_addr,
                    std::time::Duration::from_millis(500),
                )?;
                let hs_keys = ssb_handshake::client(&mut tcp_stream, net_key, pk, sk, server_pk)?;

                //event_bus.send(PeerManagerEvent {
                //    event: PeerEvent::HandshakeResult(Ok(hs_keys)),
                //    peer: peer.clone(),
                //});

                let key = hs_keys.read_key;
                let noncegen = hs_keys.read_noncegen;

                let mut box_reader = BoxReader::new(tcp_stream, key, noncegen);

                loop {
                    let maybe_bytes = box_reader.recv()?;

                    let peer_msg = match maybe_bytes {
                        Some(raw_bytes) => String::from_utf8(raw_bytes.clone()).unwrap_or(
                            format!("Raw bytes: {:?}", raw_bytes)),
                        None => "Goodbye!".to_string(),
                    };

                    event_bus.send(PeerManagerEvent {
                        event: PeerEvent::MessageReceived(peer_msg),
                        peer: peer.clone(),
                    });
                    //let mut header = [0; 34];
                    //tcp_stream.read_exact(&mut header)?;

                    //event_bus.send(PeerManagerEvent {
                    //    event: PeerEvent::MessageReceived(format!("Found {} bytes", header.len())),
                    //    peer: peer.clone(),
                    //});

                    //let mut body = Vec::new();

                    //let num_bytes = tcp_stream.read_to_end(&mut body)?;

                    //event_bus.send(PeerManagerEvent {
                    //    event: PeerEvent::MessageReceived(format!("Found in body {} bytes", num_bytes)),
                    //    peer: peer.clone(),
                    //});
                }
            };

            let conn_result = init_connection();

            event_bus
                .send(PeerManagerEvent {
                    event: PeerEvent::ConnectionClosed(conn_result),
                    peer: peer.clone(),
                })
                .unwrap();

            //let buf_str = hex::encode(buf);

            //event_bus.send(PeerManagerEvent {
            //    event: PeerEvent::MessageReceived(format!("0x{}", buf_str)),
            //    peer: peer.clone(),
            //});

            // maybe use Arc<TcpStream> ?
            // instead of peer_rx ?
            // would be nice to only have one loop in this thread
            // use just for reading from this peer's TCP stream,
            // and fwding messages to the event bus
            // this way, the event bus itself can have direct access
            // to writing to this TCP stream (via an Arc or mutex lock?)
        })
    }
}
