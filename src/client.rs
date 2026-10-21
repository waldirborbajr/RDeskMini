//! Controller side: asks the relay for a session with the typed ID, shows the
//! remote screen (video connection) and sends our input (input connection).

use openh264::formats::YUVSource;
use rustls::{ClientConnection, StreamOwned};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use crate::cert;
use crate::i18n;
use crate::identity::format_id;
use crate::protocol::{BackendMsg, InputEvent, RELAY_ADDR};
use crate::relay_proto::{read_msg, write_msg, Channel, Hello, Reply};
use crate::video::yuv_to_rgb;

const MAX_VIDEO_FRAME: usize = 16 * 1024 * 1024;

fn io_err<E: std::fmt::Display>(e: E) -> std::io::Error {
    std::io::Error::other(e.to_string())
}

pub(crate) fn spawn_client_backend(
    id: String,
    expected_fingerprint: [u8; 32],
    tx: Sender<BackendMsg>,
    input_rx: Receiver<InputEvent>,
) -> Arc<AtomicBool> {
    let stop = Arc::new(AtomicBool::new(false));
    let stop_clone = stop.clone();
    thread::spawn(move || {
        if let Err(e) = run_client(&id, expected_fingerprint, tx.clone(), stop_clone, input_rx) {
            let _ = tx.send(BackendMsg::Error(e.to_string()));
        }
        let _ = tx.send(BackendMsg::Disconnected);
    });
    stop
}

fn run_client(
    id: &str,
    expected_fingerprint: [u8; 32],
    tx: Sender<BackendMsg>,
    stop: Arc<AtomicBool>,
    input_rx: Receiver<InputEvent>,
) -> std::io::Result<()> {
    let _ = tx.send(BackendMsg::Status(
        i18n::t("status_connecting").replace("{}", &format_id(id)),
    ));

    let session = request_session(id)?;
    let (mut video, video_raw) = open_join(session, Channel::Video, expected_fingerprint)?;
    let (mut input, input_raw) = open_join(session, Channel::Input, expected_fingerprint)?;
    let _ = tx.send(BackendMsg::Status(
        i18n::t("status_waiting_approval")
            .replace("{}", &format_id(id)),
    ));

    let over = Arc::new(AtomicBool::new(false));

    // Watcher: when stopped (user disconnects) or when the session ends,
    // shut both connections down so blocking reads and writes return.
    {
        let (over, stop) = (over.clone(), stop.clone());
        thread::spawn(move || loop {
            thread::sleep(Duration::from_millis(100));
            if stop.load(Ordering::SeqCst) || over.load(Ordering::SeqCst) {
                let _ = video_raw.shutdown(std::net::Shutdown::Both);
                let _ = input_raw.shutdown(std::net::Shutdown::Both);
                break;
            }
        });
    }

    // Input writer: sends our mouse and keyboard events to the host.
    let over_w = over.clone();
    thread::spawn(move || {
        loop {
            match input_rx.recv_timeout(Duration::from_millis(100)) {
                Ok(ev) => {
                    let Ok(data) = bincode::serialize(&ev) else {
                        continue;
                    };
                    let len = (data.len() as u32).to_be_bytes();
                    let sent = input
                        .write_all(&len)
                        .and_then(|_| input.write_all(&data))
                        .and_then(|_| input.flush());
                    if sent.is_err() {
                        break;
                    }
                }
                Err(RecvTimeoutError::Timeout) => {
                    if over_w.load(Ordering::SeqCst) {
                        break;
                    }
                }
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }
        over_w.store(true, Ordering::SeqCst);
    });

    let result = receive_video(&mut video, &tx, &over, id);
    over.store(true, Ordering::SeqCst);
    result
}

/// Asks the relay to open a session with the host that owns this ID.
fn request_session(id: &str) -> std::io::Result<u64> {
    let mut s = TcpStream::connect(RELAY_ADDR)?;
    s.set_read_timeout(Some(Duration::from_secs(10)))?;
    write_msg(&mut s, &Hello::Connect { id: id.to_string() })?;
    match read_msg::<Reply>(&mut s)? {
        Reply::Session { session } => Ok(session),
        Reply::NotFound => Err(io_err(i18n::t("peer_not_found"))),
        Reply::Error(e) => Err(io_err(e)),
        _ => Err(io_err("unexpected reply from relay")),
    }
}

/// Joins a session on one channel and completes the TLS handshake (controller is the TLS client).
fn open_join(
    session: u64,
    channel: Channel,
    expected_fingerprint: [u8; 32],
) -> std::io::Result<(StreamOwned<ClientConnection, TcpStream>, TcpStream)> {
    let mut sock = TcpStream::connect(RELAY_ADDR)?;
    sock.set_nodelay(true)?;
    sock.set_read_timeout(Some(Duration::from_secs(35)))?;
    sock.set_write_timeout(Some(Duration::from_secs(35)))?;
    write_msg(&mut sock, &Hello::Join { session, channel })?;
    let raw = sock.try_clone()?;
    let tls_cfg = cert::client_tls_config(expected_fingerprint);
    let sni = rustls::pki_types::ServerName::try_from("localhost")
        .map_err(io_err)?
        .to_owned();
    let conn = ClientConnection::new(tls_cfg, sni).map_err(io_err)?;
    let mut tls = StreamOwned::new(conn, sock);
    tls.conn
        .complete_io(&mut tls.sock)
        .map_err(|e| io_err(format!("TLS: {e}")))?;
    tls.sock.set_read_timeout(None)?;
    tls.sock.set_write_timeout(None)?;
    Ok((tls, raw))
}

/// Reads video frames, decodes them and sends each picture to the GUI until the session ends.
fn receive_video(
    video: &mut StreamOwned<ClientConnection, TcpStream>,
    tx: &Sender<BackendMsg>,
    over: &Arc<AtomicBool>,
    id: &str,
) -> std::io::Result<()> {
    let mut decoder = openh264::decoder::Decoder::new().map_err(io_err)?;
    let mut frames: u32 = 0;
    let mut last = Instant::now();
    let mut announced_connected = false;

    while !over.load(Ordering::SeqCst) {
        match read_frame(video) {
            Ok(payload) if payload.is_empty() => {
                return Err(io_err(i18n::t("status_request_denied_remote")));
            }
            Ok(payload) => {
                if let Ok(Some(yuv)) = decoder.decode(&payload) {
                    let (w, h) = yuv.dimensions();
                    let rgb = yuv_to_rgb(&yuv);
                    if !announced_connected {
                        let _ = tx.send(BackendMsg::Status(
                            i18n::t("status_connected").replace("{}", &format_id(id)),
                        ));
                        announced_connected = true;
                    }
                    let _ = tx.send(BackendMsg::Frame {
                        width: w as u32,
                        height: h as u32,
                        rgb,
                    });
                    frames += 1;
                }
            }
            // A read error after we asked to stop is the normal way out.
            Err(e) => {
                return if over.load(Ordering::SeqCst) {
                    Ok(())
                } else {
                    Err(e)
                };
            }
        }

        if last.elapsed() >= Duration::from_secs(1) {
            let _ = tx.send(BackendMsg::Stats {
                fps: frames as f32 / last.elapsed().as_secs_f32(),
                latency_ms: 0.0,
                packet_loss_pct: 0.0,
            });
            frames = 0;
            last = Instant::now();
        }
    }
    Ok(())
}

/// One length-prefixed H.264 access unit.
fn read_frame(r: &mut impl Read) -> std::io::Result<Vec<u8>> {
    let mut len_buf = [0u8; 4];
    r.read_exact(&mut len_buf)?;
    let len = u32::from_be_bytes(len_buf) as usize;
    if len > MAX_VIDEO_FRAME {
        return Err(io_err("video frame too large"));
    }
    let mut payload = vec![0u8; len];
    r.read_exact(&mut payload)?;
    Ok(payload)
}
