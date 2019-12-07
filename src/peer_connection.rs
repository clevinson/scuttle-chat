use crate::discovery::{PeerAddr, Protocol};
use ssb_crypto::handshake::HandshakeKeys;
use ssb_crypto::{NetworkKey, PublicKey, SecretKey};
use ssb_handshake::HandshakeError;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::mpsc;
use std::{io, thread};

use crate::box_stream::{BoxReader, BoxWriter};
use crate::peer_manager::{PeerEvent, PeerManagerEvent};

type PeerMsg = String;

pub struct PeerConnection {
    pub peer: PeerAddr,
    pub box_writer_tx: mpsc::Sender<PeerMsg>,
    _box_reader_handle: BoxReaderHandle,
    _box_writer_handle: BoxWriterHandle,
}

type BoxWriterHandle = thread::JoinHandle<Result<(), mpsc::RecvError>>;
type BoxReaderHandle = thread::JoinHandle<Result<(), io::Error>>;

fn spawn_reader_loop<R>(
    tx: mpsc::Sender<PeerManagerEvent>,
    peer: PeerAddr,
    mut box_reader: BoxReader<R>,
) -> BoxReaderHandle
where
    R: Read + Send + 'static,
{
    thread::spawn(move || -> Result<(), io::Error> {
        loop {
            let maybe_bytes = box_reader.recv()?;

            let peer_msg = match maybe_bytes {
                Some(raw_bytes) => String::from_utf8(raw_bytes.clone())
                    .unwrap_or(format!("Raw bytes: {:?}", raw_bytes)),
                None => "Goodbye!".to_string(),
            };

            tx.send(PeerManagerEvent {
                peer,
                event: PeerEvent::MessageReceived(peer_msg),
            });
        }
    })
}

fn spawn_writer_loop<W>(mut box_writer: BoxWriter<W>) -> (mpsc::Sender<PeerMsg>, BoxWriterHandle)
where
    W: Write + Send + 'static,
{
    let (tx, rx) = mpsc::channel::<PeerMsg>();
    let handle: BoxWriterHandle = thread::spawn(move || -> Result<(), mpsc::RecvError> {
        loop {
            let peer_msg = rx.recv().map(String::into_bytes)?;
            // should "?" and have a BoxWriterHandleError
            box_writer.send(peer_msg).unwrap();
        }
    });

    (tx, handle)
}

impl PeerConnection {
    pub fn from_handshake<F>(
        event_bus: mpsc::Sender<PeerManagerEvent>,
        mut tcp_stream: TcpStream,
        perform_handshake: F,
    ) -> io::Result<PeerConnection>
    where
        F: Fn(&mut TcpStream) -> Result<(PeerAddr, HandshakeKeys), HandshakeError> + Send + 'static,
    {
        let (peer, hs_keys) = perform_handshake(&mut tcp_stream)?;

        let write_stream = tcp_stream.try_clone()?;
        let mut box_writer =
            BoxWriter::new(write_stream, hs_keys.write_key, hs_keys.write_noncegen);
        let (box_writer_tx, _box_writer_handle) = spawn_writer_loop(box_writer);

        let mut box_reader = BoxReader::new(tcp_stream, hs_keys.read_key, hs_keys.read_noncegen);
        let _box_reader_handle = spawn_reader_loop(event_bus.clone(), peer.clone(), box_reader);

        let peer_connection = PeerConnection {
            peer,
            box_writer_tx,
            _box_reader_handle,
            _box_writer_handle,
        };

        Ok(peer_connection)
    }
}

#[derive(Clone)]
pub struct Handshaker {
    event_bus: mpsc::Sender<PeerManagerEvent>,
    public_key: PublicKey,
    secret_key: SecretKey,
    network_key: NetworkKey,
}

impl Handshaker {
    pub fn new(
        event_bus: mpsc::Sender<PeerManagerEvent>,
        public_key: PublicKey,
        secret_key: SecretKey,
        network_key: NetworkKey,
    ) -> Handshaker {
        Handshaker {
            event_bus,
            public_key,
            secret_key,
            network_key,
        }
    }

    pub fn client_handshake(&self, peer: PeerAddr) -> io::Result<PeerConnection> {
        let tcp_stream =
            TcpStream::connect_timeout(&peer.socket_addr, std::time::Duration::from_millis(500))
                .unwrap();

        let config = self.clone();

        PeerConnection::from_handshake(self.event_bus.clone(), tcp_stream, move |stream| {
            let keys = ssb_handshake::client(
                stream,
                config.network_key.clone(),
                config.public_key,
                config.secret_key.clone(),
                peer.public_key,
            )?;
            Ok((peer.clone(), keys))
        })
    }

    pub fn server_handshake(&self, stream: TcpStream) -> io::Result<PeerConnection> {
        let config = self.clone();

        PeerConnection::from_handshake(self.event_bus.clone(), stream, move |stream| {
            let client_addr = stream.peer_addr()?;

            let (client_pk, keys) = ssb_handshake::server_with_client_pk(
                stream,
                config.network_key.clone(),
                config.public_key,
                config.secret_key.clone(),
            )?;

            let peer = PeerAddr {
                public_key: client_pk,
                socket_addr: client_addr,
                protocol: Protocol::Net,
            };

            Ok((peer, keys))
        })
    }
}
