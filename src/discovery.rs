use base64;
use net2::unix::UnixUdpBuilderExt;
use net2::UdpBuilder;
use nix::sys::socket::{InetAddr, SockAddr};
use regex::Regex;
use ssb_crypto::PublicKey;
use std::error;
use std::fmt;
use std::io;
use std::net::{SocketAddr, UdpSocket};
use std::str::FromStr;
use std::thread;
use std::time::Duration;

use crate::peer_manager::HANDSHAKE_LISTENER_PORT;

pub const PEER_DISCOVERY_PORT: u16 = 45982;

#[derive(Debug, Clone)]
pub struct PeerAddr {
    pub protocol: Protocol,
    pub socket_addr: SocketAddr,
    pub public_key: PublicKey,
}

impl fmt::Display for PeerAddr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let feed_id = base64::encode(&self.public_key.0);
        let protocol = match &self.protocol {
            Protocol::WebSocket => "ws://",
            Protocol::Net => "net:",
        };

        write!(f, "{}{}~shs:{}", protocol, &self.socket_addr, feed_id)
    }
}

#[derive(Debug, Clone)]
pub enum Protocol {
    WebSocket,
    Net,
}

impl PeerAddr {
    pub fn feed_id(&self) -> String {
        let encoded_bytes = base64::encode(&self.public_key.0);
        format!("@{}.ed25519", encoded_bytes)
    }
}

#[derive(Debug)]
pub struct ParsePeerAddrError();

impl fmt::Display for ParsePeerAddrError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Failed to parse PeerAddr string")
    }
}

// This is important for other errors to wrap this one.
impl error::Error for ParsePeerAddrError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        // Generic error, underlying cause isn't tracked.
        None
    }
}

impl FromStr for PeerAddr {
    type Err = ParsePeerAddrError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let re = Regex::new(r"(ws://|net:)(.*?)~shs:(.*)").unwrap();

        let groups = re.captures(s).ok_or(ParsePeerAddrError())?;

        let protocol = match &groups[1] {
            "ws://" => Some(Protocol::WebSocket),
            "net:" => Some(Protocol::Net),
            _ => None,
        }
        .ok_or(ParsePeerAddrError())?;

        let socket_addr: SocketAddr = groups[2].parse().map_err(|_| ParsePeerAddrError())?;
        let pk_bytes = base64::decode(&groups[3]).map_err(|_| ParsePeerAddrError())?;
        let public_key = PublicKey::from_slice(&pk_bytes).ok_or(ParsePeerAddrError())?;

        Ok(PeerAddr {
            protocol,
            socket_addr,
            public_key,
        })
    }
}

#[derive(Debug)]
pub enum DiscoveryServiceError {
    ParseError(ParsePeerAddrError),
    GetLocalAddrError,
    IoError(std::io::Error),
}

impl fmt::Display for DiscoveryServiceError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            DiscoveryServiceError::ParseError(e) => fmt::Display::fmt(e, f),
            DiscoveryServiceError::IoError(e) => fmt::Display::fmt(e, f),
            DiscoveryServiceError::GetLocalAddrError => {
                fmt::Display::fmt("Failed to determine local IP Address", f)
            }
        }
    }
}

// This is important for other errors to wrap this one.
impl error::Error for DiscoveryServiceError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            DiscoveryServiceError::ParseError(e) => Some(e),
            DiscoveryServiceError::IoError(e) => Some(e),
            DiscoveryServiceError::GetLocalAddrError => None,
        }
    }
}

impl From<io::Error> for DiscoveryServiceError {
    fn from(error: io::Error) -> DiscoveryServiceError {
        DiscoveryServiceError::IoError(error)
    }
}

impl From<ParsePeerAddrError> for DiscoveryServiceError {
    fn from(error: ParsePeerAddrError) -> DiscoveryServiceError {
        DiscoveryServiceError::ParseError(error)
    }
}

pub struct DiscoveryService {
    announce_listener: UdpSocket,
    announcer_handle: thread::JoinHandle<Result<(), io::Error>>,
    public_key: PublicKey,
}

fn get_local_addr() -> Option<SocketAddr> {
    let addrs = nix::ifaddrs::getifaddrs().ok()?;

    addrs
        .flat_map(|ifaddr| match ifaddr.address {
            Some(SockAddr::Inet(address @ InetAddr::V4(_)))
                if (address.to_str() != "127.0.0.1:0") =>
            {
                Some(address.to_std())
            }
            _ => None,
        })
        .next()
}

fn init_announcer(
    socket_addr: SocketAddr,
    public_key: PublicKey,
) -> thread::JoinHandle<Result<(), io::Error>> {
    let ann_peer = PeerAddr {
        protocol: Protocol::Net,
        socket_addr,
        public_key,
    }
    .to_string();

    thread::spawn(move || {
        let socket = UdpSocket::bind("0.0.0.0:0")?;
        socket.set_broadcast(true)?;

        let buf_bytes = ann_peer.as_bytes();

        loop {
            socket.send_to(
                &buf_bytes,
                format!("255.255.255.255:{}", PEER_DISCOVERY_PORT),
            )?;
            thread::sleep(Duration::from_secs_f32(2.0));
        }
    })
}

impl DiscoveryService {
    pub fn new(public_key: PublicKey) -> Result<Self, DiscoveryServiceError> {
        let socket_addr = format!("0.0.0.0:{}", PEER_DISCOVERY_PORT);
        let announce_listener = UdpBuilder::new_v4()?.reuse_port(true)?.bind(&socket_addr)?;

        let mut hs_listener_socket_addr =
            get_local_addr().ok_or(DiscoveryServiceError::GetLocalAddrError)?;
        hs_listener_socket_addr.set_port(HANDSHAKE_LISTENER_PORT);

        let announcer_handle = init_announcer(hs_listener_socket_addr, public_key);

        Ok(DiscoveryService {
            announce_listener,
            announcer_handle,
            public_key,
        })
    }

    pub fn recv(&self) -> Result<PeerAddr, DiscoveryServiceError> {
        let socket = &self.announce_listener;

        let mut buf = [0; 1024];
        let received = socket.recv(&mut buf)?;
        let buf_str = std::str::from_utf8(&buf[..received]).unwrap();
        let peer = buf_str
            .split(";")
            .map(str::parse::<PeerAddr>)
            .next()
            .ok_or(ParsePeerAddrError())??;

        if self.public_key == peer.public_key {
            self.recv()
        } else {
          Ok(peer)
        }
    }
}
