// ============================================================
// RustDesk-Mini — Remote desktop with H.264, TLS, and UDP+FEC
//
// Usage:
//   Server (machine to control): cargo run --release -- server 0.0.0.0:9000 [--lang en|pt]
//   Client (machine controlling): cargo run --release -- client 192.168.0.10:9000 [--lang en|pt]
//
// NOTE: TLS certificates (cert.pem, key.pem) must exist in the working directory.
// Generate self-signed certs with:
//   openssl req -x509 -newkey rsa:4096 -nodes -keyout key.pem -out cert.pem -days 365 -subj '/CN=localhost'
// ============================================================

mod i18n {
    use std::sync::atomic::{AtomicU8, Ordering};

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Language {
        English = 0,
        Portuguese = 1,
    }

    static CURRENT_LANG: AtomicU8 = AtomicU8::new(0); // default: English

    pub fn set_language(lang: Language) {
        CURRENT_LANG.store(lang as u8, Ordering::SeqCst);
    }

    pub fn current() -> Language {
        match CURRENT_LANG.load(Ordering::SeqCst) {
            1 => Language::Portuguese,
            _ => Language::English,
        }
    }

    /// Message catalog. Add new keys here.
    pub fn t(key: &str) -> &'static str {
        match (current(), key) {
            (_, "app_title") => match current() {
                Language::English => "RustDesk-Mini",
                Language::Portuguese => "RustDesk-Mini",
            },
            (_, "server_waiting") => match current() {
                Language::English => "[server] Waiting for connection on {}",
                Language::Portuguese => "[servidor] Aguardando conexão em {}",
            },
            (_, "client_connected") => match current() {
                Language::English => "[client] Connected to {}",
                Language::Portuguese => "[cliente] Conectado a {}",
            },
            (_, "server_client_connected") => match current() {
                Language::English => "[server] Client connected: {}",
                Language::Portuguese => "[servidor] Cliente conectado: {}",
            },
            (_, "screen_info") => match current() {
                Language::English => "[server] Screen: {}x{}",
                Language::Portuguese => "[servidor] Tela: {}x{}",
            },
            (_, "remote_screen_info") => match current() {
                Language::English => "[client] Remote screen: {}x{}",
                Language::Portuguese => "[cliente] Tela remota: {}x{}",
            },
            (_, "client_disconnected") => match current() {
                Language::English => "[server] Client disconnected.",
                Language::Portuguese => "[servidor] Cliente desconectado.",
            },
            (_, "connection_lost") => match current() {
                Language::English => "[server] Connection lost: {}",
                Language::Portuguese => "[servidor] Conexão perdida: {}",
            },
            (_, "usage") => match current() {
                Language::English => "Usage:\n  server: cargo run -- server 0.0.0.0:9000 [--lang en|pt]\n  client: cargo run -- client <server_ip>:9000 [--lang en|pt]",
                Language::Portuguese => "Uso:\n  servidor: cargo run -- server 0.0.0.0:9000 [--lang en|pt]\n  cliente: cargo run -- client <ip_servidor>:9000 [--lang en|pt]",
            },
            (_, "invalid_mode") => match current() {
                Language::English => "Invalid mode: use 'server' or 'client'",
                Language::Portuguese => "Modo inválido: use 'server' ou 'client'",
            },
            (_, "error") => match current() {
                Language::English => "Error: {}",
                Language::Portuguese => "Erro: {}",
            },
            (_, "tls_cert_error") => match current() {
                Language::English => "Failed to load TLS certificates: {}",
                Language::Portuguese => "Falha ao carregar certificados TLS: {}",
            },
            (_, "tls_handshake_error") => match current() {
                Language::English => "TLS handshake failed: {}",
                Language::Portuguese => "Handshake TLS falhou: {}",
            },
            (_, "encoder_error") => match current() {
                Language::English => "H.264 encoder error: {}",
                Language::Portuguese => "Erro no encoder H.264: {}",
            },
            (_, "decoder_error") => match current() {
                Language::English => "H.264 decoder error: {}",
                Language::Portuguese => "Erro no decoder H.264: {}",
            },
            (_, "fec_error") => match current() {
                Language::English => "FEC error: {}",
                Language::Portuguese => "Erro FEC: {}",
            },
            (_, "escape_hint") => match current() {
                Language::English => "RustDesk-Mini — Press ESC to exit",
                Language::Portuguese => "RustDesk-Mini — Pressione ESC para sair",
            },
            (_, "shutting_down") => match current() {
                Language::English => "[client] Shutting down.",
                Language::Portuguese => "[cliente] Encerrando.",
            },
            _ => "[missing translation]",
        }
    }
}

use std::collections::HashSet;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use bincode;
use enigo::{Button, Coordinate, Direction, Enigo, Keyboard, Mouse, Settings};
use image::DynamicImage;
use minifb::{Key, MouseButton, MouseMode, Window, WindowOptions};
use openh264::encoder::{Encoder, EncoderConfig};
use openh264::formats::YUVSource;
use reed_solomon_erasure::galois_8::ReedSolomon;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rustls::{ClientConfig, ClientConnection, RootCertStore, ServerConfig, ServerConnection, StreamOwned};
use screenshots::Screen;
use serde::{Deserialize, Serialize};

// ------------------------------------------------------------
// Protocol constants
// ------------------------------------------------------------
const MAX_FRAME_BYTES: usize = 4 * 1024 * 1024; // 4 MB per compressed frame
const FEC_DATA_SHARDS: usize = 8;
const FEC_PARITY_SHARDS: usize = 2;
const FEC_SHARD_SIZE: usize = 1200; // must fit in UDP datagram
const MAX_UDP_PAYLOAD: usize = 1400;

// ------------------------------------------------------------
// Input event protocol (client -> server)
// ------------------------------------------------------------
#[derive(Serialize, Deserialize, Debug)]
enum InputEvent {
    MouseMove { x: i32, y: i32 },
    MouseDown { button: u8 },
    MouseUp { button: u8 },
    Scroll { delta: i32 },
    KeyDown { code: u32 },
    KeyUp { code: u32 },
}

// ------------------------------------------------------------
// Frame protocol helpers (TCP control channel)
// ------------------------------------------------------------
fn write_frame(stream: &mut impl Write, w: u32, h: u32, h264: &[u8]) -> std::io::Result<()> {
    stream.write_all(&w.to_be_bytes())?;
    stream.write_all(&h.to_be_bytes())?;
    stream.write_all(&(h264.len() as u32).to_be_bytes())?;
    stream.write_all(h264)?;
    stream.flush()
}

fn read_frame(stream: &mut impl Read) -> std::io::Result<(u32, u32, Vec<u8>)> {
    let mut buf4 = [0u8; 4];
    stream.read_exact(&mut buf4)?;
    let w = u32::from_be_bytes(buf4);
    stream.read_exact(&mut buf4)?;
    let h = u32::from_be_bytes(buf4);
    stream.read_exact(&mut buf4)?;
    let len = u32::from_be_bytes(buf4) as usize;
    if len > MAX_FRAME_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "frame too large",
        ));
    }
    let mut h264 = vec![0u8; len];
    stream.read_exact(&mut h264)?;
    Ok((w, h, h264))
}

// ------------------------------------------------------------
// FEC helpers using reed-solomon-erasure (Galois 8-bit)
// ------------------------------------------------------------
struct FecCodec {
    rs: ReedSolomon,
}

impl FecCodec {
    fn new() -> Result<Self, String> {
        let rs = ReedSolomon::new(FEC_DATA_SHARDS, FEC_PARITY_SHARDS)
            .map_err(|e| format!("ReedSolomon::new failed: {e}"))?;
        Ok(Self { rs })
    }

    /// Split payload into data shards, compute parity, return all shards.
    fn encode(&self, payload: &[u8]) -> Result<Vec<Vec<u8>>, String> {
        let mut shards: Vec<Vec<u8>> = Vec::with_capacity(FEC_DATA_SHARDS + FEC_PARITY_SHARDS);
        // Data shards
        for i in 0..FEC_DATA_SHARDS {
            let start = i * FEC_SHARD_SIZE;
            let end = ((i + 1) * FEC_SHARD_SIZE).min(payload.len());
            let mut shard = vec![0u8; FEC_SHARD_SIZE];
            if start < payload.len() {
                let chunk = &payload[start..end];
                shard[..chunk.len()].copy_from_slice(chunk);
            }
            shards.push(shard);
        }
        // Parity shards
        for _ in 0..FEC_PARITY_SHARDS {
            shards.push(vec![0u8; FEC_SHARD_SIZE]);
        }
        // Compute parity
        self.rs
            .encode(&mut shards)
            .map_err(|e| format!("FEC encode failed: {e}"))?;
        Ok(shards)
    }

    /// Attempt to reconstruct payload from any FEC_DATA_SHARDS out of FEC_DATA_SHARDS+FEC_PARITY_SHARDS.
    /// `present` is a Vec of (index, shard) pairs for received shards.
    fn decode(
        &self,
        present: Vec<(usize, Vec<u8>)>,
        original_len: usize,
    ) -> Result<Vec<u8>, String> {
        if present.len() < FEC_DATA_SHARDS {
            return Err(format!(
                "Not enough shards: have {}, need {}",
                present.len(),
                FEC_DATA_SHARDS
            ));
        }
        // Build optional shard slots
        let mut shards: Vec<Option<Vec<u8>>> =
            vec![None; FEC_DATA_SHARDS + FEC_PARITY_SHARDS];
        for (idx, data) in present {
            if idx < shards.len() {
                shards[idx] = Some(data);
            }
        }
        // Try to reconstruct
        self.rs
            .reconstruct(&mut shards)
            .map_err(|e| format!("FEC reconstruct failed: {e}"))?;
        // Concatenate data shards
        let mut out = Vec::with_capacity(original_len);
        for shard in shards.iter().take(FEC_DATA_SHARDS) {
            if let Some(s) = shard {
                out.extend_from_slice(s);
            }
        }
        out.truncate(original_len);
        Ok(out)
    }
}

/// Wire format for a UDP datagram:
/// [u32 block_id][u16 shard_idx][u16 total_shards][u32 original_len][payload...]
fn encode_udp_datagram(
    block_id: u32,
    shard_idx: u16,
    total_shards: u16,
    original_len: u32,
    shard: &[u8],
) -> Vec<u8> {
    let mut buf = Vec::with_capacity(12 + shard.len());
    buf.extend_from_slice(&block_id.to_be_bytes());
    buf.extend_from_slice(&shard_idx.to_be_bytes());
    buf.extend_from_slice(&total_shards.to_be_bytes());
    buf.extend_from_slice(&original_len.to_be_bytes());
    buf.extend_from_slice(shard);
    buf
}

fn decode_udp_datagram(data: &[u8]) -> Option<(u32, u16, u16, u32, &[u8])> {
    if data.len() < 12 {
        return None;
    }
    let block_id = u32::from_be_bytes(data[0..4].try_into().ok()?);
    let shard_idx = u16::from_be_bytes(data[4..6].try_into().ok()?);
    let total_shards = u16::from_be_bytes(data[6..8].try_into().ok()?);
    let original_len = u32::from_be_bytes(data[8..12].try_into().ok()?);
    Some((block_id, shard_idx, total_shards, original_len, &data[12..]))
}

// ------------------------------------------------------------
// TLS helpers (rustls)
// ------------------------------------------------------------
fn load_certs(path: &str) -> std::io::Result<Vec<CertificateDer<'static>>> {
    let file = std::fs::File::open(path)?;
    let mut reader = std::io::BufReader::new(file);
    let certs: Vec<_> = rustls_pemfile::certs(&mut reader)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(certs)
}

fn load_private_key(path: &str) -> std::io::Result<PrivateKeyDer<'static>> {
    let file = std::fs::File::open(path)?;
    let mut reader = std::io::BufReader::new(file);
    rustls_pemfile::private_key(&mut reader)?
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "no private key"))
}

fn make_server_tls_config(cert_path: &str, key_path: &str) -> Result<Arc<ServerConfig>, String> {
    let certs = load_certs(cert_path).map_err(|e| format!("certs: {e}"))?;
    let key = load_private_key(key_path).map_err(|e| format!("key: {e}"))?;
    let config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|e| format!("ServerConfig: {e}"))?;
    Ok(Arc::new(config))
}

fn make_client_tls_config() -> Result<Arc<ClientConfig>, String> {
    let mut roots = RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    // Also accept our self-signed cert if present
    if let Ok(certs) = load_certs("cert.pem") {
        for c in certs {
            let _ = roots.add(c);
        }
    }
    let config = ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    Ok(Arc::new(config))
}

// ------------------------------------------------------------
// H.264 helpers (openh264)
// ------------------------------------------------------------
struct H264EncoderState {
    encoder: Encoder,
    width: u32,
    height: u32,
}

impl H264EncoderState {
    fn new(width: u32, height: u32) -> Result<Self, String> {
        let config = EncoderConfig::new(width, height);
        let encoder =
            Encoder::with_config(config).map_err(|e| format!("Encoder::with_config: {e}"))?;
        Ok(Self {
            encoder,
            width,
            height,
        })
    }

    fn encode_rgb(&mut self, rgb: &[u8]) -> Result<Vec<u8>, String> {
        // OpenH264 expects YUV420; convert RGB -> YUV
        let yuv = rgb_to_yuv420(rgb, self.width, self.height);
        let bitstream = self
            .encoder
            .encode(&yuv)
            .map_err(|e| format!("encode: {e}"))?;
        Ok(bitstream.to_vec())
    }
}

/// Minimal RGB->YUV420 conversion (BT.601 full-range approximation).
fn rgb_to_yuv420(rgb: &[u8], w: u32, h: u32) -> openh264::formats::YUVBuffer {
    let wu = w as usize;
    let hu = h as usize;
    let mut y_plane = vec![0u8; wu * hu];
    let mut u_plane = vec![0u8; (wu / 2) * (hu / 2)];
    let mut v_plane = vec![0u8; (wu / 2) * (hu / 2)];

    for y in 0..hu {
        for x in 0..wu {
            let idx = (y * wu + x) * 3;
            let r = rgb[idx] as f32;
            let g = rgb[idx + 1] as f32;
            let b = rgb[idx + 2] as f32;
            let yv = 0.299 * r + 0.587 * g + 0.114 * b;
            y_plane[y * wu + x] = yv.clamp(0.0, 255.0) as u8;
        }
    }
    for y in (0..hu).step_by(2) {
        for x in (0..wu).step_by(2) {
            let mut rsum = 0.0f32;
            let mut gsum = 0.0f32;
            let mut bsum = 0.0f32;
            for dy in 0..2 {
                for dx in 0..2 {
                    let px = x + dx;
                    let py = y + dy;
                    if px < wu && py < hu {
                        let idx = (py * wu + px) * 3;
                        rsum += rgb[idx] as f32;
                        gsum += rgb[idx + 1] as f32;
                        bsum += rgb[idx + 2] as f32;
                    }
                }
            }
            let r = rsum / 4.0;
            let g = gsum / 4.0;
            let b = bsum / 4.0;
            let u = -0.169 * r - 0.331 * g + 0.5 * b + 128.0;
            let v = 0.5 * r - 0.419 * g - 0.081 * b + 128.0;
            let ui = (y / 2) * (wu / 2) + (x / 2);
            u_plane[ui] = u.clamp(0.0, 255.0) as u8;
            v_plane[ui] = v.clamp(0.0, 255.0) as u8;
        }
    }

    openh264::formats::YUVBuffer::new_from_yuv(y_plane, u_plane, v_plane)
}

// ------------------------------------------------------------
// Server: capture screen, encode H.264, send via UDP+FEC,
//         receive input over TLS TCP
// ------------------------------------------------------------
fn run_server(bind: &str) -> std::io::Result<()> {
    let tls_config = make_server_tls_config("cert.pem", "key.pem")
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

    // TCP listener for control (input events) over TLS
    let listener = TcpListener::bind(bind)?;
    println!(
        "{}",
        i18n::t("server_waiting").replace("{}", bind)
    );

    // UDP socket for video frames
    let udp_bind = bind.replace("0.0.0.0", "0.0.0.0"); // keep same
    let udp_socket = UdpSocket::bind(&udp_bind)?;
    udp_socket.set_nonblocking(false)?;

    // Accept one TCP client
    let (tcp_stream, peer) = listener.accept()?;
    println!(
        "{}",
        i18n::t("server_client_connected").replace("{}", &peer.to_string())
    );

    let conn = ServerConnection::new(tls_config)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    let mut tls_stream = StreamOwned::new(conn, tcp_stream);
    tls_stream
        .conn
        .complete_io(&mut tls_stream.sock)
        .map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("TLS handshake: {e}"),
            )
        })?;

    // Discover screen
    let screens = Screen::all().map_err(|e| {
        std::io::Error::new(std::io::ErrorKind::Other, format!("screens: {e}"))
    })?;
    let screen = screens
        .into_iter()
        .next()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "no screen"))?;
    let sw = screen.display_info.width;
    let sh = screen.display_info.height;
    println!("{}", i18n::t("screen_info").replace("{}", &format!("{}x{}", sw, sh)));

    // The remote client address (for UDP frame sending)
    let client_addr: SocketAddr = peer;

    // Encoder
    let mut encoder = H264EncoderState::new(sw, sh)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    let fec = FecCodec::new()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

    // Shared flag to stop on client disconnect
    let connected = Arc::new(AtomicBool::new(true));
    let connected_clone = connected.clone();

    // Input thread: receives InputEvents from TLS stream
    let mut input_stream = tls_stream
        .sock
        .try_clone()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    thread::spawn(move || {
        let mut conn = match ServerConnection::new(tls_config) {
            Ok(c) => c,
            Err(_) => {
                connected_clone.store(false, Ordering::SeqCst);
                return;
            }
        };
        // Note: this is a simplified approach — in production, reuse the
        // same connection. Here we just read raw events from the TLS stream.
        let mut enigo = match Enigo::new(&Settings::default()) {
            Ok(e) => e,
            Err(e) => {
                eprintln!("[server] Enigo init failed: {e}");
                return;
            }
        };
        loop {
            let mut len_buf = [0u8; 4];
            if input_stream.read_exact(&mut len_buf).is_err() {
                connected_clone.store(false, Ordering::SeqCst);
                break;
            }
            let len = u32::from_be_bytes(len_buf) as usize;
            if len > 4096 {
                connected_clone.store(false, Ordering::SeqCst);
                break;
            }
            let mut data = vec![0u8; len];
            if input_stream.read_exact(&mut data).is_err() {
                connected_clone.store(false, Ordering::SeqCst);
                break;
            }
            if let Ok(ev) = bincode::deserialize::<InputEvent>(&data) {
                apply_event(&mut enigo, ev);
            }
        }
        let _ = conn; // keep connection alive (simplified)
    });

    // Main loop: capture, encode, FEC-send over UDP
    let target_fps = 15u64;
    let frame_dur = Duration::from_millis(1000 / target_fps);
    let mut last = Instant::now();
    let mut block_id: u32 = 0;

    while connected.load(Ordering::SeqCst) {
        let img = screen.capture().map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, format!("capture: {e}"))
        })?;

        // Build raw RGB slice
        let rgb_raw = img.into_raw(); // Vec<u8>, RGBA or RGB depending on crate
        // screenshots crate returns RGB (3 bytes per pixel)
        let rgb_slice = &rgb_raw[..(sw * sh * 3) as usize];

        // Encode H.264
        let h264 = match encoder.encode_rgb(rgb_slice) {
            Ok(h) => h,
            Err(e) => {
                eprintln!("{}", i18n::t("encoder_error").replace("{}", &e));
                continue;
            }
        };

        // Send over UDP with FEC
        if let Err(e) = send_fec_frame(&udp_socket, &client_addr, &fec, block_id, &h264) {
            eprintln!("{}", i18n::t("fec_error").replace("{}", &e));
        }
        block_id = block_id.wrapping_add(1);

        let elapsed = last.elapsed();
        if elapsed < frame_dur {
            thread::sleep(frame_dur - elapsed);
        }
        last = Instant::now();
    }

    println!("{}", i18n::t("client_disconnected"));
    Ok(())
}

fn send_fec_frame(
    udp: &UdpSocket,
    dest: &SocketAddr,
    fec: &FecCodec,
    block_id: u32,
    payload: &[u8],
) -> Result<(), String> {
    let shards = fec.encode(payload)?;
    let total = shards.len() as u16;
    for (idx, shard) in shards.iter().enumerate() {
        let dg = encode_udp_datagram(block_id, idx as u16, total, payload.len() as u32, shard);
        if dg.len() > MAX_UDP_PAYLOAD {
            return Err(format!("datagram too large: {}", dg.len()));
        }
        udp.send_to(&dg, dest)
            .map_err(|e| format!("send_to: {e}"))?;
    }
    Ok(())
}

// ------------------------------------------------------------
// Client: receive UDP+FEC frames, decode H.264, display,
//         send input over TLS TCP
// ------------------------------------------------------------
fn run_client(addr: &str) -> std::io::Result<()> {
    let tls_config = make_client_tls_config()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

    // TCP for input events
    let server_addr: SocketAddr = addr
        .parse()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, format!("addr: {e}")))?;
    let tcp_stream = TcpStream::connect(server_addr)?;
    tcp_stream.set_nodelay(true).ok();

    let server_name = rustls::pki_types::ServerName::try_from("localhost")
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, format!("sni: {e}")))?
        .to_owned();
    let conn = ClientConnection::new(tls_config, server_name)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    let mut tls_stream = StreamOwned::new(conn, tcp_stream);
    tls_stream
        .conn
        .complete_io(&mut tls_stream.sock)
        .map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("TLS handshake: {e}"),
            )
        })?;

    println!(
        "{}",
        i18n::t("client_connected").replace("{}", addr)
    );

    // UDP socket for receiving frames
    let udp = UdpSocket::bind("0.0.0.0:0")?;
    udp.set_read_timeout(Some(Duration::from_millis(100)))?;

    let fec = FecCodec::new()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

    // H.264 decoder
    let mut decoder = openh264::decoder::Decoder::new()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("decoder: {e}")))?;

    // Window will be created after first frame
    let mut window: Option<Window> = None;
    let mut frame_buf: Vec<u32> = Vec::new();

    // State for reassembly
    let mut pending: std::collections::HashMap<u32, (u32, Vec<Option<Vec<u8>>>)> =
        std::collections::HashMap::new();

    // Input state
    let mut last_mouse: (f32, f32) = (0.0, 0.0);
    let mut buttons_down = [false; 3];
    let mut keys_down: HashSet<Key> = HashSet::new();

    loop {
        // --- Receive UDP datagrams and reassemble ---
        let mut recv_buf = [0u8; 2048];
        match udp.recv_from(&mut recv_buf) {
            Ok((n, _src)) => {
                if let Some((block_id, shard_idx, total, original_len, shard)) =
                    decode_udp_datagram(&recv_buf[..n])
                {
                    let entry = pending.entry(block_id).or_insert_with(|| {
                        (original_len, vec![None; total as usize])
                    });
                    if (shard_idx as usize) < entry.1.len() {
                        entry.1[shard_idx as usize] = Some(shard.to_vec());
                    }

                    // Try to decode once we have enough shards
                    let received: Vec<(usize, Vec<u8>)> = entry
                        .1
                        .iter()
                        .enumerate()
                        .filter_map(|(i, opt)| opt.as_ref().map(|s| (i, s.clone())))
                        .collect();
                    if received.len() >= FEC_DATA_SHARDS {
                        if let Ok(payload) = fec.decode(received, entry.0 as usize) {
                            // Decode H.264
                            match decoder.decode(&payload) {
                                Ok(Some(yuv)) => {
                                    let w = yuv.width() as u32;
                                    let h = yuv.height() as u32;
                                    // Convert YUV -> RGB for display
                                    let rgb = yuv_to_rgb(&yuv);
                                    frame_buf = rgb_to_u32_buffer(&rgb, w, h);

                                    // Create window on first frame
                                    if window.is_none() {
                                        let win = Window::new(
                                            i18n::t("escape_hint"),
                                            w as usize,
                                            h as usize,
                                            WindowOptions::default(),
                                        )
                                        .map_err(|e| {
                                            std::io::Error::new(
                                                std::io::ErrorKind::Other,
                                                e.to_string(),
                                            )
                                        })?;
                                        win.set_target_fps(60);
                                        window = Some(win);
                                        println!(
                                            "{}",
                                            i18n::t("remote_screen_info")
                                                .replace("{}", &format!("{}x{}", w, h))
                                        );
                                    }
                                }
                                Ok(None) => { /* need more data */ }
                                Err(e) => {
                                    eprintln!("{}", i18n::t("decoder_error").replace("{}", &e));
                                }
                            }
                        }
                        pending.remove(&block_id);
                    }
                }
            }
            Err(ref e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                // no data, continue
            }
            Err(e) => {
                eprintln!("[client] UDP recv error: {e}");
            }
        }

        // --- Window rendering and input ---
        if let Some(ref mut win) = window {
            if !win.is_open() || win.is_key_down(Key::Escape) {
                break;
            }

            // Mouse move
            if let Some((mx, my)) = win.get_mouse_pos(MouseMode::Discard) {
                if (mx, my) != last_mouse {
                    let ev = InputEvent::MouseMove {
                        x: mx as i32,
                        y: my as i32,
                    };
                    send_event_tls(&mut tls_stream, &ev);
                    last_mouse = (mx, my);
                }
            }

            // Mouse buttons
            let mb_states = [
                (0u8, MouseButton::Left),
                (1u8, MouseButton::Right),
                (2u8, MouseButton::Middle),
            ];
            for (idx, (btn_id, btn)) in mb_states.iter().enumerate() {
                let down = win.get_mouse_down(*btn);
                if down != buttons_down[idx] {
                    let ev = if down {
                        InputEvent::MouseDown { button: *btn_id }
                    } else {
                        InputEvent::MouseUp { button: *btn_id }
                    };
                    send_event_tls(&mut tls_stream, &ev);
                    buttons_down[idx] = down;
                }
            }

            // Scroll
            if let Some((_, wy)) = win.get_scroll_wheel() {
                if wy.abs() > 0.01 {
                    let ev = InputEvent::Scroll {
                        delta: if wy > 0.0 { 1 } else { -1 },
                    };
                    send_event_tls(&mut tls_stream, &ev);
                }
            }

            // Keys
            let pressed = win.get_keys();
            for k in &pressed {
                if !keys_down.contains(k) {
                    keys_down.insert(*k);
                    if let Some(code) = key_to_code(*k) {
                        send_event_tls(&mut tls_stream, &InputEvent::KeyDown { code });
                    }
                }
            }
            let released: Vec<Key> = keys_down
                .iter()
                .filter(|k| !pressed.contains(k))
                .cloned()
                .collect();
            for k in released {
                keys_down.remove(&k);
                if let Some(code) = key_to_code(k) {
                    send_event_tls(&mut tls_stream, &InputEvent::KeyUp { code });
                }
            }

            // Render
            if !frame_buf.is_empty() {
                let (w, h) = (win.get_size().0, win.get_size().1);
                // Pad or truncate buffer to window size if needed
                if frame_buf.len() < w * h {
                    let mut padded = frame_buf.clone();
                    padded.resize(w * h, 0);
                    let _ = win.update_with_buffer(&padded, w, h);
                } else {
                    let _ = win.update_with_buffer(&frame_buf[..w * h], w, h);
                }
            }
        }
    }

    println!("{}", i18n::t("shutting_down"));
    Ok(())
}

fn send_event_tls(stream: &mut impl Write, ev: &InputEvent) {
    if let Ok(data) = bincode::serialize(ev) {
        let len = (data.len() as u32).to_be_bytes();
        let _ = stream.write_all(&len);
        let _ = stream.write_all(&data);
        let _ = stream.flush();
    }
}

/// Convert YUV420 buffer (openh264) to RGB bytes.
fn yuv_to_rgb(yuv: &openh264::formats::YUVBuffer) -> Vec<u8> {
    let w = yuv.width() as usize;
    let h = yuv.height() as usize;
    let y_plane = yuv.y_plane();
    let u_plane = yuv.u_plane();
    let v_plane = yuv.v_plane();
    let mut rgb = vec![0u8; w * h * 3];
    for y in 0..h {
        for x in 0..w {
            let yi = y * w + x;
            let ui = (y / 2) * (w / 2) + (x / 2);
            let vi = (y / 2) * (w / 2) + (x / 2);
            let yv = y_plane[yi] as f32;
            let uv = u_plane[ui] as f32 - 128.0;
            let vv = v_plane[vi] as f32 - 128.0;
            let r = (yv + 1.402 * vv).clamp(0.0, 255.0) as u8;
            let g = (yv - 0.344 * uv - 0.714 * vv).clamp(0.0, 255.0) as u8;
            let b = (yv + 1.772 * uv).clamp(0.0, 255.0) as u8;
            let o = yi * 3;
            rgb[o] = r;
            rgb[o + 1] = g;
            rgb[o + 2] = b;
        }
    }
    rgb
}

fn rgb_to_u32_buffer(rgb: &[u8], w: u32, h: u32) -> Vec<u32> {
    let mut buf = Vec::with_capacity((w * h) as usize);
    for i in 0..(w * h) as usize {
        let r = rgb[i * 3] as u32;
        let g = rgb[i * 3 + 1] as u32;
        let b = rgb[i * 3 + 2] as u32;
        buf.push((r << 16) | (g << 8) | b);
    }
    buf
}

// ------------------------------------------------------------
// Input mapping (server side)
// ------------------------------------------------------------
fn apply_event(enigo: &mut Enigo, ev: InputEvent) {
    match ev {
        InputEvent::MouseMove { x, y } => {
            let _ = enigo.move_mouse(x, y, Coordinate::Abs);
        }
        InputEvent::MouseDown { button } => {
            let _ = enigo.button(map_button(button), Direction::Press);
        }
        InputEvent::MouseUp { button } => {
            let _ = enigo.button(map_button(button), Direction::Release);
        }
        InputEvent::Scroll { delta } => {
            let _ = enigo.scroll(delta.abs() * 3, enigo::Axis::Vertical);
        }
        InputEvent::KeyDown { code } => {
            if let Some(k) = map_key(code) {
                let _ = enigo.key(k, Direction::Press);
            }
        }
        InputEvent::KeyUp { code } => {
            if let Some(k) = map_key(code) {
                let _ = enigo.key(k, Direction::Release);
            }
        }
    }
}

fn map_button(b: u8) -> Button {
    match b {
        0 => Button::Left,
        1 => Button::Right,
        2 => Button::Middle,
        _ => Button::Left,
    }
}

fn map_key(code: u32) -> Option<enigo::Key> {
    use enigo::Key::*;
    match code {
        65 => Some(Unicode('a')),
        66 => Some(Unicode('b')),
        67 => Some(Unicode('c')),
        68 => Some(Unicode('d')),
        69 => Some(Unicode('e')),
        70 => Some(Unicode('f')),
        71 => Some(Unicode('g')),
        72 => Some(Unicode('h')),
        73 => Some(Unicode('i')),
        74 => Some(Unicode('j')),
        75 => Some(Unicode('k')),
        76 => Some(Unicode('l')),
        77 => Some(Unicode('m')),
        78 => Some(Unicode('n')),
        79 => Some(Unicode('o')),
        80 => Some(Unicode('p')),
        81 => Some(Unicode('q')),
        82 => Some(Unicode('r')),
        83 => Some(Unicode('s')),
        84 => Some(Unicode('t')),
        85 => Some(Unicode('u')),
        86 => Some(Unicode('v')),
        87 => Some(Unicode('w')),
        88 => Some(Unicode('x')),
        89 => Some(Unicode('y')),
        90 => Some(Unicode('z')),
        48..=57 => Some(Unicode((b'0' + (code - 48) as u8) as char)),
        32 => Some(Space),
        13 => Some(Return),
        27 => Some(Escape),
        8 => Some(Backspace),
        9 => Some(Tab),
        37 => Some(LeftArrow),
        38 => Some(UpArrow),
        39 => Some(RightArrow),
        40 => Some(DownArrow),
        _ => None,
    }
}

// minifb Key -> u32 code (client side)
fn key_to_code(k: Key) -> Option<u32> {
    Some(match k {
        Key::A => 65,
        Key::B => 66,
        Key::C => 67,
        Key::D => 68,
        Key::E => 69,
        Key::F => 70,
        Key::G => 71,
        Key::H => 72,
        Key::I => 73,
        Key::J => 74,
        Key::K => 75,
        Key::L => 76,
        Key::M => 77,
        Key::N => 78,
        Key::O => 79,
        Key::P => 80,
        Key::Q => 81,
        Key::R => 82,
        Key::S => 83,
        Key::T => 84,
        Key::U => 85,
        Key::V => 86,
        Key::W => 87,
        Key::X => 88,
        Key::Y => 89,
        Key::Z => 90,
        Key::Key0 => 48,
        Key::Key1 => 49,
        Key::Key2 => 50,
        Key::Key3 => 51,
        Key::Key4 => 52,
        Key::Key5 => 53,
        Key::Key6 => 54,
        Key::Key7 => 55,
        Key::Key8 => 56,
        Key::Key9 => 57,
        Key::Space => 32,
        Key::Enter => 13,
        Key::Escape => 27,
        Key::Backspace => 8,
        Key::Tab => 9,
        Key::Left => 37,
        Key::Up => 38,
        Key::Right => 39,
        Key::Down => 40,
        _ => return None,
    })
}

// ------------------------------------------------------------
// Main
// ------------------------------------------------------------
fn main() {
    let args: Vec<String> = std::env::args().collect();

    // Parse --lang flag (default English)
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--lang" && i + 1 < args.len() {
            match args[i + 1].as_str() {
                "pt" | "pt-BR" | "portuguese" => {
                    i18n::set_language(i18n::Language::Portuguese);
                }
                _ => {
                    i18n::set_language(i18n::Language::English);
                }
            }
            i += 2;
        } else {
            i += 1;
        }
    }

    if args.len() < 3 {
        eprintln!("{}", i18n::t("usage"));
        std::process::exit(1);
    }
    let mode = args[1].as_str();
    let addr = args[2].as_str();

    let result = match mode {
        "server" => run_server(addr),
        "client" => run_client(addr),
        _ => {
            eprintln!("{}", i18n::t("invalid_mode"));
            std::process::exit(1);
        }
    };

    if let Err(e) = result {
        eprintln!("{}", i18n::t("error").replace("{}", &e.to_string()));
        std::process::exit(1);
    }
}
