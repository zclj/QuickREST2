use crate::amos::{self, Operation, Parameter, Schema};
use crate::amos_buckets::{BucketItem, Buckets};
use crate::amos_relations::{self, Relation};
use proptest::prelude::*;
use serde::{Deserialize, Serialize};
use tracing::warn;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ParameterValue {
    StringValue {
        value: String,
        seed: i32,
        active: bool,
    },
    IntValue {
        value: i64,
        seed: i32,
        active: bool,
    },
    BoolValue {
        value: bool,
        seed: i32,
        active: bool,
    },
    DoubleValue {
        value: f64,
        seed: i32,
        active: bool,
    },
    IPV4Value {
        value: (u8, u8, u8, u8),
        seed: i32,
        active: bool,
    },
    Reference {
        active: bool,
        idx: [usize; 2],
        fallback: Box<ParameterValue>,
        relation: Relation,
    },
    ArrayOfString {
        value: Vec<String>,
        seed: i32,
        active: bool,
    },
    File {
        value: u8,
        seed: i32,
        active: bool,
    },

    Empty,
}

impl ParameterValue {
    pub fn as_string_value(&self) -> String {
        match self {
            ParameterValue::StringValue { value, .. } => value.clone(),
            _ => panic!("as_string_value() called on unsupported enum: {:?}", self),
        }
    }

    pub fn as_int_value(&self) -> i64 {
        match self {
            ParameterValue::IntValue { value, .. } => *value,
            _ => panic!("as_int_value() called on unsupported enum: {:?}", self),
        }
    }

    pub fn seed(&self) -> i32 {
        match self {
            ParameterValue::StringValue { seed, .. } => *seed,
            ParameterValue::IntValue { seed, .. } => *seed,
            ParameterValue::File { seed, .. } => *seed,
            _ => panic!("seed() called on unsupported enum"),
        }
    }

    pub fn active(&self) -> bool {
        match self {
            ParameterValue::StringValue { active, .. } => *active,
            ParameterValue::IntValue { active, .. } => *active,
            ParameterValue::BoolValue { active, .. } => *active,
            ParameterValue::DoubleValue { active, .. } => *active,
            ParameterValue::Reference { active, .. } => *active,
            ParameterValue::ArrayOfString { active, .. } => *active,
            ParameterValue::IPV4Value { active, .. } => *active,
            ParameterValue::File { active, .. } => *active,
            ParameterValue::Empty => false,
            //_ => panic!("active() called on unsupported enum"),
        }
    }
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct GeneratedParameter {
    pub name: String,
    pub value: ParameterValue,
    pub ref_path: Option<String>,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct GeneratedOperation {
    pub name: String,
    pub parameters: Vec<GeneratedParameter>,
}

pub type GenerationOperationWithParameters = (Operation, [ParameterValue; 10]);

// EXPERIMENT: make a path UI -> gen
pub fn gen_banana_cake_value(expr: String) -> BoxedStrategy<ParameterValue> {
    let regex = proptest::string::string_regex(&expr).unwrap();

    (regex, (1..10i32), proptest::bool::weighted(0.5))
        .prop_map(|(s, seed, active)| ParameterValue::StringValue {
            value: s,
            seed,
            active,
        })
        .boxed()
}

fn gen_parameter_value(t: Option<&Parameter>) -> BoxedStrategy<ParameterValue> {
    match t {
        Some(tt) => {
            let ref_weight = match tt.ownership {
                amos::ParameterOwnership::Owned => 0.95,
                amos::ParameterOwnership::Dependency => 0.95,
                amos::ParameterOwnership::Unknown => 0.5,
            };

            match &tt.schema {
                Schema::StringRegex { regex } => {
                    // TODO: would be strongly prefered if we check and store the regex
                    //  at entry/parse time, not at generation time
                    let regex = proptest::string::string_regex(regex).unwrap();

                    (regex, (1..10i32), proptest::bool::weighted(ref_weight))
                        .prop_map(|(s, seed, active)| ParameterValue::StringValue {
                            value: s,
                            seed,
                            active,
                        })
                        .boxed()
                }
                Schema::String => ("[a-z]*", (1..10i32), proptest::bool::weighted(ref_weight))
                    .prop_map(|(s, seed, active)| ParameterValue::StringValue {
                        value: s,
                        seed,
                        active,
                    })
                    .boxed(),
                Schema::StringNonEmpty => {
                    ("[a-z]+", (1..10i32), proptest::bool::weighted(ref_weight))
                        .prop_map(|(s, seed, active)| ParameterValue::StringValue {
                            value: s,
                            seed,
                            active,
                        })
                        .boxed()
                }
                Schema::Int8 => (0..256i64, (1..10i32), proptest::bool::weighted(ref_weight))
                    .prop_map(|(i, seed, active)| ParameterValue::IntValue {
                        value: i,
                        seed,
                        active,
                    })
                    .boxed(),
                Schema::Int => (
                    //0..u64::MAX,
                    //-1000..1000,
                    prop_oneof![
                        8 => -1000i64..=1000i64,
                        1 => i64::MIN..-1000,
                        1 => 1000..i64::MAX,
                    ],
                    (1..10i32),
                    proptest::bool::weighted(ref_weight),
                )
                    .prop_map(|(i, seed, active)| ParameterValue::IntValue {
                        value: i,
                        seed,
                        active,
                    })
                    .boxed(),

                Schema::Int32 => (
                    //0..u32::MAX as u64,
                    //-1000..1000,
                    prop_oneof![
                        8 => -1000..=1000,
                        1 => i32::MIN..-1000,
                        1 => 1000..i32::MAX,
                    ],
                    (1..10i32),
                    proptest::bool::weighted(ref_weight),
                )
                    .prop_map(|(i, seed, active)| ParameterValue::IntValue {
                        value: i as i64,
                        seed,
                        active,
                    })
                    .boxed(),

                Schema::Double => (
                    // TODO: What's a 'good' default range?
                    //   also, enable this to be configured
                    //f64::MIN..f64::MAX,
                    //-100.0..100.0,
                    prop_oneof![
                        8 => -100.0..=100.0,
                        1 => f64::MIN..-100.0,
                        1 => 100.0..f64::MAX,
                    ],
                    (1..10i32),
                    proptest::bool::weighted(ref_weight),
                )
                    .prop_map(|(i, seed, active)| ParameterValue::DoubleValue {
                        value: i,
                        seed,
                        active,
                    })
                    .boxed(),

                Schema::Float => (
                    (f32::MIN..f32::MAX),
                    (1..10i32),
                    proptest::bool::weighted(ref_weight),
                )
                    .prop_map(|(i, seed, active)| ParameterValue::DoubleValue {
                        value: i as f64,
                        seed,
                        active,
                    })
                    .boxed(),

                Schema::Bool => (
                    proptest::bool::ANY,
                    (1..10i32),
                    proptest::bool::weighted(ref_weight),
                )
                    .prop_map(|(i, seed, active)| ParameterValue::BoolValue {
                        value: i,
                        seed,
                        active,
                    })
                    .boxed(),

                Schema::IPV4 => (
                    (
                        proptest::num::u8::ANY,
                        proptest::num::u8::ANY,
                        proptest::num::u8::ANY,
                        proptest::num::u8::ANY,
                    ),
                    (1..10i32),
                    proptest::bool::weighted(ref_weight),
                )
                    .prop_map(|(i, seed, active)| ParameterValue::IPV4Value {
                        value: i,
                        seed,
                        active,
                    })
                    .boxed(),

                Schema::File => (
                    proptest::num::u8::ANY,
                    (1..10i32),
                    proptest::bool::weighted(ref_weight),
                )
                    .prop_map(|(i, seed, active)| ParameterValue::File {
                        value: i,
                        seed,
                        active,
                    })
                    .boxed(),

                Schema::ArrayOfString => (
                    prop::collection::vec("[a-z]*", 0..10),
                    (1..10i32),
                    proptest::bool::weighted(ref_weight),
                )
                    .prop_map(|(i, seed, active)| ParameterValue::ArrayOfString {
                        value: i,
                        seed,
                        active,
                    })
                    .boxed(),
                Schema::StringDateTime => (
                    0..10000,
                    1..13,
                    1..32,
                    0..24,
                    0..60,
                    0..60,
                    (1..10i32),
                    proptest::bool::weighted(ref_weight),
                )
                    .prop_map(|(y, m, d, h, min, sec, seed, active)| {
                        // https://datatracker.ietf.org/doc/html/rfc3339#section-5.6
                        // date-time       = full-date "T" full-time
                        // for example, 2017-07-21T17:32:28Z
                        let date_str =
                            format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", y, m, d, h, min, sec);
                        ParameterValue::StringValue {
                            value: date_str,
                            seed,
                            active,
                        }
                    })
                    .boxed(),
                Schema::Reference(_)
                | Schema::Object { .. }
                | Schema::DateTime
                | Schema::ArrayOfUniqueRefItems(_)
                | Schema::ArrayOfRefItems(_)
                | Schema::Number => {
                    warn!(
                        "Generating with undecided schema, parameter: {}:{}",
                        tt.name, tt.schema
                    );
                    Just(ParameterValue::Empty).boxed()
                }
                Schema::Unsupported => {
                    warn!("Generating with unsupported schema, parameter: {}", tt.name);
                    Just(ParameterValue::Empty).boxed()
                }
            }
        }
        None => Just(ParameterValue::Empty).boxed(),
    }
}

#[allow(clippy::get_first)]
pub fn gen_param_array(params: &[Parameter]) -> BoxedStrategy<[ParameterValue; 10]> {
    [
        gen_parameter_value(params.get(0)),
        gen_parameter_value(params.get(1)),
        gen_parameter_value(params.get(2)),
        gen_parameter_value(params.get(3)),
        gen_parameter_value(params.get(4)),
        gen_parameter_value(params.get(5)),
        gen_parameter_value(params.get(6)),
        gen_parameter_value(params.get(7)),
        gen_parameter_value(params.get(8)),
        gen_parameter_value(params.get(9)),
    ]
    .boxed()
}

fn gen_operation(ops: Vec<Operation>) -> impl Strategy<Value = Operation> {
    debug_assert!(!ops.is_empty(), "Operations to select from cannot be empty");
    (0..ops.len()).prop_map(move |idx| ops[idx].clone())
}

pub fn gen_operation_with_params(
    ops: Vec<Operation>,
) -> impl Strategy<Value = (Operation, [ParameterValue; 10])> {
    gen_operation(ops).prop_flat_map(|op| (Just(op.clone()), gen_param_array(&op.parameters)))
}

pub fn gen_operation_sequence(
    ops: Vec<Operation>,
) -> impl Strategy<Value = Vec<(Operation, [ParameterValue; 10])>> {
    // TODO: set the 0..5 via config
    prop::collection::vec(gen_operation_with_params(ops), 0..5)
}

// NOTE: We don't need a Vec here, but that is the current upstream interface
pub fn gen_static_operation_with_params(
    op: Operation,
) -> impl Strategy<Value = (u8, Vec<(Operation, [ParameterValue; 10])>)> {
    (Just(op.clone()), gen_param_array(&op.parameters)).prop_map(|op| (0, vec![op]))
}

pub fn gen_operation_sequence_with_pinned(
    pinned_op: Operation,
    ops: Vec<Operation>,
    min: u8,
    max: u8,
) -> impl Strategy<Value = Vec<(Operation, [ParameterValue; 10])>> {
    (
        (
            Just(pinned_op.clone()),
            gen_param_array(&pinned_op.parameters),
        ),
        prop::collection::vec(
            gen_operation_with_params(ops),
            (min as usize)..=(max as usize),
        ),
    )
        .prop_map(|(pin, mut gen_ops)| {
            let mut pinned_seq = vec![pin];
            pinned_seq.append(&mut gen_ops);
            pinned_seq
        })
}

pub fn gen_pinned_operation_sequence_with_params(
    pinned_op: Operation,
    ops: Vec<Operation>,
    min: u8,
    max: u8,
) -> impl Strategy<Value = (u8, Vec<(Operation, [ParameterValue; 10])>)> {
    (
        Just(0),
        gen_operation_sequence_added_params(gen_operation_sequence_with_pinned(
            pinned_op, ops, min, max,
        )),
    )
}

////////////////////////////////////////
// Experiment
fn resolve_parameters(
    op: &mut (Operation, [ParameterValue; 10]),
    related_candidates: &[GenerationOperationWithParameters],
) {
    // bucket_2[i] (op)
    // bucket_1 -> related_candidates
    for pidx in 0..op.0.parameters.len() {
        let param = &op.0.parameters[pidx]; //&bucket_2[i].0.parameters[j];
        let param_value = &op.1[pidx];

        if param_value.active() {
            let possible_params_relations =
                amos_relations::related_parameters(related_candidates, param);

            if !possible_params_relations.is_empty() {
                let choose = param_value.seed() % (possible_params_relations.len()) as i32;
                let selected_rel = &possible_params_relations[choose as usize];

                let idx = match selected_rel {
                    Relation::Parameter(r) | Relation::Response(r) => [r.op_idx, r.idx],
                };

                op.1[pidx] = ParameterValue::Reference {
                    active: param_value.active(),
                    idx,
                    relation: selected_rel.clone(),
                    fallback: Box::new(op.1[pidx].clone()),
                };
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum GenOperation {
    Generated((Operation, [ParameterValue; 10])),
    Empty,
}

fn gen_bucket_operation(
    bucket: u8,
    bucket_len: u8,
    ops: &[Operation],
) -> BoxedStrategy<GenOperation> {
    if bucket > bucket_len || ops.is_empty() {
        Just(GenOperation::Empty).boxed()
    } else {
        gen_operation_with_params(ops.to_vec())
            .prop_map(GenOperation::Generated)
            .boxed()
    }
}

pub struct QueryOptions {
    pub precedence: u8,
    pub slack_min: u8,
    pub slack_max: u8,
}

// TODO: If the GET op is in first position and have a param, it's ok for it
// to generate, so It can be changed to 'Owned' (However, as there is nothing to dep
//  on it will revert to 'gen'. BUUT a CREATE following such a GET should be fine
//  with refering to its param)
pub fn gen_buckets_5(
    query_op: Operation,
    query_options: QueryOptions,
    buckets: Buckets,
    //TODO: should be ids
    bucketize: fn(&Buckets, u8) -> Vec<Vec<&BucketItem>>,
    operations: Vec<Operation>,
) -> impl Strategy<Value = (u8, Vec<(Operation, [ParameterValue; 10])>)> {
    ({
        let lower = if query_options.slack_min > query_options.precedence {
            0
        } else {
            query_options.precedence - query_options.slack_min
        };
        lower..=(query_options.precedence + query_options.slack_max)
    })
    .prop_flat_map(move |q_pos| {
        // We need to bucketize based on seq length
        let seq_length = q_pos + 1;
        let bucketized = bucketize(&buckets, seq_length.clamp(2, 5));

        let ops_bucket_1 = bucketized[0]
            .iter()
            .map(|bucket_item| {
                let op = operations
                    .iter()
                    .find(|op| op.info.name == bucket_item.name);
                op.unwrap().clone()
            })
            .collect::<Vec<Operation>>();

        let ops_bucket_2 = bucketized[1]
            .iter()
            .map(|bucket_item| {
                let op = operations
                    .iter()
                    .find(|op| op.info.name == bucket_item.name);
                op.unwrap().clone()
            })
            .collect::<Vec<Operation>>();

        let ops_bucket_3 = bucketized[2]
            .iter()
            .map(|bucket_item| {
                let op = operations
                    .iter()
                    .find(|op| op.info.name == bucket_item.name);
                op.unwrap().clone()
            })
            .collect::<Vec<Operation>>();

        let ops_bucket_4 = bucketized[3]
            .iter()
            .map(|bucket_item| {
                let op = operations
                    .iter()
                    .find(|op| op.info.name == bucket_item.name);
                op.unwrap().clone()
            })
            .collect::<Vec<Operation>>();

        let ops_bucket_5 = bucketized[4]
            .iter()
            .map(|bucket_item| {
                let op = operations
                    .iter()
                    .find(|op| op.info.name == bucket_item.name);
                op.unwrap().clone()
            })
            .collect::<Vec<Operation>>();

        (
            (Just(q_pos)),
            (
                Just(query_op.clone()),
                gen_param_array(&query_op.parameters),
            ),
            gen_bucket_operation(1, seq_length, &ops_bucket_1),
            gen_bucket_operation(2, seq_length, &ops_bucket_2),
            gen_bucket_operation(3, seq_length, &ops_bucket_3),
            gen_bucket_operation(4, seq_length, &ops_bucket_4),
            gen_bucket_operation(5, seq_length, &ops_bucket_5),
        )
    })
    .prop_map(move |(q_pos, mut q_op, op_1, op_2, op_3, op_4, op_5)| {
        // First OP can ref the query OP. Even though the query OP might
        //  be put in a later position at synth., it is conceptually the
        //  'root' of the sequence
        let gen_ops = vec![op_1, op_2, op_3, op_4, op_5];

        let mut gen_op_seq = vec![];

        // pull out the generated operations
        for gen_op in gen_ops {
            match gen_op {
                GenOperation::Generated(op) => gen_op_seq.push(op),
                GenOperation::Empty => (),
            }
        }

        // NOTE: This is 0 indexed while the q_pos is 1 indexed.
        //  For now, adjust q_pos, but this should be normalized throughout
        let zero_based_q_pos = if q_pos == 0 { 0 } else { q_pos - 1 };
        // The query position can not be outside the length of the sequence
        let q_pos = if zero_based_q_pos > 0 && zero_based_q_pos > ((gen_op_seq.len() - 1) as u8) {
            (gen_op_seq.len() - 1) as u8
        } else {
            zero_based_q_pos
        };

        let mut final_seq = vec![];
        for (idx, mut op) in gen_op_seq.clone().into_iter().enumerate() {
            //if idx == query_options.precedence as usize {
            #[allow(clippy::comparison_chain)] // Hot path, match might not inline
            if idx == q_pos as usize {
                // Time the inject the query-op, resolve/add it and then
                //  resolve/add the current generated op
                resolve_parameters(&mut q_op, &final_seq[0..idx]);
                final_seq.push(q_op.clone());
                resolve_parameters(&mut op, &final_seq[0..idx + 1]);
                final_seq.push(op.clone());
            } else if idx > q_pos as usize {
                // Resolve/add the current generated operation.
                // We have passed the point of where the query-op is injected,
                // so we can now reference idx + 1
                resolve_parameters(&mut op, &final_seq[0..idx + 1]);
                final_seq.push(op.clone());
            } else {
                // Resolve/add the current generated operation.
                // Query op has not yet been added.
                resolve_parameters(&mut op, &final_seq[0..idx]);
                final_seq.push(op.clone());
            }
        }

        (q_pos, final_seq)
    })
}

pub fn gen_operation_sequence_added_params(
    seq_gen: impl Strategy<Value = Vec<(Operation, [ParameterValue; 10])>>,
) -> impl Strategy<Value = Vec<(Operation, [ParameterValue; 10])>> {
    seq_gen.prop_map(|mut gen_ops| {
        if gen_ops.len() > 1 {
            for i in 0..gen_ops.len() {
                for j in 0..gen_ops[i].0.parameters.len() {
                    let param = &gen_ops[i].0.parameters[j];
                    let param_value = &gen_ops[i].1[j];

                    if param_value.active() {
                        let possible_params_relations =
                            amos_relations::related_parameters(&gen_ops[0..i], param);

                        if !possible_params_relations.is_empty() {
                            let choose =
                                param_value.seed() % (possible_params_relations.len()) as i32;
                            let selected_rel = &possible_params_relations[choose as usize];

                            let idx = match selected_rel {
                                Relation::Parameter(r) | Relation::Response(r) => [r.op_idx, r.idx],
                            };

                            gen_ops[i].1[j] = ParameterValue::Reference {
                                active: param_value.active(),
                                idx,
                                relation: selected_rel.clone(),
                                fallback: Box::new(gen_ops[i].1[j].clone()),
                            };
                        }
                    }
                }
            }
            gen_ops
        } else {
            gen_ops
        }
    })
}

#[cfg(test)]
mod tests {
    use crate::{
        amos::{Operation, OperationInfo, Parameter, Response, Schema},
        amos_generation::ParameterValue,
    };
    use proptest::{
        strategy::{Strategy, ValueTree},
        test_runner::{Config, FileFailurePersistence, RngAlgorithm, TestRng, TestRunner},
    };

    use crate::amos::ParameterOwnership;
    use crate::amos_generation::ParameterValue::*;
    use crate::amos_generation::Schema::*;
    use crate::amos_generation::*;

    fn create_runner() -> TestRunner {
        let rng = TestRng::from_seed(
            RngAlgorithm::ChaCha,
            &[
                2, 2, 3, 4, 1, 2, 3, 4, 1, 2, 3, 4, 1, 2, 3, 4, 1, 2, 3, 4, 1, 2, 3, 4, 1, 2, 3, 4,
                1, 2, 3, 4,
            ],
        );

        let mut runner = TestRunner::new_with_rng(
            Config {
                failure_persistence: Some(Box::new(FileFailurePersistence::Off)),
                ..Config::default()
            },
            rng,
        );

        runner
    }

    #[test]
    fn parameter_value_as_string_value() {
        let p = ParameterValue::StringValue {
            value: "abc".to_string(),
            seed: 123,
            active: true,
        };
        let p_str = p.as_string_value();

        // int for failure
        let pi = ParameterValue::IntValue {
            value: 123,
            seed: 123,
            active: true,
        };
        let failed_str = std::panic::catch_unwind(|| pi.as_string_value());

        assert!(failed_str.is_err());
        assert_eq!(p_str, "abc".to_string());
    }

    #[test]
    fn parameter_value_as_int_value() {
        let pi = ParameterValue::IntValue {
            value: 123,
            seed: 123,
            active: true,
        };
        let p_int = pi.as_int_value();

        // int for failure
        let pi = ParameterValue::StringValue {
            value: "123".to_string(),
            seed: 123,
            active: true,
        };
        let failed_int = std::panic::catch_unwind(|| pi.as_int_value());

        assert!(failed_int.is_err());
        assert_eq!(p_int, 123);
    }

    #[test]
    fn parameter_value_seed() {
        let ps = ParameterValue::StringValue {
            value: "abc".to_string(),
            seed: 123,
            active: true,
        };
        let pi = ParameterValue::IntValue {
            value: 123,
            seed: 123,
            active: true,
        };
        // failing
        let pb = ParameterValue::BoolValue {
            value: false,
            seed: 123,
            active: true,
        };
        let failed_seed = std::panic::catch_unwind(|| pb.seed());

        assert!(failed_seed.is_err());
        assert_eq!(123, ps.seed());
        assert_eq!(123, pi.seed());
    }

    #[test]
    fn parameter_value_active() {
        let ps = ParameterValue::StringValue {
            value: "abc".to_string(),
            seed: 123,
            active: true,
        };
        let pi = ParameterValue::IntValue {
            value: 123,
            seed: 123,
            active: true,
        };
        let pb = ParameterValue::BoolValue {
            value: false,
            seed: 123,
            active: true,
        };
        let pd = ParameterValue::DoubleValue {
            value: 1.0,
            seed: 123,
            active: false,
        };
        let pr = ParameterValue::Reference {
            active: false,
            idx: [0, 0],
            fallback: Box::new(ParameterValue::Empty),
            relation: Relation::Response(amos_relations::RelationInfo {
                operation: "foo".to_string(),
                name: "bar".to_string(),
                schema: Schema::ArrayOfString,
                strength: 1,
                op_idx: 0,
                idx: 0,
            }),
        };
        let pa = ParameterValue::ArrayOfString {
            value: vec![],
            seed: 123,
            active: true,
        };
        let pip = ParameterValue::IPV4Value {
            value: (1, 1, 1, 1),
            seed: 123,
            active: false,
        };
        let pe = ParameterValue::Empty;

        assert_eq!(true, ps.active());
        assert_eq!(true, pi.active());
        assert_eq!(true, pb.active());
        assert_eq!(false, pd.active());
        assert_eq!(false, pr.active());
        assert_eq!(true, pa.active());
        assert_eq!(false, pip.active());
        assert_eq!(false, pe.active());
    }

    #[test]
    fn gen_parameter_value_string_regex() {
        let mut runner = create_runner();

        let param = Parameter {
            name: "name".to_string(),
            schema: Schema::StringRegex {
                regex: "a*".to_string(),
            },
            required: true,
            ownership: ParameterOwnership::Owned,
            meta_data: None,
        };

        let gen = gen_parameter_value(Some(&param));
        let value = gen.new_tree(&mut runner).unwrap();

        assert_eq!(
            value.current(),
            ParameterValue::StringValue {
                value: "aaaaaaaaaaa".to_string(),
                seed: 4,
                active: true
            }
        )
    }

    #[test]
    fn gen_parameter_value_string_non_empty() {
        let mut runner = create_runner();

        let param = Parameter {
            name: "name".to_string(),
            schema: Schema::StringNonEmpty,
            required: true,
            ownership: ParameterOwnership::Owned,
            meta_data: None,
        };

        let gen = gen_parameter_value(Some(&param));
        let value = gen.new_tree(&mut runner).unwrap();

        assert_eq!(
            value.current(),
            ParameterValue::StringValue {
                value: "jieewpsmyqgd".to_string(),
                seed: 5,
                active: true
            }
        )
    }

    #[test]
    fn gen_parameter_value_int() {
        let mut runner = create_runner();

        let param = Parameter {
            name: "name".to_string(),
            schema: Schema::Int,
            required: true,
            ownership: ParameterOwnership::Dependency,
            meta_data: None,
        };

        let gen = gen_parameter_value(Some(&param));
        let value = gen.new_tree(&mut runner).unwrap();

        assert_eq!(
            value.current(),
            ParameterValue::IntValue {
                value: 293,
                seed: 7,
                active: true
            }
        )
    }

    #[test]
    fn gen_parameter_value_int32() {
        let mut runner = create_runner();

        let param = Parameter {
            name: "name".to_string(),
            schema: Schema::Int32,
            required: true,
            ownership: ParameterOwnership::Dependency,
            meta_data: None,
        };

        let gen = gen_parameter_value(Some(&param));
        let value = gen.new_tree(&mut runner).unwrap();

        assert_eq!(
            value.current(),
            ParameterValue::IntValue {
                value: -119,
                seed: 6,
                active: true
            }
        )
    }

    #[test]
    fn gen_parameter_value_double() {
        let mut runner = create_runner();

        let param = Parameter {
            name: "name".to_string(),
            schema: Schema::Double,
            required: true,
            ownership: ParameterOwnership::Unknown,
            meta_data: None,
        };

        let gen = gen_parameter_value(Some(&param));
        let value = gen.new_tree(&mut runner).unwrap();

        assert_eq!(
            value.current(),
            ParameterValue::DoubleValue {
                value: 29.270404690910443,
                seed: 4,
                active: true
            }
        )
    }

    #[test]
    fn gen_parameter_value_float() {
        let mut runner = create_runner();

        let param = Parameter {
            name: "name".to_string(),
            schema: Schema::Float,
            required: true,
            ownership: ParameterOwnership::Unknown,
            meta_data: None,
        };

        let gen = gen_parameter_value(Some(&param));
        let value = gen.new_tree(&mut runner).unwrap();

        assert_eq!(
            value.current(),
            ParameterValue::DoubleValue {
                value: 3.1796461751356035e38,
                seed: 7,
                active: true
            }
        )
    }

    #[test]
    fn gen_parameter_value_bool() {
        let mut runner = create_runner();

        let param = Parameter {
            name: "name".to_string(),
            schema: Schema::Bool,
            required: true,
            ownership: ParameterOwnership::Unknown,
            meta_data: None,
        };

        let gen = gen_parameter_value(Some(&param));
        let value = gen.new_tree(&mut runner).unwrap();

        assert_eq!(
            value.current(),
            ParameterValue::BoolValue {
                value: true,
                seed: 4,
                active: false
            }
        )
    }

    #[test]
    fn gen_parameter_value_ipv4() {
        let mut runner = create_runner();

        let param = Parameter {
            name: "name".to_string(),
            schema: Schema::IPV4,
            required: true,
            ownership: ParameterOwnership::Unknown,
            meta_data: None,
        };

        let gen = gen_parameter_value(Some(&param));
        let value = gen.new_tree(&mut runner).unwrap();

        assert_eq!(
            value.current(),
            ParameterValue::IPV4Value {
                value: (50, 136, 50, 130),
                seed: 7,
                active: true
            }
        )
    }

    #[test]
    fn gen_parameter_value_array_of_string() {
        let mut runner = create_runner();

        let param = Parameter {
            name: "name".to_string(),
            schema: Schema::ArrayOfString,
            required: true,
            ownership: ParameterOwnership::Unknown,
            meta_data: None,
        };

        let gen = gen_parameter_value(Some(&param));
        let value = gen.new_tree(&mut runner).unwrap();

        assert_eq!(
            value.current(),
            ParameterValue::ArrayOfString {
                value: vec![
                    "rnuflixbvcdnlmtcpckwy".to_string(),
                    "gdgbgrrjzhlucjfcndxdo".to_string(),
                    "udwzochuzvba".to_string()
                ],
                seed: 3,
                active: true
            }
        )
    }

    #[test]
    fn gen_parameter_value_string_date_time() {
        let mut runner = create_runner();

        let param = Parameter {
            name: "name".to_string(),
            schema: Schema::StringDateTime,
            required: true,
            ownership: ParameterOwnership::Unknown,
            meta_data: None,
        };

        let gen = gen_parameter_value(Some(&param));
        let value = gen.new_tree(&mut runner).unwrap();

        assert_eq!(
            value.current(),
            ParameterValue::StringValue {
                value: "9672-05-14T15:43:35Z".to_string(),
                seed: 1,
                active: true
            }
        )
    }

    #[test]
    fn gen_parameter_value_undecided() {
        let mut runner = create_runner();

        let param = Parameter {
            name: "name".to_string(),
            schema: Schema::Number,
            required: true,
            ownership: ParameterOwnership::Unknown,
            meta_data: None,
        };

        let gen = gen_parameter_value(Some(&param));
        let value = gen.new_tree(&mut runner).unwrap();

        assert_eq!(value.current(), ParameterValue::Empty,)
    }

    #[test]
    fn gen_parameter_value_unsupported() {
        let mut runner = create_runner();

        let param = Parameter {
            name: "name".to_string(),
            schema: Schema::Unsupported,
            required: true,
            ownership: ParameterOwnership::Unknown,
            meta_data: None,
        };

        let gen = gen_parameter_value(Some(&param));
        let value = gen.new_tree(&mut runner).unwrap();

        assert_eq!(value.current(), ParameterValue::Empty,)
    }

    #[test]
    fn gen_static_operation_with_params_test() {
        let mut runner = create_runner();

        let op = Operation {
            info: OperationInfo {
                name: "get_persons".to_string(),
                key: "operation/get_persons".to_string(),
            },
            parameters: vec![Parameter {
                name: "name".to_string(),
                schema: Schema::String,
                required: true,
                ownership: ParameterOwnership::Owned,
                meta_data: None,
            }],
            responses: vec![Response {
                name: "successful operation".to_string(),
                schema: Schema::ArrayOfRefItems("person".to_string()),
            }],
            meta_data: None,
        };

        let gen = gen_static_operation_with_params(op);
        let value = gen.new_tree(&mut runner).unwrap();

        assert_eq!(
            (
                0,
                vec![(
                    Operation {
                        info: OperationInfo {
                            name: "get_persons".to_string(),
                            key: "operation/get_persons".to_string()
                        },
                        parameters: vec![Parameter {
                            name: "name".to_string(),
                            schema: String,
                            required: true,
                            ownership: ParameterOwnership::Owned,
                            meta_data: None
                        }],
                        responses: vec![Response {
                            name: "successful operation".to_string(),
                            schema: Schema::ArrayOfRefItems("person".to_string())
                        }],
                        meta_data: None
                    },
                    [
                        StringValue {
                            value: "jieewpsmyqg".to_string(),
                            seed: 5,
                            active: true
                        },
                        Empty,
                        Empty,
                        Empty,
                        Empty,
                        Empty,
                        Empty,
                        Empty,
                        Empty,
                        Empty
                    ]
                )]
            ),
            value.current(),
        )
    }

    #[test]
    fn gen_pinned_operation_sequence_with_params_test() {
        let mut runner = create_runner();

        let get_persons_op = Operation {
            info: OperationInfo {
                name: "get_persons".to_string(),
                key: "operation/get_persons".to_string(),
            },
            parameters: vec![],
            responses: vec![Response {
                name: "successful operation".to_string(),
                schema: Schema::ArrayOfRefItems("person".to_string()),
            }],
            meta_data: None,
        };

        let delete_person_op = Operation {
            info: OperationInfo {
                name: "delete_person".to_string(),
                key: "operation/delete_person".to_string(),
            },
            parameters: vec![Parameter {
                name: "name".to_string(),
                schema: Schema::String,
                required: true,
                ownership: ParameterOwnership::Owned,
                meta_data: None,
            }],
            responses: vec![Response {
                name: "successful operation".to_string(),
                schema: Schema::Reference("person".to_string()),
            }],
            meta_data: None,
        };

        let post_person_op = Operation {
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

        let operations = vec![get_persons_op.clone(), post_person_op, delete_person_op];
        let gen = gen_pinned_operation_sequence_with_params(get_persons_op, operations, 1, 3);
        let value = gen.new_tree(&mut runner).unwrap();

        assert_eq!(
            (
                0,
                vec![
                    (
                        Operation {
                            info: OperationInfo {
                                name: "get_persons".to_string(),
                                key: "operation/get_persons".to_string()
                            },
                            parameters: vec![],
                            responses: vec![Response {
                                name: "successful operation".to_string(),
                                schema: Schema::ArrayOfRefItems("person".to_string())
                            }],
                            meta_data: None
                        },
                        [Empty, Empty, Empty, Empty, Empty, Empty, Empty, Empty, Empty, Empty]
                    ),
                    (
                        Operation {
                            info: OperationInfo {
                                name: "post_person".to_string(),
                                key: "operation/post_person".to_string()
                            },
                            parameters: vec![
                                Parameter {
                                    name: "name".to_string(),
                                    schema: String,
                                    required: true,
                                    ownership: ParameterOwnership::Owned,
                                    meta_data: None
                                },
                                Parameter {
                                    name: "age".to_string(),
                                    schema: Int8,
                                    required: true,
                                    ownership: ParameterOwnership::Owned,
                                    meta_data: None
                                }
                            ],
                            responses: vec![Response {
                                name: "successful operation".to_string(),
                                schema: Schema::Reference("person".to_string())
                            }],
                            meta_data: None
                        },
                        [
                            StringValue {
                                value: "tieewpsmyqgdnlmtcpc".to_string(),
                                seed: 1,
                                active: true
                            },
                            IntValue {
                                value: 188,
                                seed: 7,
                                active: true
                            },
                            Empty,
                            Empty,
                            Empty,
                            Empty,
                            Empty,
                            Empty,
                            Empty,
                            Empty
                        ]
                    ),
                    (
                        Operation {
                            info: OperationInfo {
                                name: "get_persons".to_string(),
                                key: "operation/get_persons".to_string()
                            },
                            parameters: vec![],
                            responses: vec![Response {
                                name: "successful operation".to_string(),
                                schema: Schema::ArrayOfRefItems("person".to_string())
                            }],
                            meta_data: None
                        },
                        [Empty, Empty, Empty, Empty, Empty, Empty, Empty, Empty, Empty, Empty]
                    )
                ]
            ),
            value.current()
        )
    }

    #[test]
    fn gen_pinned_operation_sequence_with_params_relations_test() {
        let mut runner = create_runner();

        let get_persons_op = Operation {
            info: OperationInfo {
                name: "get_persons".to_string(),
                key: "operation/get_persons".to_string(),
            },
            parameters: vec![Parameter {
                name: "name".to_string(),
                schema: Schema::String,
                required: true,
                ownership: ParameterOwnership::Owned,
                meta_data: None,
            }],
            responses: vec![Response {
                name: "successful operation".to_string(),
                schema: Schema::ArrayOfRefItems("person".to_string()),
            }],
            meta_data: None,
        };

        let delete_person_op = Operation {
            info: OperationInfo {
                name: "delete_person".to_string(),
                key: "operation/delete_person".to_string(),
            },
            parameters: vec![Parameter {
                name: "name".to_string(),
                schema: Schema::String,
                required: true,
                ownership: ParameterOwnership::Owned,
                meta_data: None,
            }],
            responses: vec![Response {
                name: "successful operation".to_string(),
                schema: Schema::Reference("person".to_string()),
            }],
            meta_data: None,
        };

        let operations = vec![get_persons_op.clone(), delete_person_op];
        let gen = gen_pinned_operation_sequence_with_params(get_persons_op, operations, 1, 1);
        let value = gen.new_tree(&mut runner).unwrap();

        assert_eq!(
            (
                0,
                vec![
                    (
                        Operation {
                            info: OperationInfo {
                                name: "get_persons".to_string(),
                                key: "operation/get_persons".to_string()
                            },
                            parameters: vec![Parameter {
                                name: "name".to_string(),
                                schema: String,
                                required: true,
                                ownership: ParameterOwnership::Owned,
                                meta_data: None
                            }],
                            responses: vec![Response {
                                name: "successful operation".to_string(),
                                schema: Schema::ArrayOfRefItems("person".to_string())
                            }],
                            meta_data: None
                        },
                        [
                            StringValue {
                                value: "jieewpsmyqg".to_string(),
                                seed: 5,
                                active: true
                            },
                            Empty,
                            Empty,
                            Empty,
                            Empty,
                            Empty,
                            Empty,
                            Empty,
                            Empty,
                            Empty
                        ]
                    ),
                    (
                        Operation {
                            info: OperationInfo {
                                name: "get_persons".to_string(),
                                key: "operation/get_persons".to_string()
                            },
                            parameters: vec![Parameter {
                                name: "name".to_string(),
                                schema: String,
                                required: true,
                                ownership: ParameterOwnership::Owned,
                                meta_data: None
                            }],
                            responses: vec![Response {
                                name: "successful operation".to_string(),
                                schema: Schema::ArrayOfRefItems("person".to_string())
                            }],
                            meta_data: None
                        },
                        [
                            ParameterValue::Reference {
                                active: true,
                                idx: [0, 0],
                                fallback: Box::new(StringValue {
                                    value: "nlmtcpckwygdgbgr".to_string(),
                                    seed: 8,
                                    active: true
                                }),
                                relation: amos_relations::Relation::Parameter(
                                    amos_relations::RelationInfo {
                                        operation: "get_persons".to_string(),
                                        name: "name".to_string(),
                                        schema: Schema::String,
                                        strength: 1,
                                        op_idx: 0,
                                        idx: 0
                                    }
                                )
                            },
                            Empty,
                            Empty,
                            Empty,
                            Empty,
                            Empty,
                            Empty,
                            Empty,
                            Empty,
                            Empty
                        ]
                    ),
                ]
            ),
            value.current()
        )
    }

    #[test]
    fn gen_operation_sequence_test() {
        let mut runner = create_runner();

        let get_persons_op = Operation {
            info: OperationInfo {
                name: "get_persons".to_string(),
                key: "operation/get_persons".to_string(),
            },
            parameters: vec![],
            responses: vec![Response {
                name: "successful operation".to_string(),
                schema: Schema::ArrayOfRefItems("person".to_string()),
            }],
            meta_data: None,
        };

        let delete_person_op = Operation {
            info: OperationInfo {
                name: "delete_person".to_string(),
                key: "operation/delete_person".to_string(),
            },
            parameters: vec![Parameter {
                name: "name".to_string(),
                schema: Schema::String,
                required: true,
                ownership: ParameterOwnership::Owned,
                meta_data: None,
            }],
            responses: vec![Response {
                name: "successful operation".to_string(),
                schema: Schema::Reference("person".to_string()),
            }],
            meta_data: None,
        };

        let post_person_op = Operation {
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

        let operations = vec![get_persons_op, post_person_op, delete_person_op];

        let gen = gen_operation_sequence(operations);

        let value = gen.new_tree(&mut runner).unwrap();

        assert_eq!(
            vec![(
                Operation {
                    info: OperationInfo {
                        name: "post_person".to_string(),
                        key: "operation/post_person".to_string(),
                    },
                    parameters: vec![
                        Parameter {
                            name: "name".to_string(),
                            schema: String,
                            required: true,
                            ownership: ParameterOwnership::Owned,
                            meta_data: None
                        },
                        Parameter {
                            name: "age".to_string(),
                            schema: Int8,
                            required: true,
                            ownership: ParameterOwnership::Owned,
                            meta_data: None
                        }
                    ],
                    responses: vec![Response {
                        name: "successful operation".to_string(),
                        schema: Schema::Reference("person".to_string()),
                    }],
                    meta_data: None,
                },
                [
                    StringValue {
                        value: "tieewpsmyqgdnlmtcpc".to_string(),
                        seed: 1,
                        active: true
                    },
                    IntValue {
                        value: 188,
                        seed: 7,
                        active: true
                    },
                    ParameterValue::Empty,
                    ParameterValue::Empty,
                    ParameterValue::Empty,
                    ParameterValue::Empty,
                    ParameterValue::Empty,
                    ParameterValue::Empty,
                    ParameterValue::Empty,
                    ParameterValue::Empty,
                ],
            )],
            value.current()
        );
    }

    #[test]
    fn gen_operation_sequence_with_pinned_test() {
        let mut runner = create_runner();

        let get_persons_op = Operation {
            info: OperationInfo {
                name: "get_persons".to_string(),
                key: "operation/get_persons".to_string(),
            },
            parameters: vec![],
            responses: vec![Response {
                name: "successful operation".to_string(),
                schema: Schema::ArrayOfRefItems("person".to_string()),
            }],
            meta_data: None,
        };

        let delete_person_op = Operation {
            info: OperationInfo {
                name: "delete_person".to_string(),
                key: "operation/delete_person".to_string(),
            },
            parameters: vec![Parameter {
                name: "name".to_string(),
                schema: Schema::String,
                required: true,
                ownership: ParameterOwnership::Owned,
                meta_data: None,
            }],
            responses: vec![Response {
                name: "successful operation".to_string(),
                schema: Schema::Reference("person".to_string()),
            }],
            meta_data: None,
        };

        let post_person_op = Operation {
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

        let pinned_op = Operation {
            info: OperationInfo {
                name: "PINNED".to_string(),
                key: "operation/PINNED".to_string(),
            },
            parameters: vec![Parameter {
                name: "name".to_string(),
                schema: Schema::String,
                required: true,
                ownership: ParameterOwnership::Owned,
                meta_data: None,
            }],
            responses: vec![Response {
                name: "successful operation".to_string(),
                schema: Schema::Reference("person".to_string()),
            }],
            meta_data: None,
        };

        let operations = vec![get_persons_op, post_person_op, delete_person_op];

        let gen = gen_operation_sequence_with_pinned(pinned_op, operations, 1, 4);

        let value = gen.new_tree(&mut runner).unwrap();

        assert_eq!(
            vec![
                (
                    Operation {
                        info: OperationInfo {
                            name: "PINNED".to_string(),
                            key: "operation/PINNED".to_string()
                        },
                        parameters: vec![Parameter {
                            name: "name".to_string(),
                            schema: String,
                            required: true,
                            ownership: ParameterOwnership::Owned,
                            meta_data: None
                        }],
                        responses: vec![Response {
                            name: "successful operation".to_string(),
                            schema: Reference("person".to_string())
                        }],
                        meta_data: None
                    },
                    [
                        StringValue {
                            value: "jieewpsmyqg".to_string(),
                            seed: 5,
                            active: true,
                        },
                        Empty,
                        Empty,
                        Empty,
                        Empty,
                        Empty,
                        Empty,
                        Empty,
                        Empty,
                        Empty
                    ]
                ),
                (
                    Operation {
                        info: OperationInfo {
                            name: "post_person".to_string(),
                            key: "operation/post_person".to_string()
                        },
                        parameters: vec![
                            Parameter {
                                name: "name".to_string(),
                                schema: String,
                                required: true,
                                ownership: ParameterOwnership::Owned,
                                meta_data: None
                            },
                            Parameter {
                                name: "age".to_string(),
                                schema: Int8,
                                required: true,
                                ownership: ParameterOwnership::Owned,
                                meta_data: None
                            }
                        ],
                        responses: vec![Response {
                            name: "successful operation".to_string(),
                            schema: Reference("person".to_string())
                        }],
                        meta_data: None
                    },
                    [
                        StringValue {
                            value: "nlmtcpckwygdgbgr".to_string(),
                            seed: 8,
                            active: true
                        },
                        IntValue {
                            value: 219,
                            seed: 9,
                            active: true
                        },
                        Empty,
                        Empty,
                        Empty,
                        Empty,
                        Empty,
                        Empty,
                        Empty,
                        Empty
                    ]
                ),
                (
                    Operation {
                        info: OperationInfo {
                            name: "delete_person".to_string(),
                            key: "operation/delete_person".to_string()
                        },
                        parameters: vec![Parameter {
                            name: "name".to_string(),
                            schema: String,
                            required: true,
                            ownership: ParameterOwnership::Owned,
                            meta_data: None
                        }],
                        responses: vec![Response {
                            name: "successful operation".to_string(),
                            schema: Reference("person".to_string())
                        }],
                        meta_data: None
                    },
                    [
                        StringValue {
                            value: "yjikjbcdoodwzochuzvbaqyrearvwqp".to_string(),
                            seed: 6,
                            active: true
                        },
                        Empty,
                        Empty,
                        Empty,
                        Empty,
                        Empty,
                        Empty,
                        Empty,
                        Empty,
                        Empty
                    ]
                ),
                (
                    Operation {
                        info: OperationInfo {
                            name: "post_person".to_string(),
                            key: "operation/post_person".to_string()
                        },
                        parameters: vec![
                            Parameter {
                                name: "name".to_string(),
                                schema: String,
                                required: true,
                                ownership: ParameterOwnership::Owned,
                                meta_data: None
                            },
                            Parameter {
                                name: "age".to_string(),
                                schema: Int8,
                                required: true,
                                ownership: ParameterOwnership::Owned,
                                meta_data: None
                            }
                        ],
                        responses: vec![Response {
                            name: "successful operation".to_string(),
                            schema: Reference("person".to_string())
                        }],
                        meta_data: None
                    },
                    [
                        StringValue {
                            value: "".to_string(),
                            seed: 1,
                            active: true
                        },
                        IntValue {
                            value: 0,
                            seed: 3,
                            active: true
                        },
                        Empty,
                        Empty,
                        Empty,
                        Empty,
                        Empty,
                        Empty,
                        Empty,
                        Empty
                    ]
                )
            ],
            value.current()
        );
    }
}
