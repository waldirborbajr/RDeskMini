// ============================================================
// RDeskMini — free remote desktop (eframe/egui), Linux, Windows and macOS.
// Run it on both computers. Each one shows its own connection code;
// type the other computer's code to control it.
// ============================================================

mod app;
mod cert;
mod client;
mod host;
mod i18n;
mod identity;
mod input;
mod protocol;
mod relay_proto;
mod video;

use app::AppState;
use eframe::egui;

pub(crate) const APP_TITLE: &str = concat!(
    env!("CARGO_PKG_NAME"),
    " v",
    env!("CARGO_PKG_VERSION")
);

fn main() -> eframe::Result<()> {
    i18n::set_language(i18n::detect_language());

    // TLS certificates are created on the first run and reused afterwards.
    if let Err(e) = cert::ensure_certificates() {
        eprintln!("Failed to prepare TLS certificates: {e}");
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1000.0, 700.0])
            .with_min_inner_size([700.0, 500.0])
            .with_title(APP_TITLE),
        ..Default::default()
    };

    eframe::run_native(
        APP_TITLE,
        options,
        Box::new(|_cc| {
            let mut app = AppState::default();
            app.start_hosting();
            Ok(Box::new(app))
        }),
    )
}
