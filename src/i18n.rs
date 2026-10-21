//! Internationalization for US English, Brazilian Portuguese, and Spanish.

use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    English = 0,
    PortugueseBrazil = 1,
    Spanish = 2,
}

static CURRENT: AtomicU8 = AtomicU8::new(Language::English as u8);

pub fn set_language(lang: Language) {
    CURRENT.store(lang as u8, Ordering::SeqCst);
}

pub fn current() -> Language {
    match CURRENT.load(Ordering::SeqCst) {
        1 => Language::PortugueseBrazil,
        2 => Language::Spanish,
        _ => Language::English,
    }
}

fn pick(en: &'static str, pt_br: &'static str, es: &'static str) -> &'static str {
    match current() {
        Language::English => en,
        Language::PortugueseBrazil => pt_br,
        Language::Spanish => es,
    }
}

pub fn t(key: &str) -> &'static str {
    match key {
        "this_device" => pick("This device", "Este dispositivo", "Este dispositivo"),
        "your_code_hint" => pick(
            "Share this ID and the fingerprint below with the person connecting to you:",
            "Compartilhe este ID e a impressão digital abaixo com quem vai se conectar:",
            "Comparte este ID y la huella digital con la persona que se conectará:",
        ),
        "copy_code" => pick("Copy ID", "Copiar ID", "Copiar ID"),
        "host_fingerprint" => pick(
            "Share this fingerprint separately and verify it with the other person:",
            "Compartilhe esta impressão digital separadamente e confirme com a outra pessoa:",
            "Comparte esta huella digital por separado y verifícala con la otra persona:",
        ),
        "copy_fingerprint" => pick(
            "Copy fingerprint",
            "Copiar impressão digital",
            "Copiar huella digital",
        ),
        "remote_fingerprint_hint" => pick(
            "Enter the host SHA-256 fingerprint",
            "Digite a impressão digital SHA-256 do anfitrião",
            "Introduce la huella digital SHA-256 del anfitrión",
        ),
        "invalid_fingerprint" => pick(
            "Enter the host's 64-digit SHA-256 fingerprint.",
            "Digite a impressão digital SHA-256 de 64 dígitos do anfitrião.",
            "Introduce la huella digital SHA-256 de 64 dígitos del anfitrión.",
        ),
        "incoming_request" => pick(
            "Incoming connection",
            "Conexão recebida",
            "Conexión entrante",
        ),
        "incoming_request_body" => pick(
            "Someone is requesting control of this device. Allow this only if you expected the request.",
            "Alguém está solicitando controlar este dispositivo. Permita somente se você esperava esta solicitação.",
            "Alguien solicita controlar este dispositivo. Permítelo solo si esperabas esta solicitud.",
        ),
        "allow" => pick("Allow", "Permitir", "Permitir"),
        "reject" => pick("Reject", "Rejeitar", "Rechazar"),
        "connect_to_device" => pick(
            "Connect to another device",
            "Conectar a outro dispositivo",
            "Conectarse a otro dispositivo",
        ),
        "remote_code_hint" => pick(
            "Enter the ID (123 456 789)",
            "Digite o ID (123 456 789)",
            "Introduce el ID (123 456 789)",
        ),
        "connect" => pick("Connect", "Conectar", "Conectar"),
        "disconnect" => pick("Disconnect", "Desconectar", "Desconectar"),
        "fullscreen" => pick("Fullscreen", "Tela cheia", "Pantalla completa"),
        "exit_fullscreen" => pick(
            "Exit fullscreen",
            "Sair de tela cheia",
            "Salir de pantalla completa",
        ),
        "fit_to_window" => pick(
            "Fit to window",
            "Ajustar à janela",
            "Ajustar a la ventana",
        ),
        "fps" => "FPS:",
        "latency" => pick("Latency:", "Latência:", "Latencia:"),
        "packet_loss" => pick("Packet loss:", "Perda de pacotes:", "Pérdida de paquetes:"),
        "language" => pick("Language", "Idioma", "Idioma"),
        "status_idle" => pick("Ready", "Pronto", "Listo"),
        "status_host_ready" => pick(
            "Ready. Waiting for connections",
            "Pronto. Aguardando conexões",
            "Listo. Esperando conexiones",
        ),
        "status_peer_connected" => pick(
            "Being controlled by another computer",
            "Sendo controlado por outro computador",
            "Otro equipo está controlando este dispositivo",
        ),
        "status_waiting_approval" => pick(
            "Waiting for {} to approve the connection…",
            "Aguardando {} aprovar a conexão…",
            "Esperando a que {} apruebe la conexión…",
        ),
        "status_request_rejected" => pick(
            "Connection request rejected",
            "Solicitação de conexão rejeitada",
            "Solicitud de conexión rechazada",
        ),
        "status_request_expired" => pick(
            "Connection request expired",
            "Solicitação de conexão expirada",
            "La solicitud de conexión ha caducado",
        ),
        "status_request_denied_remote" => pick(
            "The host rejected the request or it expired",
            "O anfitrião rejeitou a solicitação ou ela expirou",
            "El anfitrión rechazó la solicitud o esta caducó",
        ),
        "status_relay_offline" => pick(
            "Server unavailable, retrying… ({})",
            "Servidor indisponível, tentando de novo… ({})",
            "Servidor no disponible, reintentando… ({})",
        ),
        "status_connecting" => pick(
            "Connecting to {}…",
            "Conectando a {}…",
            "Conectando con {}…",
        ),
        "status_connected" => pick(
            "Connected to {}",
            "Conectado a {}",
            "Conectado con {}",
        ),
        "status_session_ended" => pick(
            "Session ended: {}",
            "Sessão encerrada: {}",
            "Sesión finalizada: {}",
        ),
        "status_disconnected" => pick("Disconnected", "Desconectado", "Desconectado"),
        "status_error" => pick("Error: {}", "Erro: {}", "Error: {}"),
        "invalid_code" => pick(
            "Invalid ID. It has 9 digits.",
            "ID inválido. Ele tem 9 dígitos.",
            "ID no válido. Debe tener 9 dígitos.",
        ),
        "peer_not_found" => pick(
            "This ID is not online",
            "Este ID não está online",
            "Este ID no está conectado",
        ),
        _ => "[?]",
    }
}

fn language_for_locale(locale: &str) -> Language {
    let locale = locale
        .split(['.', '@'])
        .next()
        .unwrap_or_default()
        .replace('-', "_")
        .to_ascii_lowercase();
    if locale == "pt" || locale.starts_with("pt_") {
        Language::PortugueseBrazil
    } else if locale == "es" || locale.starts_with("es_") {
        Language::Spanish
    } else {
        Language::English
    }
}

/// Uses the process locale when available; defaults to US English.
pub fn detect_language() -> Language {
    ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .filter_map(|key| std::env::var(key).ok())
        .find(|locale| !locale.is_empty())
        .map(|locale| language_for_locale(&locale))
        .unwrap_or(Language::English)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_requested_locales() {
        assert_eq!(language_for_locale("pt_BR.UTF-8"), Language::PortugueseBrazil);
        assert_eq!(language_for_locale("es_ES.UTF-8"), Language::Spanish);
        assert_eq!(language_for_locale("es-MX"), Language::Spanish);
        assert_eq!(language_for_locale("en_US.UTF-8"), Language::English);
        assert_eq!(language_for_locale("US_ENG"), Language::English);
    }
}
