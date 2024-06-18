use eframe::egui;

use qr_explore::amos;
use qr_explore::amos_generation as agen;
use qr_explore::behaviours;
use qr_explore::explore;

use crate::app::App;
use crate::fuzzing;
use crate::main_navigation;

pub fn navigation_panels(ctx: &egui::Context, app: &mut App) {
    egui::SidePanel::left("nav_panel")
        .resizable(true)
        .default_width(150.0)
        .width_range(100.0..=300.0)
        .show(ctx, |ui| nav_panel(app, ui));

    egui::SidePanel::left("nav_details_panel")
        .resizable(true)
        .default_width(350.0)
        .width_range(80.0..=500.0)
        .show(ctx, |ui| {
            egui::ScrollArea::both()
                .auto_shrink([false; 2])
                .show(ui, |ui| nav_details_panel(app, ui));
        });
}

fn nav_panel(app: &mut App, ui: &mut egui::Ui) {
    ui.with_layout(egui::Layout::top_down_justified(egui::Align::LEFT), |ui| {
        ui.label(
            egui::RichText::new("Places")
                .color(egui::Color32::from_rgb(23, 147, 209))
                .size(15.0),
        );

        ui.toggle_value(&mut false, "Current Project");

        ui.separator();

        ui.label(
            egui::RichText::new("Categories")
                .color(egui::Color32::from_rgb(23, 147, 209))
                .size(15.0),
        );

        ui.selectable_value(
            &mut app.app_state.selected_navigation,
            main_navigation::MainNavigation::Exploration,
            "❓ Exploration",
        );
        ui.selectable_value(
            &mut app.app_state.selected_navigation,
            main_navigation::MainNavigation::Fuzzing,
            "💥 Fuzzing",
        );
        ui.selectable_value(
            &mut app.app_state.selected_navigation,
            main_navigation::MainNavigation::Generation,
            "🖩 Generation",
        );
        ui.separator();

        ui.label(
            egui::RichText::new("API")
                .color(egui::Color32::from_rgb(23, 147, 209))
                .size(15.0),
        );

        ui.selectable_value(
            &mut app.app_state.selected_navigation,
            main_navigation::MainNavigation::Operations,
            "🔁 Operations",
        );

        ui.selectable_value(
            &mut app.app_state.selected_navigation,
            main_navigation::MainNavigation::Definitions,
            "🖹 Definitions",
        );

        ui.separator();

        ui.label(
            egui::RichText::new("Exploration")
                .color(egui::Color32::from_rgb(23, 147, 209))
                .size(15.0),
        );

        ui.selectable_value(
            &mut app.app_state.selected_navigation,
            main_navigation::MainNavigation::Sequences,
            "⛓ Sequences",
        );
    });
}

fn nav_details_panel(app: &mut App, ui: &mut egui::Ui) {
    match app.app_state.selected_navigation {
        main_navigation::MainNavigation::Sequences => {
            nav_details_sequences_ui(app, ui);
        }

        main_navigation::MainNavigation::Exploration => {
            nav_details_exploration_ui(app, ui);
        }
        main_navigation::MainNavigation::Fuzzing => {
            nav_details_fuzzing_ui(app, ui);
        }
        main_navigation::MainNavigation::Generation => {
            ui.label("Generation");

            ui.text_edit_singleline(&mut app.generate_string_expression);

            if ui.button("Banana Cakes").clicked() {
                let value = explore::banana_cakes_generation(&app.generate_string_expression);
                if let agen::ParameterValue::StringValue { value, .. } = value {
                    app.generated_values.push(value);
                }
            }

            for v in &app.generated_values {
                ui.label(v.clone());
            }
        }
        main_navigation::MainNavigation::Operations => {
            egui::collapsing_header::CollapsingHeader::new(app.amos.name.clone()).show(ui, |ui| {
                for idx in 0..app.amos.operations.len() {
                    let current_op = &app.amos.operations[idx];
                    let id = ui.make_persistent_id(current_op.info.name.clone());
                    egui::collapsing_header::CollapsingState::load_with_default_open(
                        ui.ctx(),
                        id,
                        false,
                    )
                    .show_header(ui, |ui| {
                        ui.selectable_value(
                            &mut app.selected_amos_operation,
                            idx,
                            current_op.info.name.clone(),
                        );
                    })
                    .body(|ui| {
                        if !&current_op.parameters.is_empty() {
                            ui.collapsing("Parameters", |ui| {
                                for param in &current_op.parameters {
                                    ui.label(format!(
                                        "{} - {}/{:?}",
                                        param.name, param.schema, param.ownership
                                    ));
                                }
                            });
                        };

                        if !&current_op.responses.is_empty() {
                            ui.collapsing("Responses", |ui| {
                                for resp in &current_op.responses {
                                    ui.label(format!("{} - {}", resp.name, resp.schema));
                                }
                            });
                        };
                    });
                }
            });
        }
        main_navigation::MainNavigation::Definitions => definition_tree(app, ui),
    };
}

fn definition_tree(app: &mut App, ui: &mut egui::Ui) {
    egui::collapsing_header::CollapsingHeader::new("Definitions").show(ui, |ui| {
        for (idx, definition) in app.amos.definitions.iter().enumerate() {
            let id = ui.make_persistent_id(definition.name.clone());
            egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, false)
                .show_header(ui, |ui| {
                    ui.selectable_value(
                        &mut app.selected_amos_definition,
                        idx,
                        definition.name.clone(),
                    );
                })
                .body(|ui| collapsing_property(&definition.schema, ui));
        }
    });
}

fn collapsing_property(schema: &amos::Schema, ui: &mut egui::Ui) {
    match schema {
        amos::Schema::Object { properties } => {
            for prop in properties {
                match prop.schema {
                    amos::Schema::Object { .. } => {
                        let id = ui.make_persistent_id(prop.name.clone());
                        egui::collapsing_header::CollapsingState::load_with_default_open(
                            ui.ctx(),
                            id,
                            false,
                        )
                        .show_header(ui, |ui| {
                            // TODO: Allow for the selection of definition parts
                            ui.label(prop.name.clone())
                            // ui.selectable_value(
                            //     &mut app.selected_amos_definition,
                            //     idx,
                            //     definition.name.clone(),
                            // );
                        })
                        .body(|ui| collapsing_property(&prop.schema, ui));
                    }
                    _ => {
                        ui.label(format!("{} - {}", prop.name, prop.schema));
                    }
                }
            }
        }
        _ => {
            ui.label(format!("{:?}", schema));
        }
    }
}

fn nav_details_exploration_ui(app: &mut App, ui: &mut egui::Ui) {
    ui.collapsing("Behaviours", |ui| {
        ui.collapsing("Response-based", |ui| {
            ui.selectable_value(
                &mut app.app_state.behaviour.selected,
                behaviours::Behaviour::ResponseEquality,
                behaviours::Behaviour::ResponseEquality.presentation(),
            );

            ui.selectable_value(
                &mut app.app_state.behaviour.selected,
                behaviours::Behaviour::ResponseInequality,
                behaviours::Behaviour::ResponseInequality.presentation(),
            );
        });
        ui.collapsing("State-based", |ui| {
            ui.selectable_value(
                &mut app.app_state.behaviour.selected,
                behaviours::Behaviour::StateMutation,
                behaviours::Behaviour::StateMutation.presentation(),
            );

            ui.selectable_value(
                &mut app.app_state.behaviour.selected,
                behaviours::Behaviour::StateIdentity,
                behaviours::Behaviour::StateIdentity.presentation(),
            );
        });
    });
}

fn nav_details_fuzzing_ui(app: &mut App, ui: &mut egui::Ui) {
    ui.collapsing("Fuzzing Properties", |ui| {
        ui.collapsing("Response-based", |ui| {
            ui.selectable_value(
                &mut app.app_state.fuzzing.selected,
                fuzzing::Property::ResponseCheck,
                fuzzing::Property::ResponseCheck.to_string(),
            );
        });
    });
}

fn nav_details_sequences_ui(app: &mut App, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        if ui.button("➕").clicked() {
            app.app_state.sequencer.add_new();
        }

        if ui.button("➖").clicked() {
            app.app_state.sequencer.remove_selected();
        }
    });

    ui.separator();

    for (idx, seq) in app.app_state.sequencer.sequences.iter().enumerate() {
        ui.selectable_value(
            &mut app.app_state.sequencer.selected_sequence_id,
            Some(idx),
            seq.name.clone(),
        );
    }
}
