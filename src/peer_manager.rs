use crate::discovery::{PeerAddr, Protocol, PEER_DISCOVERY_PORT};
//use ssb_crypto::handshake::HandshakeKeys;
use ssb_crypto::handshake::HandshakeKeys;
use ssb_crypto::{NetworkKey, PublicKey, SecretKey};
use ssb_handshake::HandshakeError;
use std::collections::HashMap;
use std::io;
use std::net::{TcpListener, TcpStream};
use std::sync::{mpsc, Arc};
use std::thread;

use crate::box_stream::{BoxReader, BoxWriter};

const HANDSHAKE_NETWORK_KEY: NetworkKey = NetworkKey::SSB_MAIN_NET;
pub const HANDSHAKE_LISTENER_PORT: u16 = PEER_DISCOVERY_PORT;

pub struct PeerManager {
    event_bus: mpsc::Sender<PeerManagerEvent>,
    handshake_listener: Option<thread::JoinHandle<io::Result<()>>>,
    pub ssb_public_key: PublicKey,
    ssb_secret_key: SecretKey,
}

type PeerMsg = String;

pub struct PeerManagerEvent {
    pub peer: Arc<PeerAddr>,
    pub event: PeerEvent,
}

pub enum PeerEvent {
    HandshakeSuccessful(mpsc::Sender<PeerMsg>),
    ConnectionClosed(io::Result<()>),
    MessageReceived(PeerMsg),
    NewConnection,
    //ConnectionReady(TcpStream),
}

impl PeerManager {
    pub fn new(
        ssb_public_key: PublicKey,
        ssb_secret_key: SecretKey,
        event_bus: mpsc::Sender<PeerManagerEvent>,
    ) -> PeerManager {
        PeerManager {
            event_bus,
            handshake_listener: None,
            ssb_public_key,
            ssb_secret_key,
        }
    }

    pub fn start_listener(&mut self) -> io::Result<()> {
        let hs_listener_socket_addr = format!("0.0.0.0:{}", HANDSHAKE_LISTENER_PORT);

        let listener = TcpListener::bind(hs_listener_socket_addr)?;
        let event_bus = self.event_bus.clone();

        let (pk, sk) = (self.ssb_public_key, self.ssb_secret_key.clone());

        let listener_handle = thread::spawn(move || -> io::Result<()> {
            for stream in listener.incoming() {
                let sk = sk.clone();
                let event_bus = event_bus.clone();

                init_chat_handle(event_bus.clone(), stream?, move |stream| {
                    let client_addr = stream.peer_addr()?;

                    let (client_pk, keys) = ssb_handshake::server_with_client_pk(
                        stream,
                        HANDSHAKE_NETWORK_KEY,
                        pk,
                        sk.clone(),
                    )?;

                    let peer = Arc::new(PeerAddr {
                        public_key: client_pk,
                        socket_addr: client_addr,
                        protocol: Protocol::Net,
                    });

                    event_bus.send(PeerManagerEvent {
                        event: PeerEvent::NewConnection,
                        peer: peer.clone(),
                    });


                    Ok((peer, keys))
                });
            }
            Ok(())
        });

        self.handshake_listener = Some(listener_handle);

        Ok(())
    }

    pub fn init_handshake(
        &mut self,
        peer: Arc<PeerAddr>,
    ) -> thread::JoinHandle<Result<(), HandshakeError>> {
        let event_bus = self.event_bus.clone();

        let tcp_stream =
            TcpStream::connect_timeout(&peer.socket_addr, std::time::Duration::from_millis(500))
                .unwrap();

        let pk = self.ssb_public_key;
        let sk = self.ssb_secret_key.clone();

        init_chat_handle(event_bus, tcp_stream, move |stream| {
            let keys = ssb_handshake::client(
                stream,
                HANDSHAKE_NETWORK_KEY,
                pk,
                sk.clone(),
                peer.public_key,
            )?;
            Ok((peer.clone(), keys))
        })
    }
}

fn init_chat_handle<F>(
    event_bus: mpsc::Sender<PeerManagerEvent>,
    mut tcp_stream: TcpStream,
    attempt_handshake: F,
) -> thread::JoinHandle<Result<(), HandshakeError>>
where
    F: Fn(&mut TcpStream) -> Result<(Arc<PeerAddr>, HandshakeKeys), HandshakeError>
        + Send
        + 'static,
{
    let (box_writer_tx, box_writer_rx) = mpsc::channel::<PeerMsg>();

    thread::spawn(move || {
        let (peer, hs_keys) = attempt_handshake(&mut tcp_stream)?;

        let tcp_stream = Arc::new(tcp_stream);

        let init_connection = || {
            // maybe change HandshakeSuccessful event to pass along
            // a BoxWriter, instead
            event_bus
                .send(PeerManagerEvent {
                    event: PeerEvent::HandshakeSuccessful(box_writer_tx),
                    peer: peer.clone(),
                })
                .unwrap();

            let write_stream = tcp_stream.clone();
            let write_key = hs_keys.write_key;
            let write_noncegen = hs_keys.write_noncegen;

            // spawn new thread for polling for user's messages
            // that should get sent to the remote peer
            //
            // =>> (this could be avoided if we just pass the
            // =>>  BoxWriter through HandshakeSuccessful event)
            //
            let _peer_tx_handle = thread::spawn(move || -> Result<(), mpsc::RecvError> {
                let mut box_writer = BoxWriter::new(&*write_stream, write_key, write_noncegen);

                loop {
                    let peer_msg = box_writer_rx.recv()?;
                    let bytes = peer_msg.as_bytes();
                    box_writer.send(bytes.to_vec()).unwrap();
                }
            });

            let mut box_reader =
                BoxReader::new(&*tcp_stream, hs_keys.read_key, hs_keys.read_noncegen);

            // loop in this thread, over any new messages from the remote peer
            // and forward them to the main event loop
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
        Ok(())
    })
}
