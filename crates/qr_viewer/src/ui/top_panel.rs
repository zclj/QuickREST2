use eframe::egui;

use crate::app::App;
use crate::app::ParseState;
use crate::command_sender::UICommand;

pub fn top_panel(ctx: &egui::Context, app: &mut App) {
    egui::TopBottomPanel::top("top_panel_main_menu")
        .resizable(true)
        .min_height(32.0)
        .show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui.button("Save...").clicked() {
                    app.command_sender.send_ui(UICommand::Save);
                }
                // Open OAS file
                if ui.button("Open file...").clicked() {
                    if let Some(path) = rfd::FileDialog::new().pick_file() {
                        app.picked_path = Some(path.display().to_string());
                        app.parse_state = ParseState::Parse;
                    }
                }
            });
        });
}
