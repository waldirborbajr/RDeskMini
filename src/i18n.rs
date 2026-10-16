//! Internationalization: English by default, Portuguese optional.

// ============================================================
// i18n — English default, Portuguese optional
// ============================================================
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
