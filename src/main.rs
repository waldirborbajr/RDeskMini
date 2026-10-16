// ============================================================
// RDeskMini — native remote desktop (eframe/egui), Linux, Windows and macOS.
// Usage:
//   rdeskmini --mode host            (default)
//   rdeskmini --mode client
//   rdeskmini --lang pt              (Portuguese UI)
// ============================================================

mod app;
mod cert;
mod client;
mod fec;
mod host;
mod i18n;
mod input;
mod protocol;
mod video;

use eframe::egui;

use app::AppState;
use protocol::AppMode;

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
