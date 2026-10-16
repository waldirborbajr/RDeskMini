// ============================================================
// RustDesk-Mini — Native GUI remote desktop (eframe/egui)
// Runs natively on Linux, Windows, and macOS. No WebView.
//
// Usage:
//   cargo run --release -- --mode host      (default)
//   cargo run --release -- --mode client
//
// TLS certificates are generated automatically on first run and
// stored per-user (no openssl needed, no user interaction).
// ============================================================

// ============================================================
// i18n — English default, Portuguese optional
// ============================================================
mod i18n {
    use std::sync::atomic::{AtomicU8, Ordering};

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Language {
        English = 0,
        Portuguese = 1,
    }

    static CURRENT: AtomicU8 = AtomicU8::new(0);

    pub fn set_language(lang: Language) {
        CURRENT.store(lang as u8, Ordering::SeqCst);
    }

    pub fn current() -> Language {
        match CURRENT.load(Ordering::SeqCst) {
            1 => Language::Portuguese,
            _ => Language::English,
        }
    }

    pub fn t(key: &str) -> &'static str {
        use Language::*;
        let en = current() == English;
        match key {
            "app_title" => "RustDesk-Mini",
            "this_device_is" => {
                if en {
                    "This device is:"
                } else {
                    "Este dispositivo é:"
                }
            }
            "mode_host" => {
                if en {
                    "Host (waiting for connection)"
                } else {
                    "Servidor (aguardando conexão)"
                }
            }
            "mode_client" => {
                if en {
                    "Client (connect to remote)"
                } else {
                    "Cliente (conectar ao remoto)"
                }
            }
            "your_address" => {
                if en {
                    "Your address:"
                } else {
                    "Seu endereço:"
                }
            }
            "remote_address" => {
                if en {
                    "Remote address:"
                } else {
                    "Endereço remoto:"
                }
            }
            "remote_addr_hint" => {
                if en {
                    "e.g. 192.168.0.10:9000"
                } else {
                    "ex.: 192.168.0.10:9000"
                }
            }
            "connect" => {
                if en {
                    "Connect"
                } else {
                    "Conectar"
                }
            }
            "start_hosting" => {
                if en {
                    "Start hosting"
                } else {
                    "Iniciar hospedagem"
                }
            }
            "stop_hosting" => {
                if en {
                    "Stop hosting"
                } else {
                    "Parar hospedagem"
                }
            }
            "disconnect" => {
                if en {
                    "Disconnect"
                } else {
                    "Desconectar"
                }
            }
            "fullscreen" => {
                if en {
                    "Fullscreen"
                } else {
                    "Tela cheia"
                }
            }
            "exit_fullscreen" => {
                if en {
                    "Exit fullscreen"
                } else {
                    "Sair de tela cheia"
                }
            }
            "fit_to_window" => {
                if en {
                    "Fit to window"
                } else {
                    "Ajustar à janela"
                }
            }
            "status_idle" => {
                if en {
                    "Idle"
                } else {
                    "Ocioso"
                }
            }
            "status_listening" => {
                if en {
                    "Listening on {}"
                } else {
                    "Escutando em {}"
                }
            }
            "status_connecting" => {
                if en {
                    "Connecting to {}…"
                } else {
                    "Conectando a {}…"
                }
            }
            "status_connected" => {
                if en {
                    "Connected to {}"
                } else {
                    "Conectado a {}"
                }
            }
            "status_error" => {
                if en {
                    "Error: {}"
                } else {
                    "Erro: {}"
                }
            }
            "status_disconnected" => {
                if en {
                    "Disconnected"
                } else {
                    "Desconectado"
                }
            }
            "fps" => {
                if en {
                    "FPS:"
                } else {
                    "FPS:"
                }
            }
            "latency" => {
                if en {
                    "Latency:"
                } else {
                    "Latência:"
                }
            }
            "packet_loss" => {
                if en {
                    "Packet loss:"
                } else {
                    "Perda de pacotes:"
                }
            }
            "language" => {
                if en {
                    "Language"
                } else {
                    "Idioma"
                }
            }
            "waiting_peer" => {
                if en {
                    "Waiting for peer…"
                } else {
                    "Aguardando par…"
                }
            }
            "session_active" => {
                if en {
                    "Session active"
                } else {
                    "Sessão ativa"
                }
            }
            _ => "[?]",
        }
    }
}

// ============================================================
// Transparent TLS certificate management
// ============================================================
mod cert {
    use std::fs;
    use std::io::Write;
    use std::path::PathBuf;
    use std::sync::Arc;

    use directories::ProjectDirs;
    use rcgen::{generate_simple_self_signed, CertifiedKey};
    use rustls::pki_types::{CertificateDer, PrivateKeyDer};
    use rustls::{ClientConfig, ServerConfig};

    fn config_dir() -> std::io::Result<PathBuf> {
        let dirs = ProjectDirs::from("com", "rustdesk-mini", "RustDesk-Mini").ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "cannot determine config directory",
            )
        })?;
        let dir = dirs.config_dir().to_path_buf();
        fs::create_dir_all(&dir)?;
        Ok(dir)
    }

    fn cert_path() -> std::io::Result<PathBuf> {
        Ok(config_dir()?.join("cert.pem"))
    }

    fn key_path() -> std::io::Result<PathBuf> {
        Ok(config_dir()?.join("key.pem"))
    }

    /// Ensures a self-signed cert/key pair exists on disk.
    /// Generates them silently on first run.
    pub fn ensure_certificates() -> std::io::Result<(PathBuf, PathBuf)> {
        let cp = cert_path()?;
        let kp = key_path()?;

        let needs_generation = !cp.exists() || !kp.exists() || !is_valid_pem(&cp, &kp);
        if needs_generation {
            generate_and_write(&cp, &kp)?;
        }
        Ok((cp, kp))
    }

    fn is_valid_pem(cp: &PathBuf, kp: &PathBuf) -> bool {
        let cert_ok = fs::read_to_string(cp)
            .map(|s| s.contains("BEGIN CERTIFICATE") && s.len() > 100)
            .unwrap_or(false);
        let key_ok = fs::read_to_string(kp)
            .map(|s| s.contains("PRIVATE KEY") && s.len() > 100)
            .unwrap_or(false);
        cert_ok && key_ok
    }

    fn generate_and_write(cp: &PathBuf, kp: &PathBuf) -> std::io::Result<()> {
        let subject_alt_names = vec![
            "localhost".to_string(),
            "127.0.0.1".to_string(),
            "::1".to_string(),
            "rustdesk-mini.local".to_string(),
        ];

        let CertifiedKey { cert, key_pair } = generate_simple_self_signed(subject_alt_names)
            .map_err(|e| {
                std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("cert generation failed: {e}"),
                )
            })?;

        {
            let mut f = fs::File::create(cp)?;
            f.write_all(cert.pem().as_bytes())?;
            f.sync_all()?;
        }
        {
            let mut opts = fs::OpenOptions::new();
            opts.write(true).create(true).truncate(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                opts.mode(0o600);
            }
            let mut f = opts.open(kp)?;
            f.write_all(key_pair.serialize_pem().as_bytes())?;
            f.sync_all()?;
        }
        Ok(())
    }

    pub fn server_tls_config() -> Result<Arc<ServerConfig>, String> {
        let (cp, kp) = ensure_certificates().map_err(|e| e.to_string())?;

        let cert_data = fs::read_to_string(&cp).map_err(|e| format!("read cert: {e}"))?;
        let key_data = fs::read_to_string(&kp).map_err(|e| format!("read key: {e}"))?;

        let certs: Vec<CertificateDer<'static>> =
            rustls_pemfile::certs(&mut std::io::BufReader::new(cert_data.as_bytes()))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| format!("parse cert: {e}"))?;

        let key: PrivateKeyDer<'static> =
            rustls_pemfile::private_key(&mut std::io::BufReader::new(key_data.as_bytes()))
                .map_err(|e| format!("parse key: {e}"))?
                .ok_or_else(|| "no private key found".to_string())?;

        let cfg = ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(certs, key)
            .map_err(|e| format!("ServerConfig: {e}"))?;

        Ok(Arc::new(cfg))
    }

    /// Client config that accepts any server certificate.
    /// Trade-off: TLS 1.3 encryption is still active, but no chain validation.
    /// Same model as AnyDesk on LAN without an account.
    pub fn client_tls_config() -> Result<Arc<ClientConfig>, String> {
        let config = ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(NoVerifier))
            .with_no_client_auth();
        Ok(Arc::new(config))
    }

    #[derive(Debug)]
    struct NoVerifier;

    impl rustls::client::danger::ServerCertVerifier for NoVerifier {
        fn verify_server_cert(
            &self,
            _end_entity: &CertificateDer<'_>,
            _intermediates: &[CertificateDer<'_>],
            _server_name: &rustls::pki_types::ServerName<'_>,
            _ocsp_response: &[u8],
            _now: rustls::pki_types::UnixTime,
        ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
            Ok(rustls::client::danger::ServerCertVerified::assertion())
        }

        fn verify_tls12_signature(
            &self,
            _message: &[u8],
            _cert: &CertificateDer<'_>,
            _dss: &rustls::DigitallySignedStruct,
        ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
            Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
        }

        fn verify_tls13_signature(
            &self,
            _message: &[u8],
            _cert: &CertificateDer<'_>,
            _dss: &rustls::DigitallySignedStruct,
        ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
            Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
        }

        fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
            use rustls::SignatureScheme::*;
            vec![
                RSA_PKCS1_SHA256,
                RSA_PKCS1_SHA384,
                RSA_PKCS1_SHA512,
                ECDSA_NISTP256_SHA256,
                ECDSA_NISTP384_SHA384,
                ED25519,
                RSA_PSS_SHA256,
                RSA_PSS_SHA384,
                RSA_PSS_SHA512,
            ]
        }
    }
}

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use bincode;
use eframe::egui;
use egui::{ColorImage, TextureHandle, TextureOptions};
use enigo::{Button, Coordinate, Direction, Enigo, Keyboard, Mouse, Settings};
use openh264::encoder::{Encoder, EncoderConfig};
use reed_solomon_erasure::galois_8::ReedSolomon;
use rustls::{ClientConnection, ServerConnection, StreamOwned};
use screenshots::Screen;
use serde::{Deserialize, Serialize};

// ------------------------------------------------------------
// Protocol constants
// ------------------------------------------------------------
const FEC_DATA_SHARDS: usize = 8;
const FEC_PARITY_SHARDS: usize = 2;
const FEC_SHARD_SIZE: usize = 1200;
const MAX_UDP_PAYLOAD: usize = 1400;

// ------------------------------------------------------------
// Input event protocol
// ------------------------------------------------------------
#[derive(Serialize, Deserialize, Debug, Clone)]
enum InputEvent {
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
enum BackendMsg {
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
enum AppMode {
    Host,
    Client,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SessionState {
    Idle,
    Listening,
    Connecting,
    Connected,
    Error,
    Disconnected,
}

// ============================================================
// FEC helpers
// ============================================================
struct FecCodec {
    rs: ReedSolomon,
}

impl FecCodec {
    fn new() -> Result<Self, String> {
        let rs = ReedSolomon::new(FEC_DATA_SHARDS, FEC_PARITY_SHARDS)
            .map_err(|e| format!("ReedSolomon::new: {e}"))?;
        Ok(Self { rs })
    }

    fn encode(&self, payload: &[u8]) -> Result<Vec<Vec<u8>>, String> {
        let mut shards: Vec<Vec<u8>> = Vec::with_capacity(FEC_DATA_SHARDS + FEC_PARITY_SHARDS);
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
        for _ in 0..FEC_PARITY_SHARDS {
            shards.push(vec![0u8; FEC_SHARD_SIZE]);
        }
        self.rs
            .encode(&mut shards)
            .map_err(|e| format!("FEC encode: {e}"))?;
        Ok(shards)
    }

    fn decode(
        &self,
        present: Vec<(usize, Vec<u8>)>,
        original_len: usize,
    ) -> Result<Vec<u8>, String> {
        if present.len() < FEC_DATA_SHARDS {
            return Err(format!(
                "Not enough shards: {} < {}",
                present.len(),
                FEC_DATA_SHARDS
            ));
        }
        let mut shards: Vec<Option<Vec<u8>>> = vec![None; FEC_DATA_SHARDS + FEC_PARITY_SHARDS];
        for (i, d) in present {
            if i < shards.len() {
                shards[i] = Some(d);
            }
        }
        self.rs
            .reconstruct(&mut shards)
            .map_err(|e| format!("FEC reconstruct: {e}"))?;
        let mut out = Vec::with_capacity(original_len);
        for s in shards.iter().take(FEC_DATA_SHARDS) {
            if let Some(sd) = s {
                out.extend_from_slice(sd);
            }
        }
        out.truncate(original_len);
        Ok(out)
    }
}

fn encode_udp_datagram(
    block_id: u32,
    shard_idx: u16,
    total: u16,
    orig_len: u32,
    shard: &[u8],
) -> Vec<u8> {
    let mut buf = Vec::with_capacity(12 + shard.len());
    buf.extend_from_slice(&block_id.to_be_bytes());
    buf.extend_from_slice(&shard_idx.to_be_bytes());
    buf.extend_from_slice(&total.to_be_bytes());
    buf.extend_from_slice(&orig_len.to_be_bytes());
    buf.extend_from_slice(shard);
    buf
}

fn decode_udp_datagram(data: &[u8]) -> Option<(u32, u16, u16, u32, &[u8])> {
    if data.len() < 12 {
        return None;
    }
    let b = u32::from_be_bytes(data[0..4].try_into().ok()?);
    let si = u16::from_be_bytes(data[4..6].try_into().ok()?);
    let t = u16::from_be_bytes(data[6..8].try_into().ok()?);
    let ol = u32::from_be_bytes(data[8..12].try_into().ok()?);
    Some((b, si, t, ol, &data[12..]))
}

// ============================================================
// H.264 helpers
// ============================================================
struct H264EncoderState {
    encoder: Encoder,
    width: u32,
    height: u32,
}

impl H264EncoderState {
    fn new(w: u32, h: u32) -> Result<Self, String> {
        let cfg = EncoderConfig::new(w, h);
        let enc = Encoder::with_config(cfg).map_err(|e| format!("Encoder: {e}"))?;
        Ok(Self {
            encoder: enc,
            width: w,
            height: h,
        })
    }

    fn encode_rgb(&mut self, rgb: &[u8]) -> Result<Vec<u8>, String> {
        let yuv = rgb_to_yuv420(rgb, self.width, self.height);
        let bits = self
            .encoder
            .encode(&yuv)
            .map_err(|e| format!("encode: {e}"))?;
        Ok(bits.to_vec())
    }
}

fn rgb_to_yuv420(rgb: &[u8], w: u32, h: u32) -> openh264::formats::YUVBuffer {
    let wu = w as usize;
    let hu = h as usize;
    let mut y_p = vec![0u8; wu * hu];
    let mut u_p = vec![0u8; (wu / 2) * (hu / 2)];
    let mut v_p = vec![0u8; (wu / 2) * (hu / 2)];

    for y in 0..hu {
        for x in 0..wu {
            let i = (y * wu + x) * 3;
            let r = rgb[i] as f32;
            let g = rgb[i + 1] as f32;
            let b = rgb[i + 2] as f32;
            y_p[y * wu + x] = (0.299 * r + 0.587 * g + 0.114 * b).clamp(0.0, 255.0) as u8;
        }
    }
    for y in (0..hu).step_by(2) {
        for x in (0..wu).step_by(2) {
            let mut rs = 0.0;
            let mut gs = 0.0;
            let mut bs = 0.0;
            for dy in 0..2 {
                for dx in 0..2 {
                    let px = (x + dx).min(wu - 1);
                    let py = (y + dy).min(hu - 1);
                    let i = (py * wu + px) * 3;
                    rs += rgb[i] as f32;
                    gs += rgb[i + 1] as f32;
                    bs += rgb[i + 2] as f32;
                }
            }
            let r = rs / 4.0;
            let g = gs / 4.0;
            let b = bs / 4.0;
            let u = -0.169 * r - 0.331 * g + 0.5 * b + 128.0;
            let v = 0.5 * r - 0.419 * g - 0.081 * b + 128.0;
            let ui = (y / 2) * (wu / 2) + (x / 2);
            u_p[ui] = u.clamp(0.0, 255.0) as u8;
            v_p[ui] = v.clamp(0.0, 255.0) as u8;
        }
    }
    openh264::formats::YUVBuffer::new_from_yuv(y_p, u_p, v_p)
}

fn yuv_to_rgb(yuv: &openh264::formats::YUVBuffer) -> Vec<u8> {
    let w = yuv.width() as usize;
    let h = yuv.height() as usize;
    let y_p = yuv.y_plane();
    let u_p = yuv.u_plane();
    let v_p = yuv.v_plane();
    let mut rgb = vec![0u8; w * h * 3];
    for y in 0..h {
        for x in 0..w {
            let yi = y * w + x;
            let ci = (y / 2) * (w / 2) + (x / 2);
            let yv = y_p[yi] as f32;
            let uv = u_p[ci] as f32 - 128.0;
            let vv = v_p[ci] as f32 - 128.0;
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

// ============================================================
// Input mapping
// ============================================================
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
        1 => Button::Right,
        2 => Button::Middle,
        _ => Button::Left,
    }
}

fn map_key(code: u32) -> Option<enigo::Key> {
    use enigo::Key::*;
    match code {
        65..=90 => Some(Unicode((b'a' + (code - 65) as u8) as char)),
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

// ============================================================
// HOST backend
// ============================================================
fn spawn_host_backend(bind_addr: String, tx: Sender<BackendMsg>) -> Arc<AtomicBool> {
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

fn run_host(bind: String, tx: Sender<BackendMsg>, stop: Arc<AtomicBool>) -> std::io::Result<()> {
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

// ============================================================
// CLIENT backend
// ============================================================
fn spawn_client_backend(
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

fn run_client(
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

// ============================================================
// GUI Application
// ============================================================
struct AppState {
    mode: AppMode,
    session: SessionState,
    status_text: String,
    server_addr_input: String,
    bind_addr_input: String,
    video_texture: Option<TextureHandle>,
    video_size: (u32, u32),
    stats_fps: f32,
    stats_latency: f32,
    stats_loss: f32,
    fit_to_window: bool,
    is_fullscreen: bool,
    backend_tx: Option<Sender<BackendMsg>>,
    backend_rx: Option<Receiver<BackendMsg>>,
    input_tx: Option<Sender<InputEvent>>,
    stop_flag: Option<Arc<AtomicBool>>,
    language: i18n::Language,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            mode: AppMode::Host,
            session: SessionState::Idle,
            status_text: String::new(),
            server_addr_input: "127.0.0.1:9000".to_string(),
            bind_addr_input: "0.0.0.0:9000".to_string(),
            video_texture: None,
            video_size: (0, 0),
            stats_fps: 0.0,
            stats_latency: 0.0,
            stats_loss: 0.0,
            fit_to_window: true,
            is_fullscreen: false,
            backend_tx: None,
            backend_rx: None,
            input_tx: None,
            stop_flag: None,
            language: i18n::Language::English,
        }
    }
}

impl AppState {
    fn disconnect(&mut self) {
        if let Some(stop) = &self.stop_flag {
            stop.store(true, Ordering::SeqCst);
        }
        self.stop_flag = None;
        self.backend_tx = None;
        self.backend_rx = None;
        self.input_tx = None;
        self.video_texture = None;
        self.video_size = (0, 0);
        self.session = SessionState::Idle;
        self.status_text = i18n::t("status_idle").to_string();
        self.stats_fps = 0.0;
        self.stats_latency = 0.0;
        self.stats_loss = 0.0;
    }

    fn start_host(&mut self) {
        self.disconnect();
        let (tx, rx) = channel();
        self.backend_tx = Some(tx.clone());
        self.backend_rx = Some(rx);
        let stop = spawn_host_backend(self.bind_addr_input.clone(), tx);
        self.stop_flag = Some(stop);
        self.session = SessionState::Listening;
        self.status_text = i18n::t("status_listening").replace("{}", &self.bind_addr_input);
    }

    fn start_client(&mut self) {
        self.disconnect();
        let (tx, rx) = channel();
        let (input_tx, input_rx) = channel();
        self.backend_tx = Some(tx.clone());
        self.backend_rx = Some(rx);
        self.input_tx = Some(input_tx);
        let stop = spawn_client_backend(self.server_addr_input.clone(), tx, input_rx);
        self.stop_flag = Some(stop);
        self.session = SessionState::Connecting;
        self.status_text = i18n::t("status_connecting").replace("{}", &self.server_addr_input);
    }

    fn drain_backend_messages(&mut self, ctx: &egui::Context) {
        let mut to_remove = false;
        if let Some(rx) = &self.backend_rx {
            loop {
                match rx.try_recv() {
                    Ok(msg) => match msg {
                        BackendMsg::Status(s) => {
                            self.status_text = s;
                            self.session = SessionState::Connected;
                        }
                        BackendMsg::Error(e) => {
                            self.status_text = i18n::t("status_error").replace("{}", &e);
                            self.session = SessionState::Error;
                        }
                        BackendMsg::Frame { width, height, rgb } => {
                            let img = ColorImage::from_rgb([width as usize, height as usize], &rgb);
                            match &mut self.video_texture {
                                Some(tex) => {
                                    tex.set(img, TextureOptions::NEAREST);
                                }
                                None => {
                                    self.video_texture = Some(ctx.load_texture(
                                        "video",
                                        img,
                                        TextureOptions::NEAREST,
                                    ));
                                }
                            }
                            self.video_size = (width, height);
                        }
                        BackendMsg::Stats {
                            fps,
                            latency_ms,
                            packet_loss_pct,
                        } => {
                            self.stats_fps = fps;
                            self.stats_latency = latency_ms;
                            self.stats_loss = packet_loss_pct;
                        }
                        BackendMsg::Disconnected => {
                            if self.session != SessionState::Idle {
                                self.session = SessionState::Disconnected;
                                self.status_text = i18n::t("status_disconnected").to_string();
                            }
                            to_remove = true;
                        }
                    },
                    Err(std::sync::mpsc::TryRecvError::Empty) => break,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        to_remove = true;
                        break;
                    }
                }
            }
        }
        if to_remove {
            self.stop_flag = None;
            self.backend_tx = None;
            self.backend_rx = None;
            self.input_tx = None;
        }
    }
}

impl eframe::App for AppState {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain_backend_messages(ctx);
        ctx.request_repaint_after(Duration::from_millis(16));

        // Top panel — title and language selector
        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading(i18n::t("app_title"));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let lang_label = match i18n::current() {
                        i18n::Language::English => "🇺🇸 EN",
                        i18n::Language::Portuguese => "🇧🇷 PT",
                    };
                    egui::ComboBox::from_label(i18n::t("language"))
                        .selected_text(lang_label)
                        .show_ui(ui, |ui| {
                            if ui
                                .selectable_label(
                                    i18n::current() == i18n::Language::English,
                                    "🇺🇸 English",
                                )
                                .clicked()
                            {
                                i18n::set_language(i18n::Language::English);
                                self.language = i18n::Language::English;
                            }
                            if ui
                                .selectable_label(
                                    i18n::current() == i18n::Language::Portuguese,
                                    "🇧🇷 Português",
                                )
                                .clicked()
                            {
                                i18n::set_language(i18n::Language::Portuguese);
                                self.language = i18n::Language::Portuguese;
                            }
                        });
                });
            });
        });

        // Bottom panel — status bar
        egui::TopBottomPanel::bottom("bottom").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(&self.status_text);
                ui.separator();
                ui.label(format!("{} {:.1}", i18n::t("fps"), self.stats_fps));
                ui.separator();
                ui.label(format!(
                    "{} {:.0} ms",
                    i18n::t("latency"),
                    self.stats_latency
                ));
                ui.separator();
                ui.label(format!(
                    "{} {:.1}%",
                    i18n::t("packet_loss"),
                    self.stats_loss
                ));
            });
        });

        let show_session_toolbar = self.session == SessionState::Connected
            && self.mode == AppMode::Client
            && self.video_texture.is_some();

        if show_session_toolbar {
            egui::TopBottomPanel::top("session_toolbar").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui.button(i18n::t("disconnect")).clicked() {
                        self.disconnect();
                    }
                    ui.separator();
                    ui.checkbox(&mut self.fit_to_window, i18n::t("fit_to_window"));
                    ui.separator();
                    let fs_label = if self.is_fullscreen {
                        i18n::t("exit_fullscreen")
                    } else {
                        i18n::t("fullscreen")
                    };
                    if ui.button(fs_label).clicked() {
                        self.is_fullscreen = !self.is_fullscreen;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(
                            self.is_fullscreen,
                        ));
                    }
                });
            });
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            if self.session == SessionState::Connected && self.video_texture.is_some() {
                let tex = self.video_texture.as_ref().unwrap();
                let tex_size = egui::vec2(self.video_size.0 as f32, self.video_size.1 as f32);
                let avail = ui.available_size();
                let display_size = if self.fit_to_window {
                    let scale = (avail.x / tex_size.x).min(avail.y / tex_size.y).min(1.0);
                    tex_size * scale
                } else {
                    tex_size
                };
                let response = ui.add(
                    egui::Image::new((tex.id(), display_size)).sense(egui::Sense::click_and_drag()),
                );

                if let Some(input_tx) = &self.input_tx {
                    if let Some(pos) = response.hover_pos() {
                        let local = pos - response.rect.min;
                        let rx = (local.x / display_size.x * tex_size.x) as i32;
                        let ry = (local.y / display_size.y * tex_size.y) as i32;
                        let _ = input_tx.send(InputEvent::MouseMove { x: rx, y: ry });
                    }
                    if response.clicked() {
                        let _ = input_tx.send(InputEvent::MouseDown { button: 0 });
                        let _ = input_tx.send(InputEvent::MouseUp { button: 0 });
                    }
                    if response.secondary_clicked() {
                        let _ = input_tx.send(InputEvent::MouseDown { button: 1 });
                        let _ = input_tx.send(InputEvent::MouseUp { button: 1 });
                    }
                }

                if let Some(input_tx) = &self.input_tx {
                    ctx.input(|i| {
                        for ev in &i.events {
                            if let egui::Event::Key { key, pressed, .. } = ev {
                                if let Some(code) = egui_key_to_code(*key) {
                                    let msg = if *pressed {
                                        InputEvent::KeyDown { code }
                                    } else {
                                        InputEvent::KeyUp { code }
                                    };
                                    let _ = input_tx.send(msg);
                                }
                            }
                        }
                    });
                }
            } else if self.session == SessionState::Connected
                || self.session == SessionState::Listening
            {
                ui.vertical_centered(|ui| {
                    ui.add_space(80.0);
                    ui.heading(i18n::t("session_active"));
                    ui.add_space(20.0);
                    ui.label(&self.status_text);
                    if self.session == SessionState::Listening {
                        ui.add_space(10.0);
                        ui.label(i18n::t("waiting_peer"));
                    }
                    ui.add_space(30.0);
                    if ui.button(i18n::t("disconnect")).clicked() {
                        self.disconnect();
                    }
                });
            } else {
                ui.add_space(20.0);
                ui.vertical_centered(|ui| {
                    ui.heading(i18n::t("this_device_is"));
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut self.mode, AppMode::Host, i18n::t("mode_host"));
                        ui.selectable_value(
                            &mut self.mode,
                            AppMode::Client,
                            i18n::t("mode_client"),
                        );
                    });
                });
                ui.add_space(30.0);

                egui::Grid::new("home_grid")
                    .num_columns(2)
                    .spacing([20.0, 12.0])
                    .show(ui, |ui| match self.mode {
                        AppMode::Host => {
                            ui.label(i18n::t("your_address"));
                            ui.add(
                                egui::TextEdit::singleline(&mut self.bind_addr_input)
                                    .desired_width(280.0),
                            );
                            ui.end_row();

                            ui.label("");
                            let btn_label = if self.session == SessionState::Listening {
                                i18n::t("stop_hosting")
                            } else {
                                i18n::t("start_hosting")
                            };
                            if ui.button(btn_label).clicked() {
                                if self.session == SessionState::Listening {
                                    self.disconnect();
                                } else {
                                    self.start_host();
                                }
                            }
                            ui.end_row();
                        }
                        AppMode::Client => {
                            ui.label(i18n::t("remote_address"));
                            ui.add(
                                egui::TextEdit::singleline(&mut self.server_addr_input)
                                    .hint_text(i18n::t("remote_addr_hint"))
                                    .desired_width(280.0),
                            );
                            ui.end_row();

                            ui.label("");
                            let can_connect = self.session != SessionState::Connecting
                                && self.session != SessionState::Connected;
                            ui.add_enabled_ui(can_connect, |ui| {
                                if ui.button(i18n::t("connect")).clicked() {
                                    self.start_client();
                                }
                            });
                            if self.session == SessionState::Connecting
                                || self.session == SessionState::Connected
                            {
                                if ui.button(i18n::t("disconnect")).clicked() {
                                    self.disconnect();
                                }
                            }
                            ui.end_row();
                        }
                    });
            }
        });
    }
}

fn egui_key_to_code(key: egui::Key) -> Option<u32> {
    use egui::Key::*;
    Some(match key {
        A => 65,
        B => 66,
        C => 67,
        D => 68,
        E => 69,
        F => 70,
        G => 71,
        H => 72,
        I => 73,
        J => 74,
        K => 75,
        L => 76,
        M => 77,
        N => 78,
        O => 79,
        P => 80,
        Q => 81,
        R => 82,
        S => 83,
        T => 84,
        U => 85,
        V => 86,
        W => 87,
        X => 88,
        Y => 89,
        Z => 90,
        Num0 => 48,
        Num1 => 49,
        Num2 => 50,
        Num3 => 51,
        Num4 => 52,
        Num5 => 53,
        Num6 => 54,
        Num7 => 55,
        Num8 => 56,
        Num9 => 57,
        Space => 32,
        Enter => 13,
        Escape => 27,
        Backspace => 8,
        Tab => 9,
        ArrowLeft => 37,
        ArrowUp => 38,
        ArrowRight => 39,
        ArrowDown => 40,
        _ => return None,
    })
}

// ============================================================
// Main
// ============================================================
fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let mut initial_mode = AppMode::Host;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--mode" | "-m" if i + 1 < args.len() => {
                initial_mode = match args[i + 1].as_str() {
                    "client" | "c" => AppMode::Client,
                    _ => AppMode::Host,
                };
                i += 2;
            }
            "--lang" | "-l" if i + 1 < args.len() => {
                match args[i + 1].as_str() {
                    "pt" | "pt-BR" | "portuguese" => {
                        i18n::set_language(i18n::Language::Portuguese);
                    }
                    _ => i18n::set_language(i18n::Language::English),
                }
                i += 2;
            }
            _ => i += 1,
        }
    }

    // Ensure TLS certificates exist before anything else.
    // Transparent to the user: generated on first run, reused thereafter.
    if let Err(e) = cert::ensure_certificates() {
        eprintln!("Failed to prepare TLS certificates: {e}");
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1000.0, 700.0])
            .with_min_inner_size([700.0, 500.0])
            .with_title("RustDesk-Mini"),
        ..Default::default()
    };

    eframe::run_native(
        "RustDesk-Mini",
        options,
        Box::new(move |_cc| {
            let mut app = AppState::default();
            app.mode = initial_mode;
            app.language = i18n::current();
            app.status_text = i18n::t("status_idle").to_string();
            Ok(Box::new(app))
        }),
    )
}
