use std::io::Error as IoError;

use tokio_tungstenite::tungstenite::{handshake::derive_accept_key, protocol::CloseFrame};

use super::{CloseCode, Message};
use crate::http::header::HeaderValue;

pub(crate) fn sign(key: &[u8]) -> HeaderValue {
    derive_accept_key(key).try_into().unwrap()
}

pub(crate) fn tungstenite_error_to_io_error(
    error: tokio_tungstenite::tungstenite::Error,
) -> IoError {
    use tokio_tungstenite::tungstenite::Error::*;
    match error {
        Io(err) => err,
        _ => IoError::other(error.to_string()),
    }
}

#[doc(hidden)]
impl From<tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode> for CloseCode {
    fn from(code: tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode) -> Self {
        let code: u16 = code.into();
        code.into()
    }
}

#[doc(hidden)]
impl From<CloseCode> for tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode {
    fn from(code: CloseCode) -> Self {
        let code: u16 = code.into();
        code.into()
    }
}

#[doc(hidden)]
impl From<tokio_tungstenite::tungstenite::Message> for Message {
    fn from(msg: tokio_tungstenite::tungstenite::Message) -> Self {
        use tokio_tungstenite::tungstenite::Message::*;

        match msg {
            Text(data) => Message::Text(data.to_string()),
            Binary(data) => Message::Binary(data.into()),
            Ping(data) => Message::Ping(data.into()),
            Pong(data) => Message::Pong(data.into()),
            Close(cf) => Message::Close(cf.map(|cf| (cf.code.into(), cf.reason.to_string()))),
            Frame(_) => unreachable!(),
        }
    }
}

#[doc(hidden)]
impl From<Message> for tokio_tungstenite::tungstenite::Message {
    fn from(msg: Message) -> Self {
        use tokio_tungstenite::tungstenite::{Bytes, Message::*};

        match msg {
            Message::Text(data) => Text(data.into()),
            Message::Binary(data) => Binary(Bytes::from_owner(data)),
            Message::Ping(data) => Ping(Bytes::from_owner(data)),
            Message::Pong(data) => Pong(Bytes::from_owner(data)),
            Message::Close(cf) => Close(cf.map(|(code, reason)| CloseFrame {
                code: code.into(),
                reason: reason.into(),
            })),
        }
    }
}
