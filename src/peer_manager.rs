use crate::net::SsbPeer;
//use ssb_crypto::handshake::HandshakeKeys;
use ssb_crypto::{NetworkKey, PublicKey, SecretKey};
//use ssb_handshake::HandshakeError;
use std::collections::HashMap;
use std::io;
use std::net::TcpStream;
use std::sync::{mpsc, Arc};
use std::thread;

use crate::box_stream::{BoxReader, BoxWriter};

pub struct PeerManager {
    event_bus: mpsc::Sender<PeerManagerEvent>,
}

//type HandshakeResult = Result<HandshakeKeys, HandshakeError>;
type PeerMsg = String;

pub struct PeerManagerEvent {
    pub peer: Arc<SsbPeer>,
    pub event: PeerEvent,
}

pub enum PeerEvent {
    HandshakeSuccessful(mpsc::Sender<PeerMsg>),
    ConnectionClosed(io::Result<()>),
    MessageReceived(PeerMsg),
    //ConnectionReady(TcpStream),
}

impl PeerManager {
    pub fn new(event_bus: mpsc::Sender<PeerManagerEvent>) -> PeerManager {
        PeerManager { event_bus }
    }


    pub fn init_handshake(
        &mut self,
        pk: PublicKey,
        sk: SecretKey,
        peer: Arc<SsbPeer>,
    ) -> thread::JoinHandle<()> {
        let event_bus = self.event_bus.clone();

        let (peer_tx, peer_rx) = mpsc::channel::<PeerMsg>();

        thread::spawn(move || {
            let init_connection = || {
                let net_key = NetworkKey::SSB_MAIN_NET;
                let server_pk = peer.public_key;

                let mut tcp_stream = TcpStream::connect_timeout(
                    &peer.socket_addr,
                    std::time::Duration::from_millis(500),
                )?;

                let hs_keys = ssb_handshake::client(&mut tcp_stream, net_key, pk, sk, server_pk)?;

                let tcp_stream = Arc::new(tcp_stream);

                event_bus
                    .send(PeerManagerEvent {
                        event: PeerEvent::HandshakeSuccessful(peer_tx),
                        peer: peer.clone(),
                    })
                    .unwrap();

                let write_stream = tcp_stream.clone();
                let write_key = hs_keys.write_key;
                let write_noncegen = hs_keys.write_noncegen;

                let _peer_tx_handle: thread::JoinHandle<Result<(), mpsc::RecvError>> =
                    thread::spawn(move || {
                        let mut box_writer =
                            BoxWriter::new(&*write_stream, write_key, write_noncegen);

                        loop {
                            let peer_msg = peer_rx.recv()?;
                            let bytes = peer_msg.as_bytes();

                            box_writer.send(bytes.to_vec()).unwrap();
                        }
                    });

                let mut box_reader =
                    BoxReader::new(&*tcp_stream, hs_keys.read_key, hs_keys.read_noncegen);

                loop {
                    let maybe_bytes = box_reader.recv()?;

                    let peer_msg = match maybe_bytes {
                        Some(raw_bytes) => String::from_utf8(raw_bytes.clone())
                            .unwrap_or(format!("Raw bytes: {:?}", raw_bytes)),
                        None => "Goodbye!".to_string(),
                    };

                    event_bus
                        .send(PeerManagerEvent {
                            event: PeerEvent::MessageReceived(peer_msg),
                            peer: peer.clone(),
                        })
                        .unwrap();
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
