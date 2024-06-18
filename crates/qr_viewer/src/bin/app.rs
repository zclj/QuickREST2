use eframe::egui;
use qr_viewer::app::App;
use tracing::info;
use tracing_subscriber::EnvFilter;

fn main() -> Result<(), eframe::Error> {
    // install global collector configured based on RUST_LOG env var.
    tracing_subscriber::fmt()
        .with_target(true)
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    info!("Starting QuickREST");

    let options = eframe::NativeOptions {
        drag_and_drop_support: true,
        initial_window_size: Some(egui::vec2(1800.0, 900.0)),
        ..Default::default()
    };

    eframe::run_native("QuickREST", options, Box::new(|_cc| Box::new(App::new())))
}
