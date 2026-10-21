//! Host side: registers with the relay and, for each controller, streams the screen
//! (video connection) and applies its input (input connection).

use enigo::{Enigo, Settings};
use rustls::{ServerConnection, StreamOwned};
use screenshots::Screen;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use crate::cert;
use crate::i18n;
use crate::identity::format_id;
use crate::input::apply_event;
use crate::protocol::{BackendMsg, InputEvent, RELAY_ADDR};
use crate::relay_proto::{read_msg, write_msg, Channel, Hello, Reply};
use crate::video::H264EncoderState;

fn io_err<E: std::fmt::Display>(e: E) -> std::io::Error {
    std::io::Error::other(e.to_string())
}

/// Keeps this device registered with the relay for the life of the program.
/// If the relay is unreachable, it retries every few seconds.
pub(crate) fn spawn_host_backend(tx: Sender<BackendMsg>) {
    thread::spawn(move || loop {
        if let Err(e) = register_and_serve(&tx) {
            let _ = tx.send(BackendMsg::Status(
                i18n::t("status_relay_offline").replace("{}", &e.to_string()),
            ));
        }
        thread::sleep(Duration::from_secs(5));
    });
}

fn register_and_serve(tx: &Sender<BackendMsg>) -> std::io::Result<()> {
    let mut reg = TcpStream::connect(RELAY_ADDR)?;
    write_msg(&mut reg, &Hello::Register)?;
    let id = match read_msg::<Reply>(&mut reg)? {
        Reply::Registered { id } => id,
        Reply::Error(e) => return Err(io_err(e)),
        _ => return Err(io_err("unexpected reply from relay")),
    };
    let _ = tx.send(BackendMsg::HostReady(format_id(&id)));
    let _ = tx.send(BackendMsg::Status(i18n::t("status_host_ready").to_string()));

    loop {
        if let Reply::Incoming { session } = read_msg::<Reply>(&mut reg)? {
            let tx = tx.clone();
            thread::spawn(move || {
                if let Err(e) = serve_session(session, &tx) {
                    let _ = tx.send(BackendMsg::Status(
                        i18n::t("status_session_ended").replace("{}", &e.to_string()),
                    ));
                }
                let _ = tx.send(BackendMsg::Status(i18n::t("status_host_ready").to_string()));
            });
        }
    }
}

/// Opens the video and input connections of one session and runs them until the controller leaves.
fn serve_session(session: u64, tx: &Sender<BackendMsg>) -> std::io::Result<()> {
    let mut video = open_accept(session, Channel::Video)?;
    let mut input = open_accept(session, Channel::Input)?;

    let (approval_tx, approval_rx) = mpsc::channel();
    let expires_at = Instant::now() + Duration::from_secs(30);
    tx.send(BackendMsg::IncomingRequest {
        approval: approval_tx,
        expires_at,
    })
    .map_err(io_err)?;
    match approval_rx.recv_timeout(Duration::from_secs(30)) {
        Ok(true) => {}
        Ok(false) => {
            send_request_denied(&mut video);
            return Err(io_err(i18n::t("status_request_rejected")));
        }
        Err(mpsc::RecvTimeoutError::Timeout) => {
            send_request_denied(&mut video);
            return Err(io_err(i18n::t("status_request_expired")));
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            send_request_denied(&mut video);
            return Err(io_err(i18n::t("status_request_expired")));
        }
    }
    let _ = tx.send(BackendMsg::Status(
        i18n::t("status_peer_connected").to_string(),
    ));

    let stop = Arc::new(AtomicBool::new(false));
    let video_stop = stop.clone();
    let video_thread = thread::spawn(move || stream_video(video, &video_stop));

    let result = apply_input(input);
    stop.store(true, Ordering::SeqCst);
    let _ = video_thread.join();
    result
}

fn send_request_denied(video: &mut StreamOwned<ServerConnection, TcpStream>) {
    let _ = video.write_all(&0u32.to_be_bytes()).and_then(|_| video.flush());
}

/// Joins the relay on one channel of a session and completes the TLS handshake (host is the TLS server).
fn open_accept(
    session: u64,
    channel: Channel,
) -> std::io::Result<StreamOwned<ServerConnection, TcpStream>> {
    let mut sock = TcpStream::connect(RELAY_ADDR)?;
    write_msg(&mut sock, &Hello::Accept { session, channel })?;
    sock.set_read_timeout(Some(Duration::from_secs(30)))?;
    let tls_cfg = cert::server_tls_config().map_err(io_err)?;
    let conn = ServerConnection::new(tls_cfg).map_err(io_err)?;
    let mut tls = StreamOwned::new(conn, sock);
    tls.conn
        .complete_io(&mut tls.sock)
        .map_err(|e| io_err(format!("TLS: {e}")))?;
    tls.sock.set_read_timeout(None)?;
    Ok(tls)
}

/// Reads input events from the controller until it disconnects.
fn apply_input(mut tls: StreamOwned<ServerConnection, TcpStream>) -> std::io::Result<()> {
    let mut enigo =
        Enigo::new(&Settings::default()).map_err(|e| io_err(format!("input: {e:?}")))?;
    loop {
        let mut len_buf = [0u8; 4];
        tls.read_exact(&mut len_buf)?;
        let len = u32::from_be_bytes(len_buf) as usize;
        if len > 4096 {
            return Err(io_err("input frame too large"));
        }
        let mut data = vec![0u8; len];
        tls.read_exact(&mut data)?;
        if let Ok(ev) = bincode::deserialize::<InputEvent>(&data) {
            apply_event(&mut enigo, ev);
        }
    }
}

/// Captures, encodes and sends the screen until the session stops or the controller leaves.
fn stream_video(
    mut tls: StreamOwned<ServerConnection, TcpStream>,
    stop: &Arc<AtomicBool>,
) -> std::io::Result<()> {
    let screen = Screen::all()
        .map_err(io_err)?
        .into_iter()
        .next()
        .ok_or_else(|| io_err("no screen found"))?;
    let sw = screen.display_info.width;
    let sh = screen.display_info.height;
    let mut encoder = H264EncoderState::new(sw, sh).map_err(io_err)?;
    let frame_dur = Duration::from_millis(1000 / 20);

    while !stop.load(Ordering::SeqCst) {
        let started = Instant::now();
        let img = screen.capture().map_err(io_err)?;
        // The capture is RGBA (4 bytes per pixel); the encoder wants RGB (3 bytes).
        let rgb: Vec<u8> = img
            .into_raw()
            .as_chunks::<4>().0.iter()
            .flat_map(|p| [p[0], p[1], p[2]])
            .collect();
        if rgb.len() != (sw * sh * 3) as usize {
            continue;
        }
        let h264 = encoder.encode_rgb(&rgb).map_err(io_err)?;
        tls.write_all(&(h264.len() as u32).to_be_bytes())?;
        tls.write_all(&h264)?;
        tls.flush()?;

        if let Some(rest) = frame_dur.checked_sub(started.elapsed()) {
            thread::sleep(rest);
        }
    }
    Ok(())
}
