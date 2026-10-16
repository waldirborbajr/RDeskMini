//! Internationalization: English by default, Portuguese when the system language is Portuguese.

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

/// Picks the Portuguese or English text for the current language.
fn pick(en: &'static str, pt: &'static str) -> &'static str {
    if current() == Language::English {
        en
    } else {
        pt
    }
}

pub fn t(key: &str) -> &'static str {
    match key {
        "app_title" => "RDeskMini",
        "this_device" => pick("This device", "Este dispositivo"),
        "your_code_hint" => pick(
            "Share this ID with the person who needs to connect to you:",
            "Envie este ID para quem precisa se conectar a você:",
        ),
        "copy_code" => pick("Copy ID", "Copiar ID"),
        "connect_to_device" => pick("Connect to another device", "Conectar a outro dispositivo"),
        "remote_code_hint" => pick("Enter the ID (123 456 789)", "Digite o ID (123 456 789)"),
        "connect" => pick("Connect", "Conectar"),
        "disconnect" => pick("Disconnect", "Desconectar"),
        "fullscreen" => pick("Fullscreen", "Tela cheia"),
        "exit_fullscreen" => pick("Exit fullscreen", "Sair de tela cheia"),
        "fit_to_window" => pick("Fit to window", "Ajustar à janela"),
        "fps" => "FPS:",
        "latency" => pick("Latency:", "Latência:"),
        "packet_loss" => pick("Packet loss:", "Perda de pacotes:"),
        "language" => pick("Language", "Idioma"),
        "status_idle" => pick("Ready", "Pronto"),
        "status_host_ready" => pick(
            "Ready. Waiting for connections",
            "Pronto. Aguardando conexões",
        ),
        "status_peer_connected" => pick(
            "Being controlled by another computer",
            "Sendo controlado por outro computador",
        ),
        "status_relay_offline" => pick(
            "Server unavailable, retrying… ({})",
            "Servidor indisponível, tentando de novo… ({})",
        ),
        "status_connecting" => pick("Connecting to {}…", "Conectando a {}…"),
        "status_connected" => pick("Connected to {}", "Conectado a {}"),
        "status_session_ended" => pick("Session ended: {}", "Sessão encerrada: {}"),
        "status_disconnected" => pick("Disconnected", "Desconectado"),
        "status_error" => pick("Error: {}", "Erro: {}"),
        "invalid_code" => pick(
            "Invalid ID. It has 9 digits.",
            "ID inválido. Ele tem 9 dígitos.",
        ),
        "peer_not_found" => pick("This ID is not online", "Este ID não está online"),
        _ => "[?]",
    }
}

/// Language from the environment: Portuguese when LANG or LC_ALL starts with "pt".
pub fn detect_language() -> Language {
    let env_lang = std::env::var("LC_ALL")
        .or_else(|_| std::env::var("LANG"))
        .unwrap_or_default();
    if env_lang.to_lowercase().starts_with("pt") {
        Language::Portuguese
    } else {
        Language::English
    }
}
