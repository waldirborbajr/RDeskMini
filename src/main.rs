// ============================================================
// RustDesk-Mini — acesso remoto básico em arquivo único
// Uso:
//   Servidor (máquina a ser controlada):  cargo run -- server 0.0.0.0:9000
//   Cliente  (máquina que controla):      cargo run -- client 192.168.0.10:9000
// ============================================================

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::{Duration, Instant};

use bincode;
use enigo::{Button, Coordinate, Direction, Enigo, Keyboard, Mouse, Settings};
use image::{codecs::jpeg::JpegEncoder, DynamicImage, RgbImage};
use minifb::{Key, MouseButton, MouseMode, Window, WindowOptions};
use screenshots::Screen;
use serde::{Deserialize, Serialize};

// ------------------------------------------------------------
// Protocolo de eventos (cliente -> servidor)
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
// Protocolo de frames (servidor -> cliente)
// ------------------------------------------------------------
// [u32 width][u32 height][u32 jpeg_len][jpeg bytes]
fn write_frame(stream: &mut TcpStream, w: u32, h: u32, jpeg: &[u8]) -> std::io::Result<()> {
    stream.write_all(&w.to_be_bytes())?;
    stream.write_all(&h.to_be_bytes())?;
    stream.write_all(&(jpeg.len() as u32).to_be_bytes())?;
    stream.write_all(jpeg)?;
    stream.flush()
}

fn read_frame(stream: &mut TcpStream) -> std::io::Result<(u32, u32, Vec<u8>)> {
    let mut buf4 = [0u8; 4];
    stream.read_exact(&mut buf4)?;
    let w = u32::from_be_bytes(buf4);
    stream.read_exact(&mut buf4)?;
    let h = u32::from_be_bytes(buf4);
    stream.read_exact(&mut buf4)?;
    let len = u32::from_be_bytes(buf4) as usize;
    let mut jpeg = vec![0u8; len];
    stream.read_exact(&mut jpeg)?;
    Ok((w, h, jpeg))
}

// ------------------------------------------------------------
// SERVIDOR — captura a tela, envia frames e aplica inputs
// ------------------------------------------------------------
fn run_server(bind: &str) -> std::io::Result<()> {
    let listener = TcpListener::bind(bind)?;
    println!("[servidor] aguardando conexão em {}", bind);

    let (mut stream, addr) = listener.accept()?;
    println!("[servidor] cliente conectado: {}", addr);

    // Descobre a tela primária
    let screens = Screen::all().map_err(|e| {
        std::io::Error::new(std::io::ErrorKind::Other, format!("erro screens: {e}"))
    })?;
    let screen = screens
        .into_iter()
        .next()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "sem tela"))?;
    println!("[servidor] tela: {}x{}", screen.display_info.width, screen.display_info.height);

    // Thread que recebe eventos de input do cliente
    let mut input_stream = stream.try_clone()?;
    thread::spawn(move || {
        let mut enigo = match Enigo::new(&Settings::default()) {
            Ok(e) => e,
            Err(e) => {
                eprintln!("[servidor] falha ao criar Enigo: {e}");
                return;
            }
        };
        loop {
            // Lê tamanho da mensagem (bincode)
            let mut len_buf = [0u8; 4];
            if input_stream.read_exact(&mut len_buf).is_err() {
                println!("[servidor] cliente desconectou (input).");
                break;
            }
            let len = u32::from_be_bytes(len_buf) as usize;
            let mut data = vec![0u8; len];
            if input_stream.read_exact(&mut data).is_err() {
                break;
            }
            match bincode::deserialize::<InputEvent>(&data) {
                Ok(ev) => apply_event(&mut enigo, ev),
                Err(e) => eprintln!("[servidor] evento inválido: {e}"),
            }
        }
    });

    // Loop principal: captura e envia frames
    let target_fps = 15u64;
    let frame_dur = Duration::from_millis(1000 / target_fps);
    let mut last = Instant::now();

    loop {
        // Captura
        let img = screen.capture().map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, format!("capture: {e}"))
        })?;

        // Converte para RGB
        let rgb: RgbImage = image::ImageBuffer::from_raw(img.width(), img.height(), img.into_raw())
            .ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::Other, "raw->RgbImage falhou")
            })?;
        let dynimg = DynamicImage::ImageRgb8(rgb);

        // Comprime em JPEG (qualidade 70 — ajuste conforme sua rede)
        let mut jpeg = Vec::new();
        let mut encoder = JpegEncoder::new_with_quality(&mut jpeg, 70);
        if let Err(e) = encoder.encode_image(&dynimg) {
            eprintln!("[servidor] encode jpeg: {e}");
            continue;
        }

        if let Err(e) = write_frame(&mut stream, img.width(), img.height(), &jpeg) {
            println!("[servidor] conexão encerrada: {e}");
            break;
        }

        // Controle de FPS
        let elapsed = last.elapsed();
        if elapsed < frame_dur {
            thread::sleep(frame_dur - elapsed);
        }
        last = Instant::now();
    }

    Ok(())
}

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
            // delta positivo = scroll pra cima, negativo = baixo
            let dir = if delta > 0 {
                enigo::Axis::Vertical
            } else {
                enigo::Axis::Vertical
            };
            let _ = enigo.scroll(delta.abs() * 3, dir);
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

// Mapeia teclas do minifb -> enigo::Key (subconjunto essencial)
fn map_key(code: u32) -> Option<enigo::Key> {
    use enigo::Key::*;
    // minifb usa códigos baseados em Key enum
    match code {
        // Letras
        65 => Some(Unicode('a')), // Key::A
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
        // Números (top row minifb usa 48..=57)
        48..=57 => Some(Unicode((b'0' + (code - 48) as u8) as char)),
        // Espaço / Enter / Esc / Backspace
        32 => Some(Space),
        13 => Some(Return),
        27 => Some(Escape),
        8 => Some(Backspace),
        9 => Some(Tab),
        // Setas
        37 => Some(LeftArrow),
        38 => Some(UpArrow),
        39 => Some(RightArrow),
        40 => Some(DownArrow),
        _ => None,
    }
}

// ------------------------------------------------------------
// CLIENTE — recebe frames, exibe e envia inputs
// ------------------------------------------------------------
fn run_client(addr: &str) -> std::io::Result<()> {
    let mut stream = TcpStream::connect(addr)?;
    stream.set_nodelay(true).ok();
    println!("[cliente] conectado a {}", addr);

    // Recebe primeiro frame para dimensionar a janela
    let (mut w, mut h, jpeg) = read_frame(&mut stream)?;
    println!("[cliente] tela remota: {}x{}", w, h);

    let mut window = Window::new(
        "RustDesk-Mini — pressione ESC na janela para sair",
        w as usize,
        h as usize,
        WindowOptions::default(),
    )
    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

    // 60 fps de render local
    window.set_target_fps(60);
    window.limit_update_rate(Some(Duration::from_micros(16_600)));

    // Aplica o primeiro frame
    let mut pixels = decode_jpeg_to_rgba(&jpeg, w, h)?;
    let mut frame_buf = pixels.clone();

    // Thread para receber frames
    let mut frame_stream = stream.try_clone()?;
    let (tx, rx) = std::sync::mpsc::channel::<(u32, u32, Vec<u8>)>();
    thread::spawn(move || loop {
        match read_frame(&mut frame_stream) {
            Ok(f) => {
                if tx.send(f).is_err() {
                    break;
                }
            }
            Err(_) => break,
        }
    });

    // Estado de inputs
    let mut last_mouse: (f32, f32) = (0.0, 0.0);
    let mut buttons_down = [false; 3];
    let mut keys_down: std::collections::HashSet<Key> = std::collections::HashSet::new();

    while window.is_open() && !window.is_key_down(Key::Escape) {
        // 1) Atualiza frame se houver novo
        if let Ok((nw, nh, jpeg)) = rx.try_recv() {
            if nw != w || nh != h {
                w = nw;
                h = nh;
                // minifb não permite redimensionar facilmente; ignoramos
            }
            if let Ok(buf) = decode_jpeg_to_rgba(&jpeg, w, h) {
                frame_buf = buf;
            }
        }

        // 2) Mouse move
        if let Some((mx, my)) = window.get_mouse_pos(MouseMode::Discard) {
            if (mx, my) != last_mouse {
                let ev = InputEvent::MouseMove {
                    x: mx as i32,
                    y: my as i32,
                };
                send_event(&mut stream, &ev);
                last_mouse = (mx, my);
            }
        }

        // 3) Botões do mouse
        let mb_states = [
            (0u8, MouseButton::Left),
            (1u8, MouseButton::Right),
            (2u8, MouseButton::Middle),
        ];
        for (idx, (btn_id, btn)) in mb_states.iter().enumerate() {
            let down = window.get_mouse_down(*btn);
            if down != buttons_down[idx] {
                let ev = if down {
                    InputEvent::MouseDown { button: *btn_id }
                } else {
                    InputEvent::MouseUp { button: *btn_id }
                };
                send_event(&mut stream, &ev);
                buttons_down[idx] = down;
            }
        }

        // 4) Scroll
        let (_, wheel_y) = window.get_scroll_wheel().unwrap_or((0.0, 0.0));
        if wheel_y.abs() > 0.01 {
            let ev = InputEvent::Scroll {
                delta: if wheel_y > 0.0 { 1 } else { -1 },
            };
            send_event(&mut stream, &ev);
        }

        // 5) Teclas
        let pressed = window.get_keys();
        for k in &pressed {
            if !keys_down.contains(k) {
                keys_down.insert(*k);
                if let Some(code) = key_to_code(*k) {
                    send_event(&mut stream, &InputEvent::KeyDown { code });
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
                send_event(&mut stream, &InputEvent::KeyUp { code });
            }
        }

        // 6) Renderiza
        window
            .update_with_buffer(&frame_buf, w as usize, h as usize)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
    }

    println!("[cliente] encerrando.");
    Ok(())
}

fn send_event(stream: &mut TcpStream, ev: &InputEvent) {
    if let Ok(data) = bincode::serialize(ev) {
        let len = (data.len() as u32).to_be_bytes();
        let _ = stream.write_all(&len);
        let _ = stream.write_all(&data);
    }
}

// minifb Key -> código u32 usado no map_key do servidor
// (mantemos os códigos ASCII/numéricos para compatibilidade)
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

fn decode_jpeg_to_rgba(jpeg: &[u8], w: u32, h: u32) -> std::io::Result<Vec<u32>> {
    let img = image::load_from_memory_with_format(jpeg, image::ImageFormat::Jpeg)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("decode: {e}")))?;
    let rgb = img.to_rgb8();
    let (iw, ih) = rgb.dimensions();
    if iw != w || ih != h {
        // redimensiona caso o servidor tenha mudado de resolução
        let resized = image::imageops::resize(&rgb, w, h, image::imageops::FilterType::Nearest);
        return Ok(rgb_to_u32(&resized));
    }
    Ok(rgb_to_u32(&rgb))
}

fn rgb_to_u32(rgb: &RgbImage) -> Vec<u32> {
    rgb.pixels()
        .map(|p| {
            let [r, g, b] = p.0;
            ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
        })
        .collect()
}

// ------------------------------------------------------------
// main
// ------------------------------------------------------------
fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Uso:");
        eprintln!("  servidor: cargo run -- server 0.0.0.0:9000");
        eprintln!("  cliente : cargo run -- client 192.168.0.10:9000");
        std::process::exit(1);
    }
    let mode = args[1].as_str();
    let addr = args[2].as_str();

    let result = match mode {
        "server" => run_server(addr),
        "client" => run_client(addr),
        _ => {
            eprintln!("Modo inválido: use 'server' ou 'client'");
            std::process::exit(1);
        }
    };

    if let Err(e) = result {
        eprintln!("Erro: {e}");
        std::process::exit(1);
    }
}
