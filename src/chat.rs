use std::fmt;
use std::sync::mpsc;

pub type FeedId = String;

#[derive(Debug)]
pub enum ChatSender {
    _You,
    Info,
    Peer(FeedId),
}

#[derive(Debug)]
pub struct ChatMsg {
    pub message: String,
    pub sender: ChatSender,
}

pub struct PeerChat {
    pub messages: Vec<ChatMsg>,
    pub input: String,
    pub peer_tx: Option<mpsc::Sender<String>>
}

impl fmt::Display for ChatSender {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ChatSender::_You => write!(f, "You"),
            ChatSender::Info => write!(f, "INFO"),
            // change to: ChatSender::Peer(feed_id) => write!(f, "{}", feed_id),
            ChatSender::Peer(_) => write!(f, "User"),
        }
    }
}
