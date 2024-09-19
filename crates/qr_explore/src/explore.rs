use crate::amos::{InvokeResult, Operation, OperationMetaData};
use crate::amos_buckets::Buckets;
use crate::amos_generation::{self, gen_buckets_5, gen_static_operation_with_params, QueryOptions};
use crate::amos_generation::{
    gen_banana_cake_value, gen_pinned_operation_sequence_with_params, GeneratedOperation,
    GeneratedParameter, GenerationOperationWithParameters, ParameterValue,
};
use crate::amos_relations::Relation;
use crate::http_translation::{translate_operation, translate_parameters, HTTPCall};
use crate::meta_properties::{
    self, check_response_equality, check_response_inequality,
    check_state_identity_with_observation, check_state_mutation,
};
use crate::{amos, amos_buckets};
use proptest::strategy::{Strategy, ValueTree};
use proptest::test_runner::{Config, FileFailurePersistence, TestRunner};
use qr_http_resource::http::{self, HTTPMethod};
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info, span, trace, warn, Level};

#[derive(Debug, PartialEq)]
pub enum LogLevel {
    Info,
    Error,
    Warning,
}

#[derive(Debug, PartialEq)]
pub struct LogMessage {
    pub level: LogLevel,
    pub message: String,
}

#[derive(Debug, PartialEq)]
pub enum ControlEvent {
    Started,
    Finished,
}

#[derive(Debug, PartialEq)]
pub enum Event {
    /// Invocation related events - track progress and results of invoking operations
    InvocationSpanEnter {
        enter: std::time::Instant,
    },
    InvocationSpanExit {
        duration: std::time::Duration,
    },
    Invocation {
        result: InvokeResult,
        sut_invocation_duration: std::time::Duration,
    },

    CurrentQueryOperation {
        operation: String,
    },

    /// Exploration
    Result {
        result: ExplorationResult,
    },

    /// Performance and understanding
    TimeLineStart {
        enter: std::time::Instant,
        message: String,
    },

    TimeLineProgress {
        time: std::time::Instant,
        message: String,
    },

    TimeLineEnd {
        time: std::time::Instant,
        message: String,
    },

    /// General log messages
    Log {
        message: LogMessage,
    },

    /// Process control related - Start/Stop etc
    Control {
        event: ControlEvent,
    },
}

impl Event {
    pub fn log(level: LogLevel, message: &str) -> Self {
        Event::Log {
            message: LogMessage {
                level,
                message: message.to_owned(),
            },
        }
    }

    pub fn progress(message: String) -> Self {
        Event::TimeLineProgress {
            time: std::time::Instant::now(),
            message,
        }
    }
}

pub fn synthesize_operation(
    ops: &[(Operation, [ParameterValue; 10])],
    generated_op: (Operation, [ParameterValue; 10]),
) -> GeneratedOperation {
    let mut sparams = Vec::with_capacity(generated_op.0.parameters.len());

    for i in 0..generated_op.1.len() {
        debug!("Synthesize: {:#?}", generated_op.0.info.name);

        match &generated_op.1[i] {
            v @ ParameterValue::StringValue { .. }
            | v @ ParameterValue::BoolValue { .. }
            | v @ ParameterValue::DoubleValue { .. }
            | v @ ParameterValue::ArrayOfString { .. }
            | v @ ParameterValue::IntValue { .. }
            | v @ ParameterValue::File { .. }
            | v @ ParameterValue::IPV4Value { .. } => {
                // There is a chance that non-required parameters are dropped
                if !generated_op.0.parameters[i].required {
                    // TODO: this value should be configurable
                    // TODO: It's probably never a good idea to drop path values
                    if v.seed() > 5 {
                        //info!("Dropped: {:?}", generated_op.0.parameters[i].name);
                        continue;
                    }
                }

                sparams.push(GeneratedParameter {
                    name: generated_op.0.parameters[i].name.clone(),
                    value: v.clone(),
                    ref_path: None,
                })
            }
            v @ ParameterValue::Reference {
                active,
                idx: _,
                fallback,
                relation,
            } => {
                let param = if *active {
                    match relation {
                        Relation::Parameter(info) => {
                            let mut ref_info = info.clone();

                            loop {
                                // resolve the next step in the chain
                                let (resolved_op, resolved_params) = &ops[ref_info.op_idx];
                                let resolved_param = resolved_params[ref_info.idx].clone();
                                debug!("Resolved Reference: {:#?}", resolved_op);
                                debug!("Resolved Param: {:#?}", resolved_param);
                                // If the resolution is to a response ref, we are done,
                                //  the actual value is a runtime value
                                match resolved_param {
                                    ParameterValue::Reference { relation, .. } => match relation {
                                        Relation::Response(_info) => {
                                            debug!("PARAM->RSP Ref");
                                            break;
                                        }
                                        Relation::Parameter(info) => {
                                            debug!("Resolved to another Parameter reference");
                                            debug!("Followed reference: {:#?}", resolved_params);
                                            ref_info = info;
                                            continue;
                                        }
                                    },
                                    ParameterValue::Empty => panic!("Broken reference chain"),
                                    _ => {
                                        // We are done
                                        break;
                                    }
                                }
                            }

                            // collect the finally resolved info
                            let (resolved_op, resolved_params) = &ops[ref_info.op_idx];
                            let resolved_param = resolved_params[ref_info.idx].clone();

                            // an Empty value is always wrong at this point
                            assert!(resolved_param != ParameterValue::Empty);

                            GeneratedParameter {
                                // the name of the current parameter we are resolving
                                name: generated_op.0.parameters[i].name.clone(),
                                value: resolved_param,
                                ref_path: Some(format!(
                                    "REFERENCE - Active - {}[{}]/{}",
                                    resolved_op.info.name, ref_info.op_idx, ref_info.name,
                                )),
                            }
                        }
                        Relation::Response(info) => {
                            debug!("Response reference: {i}/{:#?}", info);
                            debug!("Response reference value: {:#?}", v);

                            GeneratedParameter {
                                // the name of the current parameter we are resolving
                                name: generated_op.0.parameters[i].name.clone(),
                                value: v.clone(),
                                ref_path: Some(format!(
                                    "RSP REFERENCE - Active - {}[{}]/{}",
                                    info.operation, info.op_idx, info.idx,
                                )),
                            }
                        }
                    }
                } else {
                    GeneratedParameter {
                        name: "REFERENCE - Inactive".to_string(),
                        value: *fallback.clone(),
                        ref_path: None,
                    }
                };
                sparams.push(param);
            }
            ParameterValue::Empty => {
                debug!("Empty parameter value: {i}");
                continue;
            }
        }
    }

    GeneratedOperation {
        name: generated_op.0.info.name,
        parameters: sparams,
    }
}

pub fn synthesize_operations(ops: &[(Operation, [ParameterValue; 10])]) -> Vec<GeneratedOperation> {
    ops.iter()
        .map(|op| synthesize_operation(ops, op.clone()))
        .collect()
}

pub fn synthesize_property_operations(
    _query_precedence: u8,
    ops: &[(Operation, [ParameterValue; 10])],
) -> Vec<GeneratedOperation> {
    synthesize_operations(ops)
}

pub fn synthesize_operations_for_response_equality(
    _query_precedence: u8,
    ops: &[(Operation, [ParameterValue; 10])],
) -> Vec<GeneratedOperation> {
    // The first generated operation is the OP that should be repeated
    vec![
        synthesize_operation(ops, ops[0].clone()),
        synthesize_operation(ops, ops[0].clone()),
    ]
}

pub fn synthesize_operations_for_response_inequality(
    query_precedence: u8,
    ops: &[(Operation, [ParameterValue; 10])],
) -> Vec<GeneratedOperation> {
    // The first generated operation is the OP that should be repeated
    synthesize_operations_for_response_equality(query_precedence, ops)
}

pub fn synthesize_operations_for_state_mutation(
    _query_precedence: u8,
    ops: &[(Operation, [ParameterValue; 10])],
) -> Vec<GeneratedOperation> {
    let mut synth_ops = vec![];
    synth_ops.extend(synthesize_operations(ops));

    // The first generated operation is the query OP and it should also be the last
    synth_ops.push(synthesize_operation(ops, ops[0].clone()));
    synth_ops
}

pub fn synthesize_operations_for_state_identity(
    query_precedence: u8,
    ops: &[(Operation, [ParameterValue; 10])],
) -> Vec<GeneratedOperation> {
    if ops.is_empty() {
        return vec![];
    }

    // The first generated operation is the query OP
    // TODO: consolidate the above info.. it is not very clear from this point
    //let mut synth_ops = vec![synthesize_operation(ops, query_operation.clone())];
    let mut synth_ops = vec![];

    let query_op = &ops[query_precedence as usize];

    // Push all up until query op
    for op in &ops[0..=query_precedence as usize] {
        //synth_ops.push(synthesize_operation(ops, ops[0].clone()));
        synth_ops.push(synthesize_operation(ops, op.clone()));
    }
    // Inject query ops
    for op in &ops[((query_precedence as usize) + 1)..ops.len()] {
        synth_ops.push(synthesize_operation(ops, op.clone()));
        synth_ops.push(synthesize_operation(ops, query_op.clone()));
    }

    //synth_ops.push(synthesize_operation(ops, query_op.clone()));

    debug!("Synthesized operations: {:#?}", synth_ops);
    synth_ops
}

type InvokeFn =
    fn(&ExplorationContext, &[Operation], &[GeneratedOperation]) -> Option<Vec<InvokeResult>>;

pub fn response_check(
    context: &ExplorationContext,
    operations: Vec<Operation>,
    explore_ops: Vec<Operation>,
    invoke: InvokeFn,
) -> Vec<ExplorationResult> {
    context.publish_event(Event::log(
        LogLevel::Info,
        "Start exploring 'Response Check'",
    ));

    // Start up events
    context.publish_event(Event::TimeLineStart {
        enter: std::time::Instant::now(),
        message: "Start exploring 'Response Check'".to_string(),
    });

    context.publish_event(Event::Control {
        event: ControlEvent::Started,
    });

    // for each operation included
    let result = explore_ops
        .iter()
        .map(|op| {
            context.publish_event(Event::TimeLineProgress {
                time: std::time::Instant::now(),
                message: format!("Starting Exploration with operation: {}", op.info.name),
            });
            context.publish_event(Event::log(
                LogLevel::Info,
                &format!("Starting Exploration with operation: {}", op.info.name),
            ));

            let result_seq = explore(
                context,
                operations.clone(),
                invoke,
                gen_static_operation_with_params(op.clone()),
                |_ctx, res| res,
                meta_properties::check_response,
                synthesize_property_operations,
            );

            let result = if let Some(minimal_seq) = result_seq {
                ExplorationResult::ResponseCheck {
                    operation: op.info.name.clone(),
                    minimal_sequence: Some(minimal_seq),
                }
            } else {
                ExplorationResult::NoExampleFound {
                    operation: op.info.name.clone(),
                }
            };

            context.publish_event(Event::Result {
                result: result.clone(),
            });

            result
        })
        .collect::<Vec<ExplorationResult>>();
    // generate op with param
    // invoke
    // check response

    // Finish up events
    context.publish_event(Event::TimeLineEnd {
        time: std::time::Instant::now(),
        message: "Completed Exploration".to_string(),
    });

    context.publish_event(Event::Control {
        event: ControlEvent::Finished,
    });

    result
}

pub fn explore_response_inequality(
    context: &ExplorationContext,
    operations: Vec<Operation>,
    explore_ops: Vec<Operation>,
    invoke: InvokeFn,
) -> Vec<ExplorationResult> {
    context.publish_event(Event::log(
        LogLevel::Info,
        "Start exploring 'Response Equality'",
    ));

    // Start up events
    context.publish_event(Event::TimeLineStart {
        enter: std::time::Instant::now(),
        message: "Start exploring 'Response Equality'".to_string(),
    });

    context.publish_event(Event::Control {
        event: ControlEvent::Started,
    });

    // Exploration

    let result = explore_ops
        .iter()
        .map(|op| {
            context.publish_event(Event::TimeLineProgress {
                time: std::time::Instant::now(),
                message: format!("Starting Exploration with operation: {}", op.info.name),
            });
            context.publish_event(Event::log(
                LogLevel::Info,
                &format!("Starting Exploration with operation: {}", op.info.name),
            ));

            let result_seq = explore(
                context,
                // TODO: is this right for the behaviour?
                //  - Well, think this belongs better in the context
                operations.clone(),
                invoke,
                gen_static_operation_with_params(op.clone()),
                |_ctx, res| res,
                check_response_inequality,
                synthesize_operations_for_response_inequality,
            );

            let result = if let Some(minimal_seq) = result_seq {
                ExplorationResult::ResponseInEquality {
                    operation: op.info.name.clone(),
                    minimal_sequence: Some(minimal_seq),
                }
            } else {
                ExplorationResult::NoExampleFound {
                    operation: op.info.name.clone(),
                }
            };

            context.publish_event(Event::Result {
                result: result.clone(),
            });

            result
        })
        .collect::<Vec<ExplorationResult>>();

    // Finish up events
    context.publish_event(Event::TimeLineEnd {
        time: std::time::Instant::now(),
        message: "Completed Exploration".to_string(),
    });

    context.publish_event(Event::Control {
        event: ControlEvent::Finished,
    });

    result
}

pub fn explore_response_equality(
    context: &ExplorationContext,
    operations: Vec<Operation>,
    explore_ops: Vec<Operation>,
    invoke: InvokeFn,
) -> Vec<ExplorationResult> {
    context.publish_event(Event::log(
        LogLevel::Info,
        "Start exploring 'Response Equality'",
    ));

    // Start up events
    context.publish_event(Event::TimeLineStart {
        enter: std::time::Instant::now(),
        message: "Start exploring 'Response Equality'".to_string(),
    });

    context.publish_event(Event::Control {
        event: ControlEvent::Started,
    });

    // Exploration

    let result = explore_ops
        .iter()
        .map(|op| {
            context.publish_event(Event::TimeLineProgress {
                time: std::time::Instant::now(),
                message: format!("Starting Exploration with operation: {}", op.info.name),
            });
            context.publish_event(Event::log(
                LogLevel::Info,
                &format!("Starting Exploration with operation: {}", op.info.name),
            ));

            let result_seq = explore(
                context,
                // TODO: is this right for the behaviour?
                //  - Well, think this belongs better in the context
                operations.clone(),
                invoke,
                gen_static_operation_with_params(op.clone()),
                |_ctx, res| res,
                check_response_equality,
                synthesize_operations_for_response_equality,
            );

            let result = if let Some(minimal_seq) = result_seq {
                ExplorationResult::ResponseEquality {
                    operation: op.info.name.clone(),
                    minimal_sequence: Some(minimal_seq),
                }
            } else {
                ExplorationResult::NoExampleFound {
                    operation: op.info.name.clone(),
                }
            };

            context.publish_event(Event::Result {
                result: result.clone(),
            });

            result
        })
        .collect::<Vec<ExplorationResult>>();

    // Finish up events
    context.publish_event(Event::TimeLineEnd {
        time: std::time::Instant::now(),
        message: "Completed Exploration".to_string(),
    });

    context.publish_event(Event::Control {
        event: ControlEvent::Finished,
    });

    result
}

pub fn explore_state_mutation(
    context: &mut ExplorationContext,
    operations: Vec<Operation>,
    query_ops: &[Operation],
    invoke: InvokeFn,
) -> Vec<ExplorationResult> {
    context.publish_event(Event::log(
        LogLevel::Info,
        "Start exploring 'State Mutation'",
    ));

    context.publish_event(Event::TimeLineStart {
        enter: std::time::Instant::now(),
        message: "Start exploring 'State Mutation'".to_string(),
    });

    // For this behaviour only state accreating operations are relevant.
    let valid_ops = operations
        .clone()
        .into_iter()
        .filter(|op| {
            if let Some(OperationMetaData::HTTP { method, .. }) = &op.meta_data {
                matches!(method, HTTPMethod::POST | HTTPMethod::PUT)
            } else {
                // If the operation do not have HTTP MetaData, allow it as we
                //  don't know if it can alter state or not
                true
            }
        })
        .collect::<Vec<Operation>>();

    context.publish_event(Event::Control {
        event: ControlEvent::Started,
    });

    let result = query_ops
        .iter()
        .map(|query_op| {
            context.publish_event(Event::TimeLineProgress {
                time: std::time::Instant::now(),
                message: format!(
                    "Starting Exploration with query operation: {}",
                    query_op.info.name
                ),
            });
            context.publish_event(Event::log(
                LogLevel::Info,
                &format!(
                    "Starting Exploration with query operation: {}",
                    query_op.info.name
                ),
            ));
            context.query_operation = Some(query_op.clone());
            let result_seq = explore(
                context,
                operations.clone(),
                invoke,
                gen_pinned_operation_sequence_with_params(
                    query_op.clone(),
                    valid_ops.clone(),
                    context.min_length,
                    context.max_length,
                ),
                |ctx, invoke_result| {
                    invoke_result.map(|r| {
                        r.into_iter()
                            .filter(|res| {
                                res.operation.name
                                    == ctx.query_operation.as_ref().unwrap().info.name
                            })
                            .collect::<Vec<InvokeResult>>()
                    })
                },
                check_state_mutation,
                synthesize_operations_for_state_mutation,
            );

            let result = if let Some(minimal_seq) = result_seq {
                ExplorationResult::StateMutation {
                    query_operation: query_op.info.name.clone(),
                    minimal_sequence: Some(minimal_seq),
                }
            } else {
                ExplorationResult::NoExampleFound {
                    operation: query_op.info.name.clone(),
                }
            };

            context.publish_event(Event::Result {
                result: result.clone(),
            });

            result
        })
        .collect::<Vec<ExplorationResult>>();

    context.publish_event(Event::TimeLineEnd {
        time: std::time::Instant::now(),
        message: "Completed Exploration".to_string(),
    });

    context.publish_event(Event::Control {
        event: ControlEvent::Finished,
    });

    result
}

pub fn explore_state_identity(
    context: &mut ExplorationContext,
    operations: Vec<Operation>,
    query_ops: &[Operation],
    invoke: InvokeFn,
) -> Vec<ExplorationResult> {
    context.publish_event(Event::log(
        LogLevel::Info,
        "Start exploring 'State Identity'",
    ));

    context.publish_event(Event::TimeLineStart {
        enter: std::time::Instant::now(),
        message: "Start exploring 'State Identity'".to_string(),
    });

    context.publish_event(Event::Control {
        event: ControlEvent::Started,
    });

    // TODO: put this in context an only do it once
    let buckets = Buckets::new(&operations);

    let result = query_ops
        .iter()
        .map(|query_op| {
            context.publish_event(Event::TimeLineProgress {
                time: std::time::Instant::now(),
                message: format!(
                    "Starting Exploration with query operation: {}",
                    query_op.info.name
                ),
            });
            context.publish_event(Event::log(
                LogLevel::Info,
                &format!(
                    "Starting Exploration with query operation: {}",
                    query_op.info.name
                ),
            ));

            context.query_operation = Some(query_op.clone());

            // what precedence do the query op have?
            // TODO: how certain are we that the op is in there?
            let query_bucket = buckets.find(query_op).unwrap();

            ////
            // Make buckets based on the suggested len
            let suggested_seq_len = query_bucket.precedence + 2;
            let query_precedence = query_bucket.precedence;
            info!(
                "Query Precedence: {query_precedence}, Suggested sequence len: {suggested_seq_len}"
            );

            let query_options = QueryOptions {
                precedence: query_bucket.precedence,
                slack_min: 0,
                slack_max: 2,
            };

            // TODO: Consolidate with buckets
            // For this behaviour only state accreating operations are relevant.
            let valid_ops = operations
                .clone()
                .into_iter()
                .filter(|op| {
                    if let Some(OperationMetaData::HTTP { method, .. }) = &op.meta_data {
                        matches!(method, HTTPMethod::POST | HTTPMethod::DELETE)
                    } else {
                        // If the operation do not have HTTP MetaData, allow it as we
                        //  don't know if it can alter state or not
                        true
                    }
                })
                .collect::<Vec<Operation>>();

            let result_seq = explore(
                context,
                operations.clone(),
                invoke,
                gen_pinned_operation_sequence_with_params(
                    query_op.clone(),
                    valid_ops.clone(),
                    context.min_length,
                    context.max_length,
                ),
                // gen_buckets_5(
                //     // State Identity want the Query OP earlier
                //     query_op.clone(),
                //     query_options,
                //     buckets.clone(),
                //     amos_buckets::bucketize_for_state_identity_strategy,
                //     //amos_buckets::bucketize_for_state_identity_update_strategy,
                //     operations.clone(),
                // ),
                |ctx, invoke_result| {
                    invoke_result.map(|r| {
                        r.into_iter()
                            .filter(|res| {
                                res.operation.name
                                    == ctx.query_operation.as_ref().unwrap().info.name
                            })
                            .collect::<Vec<InvokeResult>>()
                    })
                },
                check_state_identity_with_observation,
                synthesize_operations_for_state_identity,
            );

            let result = if let Some(minimal_seq) = result_seq {
                ExplorationResult::StateIdentity {
                    query_operation: query_op.info.name.clone(),
                    minimal_sequence: Some(minimal_seq),
                }
            } else {
                ExplorationResult::NoExampleFound {
                    operation: query_op.info.name.clone(),
                }
            };

            context.publish_event(Event::Result {
                result: result.clone(),
            });

            result
        })
        .collect::<Vec<ExplorationResult>>();

    context.publish_event(Event::TimeLineEnd {
        time: std::time::Instant::now(),
        message: "Completed Exploration".to_string(),
    });

    context.publish_event(Event::Control {
        event: ControlEvent::Finished,
    });

    result
}

#[derive(Debug, Clone)]
pub struct HTTPConfiguration {
    pub base_url: String,
    pub port: u16,
    pub protocol: http::Protocol,
}

impl HTTPConfiguration {
    pub fn new(base_url: String, port: u16, protocol: http::Protocol) -> Self {
        Self {
            base_url,
            port,
            protocol,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Target {
    HTTP { config: HTTPConfiguration },
}

pub struct ExplorationContext {
    pub http_client: reqwest::blocking::Client,
    // TODO: clean up the inputs
    pub http_send_fn:
        fn(&ExplorationContext, HTTPCall, &GeneratedOperation, String) -> Option<InvokeResult>,

    pub target: Target,

    // TODO: Index into the ops?
    pub query_operation: Option<Operation>,

    pub number_of_tests: u16,

    pub tx: Option<std::sync::mpsc::Sender<Event>>,

    pub min_length: u8,
    pub max_length: u8,
}

impl ExplorationContext {
    pub fn publish_event(&self, event: Event) {
        if let Some(tx) = &self.tx {
            tx.send(event).unwrap();
        };
    }
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub enum ExplorationResult {
    NoExampleFound {
        operation: String,
    },
    StateMutation {
        query_operation: String,
        minimal_sequence: Option<Vec<GeneratedOperation>>,
    },
    StateIdentity {
        query_operation: String,
        minimal_sequence: Option<Vec<GeneratedOperation>>,
    },
    ResponseEquality {
        operation: String,
        minimal_sequence: Option<Vec<GeneratedOperation>>,
    },
    ResponseInEquality {
        operation: String,
        minimal_sequence: Option<Vec<GeneratedOperation>>,
    },
    ResponseCheck {
        operation: String,
        minimal_sequence: Option<Vec<GeneratedOperation>>,
    },
}

// EXPERIMENT: make a path UI -> gen
pub fn banana_cakes_generation(expr: &str) -> amos_generation::ParameterValue {
    let mut runner = TestRunner::new(Config {
        failure_persistence: Some(Box::new(FileFailurePersistence::Off)),
        ..Config::default()
    });

    let gen = gen_banana_cake_value(expr.to_string())
        .new_tree(&mut runner)
        .unwrap();

    gen.current()
}

pub fn sequence_invoke(
    ctx: &ExplorationContext,
    operations: Vec<Operation>,
    invoke: InvokeFn,
    operations_to_invoke: Vec<Operation>,
) -> Option<Vec<InvokeResult>> {
    ctx.publish_event(Event::log(LogLevel::Info, "Start sequence invocation"));

    ctx.publish_event(Event::TimeLineStart {
        enter: std::time::Instant::now(),
        message: "Start invocation".to_string(),
    });

    let mut runner = TestRunner::new(Config {
        failure_persistence: Some(Box::new(FileFailurePersistence::Off)),
        ..Config::default()
    });

    let mut syn_ops = vec![];
    for op in operations_to_invoke {
        let gen = amos_generation::gen_param_array(&op.parameters)
            .new_tree(&mut runner)
            .unwrap();

        let v = gen.current();

        // NOTE: if references should be supported, we need to potential seq of
        //  operations to reference
        syn_ops.push(synthesize_operation(&[], (op.clone(), v)));
    }

    let results = invoke(ctx, &operations, &syn_ops);

    ctx.publish_event(Event::Control {
        event: ControlEvent::Finished,
    });

    ctx.publish_event(Event::TimeLineEnd {
        time: std::time::Instant::now(),
        message: "Completed invocation".to_string(),
    });

    results
}

/////
type ProcessResultFn =
    fn(&ExplorationContext, Option<Vec<InvokeResult>>) -> Option<Vec<InvokeResult>>;

pub fn explore(
    ctx: &ExplorationContext,
    operations: Vec<Operation>,
    invoke: InvokeFn,
    generator: impl Strategy<Value = (u8, Vec<GenerationOperationWithParameters>)>,
    process_result: ProcessResultFn,
    check: fn(&[InvokeResult]) -> bool,
    synthesize_operations: fn(u8, &[GenerationOperationWithParameters]) -> Vec<GeneratedOperation>,
) -> Option<Vec<GeneratedOperation>> {
    // TODO: put this in the context, no reason to re-creating it
    let mut runner = TestRunner::new(Config {
        failure_persistence: Some(Box::new(FileFailurePersistence::Off)),
        ..Config::default()
    });

    for test_n in 0..ctx.number_of_tests {
        ctx.publish_event(Event::progress(format!("Start test {}", test_n,)));

        // Invoke the generator to get a new generated sequence
        let mut val = generator.new_tree(&mut runner).unwrap();

        // Synthesize to operations, Gen.ops -> Actual ops.
        ctx.publish_event(Event::progress(format!(
            "Generating operations for test {} starting..",
            test_n,
        )));
        let (query_pos, op_seq) = &val.current();
        let gen_ops = synthesize_operations(*query_pos, op_seq);
        ctx.publish_event(Event::progress(format!(
            "Generating operations for test {} done",
            test_n,
        )));

        // Invoke the operations
        ctx.publish_event(Event::progress(format!(
            "Invoke of operations for test {} starting..",
            test_n,
        )));
        let invoke_result = invoke(ctx, &operations, &gen_ops);
        ctx.publish_event(Event::progress(format!(
            "Invoke of operations for test {} done",
            test_n,
        )));

        // Let the behaviour process the result before the check
        let potential_query_results = process_result(ctx, invoke_result);

        // Check if we could produce a result
        let Some(query_results) = potential_query_results else {
            continue;
        };

        // Check if the result fit the behaviour or not
        if check(&query_results) {
            // Test passed
            continue;
        }

        // FAILED check, i.e, we match the behaviour
        ctx.publish_event(Event::progress(
            "Found failing sequence, start Shrinking".to_string(),
        ));

        // NOTE: Dealing with state-ful systems we should not use the same
        //  value twice, hence do NOT use val.current, simplify first
        let mut shrink_count = 0;
        val.simplify();
        loop {
            shrink_count += 1;
            // Run the simplified sequence
            let (query_pos, op_seq) = &val.current();
            let gen_ops = synthesize_operations(*query_pos, op_seq);
            let invoke_result = invoke(ctx, &operations, &gen_ops);
            // Let the behaviour process the result before the check
            let potential_query_results = process_result(ctx, invoke_result);

            // Check if we could produce a result
            let Some(query_results) = potential_query_results else {
                val.complicate();
                continue;
            };

            // Check the simplified result
            if !check(&query_results) {
                // Still failing, find a simpler example if we can
                ctx.publish_event(Event::progress(format!(
                    "Simpler sequence failed, keep Shrinking - {shrink_count}",
                )));
                if !val.simplify() {
                    break;
                }
            } else if !val.complicate() {
                // Passed this input, back up
                break;
            };
        }

        // Shrinking is done, take the smallest sequence and make it into actual ops
        ctx.publish_event(Event::progress("Shrinking done".to_string()));
        let (query_pos, op_seq) = &val.current();
        let minimal_ops = synthesize_operations(*query_pos, op_seq);

        return Some(minimal_ops);
    }

    // Didn't find any example
    None
}

fn build_reqwest_request(
    ctx: &ExplorationContext,
    http_operation: &HTTPCall,
) -> reqwest::blocking::RequestBuilder {
    let con_method = match http_operation.method {
        HTTPMethod::GET => reqwest::Method::GET,
        HTTPMethod::POST => reqwest::Method::POST,
        HTTPMethod::DELETE => reqwest::Method::DELETE,
        HTTPMethod::PUT => reqwest::Method::PUT,
        _ => todo!(),
    };
    let init_request = ctx
        .http_client
        .request(con_method, http_operation.url.clone());

    let request_with_form_data = if let Some(payload) = &http_operation.parameters.form_data {
        init_request.form(&payload)
    } else {
        init_request
    };

    // With form-data present and no other evidence, we default to
    // 'application/x-www-form-urlencoded'. 'multipart/form-data' should
    // be used for binary data or data of 'significant' size. Thus, currently,
    // the operation need to state that specific mime-type in 'consumes', or
    // the type of the parameter is 'file'.
    let request_with_form_and_file = if let Some(file) = &http_operation.parameters.file_data {
        let file_form = reqwest::blocking::multipart::Form::new();
        let mut payload = reqwest::blocking::multipart::Part::bytes("Uninitialized".as_bytes());
        let mut param_name = "Uninitialized".to_string();
        // Currently, only support one file parameter
        for (k, v) in file {
            payload = reqwest::blocking::multipart::Part::bytes(v.as_bytes().to_owned())
                .file_name("foo.bar")
                .mime_str("application/octet-stream")
                .unwrap();
            param_name = k.to_string();
        }

        let final_form = file_form.part(param_name, payload);
        request_with_form_data.multipart(final_form)
    } else {
        request_with_form_data
    };

    if let Some(body) = &http_operation.parameters.body {
        // TODO: respect the operations "consumes" mime type
        request_with_form_and_file.json(&body)
    } else {
        request_with_form_and_file
    }
}

fn build_request(
    ctx: &ExplorationContext,
    ops: &[Operation],
    gen_op: &GeneratedOperation,
    results: &[InvokeResult],
) -> Option<(HTTPCall, String)> {
    // TODO: Fix this meta crap
    let matching_op = ops.iter().find(|op| op.info.name == gen_op.name);

    let amos_op = matching_op.unwrap();
    let op_meta = amos_op.meta_data.clone();

    let config = match &ctx.target {
        Target::HTTP { config } => config,
    };

    let http_operation = translate_operation(config, gen_op, &op_meta, amos_op, results)?;
    debug!(?http_operation);

    // TODO: This should not be nessesary
    let url = http_operation.url.clone();
    Some((http_operation, url))
}

fn process_response(
    ctx: &ExplorationContext,
    response: Result<reqwest::blocking::Response, reqwest::Error>,
    gen_op: &GeneratedOperation,
    url: String,
    request_duration: std::time::Duration,
) -> Option<InvokeResult> {
    match response {
        Err(e) => {
            error!("HTTP Invoke error: {}", e);
            None
        }
        Ok(r) => {
            debug!("Response: {:#?}", r);
            let status = r.status();
            let _server_error = &r.status().is_server_error();
            let success = &r.status().is_success();

            if let Ok(t) = &r.text() {
                let result = InvokeResult::new(
                    gen_op.clone(),
                    t.clone(),
                    //content,
                    *success,
                    Some(amos::ResultMetaData::HTTP {
                        url,
                        status: match status.as_u16() {
                            200 => http::HTTPStatus::OK,
                            201 => http::HTTPStatus::Created,
                            204 => http::HTTPStatus::NoContent,
                            400 => http::HTTPStatus::BadRequest,
                            401 => http::HTTPStatus::Unauthorized,
                            403 => http::HTTPStatus::Forbidden,
                            404 => http::HTTPStatus::NotFound,
                            405 => http::HTTPStatus::MethodNotAllowed,
                            415 => http::HTTPStatus::UnsupportedMediaType,
                            500 => http::HTTPStatus::InternalServerError,
                            _ => {
                                warn!("Unsupported status code: {}", status.as_u16());
                                http::HTTPStatus::Unsupported
                            }
                        },
                    }),
                );
                // TODO: Pull this out
                ctx.publish_event(Event::Invocation {
                    result: result.clone(),
                    sut_invocation_duration: request_duration,
                });
                return Some(result);
            };

            None
        }
    }
}

pub fn invoke_with_reqwest(
    ctx: &ExplorationContext,
    http_operation: HTTPCall,
    gen_op: &GeneratedOperation,
    url: String,
) -> Option<InvokeResult> {
    let request = build_reqwest_request(ctx, &http_operation);
    let request_start_time = std::time::Instant::now();
    let resp = request.send();
    let request_duration = request_start_time.elapsed();
    process_response(ctx, resp, gen_op, url, request_duration)
}

pub fn invoke_dry(
    ctx: &ExplorationContext,
    _http_operation: HTTPCall,
    gen_op: &GeneratedOperation,
    url: String,
) -> Option<InvokeResult> {
    let request_start_time = std::time::Instant::now();
    // This is what dry invoke simulate, so make up a result for now.
    // The result could possible be controlled from outside
    let result = InvokeResult::new(
        gen_op.clone(),
        "[\"Fake result\"]".to_string(),
        true,
        Some(amos::ResultMetaData::HTTP {
            url,
            status: http::HTTPStatus::OK,
        }),
    );

    let request_duration = request_start_time.elapsed();
    // TODO: remove this when it's pulled out from 'process_response' and into the main
    // invoke
    ctx.publish_event(Event::Invocation {
        result: result.clone(),
        sut_invocation_duration: request_duration,
    });

    Some(result)
}

pub fn invoke(
    ctx: &ExplorationContext,
    ops: &[Operation],
    gen_ops: &[GeneratedOperation],
) -> Option<Vec<InvokeResult>> {
    let span = span!(Level::TRACE, "HTTP invoke span");
    let _enter = span.enter();

    let span_start_time = std::time::Instant::now();
    ctx.publish_event(Event::InvocationSpanEnter {
        enter: span_start_time,
    });
    // TODO: package this with the span

    let mut results = Vec::with_capacity(gen_ops.len());

    for gen_op in gen_ops {
        debug!(operation_name = gen_op.name,);
        debug!("Invoke: {gen_op:#?}");

        let (final_request, url) = build_request(ctx, ops, gen_op, &results)?;
        trace!("{final_request:#?}");

        let resp = (ctx.http_send_fn)(ctx, final_request, gen_op, url);

        if let Some(invoke_result) = resp {
            results.push(invoke_result)
        }
    }

    debug!("Invocation span exit");
    ctx.publish_event(Event::InvocationSpanExit {
        duration: span_start_time.elapsed(),
    });
    Some(results)
}

#[cfg(test)]
mod tests {

    use std::thread;

    use qr_http_resource::http::{self, HTTPMethod, HTTPParameterTarget};

    use crate::{
        amos::{
            InvokeResult, Operation, OperationInfo, OperationMetaData, Parameter,
            ParameterMetaData, ParameterOwnership, Response, ResultMetaData, Schema,
        },
        amos_generation::{GeneratedOperation, GeneratedParameter, ParameterValue},
        explore as sut,
    };

    #[test]
    fn explore_response_inequality_no_example() {
        let target = sut::Target::HTTP {
            config: sut::HTTPConfiguration::new("foo".to_string(), 123, http::Protocol::HTTP),
        };

        let (exploration_log_tx, exploration_log_rx) = std::sync::mpsc::channel();

        let ctx = sut::ExplorationContext {
            http_client: reqwest::blocking::Client::new(),
            http_send_fn: sut::invoke_dry,
            target,
            query_operation: None,
            tx: Some(exploration_log_tx),
            number_of_tests: 1,
            min_length: 1,
            max_length: 1,
        };

        let op = Operation {
            info: OperationInfo {
                name: "get_persons".to_string(),
                key: "operation/get_persons".to_string(),
            },
            parameters: vec![],
            responses: vec![Response {
                name: "successful operation".to_string(),
                schema: Schema::ArrayOfRefItems("person".to_string()),
            }],
            meta_data: Some(OperationMetaData::HTTP {
                url: "/persons".to_string(),
                method: HTTPMethod::GET,
            }),
        };

        let operations = vec![op.clone()];
        let explore_ops = vec![op];

        let invoke =
            |ctx: &sut::ExplorationContext, _ops: &[Operation], gen_ops: &[GeneratedOperation]| {
                ctx.publish_event(sut::Event::Log {
                    message: sut::LogMessage {
                        level: sut::LogLevel::Info,
                        message: "Invoke".to_string(),
                    },
                });
                // Setup equal result, i.e., no example found for this exploration
                let result: Vec<InvokeResult> = vec![
                    InvokeResult::new(
                        gen_ops[0].clone(),
                        "[\"Fake result\"]".to_string(),
                        true,
                        Some(ResultMetaData::HTTP {
                            url: "url".to_string(),
                            status: http::HTTPStatus::OK,
                        }),
                    ),
                    InvokeResult::new(
                        gen_ops[0].clone(),
                        "[\"Fake result\"]".to_string(),
                        true,
                        Some(ResultMetaData::HTTP {
                            url: "url".to_string(),
                            status: http::HTTPStatus::OK,
                        }),
                    ),
                ];

                Some(result)
            };

        thread::spawn(move || {
            sut::explore_response_inequality(&ctx, operations, explore_ops, invoke);
        });

        let mut messages: Vec<sut::Event> = vec![];
        while let Ok(value) = exploration_log_rx.recv() {
            messages.push(value)
        }

        let result = messages.iter().find(|m| match m {
            sut::Event::Result { .. } => true,
            _ => false,
        });

        assert_eq!(
            Some(&sut::Event::Result {
                result: sut::ExplorationResult::NoExampleFound {
                    operation: "get_persons".to_string()
                },
            }),
            result
        );
    }

    #[test]
    fn explore_response_inequality_with_example() {
        let target = sut::Target::HTTP {
            config: sut::HTTPConfiguration::new("foo".to_string(), 123, http::Protocol::HTTP),
        };

        let (exploration_log_tx, exploration_log_rx) = std::sync::mpsc::channel();

        let ctx = sut::ExplorationContext {
            http_client: reqwest::blocking::Client::new(),
            http_send_fn: sut::invoke_dry,
            target,
            query_operation: None,
            tx: Some(exploration_log_tx),
            number_of_tests: 1,
            min_length: 1,
            max_length: 1,
        };

        let op = Operation {
            info: OperationInfo {
                name: "get_persons".to_string(),
                key: "operation/get_persons".to_string(),
            },
            parameters: vec![],
            responses: vec![Response {
                name: "successful operation".to_string(),
                schema: Schema::ArrayOfRefItems("person".to_string()),
            }],
            meta_data: Some(OperationMetaData::HTTP {
                url: "/persons".to_string(),
                method: HTTPMethod::GET,
            }),
        };

        let operations = vec![op.clone()];
        let explore_ops = vec![op];

        let invoke =
            |ctx: &sut::ExplorationContext, _ops: &[Operation], gen_ops: &[GeneratedOperation]| {
                ctx.publish_event(sut::Event::Log {
                    message: sut::LogMessage {
                        level: sut::LogLevel::Info,
                        message: "Invoke".to_string(),
                    },
                });
                // Setup unequal result, i.e., example found for this exploration
                let result: Vec<InvokeResult> = vec![
                    InvokeResult::new(
                        gen_ops[0].clone(),
                        "[\"Fake result\"]".to_string(),
                        true,
                        Some(ResultMetaData::HTTP {
                            url: "url".to_string(),
                            status: http::HTTPStatus::OK,
                        }),
                    ),
                    InvokeResult::new(
                        gen_ops[0].clone(),
                        "[\"Totally not Fake result\"]".to_string(),
                        true,
                        Some(ResultMetaData::HTTP {
                            url: "url".to_string(),
                            status: http::HTTPStatus::OK,
                        }),
                    ),
                ];

                Some(result)
            };

        thread::spawn(move || {
            sut::explore_response_inequality(&ctx, operations, explore_ops, invoke);
        });

        let mut messages: Vec<sut::Event> = vec![];
        while let Ok(value) = exploration_log_rx.recv() {
            messages.push(value)
        }

        let result = messages.iter().find(|m| match m {
            sut::Event::Result { .. } => true,
            _ => false,
        });

        assert_eq!(
            Some(&sut::Event::Result {
                result: sut::ExplorationResult::ResponseInEquality {
                    operation: "get_persons".to_string(),
                    minimal_sequence: Some(vec![
                        GeneratedOperation {
                            name: "get_persons".to_string(),
                            parameters: vec![]
                        },
                        GeneratedOperation {
                            name: "get_persons".to_string(),
                            parameters: vec![]
                        }
                    ])
                },
            }),
            result
        );
    }

    #[test]
    fn explore_response_equality_with_example() {
        let target = sut::Target::HTTP {
            config: sut::HTTPConfiguration::new("foo".to_string(), 123, http::Protocol::HTTP),
        };

        let (exploration_log_tx, exploration_log_rx) = std::sync::mpsc::channel();

        let ctx = sut::ExplorationContext {
            http_client: reqwest::blocking::Client::new(),
            http_send_fn: sut::invoke_dry,
            target,
            query_operation: None,
            tx: Some(exploration_log_tx),
            number_of_tests: 1,
            min_length: 1,
            max_length: 1,
        };

        let op = Operation {
            info: OperationInfo {
                name: "get_persons".to_string(),
                key: "operation/get_persons".to_string(),
            },
            parameters: vec![],
            responses: vec![Response {
                name: "successful operation".to_string(),
                schema: Schema::ArrayOfRefItems("person".to_string()),
            }],
            meta_data: Some(OperationMetaData::HTTP {
                url: "/persons".to_string(),
                method: HTTPMethod::GET,
            }),
        };

        let operations = vec![op.clone()];
        let explore_ops = vec![op];

        let invoke =
            |ctx: &sut::ExplorationContext, _ops: &[Operation], gen_ops: &[GeneratedOperation]| {
                ctx.publish_event(sut::Event::Log {
                    message: sut::LogMessage {
                        level: sut::LogLevel::Info,
                        message: "Invoke".to_string(),
                    },
                });

                let result: Vec<InvokeResult> = vec![
                    InvokeResult::new(
                        gen_ops[0].clone(),
                        "[\"Fake result\"]".to_string(),
                        true,
                        Some(ResultMetaData::HTTP {
                            url: "url".to_string(),
                            status: http::HTTPStatus::OK,
                        }),
                    ),
                    InvokeResult::new(
                        gen_ops[0].clone(),
                        "[\"Fake result\"]".to_string(),
                        true,
                        Some(ResultMetaData::HTTP {
                            url: "url".to_string(),
                            status: http::HTTPStatus::OK,
                        }),
                    ),
                ];

                Some(result)
            };

        thread::spawn(move || {
            sut::explore_response_equality(&ctx, operations, explore_ops, invoke);
        });

        let mut messages: Vec<sut::Event> = vec![];
        while let Ok(value) = exploration_log_rx.recv() {
            messages.push(value)
        }

        let result = messages.iter().find(|m| match m {
            sut::Event::Result { .. } => true,
            _ => false,
        });

        assert_eq!(
            Some(&sut::Event::Result {
                result: sut::ExplorationResult::ResponseEquality {
                    operation: "get_persons".to_string(),
                    minimal_sequence: Some(vec![
                        GeneratedOperation {
                            name: "get_persons".to_string(),
                            parameters: vec![]
                        },
                        GeneratedOperation {
                            name: "get_persons".to_string(),
                            parameters: vec![]
                        }
                    ])
                },
            }),
            result
        );
    }

    #[test]
    fn explore_response_equality_no_example() {
        let target = sut::Target::HTTP {
            config: sut::HTTPConfiguration::new("foo".to_string(), 123, http::Protocol::HTTP),
        };

        let (exploration_log_tx, exploration_log_rx) = std::sync::mpsc::channel();

        let ctx = sut::ExplorationContext {
            http_client: reqwest::blocking::Client::new(),
            http_send_fn: sut::invoke_dry,
            target,
            query_operation: None,
            tx: Some(exploration_log_tx),
            number_of_tests: 1,
            min_length: 1,
            max_length: 1,
        };

        let op = Operation {
            info: OperationInfo {
                name: "get_persons".to_string(),
                key: "operation/get_persons".to_string(),
            },
            parameters: vec![],
            responses: vec![Response {
                name: "successful operation".to_string(),
                schema: Schema::ArrayOfRefItems("person".to_string()),
            }],
            meta_data: Some(OperationMetaData::HTTP {
                url: "/persons".to_string(),
                method: HTTPMethod::GET,
            }),
        };

        let operations = vec![op.clone()];
        let explore_ops = vec![op];

        let invoke =
            |ctx: &sut::ExplorationContext, _ops: &[Operation], gen_ops: &[GeneratedOperation]| {
                ctx.publish_event(sut::Event::Log {
                    message: sut::LogMessage {
                        level: sut::LogLevel::Info,
                        message: "Invoke".to_string(),
                    },
                });

                let result: Vec<InvokeResult> = vec![
                    InvokeResult::new(
                        gen_ops[0].clone(),
                        "[\"Fake result\"]".to_string(),
                        true,
                        Some(ResultMetaData::HTTP {
                            url: "url".to_string(),
                            status: http::HTTPStatus::OK,
                        }),
                    ),
                    InvokeResult::new(
                        gen_ops[0].clone(),
                        "[\"Fake rddddddesult\"]".to_string(),
                        true,
                        Some(ResultMetaData::HTTP {
                            url: "udddrl".to_string(),
                            status: http::HTTPStatus::OK,
                        }),
                    ),
                ];

                Some(result)
            };

        thread::spawn(move || {
            sut::explore_response_equality(&ctx, operations, explore_ops, invoke);
        });

        let mut messages: Vec<sut::Event> = vec![];
        while let Ok(value) = exploration_log_rx.recv() {
            messages.push(value)
        }

        let result = messages.iter().find(|m| match m {
            sut::Event::Result { .. } => true,
            _ => false,
        });

        assert_eq!(
            Some(&sut::Event::Result {
                result: sut::ExplorationResult::NoExampleFound {
                    operation: "get_persons".to_string()
                },
            }),
            result
        );
    }

    #[test]
    fn explore_state_mutation_with_example() {
        let target = sut::Target::HTTP {
            config: sut::HTTPConfiguration::new("foo".to_string(), 123, http::Protocol::HTTP),
        };

        let (exploration_log_tx, exploration_log_rx) = std::sync::mpsc::channel();

        let mut ctx = sut::ExplorationContext {
            http_client: reqwest::blocking::Client::new(),
            http_send_fn: sut::invoke_dry,
            target,
            query_operation: None,
            tx: Some(exploration_log_tx),
            number_of_tests: 1,
            min_length: 1,
            max_length: 1,
        };

        let post_op = Operation {
            info: OperationInfo {
                name: "post_person".to_string(),
                key: "operation/post_person".to_string(),
            },
            parameters: vec![
                Parameter {
                    name: "name".to_string(),
                    schema: Schema::String,
                    required: true,
                    ownership: ParameterOwnership::Owned,
                    meta_data: None,
                },
                Parameter {
                    name: "age".to_string(),
                    schema: Schema::Int8,
                    required: true,
                    ownership: ParameterOwnership::Owned,
                    meta_data: None,
                },
            ],
            responses: vec![Response {
                name: "successful operation".to_string(),
                schema: Schema::Reference("person".to_string()),
            }],
            meta_data: None,
        };

        let get_op = Operation {
            info: OperationInfo {
                name: "get_persons".to_string(),
                key: "operation/get_persons".to_string(),
            },
            parameters: vec![],
            responses: vec![Response {
                name: "successful operation".to_string(),
                schema: Schema::ArrayOfRefItems("person".to_string()),
            }],
            meta_data: Some(OperationMetaData::HTTP {
                url: "/persons".to_string(),
                method: HTTPMethod::GET,
            }),
        };

        let operations = vec![post_op.clone()];

        let invoke =
            |ctx: &sut::ExplorationContext, _ops: &[Operation], gen_ops: &[GeneratedOperation]| {
                ctx.publish_event(sut::Event::Log {
                    message: sut::LogMessage {
                        level: sut::LogLevel::Info,
                        message: "Invoke".to_string(),
                    },
                });

                let result: Vec<InvokeResult> = vec![
                    InvokeResult::new(
                        gen_ops[0].clone(),
                        "[\"Fake result\"]".to_string(),
                        true,
                        Some(ResultMetaData::HTTP {
                            url: "url".to_string(),
                            status: http::HTTPStatus::OK,
                        }),
                    ),
                    InvokeResult::new(
                        gen_ops[0].clone(),
                        "[\"Another Fake result\"]".to_string(),
                        true,
                        Some(ResultMetaData::HTTP {
                            url: "url".to_string(),
                            status: http::HTTPStatus::OK,
                        }),
                    ),
                ];

                Some(result)
            };

        let query_ops = vec![get_op.clone()];

        thread::spawn(move || {
            sut::explore_state_mutation(&mut ctx, operations, &query_ops, invoke);
        });

        let mut messages: Vec<sut::Event> = vec![];
        while let Ok(value) = exploration_log_rx.recv() {
            messages.push(value)
        }

        let result = messages.iter().find(|m| match m {
            sut::Event::Result { .. } => true,
            _ => false,
        });

        //assert_eq!(messages, vec![]);

        assert_eq!(
            Some(&sut::Event::Result {
                result: sut::ExplorationResult::StateMutation {
                    query_operation: "get_persons".to_string(),
                    minimal_sequence: Some(vec![
                        GeneratedOperation {
                            name: "get_persons".to_string(),
                            parameters: vec![]
                        },
                        GeneratedOperation {
                            name: "post_person".to_string(),
                            parameters: vec![
                                GeneratedParameter {
                                    name: "name".to_string(),
                                    value: ParameterValue::StringValue {
                                        value: "".to_string(),
                                        seed: 1,
                                        active: false
                                    },
                                    ref_path: None
                                },
                                GeneratedParameter {
                                    name: "age".to_string(),
                                    value: ParameterValue::IntValue {
                                        value: 0,
                                        seed: 1,
                                        active: false
                                    },
                                    ref_path: None
                                }
                            ]
                        },
                        GeneratedOperation {
                            name: "get_persons".to_string(),
                            parameters: vec![]
                        }
                    ])
                }
            }),
            result
        );
    }

    #[test]
    fn explore_state_mutation_no_example() {
        let target = sut::Target::HTTP {
            config: sut::HTTPConfiguration::new("foo".to_string(), 123, http::Protocol::HTTP),
        };

        let (exploration_log_tx, exploration_log_rx) = std::sync::mpsc::channel();

        let mut ctx = sut::ExplorationContext {
            http_client: reqwest::blocking::Client::new(),
            http_send_fn: sut::invoke_dry,
            target,
            query_operation: None,
            tx: Some(exploration_log_tx),
            number_of_tests: 1,
            min_length: 1,
            max_length: 1,
        };

        let post_op = Operation {
            info: OperationInfo {
                name: "post_person".to_string(),
                key: "operation/post_person".to_string(),
            },
            parameters: vec![
                Parameter {
                    name: "name".to_string(),
                    schema: Schema::String,
                    required: true,
                    ownership: ParameterOwnership::Owned,
                    meta_data: None,
                },
                Parameter {
                    name: "age".to_string(),
                    schema: Schema::Int8,
                    required: true,
                    ownership: ParameterOwnership::Owned,
                    meta_data: None,
                },
            ],
            responses: vec![Response {
                name: "successful operation".to_string(),
                schema: Schema::Reference("person".to_string()),
            }],
            meta_data: None,
        };

        let get_op = Operation {
            info: OperationInfo {
                name: "get_persons".to_string(),
                key: "operation/get_persons".to_string(),
            },
            parameters: vec![],
            responses: vec![Response {
                name: "successful operation".to_string(),
                schema: Schema::ArrayOfRefItems("person".to_string()),
            }],
            meta_data: Some(OperationMetaData::HTTP {
                url: "/persons".to_string(),
                method: HTTPMethod::GET,
            }),
        };

        let operations = vec![post_op.clone()];

        let invoke =
            |ctx: &sut::ExplorationContext, _ops: &[Operation], gen_ops: &[GeneratedOperation]| {
                ctx.publish_event(sut::Event::Log {
                    message: sut::LogMessage {
                        level: sut::LogLevel::Info,
                        message: "Invoke".to_string(),
                    },
                });

                let result: Vec<InvokeResult> = vec![
                    InvokeResult::new(
                        gen_ops[0].clone(),
                        "[\"Fake result\"]".to_string(),
                        true,
                        Some(ResultMetaData::HTTP {
                            url: "url".to_string(),
                            status: http::HTTPStatus::OK,
                        }),
                    ),
                    InvokeResult::new(
                        gen_ops[0].clone(),
                        "[\"Fake result\"]".to_string(),
                        true,
                        Some(ResultMetaData::HTTP {
                            url: "url".to_string(),
                            status: http::HTTPStatus::OK,
                        }),
                    ),
                ];

                Some(result)
            };

        let query_ops = vec![get_op.clone()];

        thread::spawn(move || {
            sut::explore_state_mutation(&mut ctx, operations, &query_ops, invoke);
        });

        let mut messages: Vec<sut::Event> = vec![];
        while let Ok(value) = exploration_log_rx.recv() {
            messages.push(value)
        }

        let result = messages.iter().find(|m| match m {
            sut::Event::Result { .. } => true,
            _ => false,
        });

        //assert_eq!(messages, vec![]);

        assert_eq!(
            Some(&sut::Event::Result {
                result: sut::ExplorationResult::NoExampleFound {
                    operation: "get_persons".to_string()
                },
            }),
            result
        );
    }

    #[test]
    fn explore_state_identity_with_example() {
        let target = sut::Target::HTTP {
            config: sut::HTTPConfiguration::new("foo".to_string(), 123, http::Protocol::HTTP),
        };

        let (exploration_log_tx, exploration_log_rx) = std::sync::mpsc::channel();

        let mut ctx = sut::ExplorationContext {
            http_client: reqwest::blocking::Client::new(),
            http_send_fn: sut::invoke_dry,
            target,
            query_operation: None,
            tx: Some(exploration_log_tx),
            number_of_tests: 1,
            min_length: 1,
            max_length: 1,
        };

        let post_op = Operation {
            info: OperationInfo {
                name: "post_person".to_string(),
                key: "operation/post_person".to_string(),
            },
            parameters: vec![
                Parameter {
                    name: "name".to_string(),
                    schema: Schema::String,
                    required: true,
                    ownership: ParameterOwnership::Owned,
                    meta_data: None,
                },
                Parameter {
                    name: "age".to_string(),
                    schema: Schema::Int8,
                    required: true,
                    ownership: ParameterOwnership::Owned,
                    meta_data: None,
                },
            ],
            responses: vec![Response {
                name: "successful operation".to_string(),
                schema: Schema::Reference("person".to_string()),
            }],
            meta_data: None,
        };

        let get_op = Operation {
            info: OperationInfo {
                name: "get_persons".to_string(),
                key: "operation/get_persons".to_string(),
            },
            parameters: vec![],
            responses: vec![Response {
                name: "successful operation".to_string(),
                schema: Schema::ArrayOfRefItems("person".to_string()),
            }],
            meta_data: Some(OperationMetaData::HTTP {
                url: "/persons".to_string(),
                method: HTTPMethod::GET,
            }),
        };

        let operations = vec![get_op.clone(), post_op.clone()];

        let invoke =
            |ctx: &sut::ExplorationContext, _ops: &[Operation], gen_ops: &[GeneratedOperation]| {
                ctx.publish_event(sut::Event::Log {
                    message: sut::LogMessage {
                        level: sut::LogLevel::Info,
                        message: "Invoke".to_string(),
                    },
                });

                let result: Vec<InvokeResult> = vec![
                    InvokeResult::new(
                        gen_ops[0].clone(),
                        "[\"Fake result\"]".to_string(),
                        true,
                        Some(ResultMetaData::HTTP {
                            url: "url".to_string(),
                            status: http::HTTPStatus::OK,
                        }),
                    ),
                    InvokeResult::new(
                        gen_ops[0].clone(),
                        "[\"Changed Fake result\"]".to_string(),
                        true,
                        Some(ResultMetaData::HTTP {
                            url: "url".to_string(),
                            status: http::HTTPStatus::OK,
                        }),
                    ),
                    InvokeResult::new(
                        gen_ops[0].clone(),
                        "[\"Fake result\"]".to_string(),
                        true,
                        Some(ResultMetaData::HTTP {
                            url: "url".to_string(),
                            status: http::HTTPStatus::OK,
                        }),
                    ),
                ];

                Some(result)
            };

        let query_ops = vec![get_op.clone()];

        thread::spawn(move || {
            sut::explore_state_identity(&mut ctx, operations, &query_ops, invoke);
        });

        let mut messages: Vec<sut::Event> = vec![];
        while let Ok(value) = exploration_log_rx.recv() {
            messages.push(value)
        }

        let result = messages.iter().find(|m| match m {
            sut::Event::Result { .. } => true,
            _ => false,
        });

        //assert_eq!(messages, vec![]);

        assert_eq!(
            Some(&sut::Event::Result {
                result: sut::ExplorationResult::StateIdentity {
                    query_operation: "get_persons".to_string(),
                    minimal_sequence: Some(vec![
                        GeneratedOperation {
                            name: "get_persons".to_string(),
                            parameters: vec![]
                        },
                        GeneratedOperation {
                            name: "post_person".to_string(),
                            parameters: vec![
                                GeneratedParameter {
                                    name: "name".to_string(),
                                    value: ParameterValue::StringValue {
                                        value: "".to_string(),
                                        seed: 1,
                                        active: false
                                    },
                                    ref_path: None
                                },
                                GeneratedParameter {
                                    name: "age".to_string(),
                                    value: ParameterValue::IntValue {
                                        value: 0,
                                        seed: 1,
                                        active: false
                                    },
                                    ref_path: None
                                }
                            ]
                        },
                        GeneratedOperation {
                            name: "get_persons".to_string(),
                            parameters: vec![]
                        }
                    ])
                }
            }),
            result
        );
    }

    #[test]
    fn explore_state_identity_no_example() {
        let target = sut::Target::HTTP {
            config: sut::HTTPConfiguration::new("foo".to_string(), 123, http::Protocol::HTTP),
        };

        let (exploration_log_tx, exploration_log_rx) = std::sync::mpsc::channel();

        let mut ctx = sut::ExplorationContext {
            http_client: reqwest::blocking::Client::new(),
            http_send_fn: sut::invoke_dry,
            target,
            query_operation: None,
            tx: Some(exploration_log_tx),
            number_of_tests: 1,
            min_length: 1,
            max_length: 1,
        };

        let post_op = Operation {
            info: OperationInfo {
                name: "post_person".to_string(),
                key: "operation/post_person".to_string(),
            },
            parameters: vec![
                Parameter {
                    name: "name".to_string(),
                    schema: Schema::String,
                    required: true,
                    ownership: ParameterOwnership::Owned,
                    meta_data: None,
                },
                Parameter {
                    name: "age".to_string(),
                    schema: Schema::Int8,
                    required: true,
                    ownership: ParameterOwnership::Owned,
                    meta_data: None,
                },
            ],
            responses: vec![Response {
                name: "successful operation".to_string(),
                schema: Schema::Reference("person".to_string()),
            }],
            meta_data: None,
        };

        let get_op = Operation {
            info: OperationInfo {
                name: "get_persons".to_string(),
                key: "operation/get_persons".to_string(),
            },
            parameters: vec![],
            responses: vec![Response {
                name: "successful operation".to_string(),
                schema: Schema::ArrayOfRefItems("person".to_string()),
            }],
            meta_data: Some(OperationMetaData::HTTP {
                url: "/persons".to_string(),
                method: HTTPMethod::GET,
            }),
        };

        let operations = vec![get_op.clone(), post_op.clone()];

        let invoke =
            |ctx: &sut::ExplorationContext, _ops: &[Operation], gen_ops: &[GeneratedOperation]| {
                ctx.publish_event(sut::Event::Log {
                    message: sut::LogMessage {
                        level: sut::LogLevel::Info,
                        message: "Invoke".to_string(),
                    },
                });

                let result: Vec<InvokeResult> = vec![
                    InvokeResult::new(
                        gen_ops[0].clone(),
                        "[\"Fake result\"]".to_string(),
                        true,
                        Some(ResultMetaData::HTTP {
                            url: "url".to_string(),
                            status: http::HTTPStatus::OK,
                        }),
                    ),
                    InvokeResult::new(
                        gen_ops[0].clone(),
                        "[\"Fake result\"]".to_string(),
                        true,
                        Some(ResultMetaData::HTTP {
                            url: "url".to_string(),
                            status: http::HTTPStatus::OK,
                        }),
                    ),
                    InvokeResult::new(
                        gen_ops[0].clone(),
                        "[\"Fake result\"]".to_string(),
                        true,
                        Some(ResultMetaData::HTTP {
                            url: "url".to_string(),
                            status: http::HTTPStatus::OK,
                        }),
                    ),
                ];

                Some(result)
            };

        let query_ops = vec![get_op.clone()];

        thread::spawn(move || {
            sut::explore_state_identity(&mut ctx, operations, &query_ops, invoke);
        });

        let mut messages: Vec<sut::Event> = vec![];
        while let Ok(value) = exploration_log_rx.recv() {
            messages.push(value)
        }

        let result = messages.iter().find(|m| match m {
            sut::Event::Result { .. } => true,
            _ => false,
        });

        //assert_eq!(messages, vec![]);

        assert_eq!(
            Some(&sut::Event::Result {
                result: sut::ExplorationResult::NoExampleFound {
                    operation: "get_persons".to_string()
                },
            }),
            result
        );
    }

    #[test]
    fn response_check_with_example() {
        let target = sut::Target::HTTP {
            config: sut::HTTPConfiguration::new("foo".to_string(), 123, http::Protocol::HTTP),
        };

        let (exploration_log_tx, exploration_log_rx) = std::sync::mpsc::channel();

        let ctx = sut::ExplorationContext {
            http_client: reqwest::blocking::Client::new(),
            http_send_fn: sut::invoke_dry,
            target,
            query_operation: None,
            tx: Some(exploration_log_tx),
            number_of_tests: 1,
            min_length: 1,
            max_length: 1,
        };

        let op = Operation {
            info: OperationInfo {
                name: "get_persons".to_string(),
                key: "operation/get_persons".to_string(),
            },
            parameters: vec![],
            responses: vec![Response {
                name: "successful operation".to_string(),
                schema: Schema::ArrayOfRefItems("person".to_string()),
            }],
            meta_data: Some(OperationMetaData::HTTP {
                url: "/persons".to_string(),
                method: HTTPMethod::GET,
            }),
        };

        let operations = vec![op.clone()];
        let explore_ops = vec![op];

        let invoke =
            |ctx: &sut::ExplorationContext, _ops: &[Operation], gen_ops: &[GeneratedOperation]| {
                ctx.publish_event(sut::Event::Log {
                    message: sut::LogMessage {
                        level: sut::LogLevel::Info,
                        message: "Invoke".to_string(),
                    },
                });

                let result: Vec<InvokeResult> = vec![InvokeResult::new(
                    gen_ops[0].clone(),
                    "[\"Fake result\"]".to_string(),
                    true,
                    Some(ResultMetaData::HTTP {
                        url: "url".to_string(),
                        status: http::HTTPStatus::InternalServerError,
                    }),
                )];

                Some(result)
            };

        thread::spawn(move || {
            sut::response_check(&ctx, operations, explore_ops, invoke);
        });

        let mut messages: Vec<sut::Event> = vec![];
        while let Ok(value) = exploration_log_rx.recv() {
            messages.push(value)
        }

        let result = messages.iter().find(|m| match m {
            sut::Event::Result { .. } => true,
            _ => false,
        });

        assert_eq!(
            Some(&sut::Event::Result {
                result: sut::ExplorationResult::ResponseCheck {
                    operation: "get_persons".to_string(),
                    minimal_sequence: Some(vec![GeneratedOperation {
                        name: "get_persons".to_string(),
                        parameters: vec![]
                    }])
                }
            }),
            result
        );
    }

    #[test]
    fn response_check_with_no_example() {
        let target = sut::Target::HTTP {
            config: sut::HTTPConfiguration::new("foo".to_string(), 123, http::Protocol::HTTP),
        };

        let (exploration_log_tx, exploration_log_rx) = std::sync::mpsc::channel();

        let ctx = sut::ExplorationContext {
            http_client: reqwest::blocking::Client::new(),
            http_send_fn: sut::invoke_dry,
            target,
            query_operation: None,
            tx: Some(exploration_log_tx),
            number_of_tests: 1,
            min_length: 1,
            max_length: 1,
        };

        let op = Operation {
            info: OperationInfo {
                name: "get_persons".to_string(),
                key: "operation/get_persons".to_string(),
            },
            parameters: vec![],
            responses: vec![Response {
                name: "successful operation".to_string(),
                schema: Schema::ArrayOfRefItems("person".to_string()),
            }],
            meta_data: Some(OperationMetaData::HTTP {
                url: "/persons".to_string(),
                method: HTTPMethod::GET,
            }),
        };

        let operations = vec![op.clone()];
        let explore_ops = vec![op];

        let invoke =
            |ctx: &sut::ExplorationContext, _ops: &[Operation], gen_ops: &[GeneratedOperation]| {
                ctx.publish_event(sut::Event::Log {
                    message: sut::LogMessage {
                        level: sut::LogLevel::Info,
                        message: "Invoke".to_string(),
                    },
                });

                let result: Vec<InvokeResult> = vec![InvokeResult::new(
                    gen_ops[0].clone(),
                    "[\"Fake result\"]".to_string(),
                    true,
                    Some(ResultMetaData::HTTP {
                        url: "url".to_string(),
                        status: http::HTTPStatus::OK,
                    }),
                )];

                Some(result)
            };

        thread::spawn(move || {
            sut::response_check(&ctx, operations, explore_ops, invoke);
        });

        let mut messages: Vec<sut::Event> = vec![];
        while let Ok(value) = exploration_log_rx.recv() {
            messages.push(value)
        }

        let result = messages.iter().find(|m| match m {
            sut::Event::Result { .. } => true,
            _ => false,
        });

        assert_eq!(
            Some(&sut::Event::Result {
                result: sut::ExplorationResult::NoExampleFound {
                    operation: "get_persons".to_string()
                },
            }),
            result
        );
    }
}
