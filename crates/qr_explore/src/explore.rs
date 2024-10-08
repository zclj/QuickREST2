use crate::amos::{InvokeResult, Operation, OperationMetaData};
use crate::amos_buckets::Buckets;
use crate::amos_generation::{
    gen_pinned_operation_sequence_with_params, GeneratedOperation,
    GenerationOperationWithParameters,
};
use crate::amos_generation::{gen_static_operation_with_params, QueryOptions};
use crate::http_translation::{translate_generated_operation_to_http_call, translate_http_result};
use crate::meta_properties::{
    self, check_response_equality, check_response_inequality,
    check_state_identity_with_observation, check_state_mutation,
};
use crate::synthesize::{
    synthesize_operations_for_response_equality, synthesize_operations_for_response_inequality,
    synthesize_operations_for_state_identity, synthesize_operations_for_state_mutation,
    synthesize_property_operations,
};
use proptest::strategy::{Strategy, ValueTree};
use proptest::test_runner::{Config, FileFailurePersistence, TestRunner};
use qr_http_resource::http::{self, HTTPCall, HTTPMethod};
use qr_http_resource::reqwest_http;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, span, trace, Level};

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

pub type InvokeFn =
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

            let _query_options = QueryOptions {
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
pub enum Target {
    HTTP { config: http::HTTPConfiguration },
}

pub struct ExplorationContext {
    pub http_client: reqwest::blocking::Client,
    pub http_send_fn: fn(&ExplorationContext, HTTPCall) -> Option<http::HTTPResult>,

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

pub fn invoke_with_reqwest(
    ctx: &ExplorationContext,
    http_operation: HTTPCall,
) -> Option<http::HTTPResult> {
    reqwest_http::invoke_with_reqwest(&ctx.http_client, http_operation)
}

pub fn invoke_dry(
    _ctx: &ExplorationContext,
    _http_operation: HTTPCall,
) -> Option<http::HTTPResult> {
    Some(http::HTTPResult {
        status: http::HTTPStatus::OK,
        payload: "[\"Fake result\"]".to_string(),
        success: true,
    })
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

    let mut results = Vec::with_capacity(gen_ops.len());

    let config = match &ctx.target {
        Target::HTTP { config } => config,
    };

    for gen_op in gen_ops {
        debug!(operation_name = gen_op.name,);
        debug!("Invoke: {gen_op:#?}");

        let (final_request, url) =
            translate_generated_operation_to_http_call(config, ops, gen_op, &results)?;
        trace!("{final_request:#?}");

        let request_start_time = std::time::Instant::now();
        let http_resp = (ctx.http_send_fn)(ctx, final_request);
        let request_duration = request_start_time.elapsed();

        if let Some(invoke_result) = http_resp {
            let resp = translate_http_result(invoke_result, gen_op, url);
            ctx.publish_event(Event::Invocation {
                result: resp.clone(),
                sut_invocation_duration: request_duration,
            });
            results.push(resp);
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

    use qr_http_resource::http::{self, HTTPMethod};

    use crate::{
        amos::{
            InvokeResult, Operation, OperationInfo, OperationMetaData, Parameter,
            ParameterOwnership, Response, ResultMetaData, Schema,
        },
        amos_generation::{GeneratedOperation, GeneratedParameter, ParameterValue},
        explore as sut,
    };

    #[test]
    fn invoke_with_dry() {
        let target = sut::Target::HTTP {
            config: http::HTTPConfiguration::new("foo".to_string(), 123, http::Protocol::HTTP),
        };

        let (exploration_log_tx, _exploration_log_rx) = std::sync::mpsc::channel();

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

        let generated = vec![
            GeneratedOperation {
                name: "get_persons".to_string(),
                parameters: vec![],
            },
            GeneratedOperation {
                name: "get_persons".to_string(),
                parameters: vec![],
            },
        ];

        let operations = vec![op.clone()];

        let invoke_result = sut::invoke(&ctx, &operations, &generated);

        assert_eq!(
            invoke_result,
            Some(vec![
                InvokeResult {
                    operation: GeneratedOperation {
                        name: "get_persons".to_string(),
                        parameters: vec![]
                    },
                    result: "[\"Fake result\"]".to_string(),
                    success: true,
                    meta_data: Some(ResultMetaData::HTTP {
                        url: "http://foo:123/persons".to_string(),
                        status: http::HTTPStatus::OK
                    })
                },
                InvokeResult {
                    operation: GeneratedOperation {
                        name: "get_persons".to_string(),
                        parameters: vec![]
                    },
                    result: "[\"Fake result\"]".to_string(),
                    success: true,
                    meta_data: Some(ResultMetaData::HTTP {
                        url: "http://foo:123/persons".to_string(),
                        status: http::HTTPStatus::OK
                    })
                }
            ])
        )
    }

    #[test]
    fn explore_response_inequality_no_example() {
        let target = sut::Target::HTTP {
            config: http::HTTPConfiguration::new("foo".to_string(), 123, http::Protocol::HTTP),
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
            config: http::HTTPConfiguration::new("foo".to_string(), 123, http::Protocol::HTTP),
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
            config: http::HTTPConfiguration::new("foo".to_string(), 123, http::Protocol::HTTP),
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
            config: http::HTTPConfiguration::new("foo".to_string(), 123, http::Protocol::HTTP),
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
            config: http::HTTPConfiguration::new("foo".to_string(), 123, http::Protocol::HTTP),
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
            config: http::HTTPConfiguration::new("foo".to_string(), 123, http::Protocol::HTTP),
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
            config: http::HTTPConfiguration::new("foo".to_string(), 123, http::Protocol::HTTP),
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
            config: http::HTTPConfiguration::new("foo".to_string(), 123, http::Protocol::HTTP),
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
            config: http::HTTPConfiguration::new("foo".to_string(), 123, http::Protocol::HTTP),
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
            config: http::HTTPConfiguration::new("foo".to_string(), 123, http::Protocol::HTTP),
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
