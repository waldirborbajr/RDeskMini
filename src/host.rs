//! Host side: captures the screen, streams video and receives input.

use enigo::{Enigo, Settings};
use rustls::{ServerConnection, StreamOwned};
use screenshots::Screen;
use std::io::Read;
use std::net::{SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use crate::cert;
use crate::fec::{encode_udp_datagram, FecCodec};
use crate::i18n;
use crate::input::apply_event;
use crate::protocol::{BackendMsg, InputEvent, MAX_UDP_PAYLOAD};
use crate::video::H264EncoderState;

// ============================================================
// HOST backend
// ============================================================
pub(crate) fn spawn_host_backend(bind_addr: String, tx: Sender<BackendMsg>) -> Arc<AtomicBool> {
    let stop = Arc::new(AtomicBool::new(false));
    let stop_clone = stop.clone();

    thread::spawn(move || {
        if let Err(e) = run_host(bind_addr, tx.clone(), stop_clone) {
            let _ = tx.send(BackendMsg::Error(e.to_string()));
        }
        let _ = tx.send(BackendMsg::Disconnected);
    });

    stop
}

pub(crate) fn run_host(
    bind: String,
    tx: Sender<BackendMsg>,
    stop: Arc<AtomicBool>,
) -> std::io::Result<()> {
    let tls_cfg =
        cert::server_tls_config().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

    let listener = TcpListener::bind(&bind)?;
    listener.set_nonblocking(true)?;
    let _ = tx.send(BackendMsg::Status(
        i18n::t("status_listening").replace("{}", &bind),
    ));

    let udp = UdpSocket::bind(&bind)?;
    udp.set_nonblocking(false)?;

    let mut maybe_stream: Option<(TcpStream, SocketAddr)> = None;
    while !stop.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((s, a)) => {
                maybe_stream = Some((s, a));
                break;
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(100));
            }
            Err(e) => return Err(e),
        }
    }
    let (tcp_stream, peer) = match maybe_stream {
        Some(x) => x,
        None => return Ok(()),
    };

    let conn = ServerConnection::new(tls_cfg)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    let mut tls_stream = StreamOwned::new(conn, tcp_stream);
    tls_stream
        .conn
        .complete_io(&mut tls_stream.sock)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("TLS: {e}")))?;

    let _ = tx.send(BackendMsg::Status(
        i18n::t("status_connected").replace("{}", &peer.to_string()),
    ));

    let screens = Screen::all()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("screens: {e}")))?;
    let screen = screens
        .into_iter()
        .next()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "no screen"))?;
    let sw = screen.display_info.width;
    let sh = screen.display_info.height;

    let mut encoder = H264EncoderState::new(sw, sh)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    let fec = FecCodec::new().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

    let mut input_stream = tls_stream
        .sock
        .try_clone()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    thread::spawn(move || {
        let mut enigo = match Enigo::new(&Settings::default()) {
            Ok(e) => e,
            Err(_) => return,
        };
        loop {
            let mut len_buf = [0u8; 4];
            if input_stream.read_exact(&mut len_buf).is_err() {
                break;
            }
            let len = u32::from_be_bytes(len_buf) as usize;
            if len > 4096 {
                break;
            }
            let mut data = vec![0u8; len];
            if input_stream.read_exact(&mut data).is_err() {
                break;
            }
            if let Ok(ev) = bincode::deserialize::<InputEvent>(&data) {
                apply_event(&mut enigo, ev);
            }
        }
    });

    let target_fps = 20u64;
    let frame_dur = Duration::from_millis(1000 / target_fps);
    let mut last = Instant::now();
    let mut block_id: u32 = 0;

    while !stop.load(Ordering::SeqCst) {
        let img = match screen.capture() {
            Ok(i) => i,
            Err(e) => {
                let _ = tx.send(BackendMsg::Error(format!("capture: {e}")));
                break;
            }
        };
        let rgb_raw = img.into_raw();
        let expected_len = (sw * sh * 3) as usize;
        if rgb_raw.len() < expected_len {
            continue;
        }
        let rgb_slice = &rgb_raw[..expected_len];

        let h264 = match encoder.encode_rgb(rgb_slice) {
            Ok(h) => h,
            Err(e) => {
                let _ = tx.send(BackendMsg::Error(format!("h264: {e}")));
                continue;
            }
        };

        let shards = match fec.encode(&h264) {
            Ok(s) => s,
            Err(e) => {
                let _ = tx.send(BackendMsg::Error(format!("fec: {e}")));
                continue;
            }
        };

        let total = shards.len() as u16;
        for (idx, shard) in shards.iter().enumerate() {
            let dg = encode_udp_datagram(block_id, idx as u16, total, h264.len() as u32, shard);
            if dg.len() <= MAX_UDP_PAYLOAD {
                if udp.send_to(&dg, peer).is_err() {
                    break;
                }
            }
        }
        block_id = block_id.wrapping_add(1);

        let el = last.elapsed();
        if el < frame_dur {
            thread::sleep(frame_dur - el);
        }
        last = Instant::now();
    }

    Ok(())
}
