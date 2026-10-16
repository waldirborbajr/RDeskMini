//! RDesk relay server.
//!
//! A host registers and receives an ID. A controller connects with that ID; the
//! server pairs the two sides and copies raw bytes between them. The server never
//! decrypts anything: TLS runs end-to-end between host and controller.

#[path = "../../src/relay_proto.rs"]
mod relay_proto;

use relay_proto::{read_msg, write_msg, Channel, Hello, Reply};
use std::collections::HashMap;
use std::io::{self, Read};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const LISTEN_ADDR: &str = "0.0.0.0:9443";
/// A half-open session (one side connected, the other not yet) is dropped after this.
const PAIR_TIMEOUT: Duration = Duration::from_secs(30);

type Writer = Arc<Mutex<TcpStream>>;

#[derive(Default)]
struct State {
    /// ID -> the registration connection of the host that owns it.
    hosts: Mutex<HashMap<String, Writer>>,
    /// Half-open pairs waiting for their other side.
    waiting: Mutex<HashMap<(u64, Channel), (Instant, TcpStream)>>,
}

static SESSIONS: AtomicU64 = AtomicU64::new(1);
static RNG: AtomicU64 = AtomicU64::new(0);

fn main() -> io::Result<()> {
    let listener = TcpListener::bind(LISTEN_ADDR)?;
    println!("rdesk relay listening on {LISTEN_ADDR}");
    let state = Arc::new(State::default());
    for conn in listener.incoming() {
        let Ok(conn) = conn else { continue };
        let state = state.clone();
        thread::spawn(move || {
            if let Err(e) = handle(conn, &state) {
                eprintln!("connection closed: {e}");
            }
        });
    }
    Ok(())
}

fn handle(mut conn: TcpStream, state: &State) -> io::Result<()> {
    match read_msg::<Hello>(&mut conn)? {
        Hello::Register => register(conn, state),
        Hello::Connect { id } => connect(conn, &id, state),
        Hello::Accept { session, channel } | Hello::Join { session, channel } => {
            pair(conn, session, channel, state)
        }
    }
}

fn register(mut conn: TcpStream, state: &State) -> io::Result<()> {
    let id = new_id(state);
    let writer: Writer = Arc::new(Mutex::new(conn.try_clone()?));
    state
        .hosts
        .lock()
        .unwrap()
        .insert(id.clone(), writer.clone());
    write_msg(
        &mut *writer.lock().unwrap(),
        &Reply::Registered { id: id.clone() },
    )?;

    // The host keeps this connection open; when it closes, the ID is released.
    let mut buf = [0u8; 64];
    while matches!(conn.read(&mut buf), Ok(n) if n > 0) {}
    state.hosts.lock().unwrap().remove(&id);
    Ok(())
}

fn connect(mut conn: TcpStream, id: &str, state: &State) -> io::Result<()> {
    let host = state.hosts.lock().unwrap().get(id).cloned();
    let Some(host) = host else {
        return write_msg(&mut conn, &Reply::NotFound);
    };
    let session = SESSIONS.fetch_add(1, Ordering::Relaxed);
    if write_msg(&mut *host.lock().unwrap(), &Reply::Incoming { session }).is_err() {
        return write_msg(&mut conn, &Reply::NotFound);
    }
    write_msg(&mut conn, &Reply::Session { session })
}

fn pair(conn: TcpStream, session: u64, channel: Channel, state: &State) -> io::Result<()> {
    let other = {
        let mut waiting = state.waiting.lock().unwrap();
        waiting.retain(|_, (since, half)| {
            if since.elapsed() < PAIR_TIMEOUT {
                true
            } else {
                let _ = half.shutdown(Shutdown::Both);
                false
            }
        });
        waiting.remove(&(session, channel))
    };
    match other {
        Some((_, other)) => pipe(conn, other),
        None => {
            state
                .waiting
                .lock()
                .unwrap()
                .insert((session, channel), (Instant::now(), conn));
            Ok(())
        }
    }
}

/// Copies bytes both ways until either side closes, then closes both.
fn pipe(a: TcpStream, b: TcpStream) -> io::Result<()> {
    let (mut a_in, mut b_in) = (a.try_clone()?, b.try_clone()?);
    let (a_kill, b_kill) = (a.try_clone()?, b.try_clone()?);
    let (mut a_out, mut b_out) = (a, b);
    let (ak1, bk1) = (a_kill.try_clone()?, b_kill.try_clone()?);
    thread::spawn(move || {
        let _ = io::copy(&mut a_in, &mut b_out);
        let _ = ak1.shutdown(Shutdown::Both);
        let _ = bk1.shutdown(Shutdown::Both);
    });
    thread::spawn(move || {
        let _ = io::copy(&mut b_in, &mut a_out);
        let _ = a_kill.shutdown(Shutdown::Both);
        let _ = b_kill.shutdown(Shutdown::Both);
    });
    Ok(())
}

/// A 9-digit ID that no online host is using.
fn new_id(state: &State) -> String {
    loop {
        let id = format!("{:09}", next_random() % 1_000_000_000);
        if !state.hosts.lock().unwrap().contains_key(&id) {
            return id;
        }
    }
}

/// Small xorshift generator; enough to spread IDs, not a security feature.
fn next_random() -> u64 {
    let mut x = RNG.load(Ordering::Relaxed);
    if x == 0 {
        x = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(1)
            | 1;
    }
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    RNG.store(x, Ordering::Relaxed);
    x
}
