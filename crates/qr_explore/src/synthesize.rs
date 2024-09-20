use tracing::debug;

use crate::{
    amos::Operation,
    amos_generation::{GeneratedOperation, GeneratedParameter, ParameterValue},
    amos_relations::Relation,
};

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
