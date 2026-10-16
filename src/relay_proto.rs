//! Wire protocol between RDesk clients (host and controller) and the relay server.
//! The server includes this file with `#[path]`, so both sides always agree.
#![allow(dead_code)]

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::io::{self, Read, Write};

/// One of the two connections of a session: video (host -> controller) or input (controller -> host).
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Channel {
    Video,
    Input,
}

/// First message on every connection to the relay.
#[derive(Serialize, Deserialize, Debug)]
pub(crate) enum Hello {
    /// Host: register this computer and receive an ID. The connection stays open.
    Register,
    /// Controller: ask for a session with the host that owns this ID.
    Connect { id: String },
    /// Host: join a session on one channel (the host side of the pair).
    Accept { session: u64, channel: Channel },
    /// Controller: join a session on one channel (the controller side of the pair).
    Join { session: u64, channel: Channel },
}

/// Messages sent by the relay.
#[derive(Serialize, Deserialize, Debug)]
pub(crate) enum Reply {
    Registered {
        id: String,
    },
    /// Controller: the session was created; open Join connections with this id.
    Session {
        session: u64,
    },
    NotFound,
    /// Host: a controller wants a session; open two Accept connections.
    Incoming {
        session: u64,
    },
    Error(String),
}

pub(crate) fn write_msg<T: Serialize>(w: &mut impl Write, msg: &T) -> io::Result<()> {
    let data = bincode::serialize(msg).map_err(|e| io::Error::other(e.to_string()))?;
    w.write_all(&(data.len() as u32).to_be_bytes())?;
    w.write_all(&data)?;
    w.flush()
}

pub(crate) fn read_msg<T: DeserializeOwned>(r: &mut impl Read) -> io::Result<T> {
    let mut len = [0u8; 4];
    r.read_exact(&mut len)?;
    let n = u32::from_be_bytes(len) as usize;
    if n > 1 << 16 {
        return Err(io::Error::other("relay message too large"));
    }
    let mut buf = vec![0u8; n];
    r.read_exact(&mut buf)?;
    bincode::deserialize(&buf).map_err(|e| io::Error::other(e.to_string()))
}
