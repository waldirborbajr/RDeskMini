//! Shared constants, messages between threads and the GUI, and session types.

use serde::{Deserialize, Serialize};

// ------------------------------------------------------------
// Protocol constants
// ------------------------------------------------------------
pub(crate) const FEC_DATA_SHARDS: usize = 8;
pub(crate) const FEC_PARITY_SHARDS: usize = 2;
pub(crate) const FEC_SHARD_SIZE: usize = 1200;
pub(crate) const MAX_UDP_PAYLOAD: usize = 1400;

// ------------------------------------------------------------
// Input event protocol
// ------------------------------------------------------------
#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) enum InputEvent {
    MouseMove { x: i32, y: i32 },
    MouseDown { button: u8 },
    MouseUp { button: u8 },
    Scroll { delta: i32 },
    KeyDown { code: u32 },
    KeyUp { code: u32 },
}

// ------------------------------------------------------------
// Messages between backend threads and GUI
// ------------------------------------------------------------
pub(crate) enum BackendMsg {
    Status(String),
    Error(String),
    Frame {
        width: u32,
        height: u32,
        rgb: Vec<u8>,
    },
    Stats {
        fps: f32,
        latency_ms: f32,
        packet_loss_pct: f32,
    },
    Disconnected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AppMode {
    Host,
    Client,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SessionState {
    Idle,
    Listening,
    Connecting,
    Connected,
    Error,
    Disconnected,
}
