use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::amos::{Operation, Parameter, Schema};
use crate::amos_generation::ParameterValue;

pub type GenerationOperationWithParameters = (Operation, [ParameterValue; 10]);

fn word_contains(a: &[String], b: &[String]) -> Vec<String> {
    let mut matches = vec![];
    for a_word in a {
        for b_word in b {
            //println!("Do {} contain {}", b_word, a_word);
            if b_word.to_lowercase().contains(&a_word.to_lowercase()) {
                matches.push(b_word.clone());
            }
        }
    }

    matches
}

fn camel_split(s: &str) -> Vec<String> {
    let mut words = vec![];

    let mut part = "".to_string();
    for c in s.chars() {
        if c.is_uppercase() {
            words.push(part.clone());
            part.clear();
        }

        part.push(c);
    }

    // add the last part
    words.push(part);

    words
}

#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub struct RelationInfo {
    pub operation: String,
    pub name: String,
    pub schema: Schema,
    pub strength: u8,
    pub op_idx: usize,
    pub idx: usize,
}

#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum Relation {
    Parameter(RelationInfo),
    Response(RelationInfo),
}

pub fn related_parameters(
    operations: &[GenerationOperationWithParameters],
    param: &Parameter,
) -> Vec<Relation> {
    let mut candidate_relations = vec![];

    let camel_param = camel_split(&param.name);

    for (op_idx, (o, _op_params)) in operations.iter().enumerate() {
        let camel_operation = camel_split(&o.info.name);

        for (param_idx, p) in o.parameters.iter().enumerate() {
            // 1. Schema match
            //  - find the parameters and responses that match the schema or sub-schema
            if p.schema == param.schema {
                // 2. name match
                if p.name == param.name {
                    candidate_relations.push(Relation::Parameter(RelationInfo {
                        operation: o.info.name.clone(),
                        name: p.name.clone(),
                        schema: p.schema.clone(),
                        strength: camel_param.len() as u8,
                        op_idx,
                        idx: param_idx,
                    }));
                } else {
                    // partial match on param?
                    let camel_op_param = camel_split(&p.name);
                    let matches = word_contains(&camel_param, &camel_op_param);

                    if !matches.is_empty() {
                        candidate_relations.push(Relation::Parameter(RelationInfo {
                            operation: o.info.name.clone(),
                            name: p.name.clone(),
                            schema: p.schema.clone(),
                            strength: matches.len() as u8,
                            op_idx,
                            idx: param_idx,
                        }))
                    }
                }
            }
        }

        for (r_idx, r) in o.responses.iter().enumerate() {
            // Does the operation name relate?
            let matches = word_contains(&camel_param, &camel_operation);
            debug!("Matching: {:#?} and {:#?}", camel_param, camel_operation);
            debug!("Matches: {:#?}", matches);
            debug!("Op: {}", o.info.name);
            debug!("Schema: {:#?}", r.schema);
            if !matches.is_empty() {
                // check the schema
                let schema_match = match r.schema {
                    Schema::ArrayOfString => param.schema == Schema::String,
                    _ => false,
                };

                debug!("Schema match: {:?}", schema_match);
                if schema_match {
                    candidate_relations.push(Relation::Response(RelationInfo {
                        operation: o.info.name.clone(),
                        name: r.name.clone(),
                        schema: r.schema.clone(),
                        strength: matches.len() as u8,
                        op_idx,
                        idx: r_idx,
                    }))
                }
            }
        }
    }

    candidate_relations
}
