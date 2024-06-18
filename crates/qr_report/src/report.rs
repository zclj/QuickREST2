use std::collections::HashSet;
use std::fs;

use qr_explore::amos::OperationMetaData;
use qr_explore::amos::AMOS;
use qr_explore::amos_generation::GeneratedOperation;
use qr_explore::amos_generation::GeneratedParameter;
use qr_http_resource::http::HTTPMethod;
use serde::Deserialize;
use serde::Serialize;
use serde_json;

use crate::Result;

use qr_explore::amos;
use qr_explore::behaviours;
use qr_explore::explore;

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct ExplorationCoverage {
    covered: Vec<String>,
    uncovered: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Report {
    pub behaviour: behaviours::Behaviour,
    pub sequences: Vec<Sequence>,
    // To be self sufficient, for now, include the AMOS
    pub amos: AMOS,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Sequence {
    pub root_operation: String,
    pub operations: Vec<Operation>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Operation {
    pub name: String,
    pub url: String,
    pub method: HTTPMethod,
    pub parameters: Vec<GeneratedParameter>,
}

pub fn read_results_for_test(path: &str) -> Result<Report> {
    let raw_results = fs::read(path)?;

    let results: Report = serde_json::from_slice(&raw_results)?;

    Ok(results)
}

fn process_results(
    amos: &AMOS,
    minimal_sequence: &Option<Vec<GeneratedOperation>>,
) -> Vec<Operation> {
    let mut out_seq = vec![];

    if let Some(min_seq) = minimal_sequence {
        for op in min_seq {
            let mut current_url = "".to_string();
            let mut current_method = HTTPMethod::Unsupported;
            if let Some(amos_op) = amos.find_operation(&op.name) {
                if let Some(meta) = &amos_op.meta_data {
                    match meta {
                        OperationMetaData::HTTP { url, method } => {
                            current_url = url.clone();
                            current_method = method.clone();
                        }
                    }
                }
            }
            out_seq.push(Operation {
                url: current_url,
                method: current_method,
                name: op.name.clone(),
                parameters: op.parameters.clone(),
            })
        }
    }

    out_seq
}

pub fn write_results_for_test(
    dir_path: &str,
    behaviour: &behaviours::Behaviour,
    amos: &AMOS,
    results: &[explore::ExplorationResult],
) -> Result<()> {
    let behaviour_name = match behaviour {
        behaviours::Behaviour::Property => "fuzz",
        behaviours::Behaviour::StateMutation => "state-mutation",
        behaviours::Behaviour::StateIdentity => "state-identity",
        behaviours::Behaviour::ResponseEquality => "response-equality",
        behaviours::Behaviour::ResponseInequality => "response-inequality",
    };

    let mut sequences = vec![];

    for result in results {
        // TODO - does it make sense to mix results?
        let (root_operation, out_seq) = match result {
            explore::ExplorationResult::ResponseCheck {
                operation,
                minimal_sequence,
            } => (operation.clone(), process_results(amos, minimal_sequence)),
            explore::ExplorationResult::ResponseEquality {
                operation,
                minimal_sequence,
            } => (operation.clone(), process_results(amos, minimal_sequence)),
            explore::ExplorationResult::ResponseInEquality {
                operation,
                minimal_sequence,
            } => (operation.clone(), process_results(amos, minimal_sequence)),
            explore::ExplorationResult::StateMutation {
                query_operation,
                minimal_sequence,
            } => (
                query_operation.clone(),
                process_results(amos, minimal_sequence),
            ),
            explore::ExplorationResult::StateIdentity {
                query_operation,
                minimal_sequence,
            } => (
                query_operation.clone(),
                process_results(amos, minimal_sequence),
            ),
            explore::ExplorationResult::NoExampleFound { operation } => (operation.clone(), vec![]), //_ => todo!("TODO: {:?}", result),
        };

        sequences.push(Sequence {
            root_operation,
            operations: out_seq,
        })
    }

    let report = Report {
        sequences,
        behaviour: behaviour.clone(),
        amos: amos.clone(),
    };

    let file_path = format!("{dir_path}/{behaviour_name}.json");

    let json_result = serde_json::to_string_pretty(&report)?;

    // TODO: move the actual writing
    fs::create_dir_all(dir_path)?;

    fs::write(file_path, json_result.as_bytes())?;

    Ok(())
}

// TODO: results and aggregation wrt behaviour should be better
pub fn write_results(
    dir_path: &str,
    behaviour: &behaviours::Behaviour,
    results: &[explore::ExplorationResult],
) -> Result<()> {
    let json_result = serde_json::to_string_pretty(results)?;

    let behaviour_name = match behaviour {
        behaviours::Behaviour::Property => "fuzz",
        behaviours::Behaviour::StateMutation => "state-mutation",
        behaviours::Behaviour::StateIdentity => "state-identity",
        behaviours::Behaviour::ResponseEquality => "response-equality",
        behaviours::Behaviour::ResponseInequality => "response-inequality",
    };

    let file_path = format!("{dir_path}/{behaviour_name}.json");

    // TODO: move the actual writing
    fs::create_dir_all(dir_path)?;

    fs::write(file_path, json_result.as_bytes())?;

    Ok(())
}

pub fn read_results(path: &str) -> Result<Vec<explore::ExplorationResult>> {
    let raw_results = fs::read(path)?;

    let results: Vec<explore::ExplorationResult> = serde_json::from_slice(&raw_results)?;

    Ok(results)
}

pub fn exploration_coverage(
    results: &[explore::ExplorationResult],
    operations: &[amos::Operation],
) -> ExplorationCoverage {
    // Find what operations are covered by the examples
    // TODO: again.. use ids..
    let mut covered_operations = HashSet::new();
    for result in results {
        match result {
            explore::ExplorationResult::NoExampleFound { .. } => continue,
            explore::ExplorationResult::ResponseCheck {
                operation,
                minimal_sequence,
            } => {
                covered_operations.insert(operation.clone());
                if let Some(seq) = minimal_sequence {
                    for op in seq {
                        covered_operations.insert(op.name.clone());
                    }
                }
            }
            explore::ExplorationResult::StateIdentity {
                query_operation,
                minimal_sequence,
            } => {
                covered_operations.insert(query_operation.clone());
                if let Some(seq) = minimal_sequence {
                    for op in seq {
                        covered_operations.insert(op.name.clone());
                    }
                }
            }
            explore::ExplorationResult::StateMutation {
                query_operation,
                minimal_sequence,
            } => {
                covered_operations.insert(query_operation.clone());
                if let Some(seq) = minimal_sequence {
                    for op in seq {
                        covered_operations.insert(op.name.clone());
                    }
                }
            }
            explore::ExplorationResult::ResponseEquality {
                operation,
                minimal_sequence,
            } => {
                covered_operations.insert(operation.clone());
                if let Some(seq) = minimal_sequence {
                    for op in seq {
                        covered_operations.insert(op.name.clone());
                    }
                }
            }
            explore::ExplorationResult::ResponseInEquality {
                operation,
                minimal_sequence,
            } => {
                covered_operations.insert(operation.clone());
                if let Some(seq) = minimal_sequence {
                    for op in seq {
                        covered_operations.insert(op.name.clone());
                    }
                }
            }
        }
    }

    let available_ops = operations
        .iter()
        .map(|op| op.info.name.clone())
        .collect::<HashSet<String>>();

    let mut uncovered = (&available_ops - &covered_operations)
        .into_iter()
        .collect::<Vec<String>>();
    uncovered.sort();

    let mut covered = covered_operations.into_iter().collect::<Vec<String>>();
    covered.sort();

    ExplorationCoverage { uncovered, covered }
}
