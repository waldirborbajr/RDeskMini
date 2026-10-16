//! Client side: connects to a host, shows the video and sends input.

use rustls::{ClientConnection, StreamOwned};
use std::collections::HashMap;
use std::io::Write;
use std::net::{SocketAddr, TcpStream, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use crate::cert;
use crate::fec::{decode_udp_datagram, FecCodec};
use crate::i18n;
use crate::protocol::{BackendMsg, InputEvent, FEC_DATA_SHARDS};
use crate::video::yuv_to_rgb;

// ============================================================
// CLIENT backend
// ============================================================
pub(crate) fn spawn_client_backend(
    server_addr: String,
    tx: Sender<BackendMsg>,
    input_rx: Receiver<InputEvent>,
) -> Arc<AtomicBool> {
    let stop = Arc::new(AtomicBool::new(false));
    let stop_clone = stop.clone();

    thread::spawn(move || {
        if let Err(e) = run_client(server_addr, tx.clone(), stop_clone, input_rx) {
            let _ = tx.send(BackendMsg::Error(e.to_string()));
        }
        let _ = tx.send(BackendMsg::Disconnected);
    });

    stop
}

pub(crate) fn run_client(
    addr: String,
    tx: Sender<BackendMsg>,
    stop: Arc<AtomicBool>,
    input_rx: Receiver<InputEvent>,
) -> std::io::Result<()> {
    let tls_cfg =
        cert::client_tls_config().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

    let _ = tx.send(BackendMsg::Status(
        i18n::t("status_connecting").replace("{}", &addr),
    ));

    let server_addr: SocketAddr = addr
        .parse()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, format!("addr: {e}")))?;

    let tcp = TcpStream::connect(server_addr)?;
    tcp.set_nodelay(true).ok();

    let sni = rustls::pki_types::ServerName::try_from("localhost")
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, format!("sni: {e}")))?
        .to_owned();
    let conn = ClientConnection::new(tls_cfg, sni)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    let mut tls_stream = StreamOwned::new(conn, tcp);
    tls_stream
        .conn
        .complete_io(&mut tls_stream.sock)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("TLS: {e}")))?;

    let _ = tx.send(BackendMsg::Status(
        i18n::t("status_connected").replace("{}", &addr),
    ));

    let udp = UdpSocket::bind("0.0.0.0:0")?;
    udp.set_read_timeout(Some(Duration::from_millis(50)))?;

    let fec = FecCodec::new().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    let mut decoder = openh264::decoder::Decoder::new()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("decoder: {e}")))?;

    let mut input_stream = tls_stream
        .sock
        .try_clone()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    let stop_for_input = stop.clone();
    thread::spawn(move || loop {
        match input_rx.recv_timeout(Duration::from_millis(100)) {
            Ok(ev) => {
                if let Ok(data) = bincode::serialize(&ev) {
                    let len = (data.len() as u32).to_be_bytes();
                    if input_stream.write_all(&len).is_err() {
                        break;
                    }
                    if input_stream.write_all(&data).is_err() {
                        break;
                    }
                    let _ = input_stream.flush();
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                if stop_for_input.load(Ordering::SeqCst) {
                    break;
                }
            }
            Err(_) => break,
        }
    });

    let mut frames_in_window: u32 = 0;
    let mut last_stats = Instant::now();
    let mut total_expected: u64 = 0;
    let mut total_received: u64 = 0;

    let mut pending: HashMap<u32, (u32, Vec<Option<Vec<u8>>>)> = HashMap::new();
    let mut recv_buf = vec![0u8; 2048];

    while !stop.load(Ordering::SeqCst) {
        match udp.recv_from(&mut recv_buf) {
            Ok((n, _)) => {
                if let Some((block_id, shard_idx, total, orig_len, shard)) =
                    decode_udp_datagram(&recv_buf[..n])
                {
                    total_received += 1;
                    let entry = pending
                        .entry(block_id)
                        .or_insert_with(|| (orig_len, vec![None; total as usize]));
                    if (shard_idx as usize) < entry.1.len() {
                        if entry.1[shard_idx as usize].is_none() {
                            entry.1[shard_idx as usize] = Some(shard.to_vec());
                        }
                    }
                    let received: Vec<(usize, Vec<u8>)> = entry
                        .1
                        .iter()
                        .enumerate()
                        .filter_map(|(i, o)| o.as_ref().map(|s| (i, s.clone())))
                        .collect();
                    if received.len() >= FEC_DATA_SHARDS {
                        if let Ok(payload) = fec.decode(received, entry.0 as usize) {
                            total_expected += 1;
                            if let Ok(Some(yuv)) = decoder.decode(&payload) {
                                let w = yuv.width() as u32;
                                let h = yuv.height() as u32;
                                let rgb = yuv_to_rgb(&yuv);
                                let _ = tx.send(BackendMsg::Frame {
                                    width: w,
                                    height: h,
                                    rgb,
                                });
                                frames_in_window += 1;
                            }
                        }
                        pending.remove(&block_id);
                    }
                }
            }
            Err(ref e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut => {}
            Err(e) => {
                let _ = tx.send(BackendMsg::Error(format!("udp: {e}")));
                break;
            }
        }

        if last_stats.elapsed() >= Duration::from_secs(1) {
            let fps = frames_in_window as f32 / last_stats.elapsed().as_secs_f32();
            let loss = if total_received > 0 {
                (1.0 - (total_expected as f32 / total_received as f32)) * 100.0
            } else {
                0.0
            };
            let _ = tx.send(BackendMsg::Stats {
                fps,
                latency_ms: 0.0,
                packet_loss_pct: loss.max(0.0),
            });
            frames_in_window = 0;
            total_expected = 0;
            total_received = 0;
            last_stats = Instant::now();
        }

        if pending.len() > 200 {
            pending.clear();
        }
    }

    Ok(())
}
