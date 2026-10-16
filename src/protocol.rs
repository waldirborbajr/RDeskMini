//! Shared constants, messages between threads and the GUI, and session types.

use serde::{Deserialize, Serialize};

/// Address of the relay server. Set this to the public address of your server
/// before building the client, e.g. "relay.example.com:9443".
pub(crate) const RELAY_ADDR: &str = "relay.example.com:9443";

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
// Messages between backend threads and the GUI
// ------------------------------------------------------------
pub(crate) enum BackendMsg {
    /// Host: registered with the relay; carries this device's ID, already formatted.
    HostReady(String),
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

/// State of the remote-control session (this device controlling another).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SessionState {
    Idle,
    Connecting,
    Connected,
    Error,
    Disconnected,
}
