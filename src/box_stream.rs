use ssb_crypto::{
    secretbox::{self, Tag, Nonce},
    NonceGen,
};

use byteorder::{BigEndian, ByteOrder};
use std::io::{self, Read, Write};

quick_error! {
    #[derive(Debug)]
    pub enum BoxStreamError {
        Io(err: io::Error) {
            description(err.description())
        }
        HeaderOpenFailed {
            description("Failed to decrypt header")
        }
        BodyOpenFailed {
            description("Failed to decrypt body")
        }
    }
}

impl From<io::Error> for BoxStreamError {
    fn from(err: io::Error) -> BoxStreamError {
        BoxStreamError::Io(err)
    }
}
impl From<BoxStreamError> for io::Error {
    fn from(err: BoxStreamError) -> io::Error {
        match err {
            BoxStreamError::Io(err) => err,
            err => io::Error::new(io::ErrorKind::InvalidData, err),
        }
    }
}

use BoxStreamError::*;

pub struct BoxReader<R: Read> {
    reader: R,
    key: secretbox::Key,
    noncegen: NonceGen,
}

impl<R: Read> BoxReader<R> {
    pub fn new(reader: R, key: secretbox::Key, noncegen: NonceGen) -> BoxReader<R> {
        BoxReader {
            reader,
            key,
            noncegen
        }
    }

    pub fn recv(&mut self) -> Result<Option<Vec<u8>>, BoxStreamError> {
        let (body_size, body_tag) = {
            let mut head_tag = Tag([0; 16]);
            self.reader.read_exact(&mut head_tag.0)?;

            let mut head_payload = [0; 18];
            self.reader.read_exact(&mut head_payload[..])?;

            secretbox::open_detached(
                &mut head_payload,
                &head_tag,
                &self.noncegen.next(),
                &self.key,
            )
            .map_err(|_| HeaderOpenFailed)?;

            let (sz, rest) = head_payload.split_at(2);
            (
                BigEndian::read_u16(sz) as usize,
                Tag::from_slice(rest).unwrap(),
            )
        };

        if body_size == 0 && body_tag.0 == [0; 16] {
            // Goodbye
            Ok(None)
        } else {
            let mut body = vec![0; body_size];
            self.reader.read_exact(&mut body)?;

            secretbox::open_detached(&mut body, &body_tag, &self.noncegen.next(), &self.key)
                .map_err(|_| BodyOpenFailed)?;

            Ok(Some(body))
        }
    }
}

#[allow(dead_code)]
pub struct BoxWriter<W: Write> {
    writer: W,
    key: secretbox::Key,
    noncegen: NonceGen,
}

fn seal_header(payload: &mut [u8; 18], nonce: Nonce, key: &secretbox::Key) -> [u8; 34] {
    let htag = secretbox::seal_detached(&mut payload[..], &nonce, &key);

    let mut hbox = [0; 34];
    hbox[..16].copy_from_slice(&htag[..]);
    hbox[16..].copy_from_slice(&payload[..]);
    hbox
}

#[allow(dead_code)]
fn seal(mut body: Vec<u8>, key: &secretbox::Key, noncegen: &mut NonceGen) -> ([u8; 34], Vec<u8>) {
    let head_nonce = noncegen.next();
    let body_nonce = noncegen.next();

    let mut head_payload = {
        // Overwrites body with ciphertext
        let btag = secretbox::seal_detached(&mut body, &body_nonce, &key);

        let mut hp = [0; 18];
        let (sz, tag) = hp.split_at_mut(2);
        BigEndian::write_u16(sz, body.len() as u16);
        tag.copy_from_slice(&btag[..]);
        hp
    };

    let head = seal_header(&mut head_payload, head_nonce, key);
    (head, body)
}

#[allow(dead_code)]
impl<W: Write> BoxWriter<W> {
    pub fn new(w: W, key: secretbox::Key, noncegen: NonceGen) -> BoxWriter<W> {
        BoxWriter {
            writer: w,
            key,
            noncegen,
        }
    }

    pub fn send(mut self, body: Vec<u8>) -> Result<(), io::Error> {
        assert!(body.len() <= 4096);

        let (head, mut cipher_body) = seal(body, &self.key, &mut self.noncegen);

        let mut r = self.writer.write_all(&head);
        if r.is_ok() {
            r = self.writer.write_all(&cipher_body);
        }

        cipher_body.clear();
        r
    }

    pub fn send_goodbye(mut self) -> (Self, Result<(), io::Error>) {
        let mut payload = [0; 18];
        let head = seal_header(&mut payload, self.noncegen.next(), &self.key);
        let r = self.writer.write_all(&head);
        (self, r)
    }
}
