use crate::app_state::AppState;
use crate::central_navigation;
use crate::command_sender::command_channel;
use crate::command_sender::{CommandReceiver, CommandSender, UICommand};
use crate::fuzzing::PropertySettings;
use crate::main_navigation;
use crate::sequencer;
use eframe::egui;
use egui_extras::{Column, TableBuilder};
use qr_explore::amos;
use qr_explore::amos::TranslationResult;
use qr_explore::amos::AMOS;
use qr_explore::behaviours::Behaviour;
use qr_explore::exploration_settings::StateMutationSettings;
use qr_explore::explore;
use qr_http_resource::http;
use qr_open_api::open_api::ParseResult;
use qr_specification_manager as spec;
use std::thread;
use tracing::{debug, error, info};

#[derive(Default)]
pub enum ParseState {
    #[default]
    Waiting,
    Parse,
    Done,
}

enum ExplorationState {
    Idle,
    Running,
}

pub struct InvocationSpan {
    pub start: usize,
    pub end: usize,
    pub duration: std::time::Duration,
}

#[derive(PartialEq)]
pub enum DetailsNavigation {
    Logs,
    Details,
    Problems,
}

struct TimeLineEvent {
    instant: std::time::Instant,
    message: String,
}

pub struct App {
    // Serialized state of the app
    pub app_state: AppState,

    // File parsing
    pub picked_path: Option<String>,
    pub parse_state: ParseState,
    pub parse_result: Option<ParseResult>,

    // Exploration
    exploration_state: ExplorationState,

    // AMOS
    pub translation_result: Option<TranslationResult>,
    pub amos: amos::AMOS,
    pub selected_amos_operation: usize,
    pub selected_amos_definition: usize,

    exploration_results: Vec<explore::ExplorationResult>,
    // the stream of exploration events (TODO: Create an event type)
    exploration_log_rx: std::sync::mpsc::Receiver<explore::Event>,
    // TODO: add this to the exploration context creation
    exploration_log_tx: std::sync::mpsc::Sender<explore::Event>,
    pub invocation_results: Vec<amos::InvokeResult>,
    invocation_durations: Vec<std::time::Duration>,
    pub invocation_spans: Vec<InvocationSpan>,
    current_invocation_span_start: usize,
    exploration_log: Vec<explore::LogMessage>,

    // Generation
    pub generated_values: Vec<String>,
    pub generate_string_expression: String,

    // Timeline
    time_line_events: Vec<TimeLineEvent>,

    pub selected_span: usize,
    pub selected_result: usize,

    pub selected_details_navigation: DetailsNavigation,
    pub selected_details_sequence_operation: usize,

    // Exploration navigation
    selected_exploration_result: usize,

    // Commands that will be run at the end of the frame
    pub command_sender: CommandSender,
    command_receiver: CommandReceiver,
}

impl App {
    pub fn new() -> Self {
        let (exploration_log_tx, exploration_log_rx) = std::sync::mpsc::channel();

        // Check for data dir
        let data_dir = std::path::Path::new("./data");

        let (amos, app_state) = if data_dir.exists() {
            // load current amos, if any
            let current_amos_path = std::path::Path::new("./data/current_amos.amos");
            if current_amos_path.exists() {
                let loaded_amos = amos::AMOS::load(current_amos_path);

                // The rest of the project file depends on the AMOS beeing loaded,
                //  thus if the load fails, we should start with a clean app-state
                match loaded_amos {
                    Ok(amos) => {
                        info!("Loading current project");
                        (
                            amos,
                            AppState::load(std::path::Path::new("./data/project.json")),
                        )
                    }
                    _ => {
                        error!(
                            "Could not load current AMOS: {}",
                            current_amos_path.display()
                        );
                        info!("Start with clean AMOS and AppState");
                        (AMOS::new(), AppState::new())
                    }
                }
            } else {
                // no existing amos, start fresh
                (AMOS::new(), AppState::new())
            }
        } else {
            // Create the dir if it don't exist and start fresh
            if let Err(e) = std::fs::create_dir(data_dir) {
                error!("Faild to create data directory: {}", e)
            }
            (AMOS::new(), AppState::new())
        };

        let (command_sender, command_receiver) = command_channel();

        Self {
            picked_path: None,
            parse_state: ParseState::Waiting,
            parse_result: None,
            amos,
            translation_result: None,
            selected_amos_operation: 0,
            selected_amos_definition: 0,
            exploration_state: ExplorationState::Idle,
            exploration_results: vec![],
            exploration_log_rx,
            exploration_log_tx,
            exploration_log: vec![],
            invocation_results: vec![],
            invocation_durations: vec![],
            invocation_spans: vec![],
            current_invocation_span_start: 0,
            selected_result: 0,
            selected_span: 0,
            app_state,

            time_line_events: vec![],

            selected_details_navigation: DetailsNavigation::Details,
            selected_details_sequence_operation: 0,

            selected_exploration_result: 0,

            generated_values: vec![],
            generate_string_expression: "[a-z]*".to_string(),

            command_sender,
            command_receiver,
        }
    }

    fn create_target_from_settings(&self) -> explore::Target {
        explore::Target::HTTP {
            config: explore::HTTPConfiguration {
                base_url: self.app_state.target.base_url.clone(),
                port: self.app_state.target.port.parse().unwrap(),
                protocol: http::Protocol::HTTP,
            },
        }
    }

    fn process_exploration_events(&mut self, _ctx: &egui::Context) {
        while let Ok(event) = self.exploration_log_rx.try_recv() {
            //info!("{}", format!("Received event {:?}", event));
            match event {
                explore::Event::CurrentQueryOperation { .. } => {}
                explore::Event::InvocationSpanEnter { .. } => {}
                explore::Event::InvocationSpanExit { duration } => {
                    let start: usize = self.current_invocation_span_start;

                    // Check that this span actually contains invocations
                    // TODO: Display operations that where disscarded as we might want
                    //  to improve on the generation of those
                    if start != self.invocation_results.len() {
                        let end = start + (self.invocation_results.len() - 1 - start);
                        self.current_invocation_span_start = self.invocation_results.len();
                        debug!(start, end, "Span");
                        self.invocation_spans.push(InvocationSpan {
                            start,
                            end,
                            duration,
                        })
                    }
                }
                explore::Event::Invocation {
                    result: r,
                    sut_invocation_duration: d,
                } => {
                    self.invocation_durations.push(d);
                    self.invocation_results.push(r)
                }
                explore::Event::Log { message } => self.exploration_log.push(message),

                explore::Event::Control { event } => {
                    match event {
                        explore::ControlEvent::Finished => {
                            self.exploration_state = ExplorationState::Idle
                        }
                        // We do not want to react to started, since the UI already
                        //  know that we started (button clicked) and can react to that
                        explore::ControlEvent::Started => (),
                    }
                }

                explore::Event::Result { result } => {
                    self.exploration_results.push(result);
                }

                // Time line
                explore::Event::TimeLineStart { enter, message } => {
                    debug!("{:?}:{:?}", enter, message);
                    self.time_line_events.push(TimeLineEvent {
                        instant: enter,
                        message,
                    });
                }
                explore::Event::TimeLineProgress { time, message } => {
                    debug!("{:?}:{:?}", time, message);
                    self.time_line_events.push(TimeLineEvent {
                        instant: time,
                        message,
                    });
                }
                explore::Event::TimeLineEnd { time, message } => {
                    debug!("{:?}:{:?}", time, message);
                    self.time_line_events.push(TimeLineEvent {
                        instant: time,
                        message,
                    });
                }
            };
        }
    }

    fn exploration_api_ui(&mut self, ui: &mut egui::Ui) {
        ui.label(self.amos.name.clone());

        egui::Grid::new("exploration_summary_grid")
            .num_columns(2)
            .spacing([40.0, 4.0])
            .striped(true)
            .show(ui, |ui| {
                ui.label("Base URL");
                ui.text_edit_singleline(&mut self.app_state.target.base_url);
                ui.end_row();

                ui.label("Protocol");
                //ui.text_edit_singleline(&mut self.app_state.target.protocol);
                ui.horizontal(|ui| {
                    ui.selectable_value(
                        &mut self.app_state.target.protocol,
                        http::Protocol::HTTP,
                        http::Protocol::HTTP.to_string(),
                    );
                    ui.selectable_value(
                        &mut self.app_state.target.protocol,
                        http::Protocol::HTTPS,
                        http::Protocol::HTTPS.to_string(),
                    );
                });

                ui.end_row();

                ui.label("Port");
                ui.text_edit_singleline(&mut self.app_state.target.port);
                ui.end_row();
            });
    }

    fn exploration_summary_ui(&mut self, ui: &mut egui::Ui) {
        egui::Grid::new("exploration_summary_grid")
            .num_columns(2)
            .spacing([40.0, 4.0])
            .striped(true)
            .show(ui, |ui| {
                ui.label("Number of invocations");
                ui.label(self.invocation_results.len().to_string());
                ui.end_row();

                ui.label("Number of sequences");
                ui.label(self.invocation_spans.len().to_string());
                ui.end_row();
            });

        ui.collapsing("Coverage", |ui| ui.label("Foo"));
    }

    fn exploration_sequences_ui(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::both()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                egui::Grid::new("exploration_sequence_grid")
                    .num_columns(3)
                    .spacing([4.0, 4.0])
                    .striped(true)
                    .show(ui, |ui| {
                        for idx in 0..self.invocation_spans.len() {
                            let current_span = &self.invocation_spans[idx];
                            let span_results =
                                &self.invocation_results[current_span.start..current_span.end + 1];
                            let span_text = span_results
                                .iter()
                                .map(|result| "[".to_string() + &result.operation.name + "]")
                                .collect::<Vec<String>>()
                                .join(" -> ");

                            ui.push_id(idx, |ui| {
                                ui.label((idx + 1).to_string());
                                ui.label(format!("{} ms", current_span.duration.as_millis()));
                                ui.selectable_value(&mut self.selected_span, idx, span_text)
                            });
                            ui.end_row();
                        }
                    });
            });
    }

    fn exploration_invocations_ui(&mut self, ui: &mut egui::Ui) {
        egui::TopBottomPanel::top("Bar").show(ui.ctx(), |ui| {
            if ui.button("🗑").clicked() {
                self.invocation_results.clear();
                self.invocation_spans.clear();
                self.current_invocation_span_start = 0;
                self.selected_span = 0;
                self.selected_result = 0;
            };
        });

        egui::CentralPanel::default().show(ui.ctx(), |ui| {
            let table = TableBuilder::new(ui)
                .striped(true)
                .resizable(true)
                .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                .column(Column::auto())
                .column(Column::auto())
                .column(Column::auto())
                .column(Column::auto())
                .column(Column::remainder().clip(true));

            table
                .header(20.0, |mut header| {
                    header.col(|ui| {
                        ui.strong("Row");
                    });

                    header.col(|ui| {
                        ui.strong("Duration");
                    });

                    header.col(|ui| {
                        ui.strong("Operation");
                    });

                    header.col(|ui| {
                        ui.strong("Status");
                    });

                    header.col(|ui| {
                        ui.strong("URL");
                    });
                })
                .body(|mut body| {
                    for idx in 0..self.invocation_results.len() {
                        body.row(18.0, |mut row| {
                            row.col(|ui| {
                                ui.label((idx + 1).to_string());
                            });

                            row.col(|ui| {
                                ui.label(format!(
                                    "{} ms",
                                    self.invocation_durations[idx].as_millis()
                                ));
                            });

                            row.col(|ui| {
                                if ui
                                    .selectable_value(
                                        &mut self.selected_result,
                                        idx,
                                        self.invocation_results[idx].operation.name.clone(),
                                    )
                                    .clicked()
                                {
                                    // TODO: set the selected span
                                    //  However, we need data in the result to
                                    //  identify the span. This might fit in the
                                    //  exploration result.
                                };
                            });

                            if let Some(meta) = &self.invocation_results[idx].meta_data {
                                match meta {
                                    amos::ResultMetaData::HTTP { url, status } => {
                                        row.col(|ui| {
                                            ui.label(status.to_string());
                                        });

                                        row.col(|ui| {
                                            ui.label(url);
                                        });
                                    }
                                }
                            }
                        })
                    }
                });
        });
    }

    fn exploration_examples_ui(&mut self, ui: &mut egui::Ui) {
        egui::SidePanel::right("details_example")
            .resizable(true)
            .default_width(400.0)
            .width_range(80.0..=650.0)
            .show(ui.ctx(), |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.heading("Example details");
                    });

                    if !self.exploration_results.is_empty() {
                        let selected = &self.exploration_results[self.selected_exploration_result];

                        match selected {
                            explore::ExplorationResult::ResponseCheck {
                                minimal_sequence, ..
                            } => {
                                if let Some(seq) = minimal_sequence {
                                    for op in seq {
                                        ui.label(format!("{:?}", op.name));
                                        for p in &op.parameters {
                                            ui.label(format!("{} - {:?}", p.name, p.value));
                                        }
                                    }
                                }
                            }
                            explore::ExplorationResult::NoExampleFound { .. } => {
                                ui.label("No example found");
                            }
                            explore::ExplorationResult::StateMutation {
                                query_operation,
                                minimal_sequence,
                            } => {
                                ui.label("Query operation:");
                                ui.label(query_operation.clone());

                                if let Some(seq) = minimal_sequence {
                                    ui.collapsing("Minimal sequence", |ui| {
                                        for (idx, op) in seq.iter().enumerate() {
                                            ui.push_id(idx, |ui| {
                                                if op.parameters.is_empty() {
                                                    ui.label(op.name.clone());
                                                } else {
                                                    ui.collapsing(op.name.clone(), |ui| {
                                                        for param in &op.parameters {
                                                            ui.label(format!(
                                                                "{} - {:?}",
                                                                param.name, param.value
                                                            ));
                                                        }
                                                    });
                                                }
                                            });
                                        }
                                    });
                                }
                            }
                            explore::ExplorationResult::StateIdentity {
                                query_operation,
                                minimal_sequence,
                            } => {
                                ui.label("Query operation:");
                                ui.label(query_operation.clone());

                                if let Some(seq) = minimal_sequence {
                                    ui.collapsing("Minimal sequence", |ui| {
                                        for (idx, op) in seq.iter().enumerate() {
                                            ui.push_id(idx, |ui| {
                                                if op.parameters.is_empty() {
                                                    ui.label(op.name.clone());
                                                } else {
                                                    ui.collapsing(op.name.clone(), |ui| {
                                                        for param in &op.parameters {
                                                            ui.label(format!(
                                                                "{} - {:?}",
                                                                param.name, param.value
                                                            ));
                                                            if let Some(rp) = &param.ref_path {
                                                                ui.label(format!(
                                                                    "Ref path: {}",
                                                                    rp
                                                                ));
                                                            }
                                                        }
                                                    });
                                                }
                                            });
                                        }
                                    });
                                }
                            }
                            explore::ExplorationResult::ResponseEquality {
                                minimal_sequence,
                                ..
                            } => {
                                if let Some(seq) = minimal_sequence {
                                    ui.collapsing("Minimal sequence", |ui| {
                                        for (idx, op) in seq.iter().enumerate() {
                                            ui.push_id(idx, |ui| {
                                                if op.parameters.is_empty() {
                                                    ui.label(op.name.clone());
                                                } else {
                                                    ui.collapsing(op.name.clone(), |ui| {
                                                        for param in &op.parameters {
                                                            ui.label(format!(
                                                                "{} - {:?}",
                                                                param.name, param.value
                                                            ));
                                                        }
                                                    });
                                                }
                                            });
                                        }
                                    });
                                }
                            }
                            explore::ExplorationResult::ResponseInEquality {
                                minimal_sequence,
                                ..
                            } => {
                                if let Some(seq) = minimal_sequence {
                                    ui.collapsing("Minimal sequence", |ui| {
                                        for (idx, op) in seq.iter().enumerate() {
                                            ui.push_id(idx, |ui| {
                                                if op.parameters.is_empty() {
                                                    ui.label(op.name.clone());
                                                } else {
                                                    ui.collapsing(op.name.clone(), |ui| {
                                                        for param in &op.parameters {
                                                            ui.label(format!(
                                                                "{} - {:?}",
                                                                param.name, param.value
                                                            ));
                                                        }
                                                    });
                                                }
                                            });
                                        }
                                    });
                                }
                            }
                        }
                    };
                });
            });

        if !self.exploration_results.is_empty() {
            // TODO: Does this make sense? The exploration result can contain
            //  any results so they should be in thier own buckets or?
            ui.collapsing("Examples", |ui| {
                for (idx, example) in self.exploration_results.iter().enumerate() {
                    match example {
                        explore::ExplorationResult::ResponseCheck { operation, .. } => {
                            ui.selectable_value(
                                &mut self.selected_exploration_result,
                                idx,
                                operation.clone().to_string(),
                            );
                        }
                        explore::ExplorationResult::NoExampleFound { operation } => {
                            ui.selectable_value(
                                &mut self.selected_exploration_result,
                                idx,
                                format!("{} - No example found", operation.clone()),
                            );
                        }
                        explore::ExplorationResult::StateMutation {
                            query_operation, ..
                        } => {
                            ui.selectable_value(
                                &mut self.selected_exploration_result,
                                idx,
                                query_operation.clone(),
                            );
                        }
                        explore::ExplorationResult::StateIdentity {
                            query_operation, ..
                        } => {
                            ui.selectable_value(
                                &mut self.selected_exploration_result,
                                idx,
                                query_operation.clone(),
                            );
                        }
                        explore::ExplorationResult::ResponseEquality { operation, .. } => {
                            ui.selectable_value(
                                &mut self.selected_exploration_result,
                                idx,
                                operation.clone(),
                            );
                        }
                        explore::ExplorationResult::ResponseInEquality { operation, .. } => {
                            ui.selectable_value(
                                &mut self.selected_exploration_result,
                                idx,
                                operation.clone(),
                            );
                        }
                    }
                    // ui.label(format!("{:#?}", example));
                    // ui.separator();
                }
            });
        } else {
            ui.label("No examples to show");
        }
        
    }

    fn exploration_progress_ui(&mut self, ui: &mut egui::Ui) {
        egui::CentralPanel::default().show(ui.ctx(), |ui| {
            let table = TableBuilder::new(ui)
                .striped(true)
                .resizable(true)
                .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                .column(Column::auto())
                .column(Column::auto())
                .column(Column::auto())
                .column(Column::remainder().clip(true));
            table
                .header(20.0, |mut header| {
                    header.col(|ui| {
                        ui.strong("Row");
                    });

                    header.col(|ui| {
                        ui.strong("Rel.");
                    });

                    header.col(|ui| {
                        ui.strong("Abs.");
                    });

                    header.col(|ui| {
                        ui.strong("Event");
                    });
                })
                .body(|mut body| {
                    for idx in 0..self.time_line_events.len() {
                        let current_event = &self.time_line_events[idx];

                        let relative_duration = if idx > 0 {
                            current_event
                                .instant
                                .duration_since(self.time_line_events[idx - 1].instant)
                        } else {
                            current_event.instant.duration_since(current_event.instant)
                        };

                        let duration = current_event
                            .instant
                            .duration_since(self.time_line_events[0].instant);

                        body.row(18.0, |mut row| {
                            row.col(|ui| {
                                ui.label((idx + 1).to_string());
                            });

                            row.col(|ui| {
                                ui.label(format!("{} ms", relative_duration.as_millis(),));
                            });

                            row.col(|ui| {
                                ui.label(format!("{} s", duration.as_secs(),));
                            });

                            row.col(|ui| {
                                ui.selectable_value(
                                    &mut self.selected_result,
                                    idx,
                                    current_event.message.clone(),
                                );
                            });
                        })
                    }
                })
        });
    }

    fn exploration_sequencer_ui(&mut self, ui: &mut egui::Ui) {
        if self.app_state.sequencer.selected_sequence().is_none() {
            ui.label("No sequence selected");
            return;
        }

        ui.horizontal_top(|ui| {
            ////
            // 'Play' of a complete sequence
            if ui.button("▶").clicked() {
                // TODO: consolidate with the 'play' on an item
                let channel = self.exploration_log_tx.clone();
                let ops = self.amos.operations.clone();

                // TODO: this need to change to be able to also include 'behaviours'.
                //  Alternatively, there will be two different (but how then should
                //  order be preserved?)
                let mut ops_to_invoke = vec![];

                if let Some(selected) = self.app_state.sequencer.selected_sequence_as_mut() {
                    for idx in 0..selected.items.len() {
                        let sequence_item = &mut selected.items[idx];
                        match sequence_item {
                            sequencer::SequenceItem::Operation {
                                amos_op_id,
                                parameters,
                            } => {
                                let current = &self.amos.operations[*amos_op_id];

                                // The params of the op should use the schemas defined
                                //  in the sequence UI
                                let mut new_op = current.clone();
                                let mut templated_params = vec![];
                                for (idx, new_param) in new_op.parameters.iter_mut().enumerate() {
                                    new_param.schema = amos::Schema::StringRegex {
                                        regex: parameters[idx].template.clone(),
                                    };
                                    templated_params.push(new_param.clone());
                                }

                                new_op.parameters = templated_params;
                                ops_to_invoke.push(new_op);
                            }
                            sequencer::SequenceItem::Behaviour { .. } => (),
                            sequencer::SequenceItem::Fuzzer { .. } => (),
                        }
                    }
                }

                // Set exploration state to let the UI react accordingly
                // TODO: Make a more representable state
                self.exploration_state = ExplorationState::Running;

                let target = self.create_target_from_settings();
                let is_dry_run =
                    if let Some(selected) = self.app_state.sequencer.selected_sequence() {
                        selected.is_dry_run
                    } else {
                        false
                    };

                let invoke = if !is_dry_run {
                    explore::invoke
                } else {
                    explore::dry_invoke
                };

                thread::spawn(move || {
                    let context = &explore::ExplorationContext {
                        http_client: reqwest::blocking::Client::new(),
                        target,
                        query_operation: None,
                        tx: Some(channel),
                        number_of_tests: 1,
                        min_length: 1,
                        max_length: 1,
                    };

                    explore::sequence_invoke(context, ops, invoke, ops_to_invoke);
                });
                self.exploration_log.push(explore::LogMessage {
                    level: explore::LogLevel::Info,
                    message: "Invoke sequence".to_string(),
                });
            }

            let add_enabled = matches!(
                self.app_state.selected_navigation,
                main_navigation::MainNavigation::Operations
                    | main_navigation::MainNavigation::Exploration
                    | main_navigation::MainNavigation::Fuzzing
            );

            if ui
                .add_enabled(add_enabled, egui::Button::new("➕"))
                .clicked()
            {
                // Until drag n drop support we need to match on which details UI
                //  is selected. We can add Ops, Explorations, or Fuzzers
                match self.app_state.selected_navigation {
                    main_navigation::MainNavigation::Fuzzing => {
                        let current = self.app_state.fuzzing.selected.clone();

                        let new_item = sequencer::SequenceItem::Fuzzer {
                            property: current,
                            settings: PropertySettings::new(),
                        };

                        self.app_state.sequencer.push_item_to_selected(new_item);

                        self.exploration_log.push(explore::LogMessage {
                            level: explore::LogLevel::Info,
                            message: format!(
                                "Added {} to sequence",
                                self.app_state.fuzzing.selected
                            ),
                        });
                    }

                    main_navigation::MainNavigation::Operations => {
                        let current_idx = self.selected_amos_operation;
                        let current_op = &self.amos.operations[current_idx];

                        let new_item =
                            sequencer::SequenceItem::new(current_idx, &current_op.parameters);

                        self.app_state.sequencer.push_item_to_selected(new_item);

                        self.exploration_log.push(explore::LogMessage {
                            level: explore::LogLevel::Info,
                            message: format!("Added {} to sequence", current_op.info.name.clone()),
                        });
                    }
                    main_navigation::MainNavigation::Exploration => {
                        let behaviour = self.app_state.behaviour.selected.clone();

                        self.exploration_log.push(explore::LogMessage {
                            level: explore::LogLevel::Info,
                            message: format!("Added {} to sequence", behaviour.presentation()),
                        });

                        let parameters = StateMutationSettings {
                            number_of_tests: 100,
                            min_length: 1,
                            max_length: 2,
                            query_operation_ids: vec![],
                            selected_query_operation: None,
                        };
                        let new_item = sequencer::SequenceItem::Behaviour {
                            behaviour,
                            parameters,
                        };

                        self.app_state.sequencer.push_item_to_selected(new_item);
                    }
                    _ => (),
                }
            }

            if ui.button("➖").clicked() {
                if let Some(selected) = self.app_state.sequencer.selected_sequence_as_mut() {
                    selected.remove_selected();
                }
            }

            // Sequence name edit
            if let Some(selected) = self.app_state.sequencer.selected_sequence_as_mut() {
                ui.add(egui::TextEdit::singleline(&mut selected.name).hint_text("New sequence"));
            }

            // Dry run mode
            if let Some(selected) = self.app_state.sequencer.selected_sequence_as_mut() {
                ui.add(egui::Checkbox::new(&mut selected.is_dry_run, "Dry run"));
            }
        });

        ui.separator();

        ui.horizontal_centered(|ui| {
            if let Some(selected_idx) = self.app_state.sequencer.selected_sequence_id {
                for idx in 0..self.app_state.sequencer.sequences[selected_idx].items.len() {
                    ui.vertical(|ui| {
                        ui.horizontal_top(|ui| {
                            let target = self.create_target_from_settings();
                            let selected = &mut self.app_state.sequencer.sequences[selected_idx];
                            match &mut selected.items[idx] {
                                sequencer::SequenceItem::Operation {
                                    amos_op_id,
                                    parameters,
                                } => {
                                    let current = &self.amos.operations[*amos_op_id];
                                    ui.selectable_value(
                                        &mut selected.selected,
                                        idx,
                                        current.info.name.clone(),
                                    );

                                    if ui.button("▶").clicked() {
                                        let channel = self.exploration_log_tx.clone();
                                        let ops = self.amos.operations.clone();

                                        // The params of the op should use the schemas defined
                                        //  in the sequence UI
                                        let mut new_op = current.clone();
                                        let mut templated_params = vec![];
                                        for (idx, new_param) in
                                            new_op.parameters.iter_mut().enumerate()
                                        {
                                            new_param.schema = amos::Schema::StringRegex {
                                                regex: parameters[idx].template.clone(),
                                            };
                                            templated_params.push(new_param.clone());
                                        }

                                        new_op.parameters = templated_params;

                                        // Set exploration state to let the UI react accordingly
                                        // TODO: Make a more representable state
                                        self.exploration_state = ExplorationState::Running;

                                        let target = self.create_target_from_settings();
                                        let is_dry_run = if let Some(selected) =
                                            self.app_state.sequencer.selected_sequence()
                                        {
                                            selected.is_dry_run
                                        } else {
                                            false
                                        };
                                        let invoke = if !is_dry_run {
                                            explore::invoke
                                        } else {
                                            explore::dry_invoke
                                        };

                                        thread::spawn(move || {
                                            let context = &explore::ExplorationContext {
                                                http_client: reqwest::blocking::Client::new(),
                                                target,
                                                query_operation: None,
                                                tx: Some(channel),
                                                number_of_tests: 1,
                                                min_length: 1,
                                                max_length: 1,
                                            };

                                            explore::sequence_invoke(
                                                context,
                                                ops,
                                                invoke,
                                                vec![new_op],
                                            );
                                        });
                                        self.exploration_log.push(explore::LogMessage {
                                            level: explore::LogLevel::Info,
                                            message: format!(
                                                "Invoke {}",
                                                current.info.name.clone()
                                            ),
                                        });
                                    }
                                }
                                sequencer::SequenceItem::Fuzzer { property, settings } => {
                                    ui.label(property.to_string());

                                    if ui.button("▶").clicked() {
                                        // Set exploration state to let the UI react accordingly
                                        self.exploration_state = ExplorationState::Running;

                                        qr_explore::spawn_exploration(
                                            &target,
                                            selected.is_dry_run,
                                            &self.amos,
                                            self.exploration_log_tx.clone(),
                                            self.amos.operations.clone(),
                                            &Behaviour::Property,
                                            // TODO: there is probably a difference between general parameters and behaviour specific ones
                                            &StateMutationSettings {
                                                number_of_tests: 100,
                                                min_length: 1,
                                                max_length: 1,
                                                query_operation_ids: settings.operations.clone(),
                                                selected_query_operation: None,
                                            }, //&self.invocation_results,
                                        );
                                    }
                                }
                                sequencer::SequenceItem::Behaviour {
                                    behaviour,
                                    parameters,
                                } => {
                                    ui.label(behaviour.presentation());

                                    if ui.button("▶").clicked() {
                                        // Set exploration state to let the UI react accordingly
                                        self.exploration_state = ExplorationState::Running;

                                        qr_explore::spawn_exploration(
                                            &target,
                                            selected.is_dry_run,
                                            &self.amos,
                                            self.exploration_log_tx.clone(),
                                            self.amos.operations.clone(),
                                            behaviour,
                                            // TODO: there is probably a difference between general parameters and behaviour specific ones
                                            parameters,
                                            //&self.invocation_results,
                                        );
                                    };
                                }
                            }
                        });

                        ui.label("Parameters");

                        let selected =
                            &mut self.app_state.sequencer.sequences[selected_idx].items[idx];

                        match selected {
                            sequencer::SequenceItem::Operation {
                                amos_op_id: _,
                                parameters,
                            } => {
                                for param in parameters {
                                    ui.label(param.name.clone());
                                    ui.text_edit_singleline(&mut param.template);
                                }
                            }
                            sequencer::SequenceItem::Fuzzer { settings, .. } => {
                                ui.horizontal_top(|ui| {
                                    ui.label("Selected Operations");

                                    if ui.add_enabled(true, egui::Button::new("➕")).clicked() {
                                        let selected_id = self.selected_amos_operation;

                                        settings.operations.push(selected_id);

                                        self.exploration_log.push(explore::LogMessage {
                                            level: explore::LogLevel::Info,
                                            message: format!(
                                                "Add operation to property: {}",
                                                selected_id,
                                            ),
                                        });
                                    }

                                    if ui.button("➖").clicked() {
                                        // TODO:
                                    }
                                });

                                for op in &settings.operations {
                                    let _name = self.amos.operations[*op].info.name.clone();
                                    // ui.collapsing(name, |ui| {
                                    //     for param in &self.amos.operations[*op].parameters {
                                    //         ui.label(format!("{}", param.name));
                                    //     }
                                    // });

                                    ui.selectable_value(
                                        &mut settings.selected_operation,
                                        Some(*op),
                                        self.amos.operations[*op].info.name.clone(),
                                    );
                                }
                            }
                            sequencer::SequenceItem::Behaviour {
                                behaviour: _,
                                parameters,
                            } => {
                                ui.add(
                                    egui::Slider::new(&mut parameters.number_of_tests, 1..=1000)
                                        .text("Number of tests/property"),
                                );

                                ui.add(
                                    egui::Slider::new(&mut parameters.min_length, 1..=10)
                                        .text("Smallest number of operations"),
                                );

                                ui.add(
                                    egui::Slider::new(&mut parameters.max_length, 1..=10)
                                        .text("Largest number of operations"),
                                );

                                ui.horizontal_top(|ui| {
                                    ui.label("Select Query ops");

                                    if ui.add_enabled(true, egui::Button::new("➕")).clicked() {
                                        let selected_id = self.selected_amos_operation;

                                        parameters.query_operation_ids.push(selected_id);
                                        self.exploration_log.push(explore::LogMessage {
                                            level: explore::LogLevel::Info,
                                            message: format!(
                                                "Add operation to behaviour {}",
                                                selected_id,
                                            ),
                                        });
                                    }

                                    if ui.button("➖").clicked() {
                                        parameters.remove_selected_query_operation();
                                    }
                                });

                                for (id, query_op) in
                                    parameters.query_operation_ids.iter().enumerate()
                                {
                                    ui.selectable_value(
                                        &mut parameters.selected_query_operation,
                                        Some(id),
                                        self.amos.operations[*query_op].info.name.clone(),
                                    );
                                }
                            }
                        }
                    });

                    ui.separator();
                }
            }
        });
    }

    fn exploration_ui(&mut self, ui: &mut egui::Ui) {
        match self.app_state.central_navigation.selected {
            central_navigation::Navigations::APIs => {
                self.exploration_api_ui(ui);
            }
            central_navigation::Navigations::Summary => {
                self.exploration_summary_ui(ui);
            }
            central_navigation::Navigations::Sequences => {
                self.exploration_sequences_ui(ui);
            }
            central_navigation::Navigations::Invocations => {
                self.exploration_invocations_ui(ui);
            }
            central_navigation::Navigations::Examples => {
                self.exploration_examples_ui(ui);
            }
            central_navigation::Navigations::Progress => {
                self.exploration_progress_ui(ui);
            }
            central_navigation::Navigations::Sequencer => {
                self.exploration_sequencer_ui(ui);
            }
        };
    }

    pub fn log_table(&self, ui: &mut egui::Ui) {
        let messages = &self.exploration_log;

        let table = TableBuilder::new(ui)
            .striped(true)
            .resizable(true)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .column(Column::auto())
            .column(Column::auto())
            .column(Column::remainder().clip(false))
            .min_scrolled_height(400.0);

        table
            .header(20.0, |mut header| {
                header.col(|ui| {
                    ui.strong("Row");
                });

                header.col(|ui| {
                    ui.strong("Level");
                });

                header.col(|ui| {
                    ui.strong("Message");
                });
            })
            .body(|mut body| {
                for (log_index, message) in messages.iter().enumerate() {
                    body.row(18.0, |mut row| {
                        row.col(|ui| {
                            ui.label((log_index + 1).to_string());
                        });

                        // Level
                        row.col(|ui| {
                            let label = match message.level {
                                explore::LogLevel::Info => "Info".to_owned(),
                                explore::LogLevel::Warning => "Warning".to_owned(),
                                explore::LogLevel::Error => "Error".to_owned(),
                            };

                            ui.label(label);
                        });

                        // Message
                        row.col(|ui| {
                            ui.label(messages[log_index].message.clone());
                        });
                    })
                }
            })
    }

    fn save(&self) {
        info!("Save file");

        // TODO: make this more production ready
        let project_path = std::path::Path::new("./data/project.json");
        self.app_state.save(project_path);
    }

    fn run_ui_command(&self, command: UICommand) {
        info!("Run ui command");

        match command {
            UICommand::Save => {
                self.save();
            }
        }
    }

    fn run_pending_ui_commands(&mut self) {
        while let Some(cmd) = self.command_receiver.receive_ui() {
            self.run_ui_command(cmd);
        }
    }

    /// The top-level ui
    fn ui(&mut self, egui_ctx: &egui::Context) {
        crate::ui::top_panel(egui_ctx, self);

        crate::ui::bottom_panel(egui_ctx, self);

        crate::ui::navigation_panels(egui_ctx, self);
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.ui(ctx);

        egui::CentralPanel::default().show(ctx, |ui| {
            // TODO: Move this somewhere..
            // Show file info
            if let Some(picked_path) = &self.picked_path {
                ui.horizontal(|ui| {
                    ui.label("Picked file:");
                    ui.monospace(picked_path);
                });

                if let ParseState::Parse = self.parse_state {
                    let Ok((parse_result, translation_result)) =
                        spec::manager::load_open_api_file_path(picked_path)
                    else {
                        // Decide how to signal errors to the user
                        todo!()
                    };
                    let path = self.app_state.current_amos_path.clone().unwrap();
                    let amos_path = std::path::Path::new(&path);

                    translation_result.amos.save(amos_path);

                    // TODO: Do we want a clone and preserve the 'original' in the result or not?
                    self.amos = translation_result.amos.clone();
                    self.translation_result = Some(translation_result);
                    self.parse_result = Some(parse_result);
                    self.parse_state = ParseState::Done;
                    // Currently, we only support one working AMOS, so clear the workspace
                    // This is not ideal, allow easy change between different AMOSes
                    self.app_state = AppState::new();
                }
            }

            egui::TopBottomPanel::top("exploration_view_top_panel").show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.selectable_value(
                        &mut self.app_state.central_navigation.selected,
                        central_navigation::Navigations::Summary,
                        "📊 Summary",
                    );

                    ui.selectable_value(
                        &mut self.app_state.central_navigation.selected,
                        central_navigation::Navigations::APIs,
                        "🕸 APIs",
                    );

                    ui.selectable_value(
                        &mut self.app_state.central_navigation.selected,
                        central_navigation::Navigations::Examples,
                        "🗊 Examples",
                    );

                    ui.selectable_value(
                        &mut self.app_state.central_navigation.selected,
                        central_navigation::Navigations::Sequences,
                        "⬇ Sequences",
                    );

                    ui.selectable_value(
                        &mut self.app_state.central_navigation.selected,
                        central_navigation::Navigations::Invocations,
                        "📝 Invocations",
                    );

                    ui.selectable_value(
                        &mut self.app_state.central_navigation.selected,
                        central_navigation::Navigations::Progress,
                        "🕟 Progress",
                    );

                    ui.selectable_value(
                        &mut self.app_state.central_navigation.selected,
                        central_navigation::Navigations::Sequencer,
                        "↻ Sequencer",
                    );
                });
            });

            egui::CentralPanel::default().show(ctx, |ui| {
                self.exploration_ui(ui);
            });

            if let ExplorationState::Running = self.exploration_state {
                self.process_exploration_events(ctx);
                // Repaint as soon as possible to draw the UI effect of the event
                ctx.request_repaint();
            };

            self.run_pending_ui_commands();
        });
    }
}
