use base64;
use net2::unix::UnixUdpBuilderExt;
use net2::UdpBuilder;
use regex::Regex;
use ssb_crypto::PublicKey;
use std::error;
use std::fmt;
use std::io;
use std::net::{SocketAddr, UdpSocket};
use std::str::FromStr;

#[derive(Debug, Clone)]
pub struct SsbPeer {
    pub protocol: Protocol,
    pub socket_addr: SocketAddr,
    pub public_key: PublicKey,
}

impl fmt::Display for SsbPeer {
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

impl SsbPeer {
    pub fn feed_id(&self) -> String {
        let encoded_bytes = base64::encode(&self.public_key.0);
        format!("@{}.ed25519", encoded_bytes)
    }
}

#[derive(Debug)]
pub struct ParseSsbPeerError();

impl fmt::Display for ParseSsbPeerError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Failed to parse SsbPeer string")
    }
}

// This is important for other errors to wrap this one.
impl error::Error for ParseSsbPeerError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        // Generic error, underlying cause isn't tracked.
        None
    }
}

impl FromStr for SsbPeer {
    type Err = ParseSsbPeerError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let re = Regex::new(r"(ws://|net:)(.*?)~shs:(.*)").unwrap();

        let groups = re.captures(s).ok_or(ParseSsbPeerError())?;

        let protocol = match &groups[1] {
            "ws://" => Some(Protocol::WebSocket),
            "net:" => Some(Protocol::Net),
            _ => None,
        }
        .ok_or(ParseSsbPeerError())?;

        let socket_addr: SocketAddr = groups[2].parse().map_err(|_| ParseSsbPeerError())?;
        let pk_bytes = base64::decode(&groups[3]).map_err(|_| ParseSsbPeerError())?;
        let public_key = PublicKey::from_slice(&pk_bytes).ok_or(ParseSsbPeerError())?;

        Ok(SsbPeer {
            protocol,
            socket_addr,
            public_key,
        })
    }
}

#[derive(Debug)]
pub enum PeerListenerError {
    ParseError(ParseSsbPeerError),
    IoError(std::io::Error),
}

impl fmt::Display for PeerListenerError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            PeerListenerError::ParseError(e) => fmt::Display::fmt(e, f),
            PeerListenerError::IoError(e) => fmt::Display::fmt(e, f),
        }
    }
}

// This is important for other errors to wrap this one.
impl error::Error for PeerListenerError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            PeerListenerError::ParseError(e) => Some(e),
            PeerListenerError::IoError(e) => Some(e),
        }
    }
}

impl From<io::Error> for PeerListenerError {
    fn from(error: io::Error) -> PeerListenerError {
        PeerListenerError::IoError(error)
    }
}

impl From<ParseSsbPeerError> for PeerListenerError {
    fn from(error: ParseSsbPeerError) -> PeerListenerError {
        PeerListenerError::ParseError(error)
    }
}

pub struct PeerListener(UdpSocket);

impl PeerListener {
    pub fn new() -> Result<Self, PeerListenerError> {
        let socket = UdpBuilder::new_v4()?
            .reuse_port(true)?
            .bind("0.0.0.0:8008")?;
        Ok(PeerListener(socket))
    }

    pub fn recv(&self) -> Result<SsbPeer, PeerListenerError> {
        let PeerListener(socket) = self;
        let mut buf = [0; 1024];
        let received = socket.recv(&mut buf)?;
        let buf_str = std::str::from_utf8(&buf[..received]).unwrap();
        let peer = buf_str
            .split(";")
            .map(str::parse::<SsbPeer>)
            .next()
            .ok_or(ParseSsbPeerError())??;

        Ok(peer)
    }
}
