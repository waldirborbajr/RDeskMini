//! Remote input: applies mouse/keyboard events and maps GUI keys to codes.

use eframe::egui;
use enigo::{Button, Coordinate, Direction, Enigo, Keyboard, Mouse};

use crate::protocol::InputEvent;

// ============================================================
// Input mapping
// ============================================================
pub(crate) fn apply_event(enigo: &mut Enigo, ev: InputEvent) {
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

pub(crate) fn map_button(b: u8) -> Button {
    match b {
        1 => Button::Right,
        2 => Button::Middle,
        _ => Button::Left,
    }
}

pub(crate) fn map_key(code: u32) -> Option<enigo::Key> {
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

pub(crate) fn egui_key_to_code(key: egui::Key) -> Option<u32> {
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
