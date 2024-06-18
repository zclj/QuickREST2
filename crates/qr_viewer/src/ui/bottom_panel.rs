use eframe::egui;
use egui_extras::{Column, TableBuilder};
use qr_explore::amos;

use crate::{
    app::{App, DetailsNavigation},
    central_navigation,
};

pub fn bottom_panel(ctx: &egui::Context, app: &mut App) {
    egui::TopBottomPanel::bottom("bottom_details_panel")
        .resizable(true)
        // TODO: Solve the separator line margin while still keeping margins for the
        // content
        // .frame(egui::Frame {
        //     inner_margin: egui::Margin::symmetric(1.0, 1.0),
        //     fill: ctx.style().visuals.panel_fill,
        //     ..Default::default()
        // })
        .min_height(150.0)
        .show(ctx, |ui| {
            egui::TopBottomPanel::top("bottom_tools_panel").show_inside(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.selectable_value(
                        &mut app.selected_details_navigation,
                        DetailsNavigation::Logs,
                        "❓ Logs",
                    );

                    ui.separator();

                    let mut problems_count = 0;
                    if let Some(result) = app.parse_result.as_ref() {
                        problems_count += result.warnings.len();
                    }
                    if let Some(result) = app.translation_result.as_ref() {
                        problems_count += result.warnings.len();
                        problems_count += result.errors.len();
                    }

                    let problems_label = if problems_count > 0 {
                        format!("Problems ({})", problems_count)
                    } else {
                        "Problems".to_string()
                    };

                    ui.selectable_value(
                        &mut app.selected_details_navigation,
                        DetailsNavigation::Problems,
                        problems_label,
                    );

                    ui.separator();

                    ui.selectable_value(
                        &mut app.selected_details_navigation,
                        DetailsNavigation::Details,
                        "⚙ Details",
                    );

                    ui.separator();
                });
            });

            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .show(ui, |ui| match app.selected_details_navigation {
                    DetailsNavigation::Logs => {
                        app.log_table(ui);
                    }
                    DetailsNavigation::Problems => {
                        problems(app, ui);
                    }
                    DetailsNavigation::Details => {
                        context_details(app, ui);
                    }
                });
        });
}

pub fn problems(app: &mut App, ui: &mut egui::Ui) {
    if let Some(result) = app.parse_result.as_ref() {
        if !result.warnings.is_empty() {
            ui.collapsing("OpenAPI Parsing", |ui| {
                let table = TableBuilder::new(ui)
                    .striped(true)
                    .resizable(true)
                    .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                    .column(Column::auto())
                    .column(Column::auto())
                    .column(Column::auto())
                    .column(Column::auto())
                    .column(Column::remainder());

                table
                    .header(20.0, |mut header| {
                        header.col(|ui| {
                            ui.strong("#");
                        });

                        header.col(|ui| {
                            ui.strong("Message");
                        });

                        header.col(|ui| {
                            ui.strong("Path");
                        });

                        header.col(|ui| {
                            ui.strong("Operation");
                        });

                        header.col(|ui| {
                            ui.strong("Method");
                        });
                    })
                    .body(|mut body| {
                        for (idx, warning) in result.warnings.iter().enumerate() {
                            body.row(18.0, |mut row| {
                                // number
                                row.col(|ui| {
                                    ui.label((idx + 1).to_string());
                                });
                                // message
                                row.col(|ui| {
                                    ui.label(&warning.message);
                                });
                                // path
                                row.col(|ui| {
                                    if let Some(path) = &warning.path {
                                        ui.label(path);
                                    } else {
                                        ui.label("None");
                                    }
                                });
                                // operation
                                row.col(|ui| {
                                    if let Some(operation) = &warning.operation {
                                        ui.label(operation);
                                    } else {
                                        ui.label("None");
                                    }
                                });
                                row.col(|ui| {
                                    // method
                                    if let Some(method) = &warning.method {
                                        ui.label(method);
                                    } else {
                                        ui.label("None");
                                    }
                                });
                            });
                        }
                    });
            });
        }
    }

    if let Some(result) = app.translation_result.as_ref() {
        if !(result.warnings.is_empty() && result.errors.is_empty()) {
            ui.collapsing("AMOS Translation", |ui| {
                let table = TableBuilder::new(ui)
                    .striped(true)
                    .resizable(true)
                    .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                    .column(Column::auto())
                    .column(Column::auto())
                    .column(Column::remainder());

                table
                    .header(20.0, |mut header| {
                        header.col(|ui| {
                            ui.strong("#");
                        });

                        header.col(|ui| {
                            ui.strong("Severity");
                        });

                        header.col(|ui| {
                            ui.strong("Message");
                        });
                    })
                    .body(|mut body| {
                        for (idx, warning) in result.warnings.iter().enumerate() {
                            body.row(18.0, |mut row| {
                                // number
                                row.col(|ui| {
                                    ui.label((idx + 1).to_string());
                                });
                                // severity
                                row.col(|ui| {
                                    ui.label("Warning");
                                });
                                // message
                                row.col(|ui| {
                                    ui.label(&warning.message);
                                });
                            });
                        }

                        for (idx, error) in result.errors.iter().enumerate() {
                            body.row(18.0, |mut row| {
                                // number
                                row.col(|ui| {
                                    ui.label((idx + 1).to_string());
                                });
                                // severity
                                row.col(|ui| {
                                    ui.label("Error");
                                });
                                // message
                                row.col(|ui| {
                                    ui.label(&error.message);
                                });
                            });
                        }
                    })
            });
        }
    }
}

pub fn context_details(app: &mut App, ui: &mut egui::Ui) {
    match app.app_state.central_navigation.selected {
        central_navigation::Navigations::Invocations => {
            context_details_invocations(app, ui);
        }
        central_navigation::Navigations::Sequences => {
            context_details_sequences(app, ui);
        }
        central_navigation::Navigations::Summary => (),
        central_navigation::Navigations::Examples => (),
        central_navigation::Navigations::Progress => (),
        central_navigation::Navigations::Sequencer => context_details_sequencer(app, ui),
        central_navigation::Navigations::APIs => (),
    }
}

fn context_details_invocations(app: &App, ui: &mut egui::Ui) {
    if !app.invocation_results.is_empty() {
        let selected = &app.invocation_results[app.selected_result];

        ui.label("Operation:");
        ui.label(
            app.invocation_results[app.selected_result]
                .operation
                .name
                .clone(),
        );

        // Result - prob some expantion here
        ui.collapsing("Result", |ui| {
            ui.monospace(selected.result.clone());
        });

        ui.monospace(selected.result.clone());
    };
}

fn context_details_sequences(app: &mut App, ui: &mut egui::Ui) {
    egui::SidePanel::left("sequence_list_details")
        .resizable(true)
        .min_width(200.0)
        .width_range(100.0..=300.0)
        .show_inside(ui, |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    if !app.invocation_spans.is_empty() {
                        let selected_span = &app.invocation_spans[app.selected_span];

                        let span_results =
                            &app.invocation_results[selected_span.start..selected_span.end + 1];

                        let table = TableBuilder::new(ui)
                            .striped(true)
                            .resizable(true)
                            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                            .column(Column::auto())
                            .column(Column::remainder());

                        table
                            .header(20.0, |mut header| {
                                header.col(|ui| {
                                    ui.strong("Order");
                                });

                                header.col(|ui| {
                                    ui.strong("Operation");
                                });
                            })
                            .body(|mut body| {
                                for (idx, op) in span_results.iter().enumerate() {
                                    body.row(18.0, |mut row| {
                                        row.col(|ui| {
                                            ui.label((idx + 1).to_string());
                                        });

                                        row.col(|ui| {
                                            if ui
                                                .selectable_value(
                                                    &mut app.selected_details_sequence_operation,
                                                    idx,
                                                    op.operation.name.clone(),
                                                )
                                                .clicked()
                                            {
                                                app.selected_result = selected_span.start + idx;
                                            };
                                        });
                                    });
                                }
                            });
                    } else {
                        // reset selections if we are empty
                        app.selected_details_sequence_operation = 0;
                    }
                });
        });

    egui::ScrollArea::both()
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            egui::SidePanel::left("operation_details")
                .default_width(450.0)
                .width_range(450.0..=600.0)
                .show_inside(ui, |ui| {
                    if !app.invocation_spans.is_empty() {
                        let selected_span = &app.invocation_spans[app.selected_span];
                        let selected_span_invocation = &app.invocation_results
                            [selected_span.start + app.selected_details_sequence_operation];
                        egui::Grid::new("operation_details_grid")
                            .num_columns(2)
                            .spacing([4.0, 4.0])
                            .striped(true)
                            .show(ui, |ui| {
                                ui.strong("Name");
                                ui.label(selected_span_invocation.operation.name.clone());
                                ui.end_row();

                                if let Some(meta) = &selected_span_invocation.meta_data {
                                    match meta {
                                        amos::ResultMetaData::HTTP { url, status } => {
                                            ui.strong("URL");
                                            ui.label(url);
                                            ui.end_row();

                                            ui.strong("Status");
                                            ui.label(status.to_string());
                                            ui.end_row();
                                        }
                                    }
                                }
                            });
                    }
                });

            egui::SidePanel::left("operation_parameters")
                .default_width(450.0)
                .width_range(450.0..=600.0)
                .show_inside(ui, |ui| {
                    if !app.invocation_spans.is_empty() {
                        let selected_span = &app.invocation_spans[app.selected_span];
                        let selected_span_invocation = &app.invocation_results
                            [selected_span.start + app.selected_details_sequence_operation];
                        let table = TableBuilder::new(ui)
                            .striped(true)
                            .resizable(true)
                            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                            .column(Column::auto())
                            .column(Column::auto())
                            .column(Column::remainder());

                        table
                            .header(20.0, |mut header| {
                                header.col(|ui| {
                                    ui.strong("Parameter");
                                });

                                header.col(|ui| {
                                    ui.strong("Value");
                                });

                                header.col(|ui| {
                                    ui.strong("Reference");
                                });
                            })
                            .body(|mut body| {
                                for param in &selected_span_invocation.operation.parameters {
                                    body.row(18.0, |mut row| {
                                        row.col(|ui| {
                                            ui.label(param.name.clone());
                                        });

                                        row.col(|ui| {
                                            // TODO: fix
                                            ui.label(format!("{:?}", param.value));
                                        });

                                        row.col(|ui| {
                                            ui.label(format!("{:?}", param.ref_path));
                                        });
                                    });
                                }
                            });
                    }
                });
        });
}

fn context_details_sequencer(_app: &mut App, ui: &mut egui::Ui) {
    ui.label("TODO".to_string());
}
