use std::thread;

use clap::{Args, Parser, Subcommand, ValueEnum};
use colored::*;
use qr_explore::meta_properties as mp;
use qr_explore::{
    amos::{self, InvokeResult, OperationMetaData, ResultMetaData},
    amos_generation::GeneratedOperation,
    behaviours,
    exploration_settings::StateMutationSettings,
    explore,
};
use qr_http_resource::http;
use qr_http_resource::http::HTTPMethod;
use qr_objective_manager as obj_mgr;
use qr_open_api::open_api;
use qr_report::report;
use qr_specification_manager as spec_mgr;
use reqwest::Url;
use tracing::info;

#[derive(Debug, Args)]
struct SUTArgs {
    /// Port number of the SUT
    #[arg(short, long, default_value_t = 80, value_parser = clap::value_parser!(u16).range(1..))]
    port: u16,

    /// Hostname of the SUT
    #[arg(short('H'), long, default_value_t= Url::parse("http://localhost").unwrap(), value_parser = valid_hostname)]
    hostname: Url,
}

#[derive(Debug, Args)]
#[group(required = true, multiple = false)]
struct OASArgs {
    /// URL of the OpenAPI specification
    #[arg(short, long, value_parser = valid_hostname)]
    url: Option<Url>,

    /// File path to OpenAPI specification
    #[arg(short, long)]
    file: Option<String>,
}

#[derive(Parser)]
struct Cli {
    /// Use QuickREST to explore or to execute test cases
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Copy, Clone, ValueEnum)]
enum Behaviour {
    /// Explore the SUT to find crashes (status code 500)
    Fuzz,
    /// Find examples of sequences where an operation return the same response when invoked twice
    ResponseEquality,
    /// Find examples of sequences where an operation return different responses when invoked twice
    ResponseInequality,
    /// Find sequences of operations where the state of a GET operation has changed
    StateMutation,
    /// Find sequences of operations where the state of a GET operation has changed, but is then undone, bringing the state back to the initial state
    StateIdentity,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Explore {
        #[command(flatten)]
        sut: SUTArgs,

        #[command(flatten)]
        oas: OASArgs,

        /// The name of the behaviour to explore
        #[arg(short, long)]
        behaviour: Vec<Behaviour>,

        /// Min number of operations per behaviour seq
        #[arg(long("min"), default_value_t = 1, value_parser = clap::value_parser!(u8).range(1..=5))]
        min_seq_size: u8,

        /// Max number of operations per behaviour seq
        #[arg(long("max"), default_value_t = 1, value_parser = clap::value_parser!(u8).range(1..=5))]
        max_seq_size: u8,

        /// The max number of tests per behaviour
        #[arg(short, long, default_value_t = 100, value_parser = clap::value_parser!(u16).range(1..=1000))]
        tests: u16,
    },
    Test {
        #[command(flatten)]
        sut: SUTArgs,

        /// File path to examples to test
        #[arg(short, long)]
        file: Vec<String>,
    },
}

struct AppState {
    results: Vec<explore::ExplorationResult>,
    invocation_results: Vec<InvokeResult>,
    invocation_spans: Vec<InvocationSpan>,
    current_invocation_span_start: usize,
    current_root_operation: String,
    start_time: Option<std::time::Instant>,
    end_time: Option<std::time::Instant>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            results: vec![],
            invocation_results: vec![],
            invocation_spans: vec![],
            current_invocation_span_start: 0,
            current_root_operation: "".to_string(),
            start_time: None,
            end_time: None,
        }
    }
}

pub struct InvocationSpan {
    pub query_operation: String,
    pub start: usize,
    pub end: usize,
    pub duration: std::time::Duration,
}

fn valid_hostname(s: &str) -> Result<Url, String> {
    let parse_result = Url::parse(s);

    match &parse_result {
        Ok(url) => Ok(url.clone()),
        Err(e) => Err(e.to_string()),
    }
}

fn main() {
    // install global collector configured based on RUST_LOG env var.
    tracing_subscriber::fmt()
        .with_target(true)
        //.with_env_filter(EnvFilter::from_default_env())
        .init();
    //tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Test { sut, file } => {
            println!("Running QuickREST in Test-mode!");
            println!("SUT Port: {}", sut.port);
            println!("SUT Hostname: {}", sut.hostname);
            println!("Test files: {:?}", file);

            println!("Starting invocations..");
            let mut app_state = AppState::new();

            for test_file in file {
                app_state.start_time = Some(std::time::Instant::now());
                match report::read_results_for_test(&test_file) {
                    Ok(report) => {
                        let scheme = match sut.hostname.scheme() {
                            "http" => http::Protocol::HTTP,
                            "https" => http::Protocol::HTTPS,
                            _ => {
                                println!(
                                    "{}: {}",
                                    "Unsupported SUT scheme: ".red(),
                                    sut.hostname.scheme()
                                );
                                std::process::exit(1)
                            }
                        };

                        let target = explore::Target::HTTP {
                            config: explore::HTTPConfiguration::new(
                                sut.hostname.host().unwrap().to_string(),
                                sut.port,
                                scheme,
                            ),
                        };

                        // TODO: pull from options
                        let min_seq_length = 0;
                        let max_seq_length = 1;
                        let number_of_tests = 100;

                        let (exploration_log_tx, exploration_log_rx) = std::sync::mpsc::channel();

                        let ctx = explore::ExplorationContext {
                            http_client: reqwest::blocking::Client::new(),
                            target,
                            query_operation: None,
                            tx: Some(exploration_log_tx),
                            number_of_tests,
                            // TODO: Adapt to the different properties
                            min_length: min_seq_length,
                            max_length: max_seq_length,
                        };

                        let amos = report.amos.clone();
                        let sequences = report.sequences.clone();
                        let thread_handle = thread::spawn(move || {
                            for seq in sequences {
                                // TODO: this do not feel like the best way of keeping track
                                //  of the spans current query_operation..
                                ctx.publish_event(explore::Event::CurrentQueryOperation {
                                    operation: seq.root_operation.clone(),
                                });

                                let ops = seq
                                    .operations
                                    .iter()
                                    .map(|op| GeneratedOperation {
                                        name: op.name.clone(),
                                        parameters: op.parameters.clone(),
                                    })
                                    .collect::<Vec<GeneratedOperation>>();
                                explore::invoke(&ctx, &amos.operations, &ops);
                            }
                        });

                        process_exploration_events(&mut app_state, exploration_log_rx);
                        thread_handle.join().expect("Invocation thread panicked");

                        info!(
                            "Test finished with {} invocations",
                            app_state.invocation_results.len()
                        );
                        info!("Checking results:");
                        for idx in 0..app_state.invocation_spans.len() {
                            println!();
                            info!("Sequence: {}", idx + 1);
                            let current_span = &app_state.invocation_spans[idx];
                            let span_results = &app_state.invocation_results
                                [current_span.start..current_span.end + 1];

                            info!("Query Operations: {}", current_span.query_operation);

                            let span_text = span_results
                                .iter()
                                .map(|result| {
                                    let meta = if let Some(meta) = &result.meta_data {
                                        match meta {
                                            ResultMetaData::HTTP { url, status } => {
                                                format!(" ({url}) - {status}")
                                            }
                                        }
                                    } else {
                                        "".to_string()
                                    };
                                    "[".to_string() + &result.operation.name + &meta + "]"
                                })
                                .collect::<Vec<String>>()
                                .join(" -> ");

                            info!("{}", span_text);

                            // Check the result based on which behaviour it was reported for
                            let check_result = match report.behaviour {
                                behaviours::Behaviour::Property => mp::check_response(span_results),
                                behaviours::Behaviour::ResponseEquality => {
                                    mp::check_response_equality(span_results)
                                }
                                behaviours::Behaviour::ResponseInequality => {
                                    mp::check_response_inequality(span_results)
                                }
                                behaviours::Behaviour::StateMutation => {
                                    let query_results = span_results
                                        .iter()
                                        .filter(|res| {
                                            res.operation.name == current_span.query_operation
                                        })
                                        .cloned()
                                        .collect::<Vec<InvokeResult>>();
                                    mp::check_state_mutation(&query_results)
                                }
                                behaviours::Behaviour::StateIdentity => {
                                    let query_results = span_results
                                        .iter()
                                        .filter(|res| {
                                            res.operation.name == current_span.query_operation
                                        })
                                        .cloned()
                                        .collect::<Vec<InvokeResult>>();
                                    mp::check_state_identity_with_observation(&query_results)
                                }
                            };

                            info!("Failing check: {}", !check_result);
                        }
                        app_state.end_time = Some(std::time::Instant::now());

                        info!(
                            "Test time: {:?}",
                            app_state.end_time.unwrap() - app_state.start_time.unwrap()
                        );
                    }
                    Err(e) => {
                        println!("Could not read file: {}", e);
                        std::process::exit(1);
                    }
                }
            }
        }
        Commands::Explore {
            sut,
            oas,
            behaviour,
            min_seq_size,
            max_seq_size,
            tests,
        } => {
            let mut app_state = AppState::new();

            println!("Running QuickREST in Explore-mode!");
            println!("SUT Port: {}", sut.port);
            println!("SUT Hostname: {}", sut.hostname);

            let amos_translation = if let Some(path) = oas.file {
                info!("Parsing OpenAPI file : {}", path);
                handle_parse_result(spec_mgr::manager::load_open_api_file_path(&path))
            } else if let Some(url) = oas.url {
                info!("URL of OpenAPI-specification: {}", url);
                handle_parse_result(spec_mgr::manager::fetch_open_api_from_url(&url))
            } else {
                println!("{}", "No source of OpenAPI-specification provided".red());
                std::process::exit(1);
            };

            println!("Exploration settings:");
            println!("Behaviour: {:?}", behaviour);
            println!(
                "Min. operations in sequences: {}, Max . operations in sequences: {}, Tests/Behaviour: {}",
                min_seq_size, max_seq_size, tests
            );

            // Steps
            // 1. setup context to call explore
            let scheme = match sut.hostname.scheme() {
                "http" => http::Protocol::HTTP,
                "https" => http::Protocol::HTTPS,
                _ => {
                    println!(
                        "{}: {}",
                        "Unsupported SUT scheme: ".red(),
                        sut.hostname.scheme()
                    );
                    std::process::exit(1)
                }
            };

            let target = explore::Target::HTTP {
                config: explore::HTTPConfiguration::new(
                    sut.hostname.host().unwrap().to_string(),
                    sut.port,
                    scheme,
                ),
            };

            // TODO: get from args
            let is_dry_run = false;

            println!("Target: {:#?}", target);

            //let (exploration_log_tx, exploration_log_rx) = std::sync::mpsc::channel();

            for b in behaviour {
                let query_ops = match b {
                    Behaviour::Fuzz
                    | Behaviour::ResponseEquality
                    | Behaviour::ResponseInequality => {
                        (0..amos_translation.amos.operations.len()).collect::<Vec<usize>>()
                    }
                    Behaviour::StateMutation | Behaviour::StateIdentity => {
                        let mut get_ops = vec![];

                        for idx in 0..amos_translation.amos.operations.len() {
                            if let Some(meta) = &amos_translation.amos.operations[idx].meta_data {
                                match meta {
                                    OperationMetaData::HTTP { method, .. } => {
                                        if *method == HTTPMethod::GET {
                                            get_ops.push(idx);
                                        }
                                    }
                                }
                            }
                        }

                        get_ops
                    }
                };

                let explore_behaviour = match b {
                    Behaviour::Fuzz => qr_explore::behaviours::Behaviour::Property,
                    Behaviour::ResponseEquality => {
                        qr_explore::behaviours::Behaviour::ResponseEquality
                    }
                    Behaviour::ResponseInequality => {
                        qr_explore::behaviours::Behaviour::ResponseInequality
                    }
                    Behaviour::StateMutation => qr_explore::behaviours::Behaviour::StateMutation,
                    Behaviour::StateIdentity => qr_explore::behaviours::Behaviour::StateIdentity,
                };

                let (handle, rx) = obj_mgr::manager::explore(
                    &target,
                    &obj_mgr::manager::Options { is_dry_run },
                    &amos_translation.amos,
                    &explore_behaviour,
                    &StateMutationSettings {
                        number_of_tests: tests,
                        min_length: min_seq_size,
                        max_length: max_seq_size,
                        query_operation_ids: query_ops,
                        selected_query_operation: None,
                    },
                );

                process_exploration_events(&mut app_state, rx);
                handle.join().expect("Exploration thread panicked");
                //println!("Got {} results", app_state.results.len());

                info!(
                    "Exploration finished with {} examples",
                    app_state.results.len()
                );
                info!(
                    "Exploration time: {:?}",
                    app_state.end_time.unwrap() - app_state.start_time.unwrap()
                );
                info!("Coverage report:");
                let coverage = report::exploration_coverage(
                    &app_state.results,
                    &amos_translation.amos.operations,
                );
                info!("{:#?}", coverage);

                info!("Write results..");

                match report::write_results_for_test(
                    "out",
                    &explore_behaviour,
                    &amos_translation.amos,
                    &app_state.results,
                ) {
                    Ok(_) => (),
                    Err(e) => {
                        println!("Failed to write result: {}", e);
                        std::process::exit(1)
                    }
                }
            }
        }
    }
}

fn process_exploration_events(
    app_state: &mut AppState,
    rx: std::sync::mpsc::Receiver<explore::Event>,
) {
    while let Ok(event) = rx.recv() {
        //info!("{}", format!("Received event {:?}", event));
        match event {
            explore::Event::Log { .. } => {
                //println!("{:?}", message)
            }

            explore::Event::Result { result } => app_state.results.push(result),
            explore::Event::Control { event } => {
                match event {
                    explore::ControlEvent::Finished => {
                        info!("Finished");
                        break;
                    }
                    // We do not want to react to started, since the UI already
                    //  know that we started (button clicked) and can react to that
                    explore::ControlEvent::Started => {
                        info!("Started")
                    }
                }
            }
            explore::Event::TimeLineStart { enter, .. } => {
                println!("Timeline start");
                app_state.start_time = Some(enter)},
            explore::Event::TimeLineEnd { time, .. } => app_state.end_time = Some(time),
            explore::Event::CurrentQueryOperation { operation } => app_state.current_root_operation = operation,
            explore::Event::InvocationSpanEnter { .. } => {
                // info!("Entering invocation")
            }
            explore::Event::InvocationSpanExit { duration } => {
                // info!("Exit invocation");
                // TODO: consolidate with UI
                let start = app_state.current_invocation_span_start;

                if start != app_state.invocation_results.len() {
                    let end = start + (app_state.invocation_results.len() - 1 - start);
                    app_state.current_invocation_span_start = app_state.invocation_results.len();
                    //debug!(start, end, "Span");
                    app_state.invocation_spans.push(InvocationSpan {
                        query_operation: app_state.current_root_operation.clone(),
                        start,
                        end,
                        duration,
                    })
                }
            }
            qr_explore::explore::Event::Invocation {
                result,
                ..//sut_invocation_duration,
            } => {
                //info!("invocation result {:#?}", result.operation);
                app_state.invocation_results.push(result);
            }
            _ => (),
        };
    }
}

fn handle_parse_result(
    result: spec_mgr::Result<(open_api::ParseResult, amos::TranslationResult)>,
) -> amos::TranslationResult {
    match result {
        Ok((parse_result, translation_result)) => {
            println!("{}", "Successfully parsed OpenAPI file".green());
            if !parse_result.warnings.is_empty() {
                println!("{}", "Warnings:".yellow());
                for (n, warning) in parse_result.warnings.iter().enumerate() {
                    println!("{} - {}", n + 1, warning.message.yellow());
                }
            }
            translation_result
        }
        Err(e) => {
            let err_str = match e {
                spec_mgr::Error::OpenAPIParseFailed(e) => match e {
                    qr_open_api::Error::OpenAPIReadFileFailure(error) => {
                        format!("Could not read file: {}", error)
                    }

                    qr_open_api::Error::OpenAPIParseFailure(msg) => {
                        format!("Could not parse file: {}", msg)
                    }
                },
                spec_mgr::Error::OpenAPIFetchFailed(e) => {
                    format!("Could not fetch Open API specification: {}", e)
                }
            };
            println!(
                "{} - {}",
                "Parsing Failed Fatally!".red().bold(),
                err_str.red()
            );

            std::process::exit(1);
        }
    }
}
