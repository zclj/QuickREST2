use std::collections::HashMap;

use qr_http_resource::http::HTTPCall;
use qr_http_resource::http::HTTPParameters;
use serde_json::Value;
use tracing::debug;
use tracing::error;
use tracing::warn;

use crate::amos::InvokeResult;
use crate::amos::Operation;
use crate::amos::OperationMetaData;
use crate::amos::Parameter;
use crate::amos::ParameterMetaData;
use crate::amos::Schema;
use crate::amos_generation::GeneratedOperation;
use crate::amos_generation::{GeneratedParameter, ParameterValue};
use crate::amos_relations::Relation;
use crate::amos_relations::RelationInfo;
use qr_http_resource::http::{HTTPConfiguration, HTTPParameterTarget};

pub fn parameters_to_form_data(params: &[GeneratedParameter]) -> HashMap<String, String> {
    let mut form_data = HashMap::new();

    for p in params {
        match &p.value {
            ParameterValue::StringValue { value, .. } => {
                form_data.insert(p.name.clone(), value.to_owned());
            }
            ParameterValue::IntValue { value, .. } => {
                form_data.insert(p.name.clone(), value.to_string());
            }
            ParameterValue::File { value, .. } => {
                form_data.insert(p.name.clone(), value.to_string());
            }
            _ => panic!("Unsupported parameter to form data conversion: {p:#?}"),
        }
    }

    form_data
}

pub fn parameters_to_json_str(params: &[GeneratedParameter]) -> String {
    let mut json_str = "{".to_owned();

    for p in params {
        json_str.push('"');
        json_str.push_str(&p.name);
        json_str.push('"');
        json_str.push(':');
        match &p.value {
            ParameterValue::StringValue { value, .. } => {
                json_str.push('"');
                json_str.push_str(value);
                json_str.push('"');
            }
            ParameterValue::IntValue { value, .. } => {
                json_str.push_str(&value.to_string());
            }
            _ => panic!("Unsupported parameter to json conversion: {p:#?}"),
        }
    }

    json_str.push('}');
    json_str
}

fn parse_response(
    param: &Parameter,
    result: &InvokeResult,
    info: &RelationInfo,
    fallback: &ParameterValue,
) -> ParameterValue {
    debug!("Parameter to parse into: {param:#?}");
    debug!("RelationInfo: {info:#?}");
    // TODO: match on the actual stated response type

    debug!("Result: {:#?}", result);
    if !result.success {
        debug!("Refered response was unsuccessfull, using fallback value");
        return fallback.clone();
    }
    // TODO: would be nice if we could leverage the response schema
    let content: Value =
        serde_json::from_str(&result.result).expect("Could not parse result as JSON");

    debug!("JSON: {content:#?}");

    match content {
        Value::Array(ref items) => {
            if items.is_empty() {
                return fallback.clone();
            }
            // TODO: use a seed to select value
            let candidate = &items[0];

            match candidate {
                Value::String(s) => {
                    if param.schema == Schema::String {
                        return ParameterValue::StringValue {
                            value: s.clone(),
                            seed: 0,
                            active: false,
                        };
                    }
                }
                _ => panic!("Unsupported JSON value"),
            }
        }
        _ => panic!("Unsupported content"),
    }

    fallback.clone()
}

pub fn translate_generated_operation_to_http_call(
    config: &HTTPConfiguration,
    ops: &[Operation],
    gen_op: &GeneratedOperation,
    results: &[InvokeResult],
) -> Option<(HTTPCall, String)> {
    // TODO: Fix this meta crap
    let matching_op = ops.iter().find(|op| op.info.name == gen_op.name);

    let amos_op = matching_op.unwrap();
    let op_meta = amos_op.meta_data.clone();

    let http_operation = translate_operation(config, gen_op, &op_meta, amos_op, results)?;
    debug!(?http_operation);

    // TODO: This should not be nessesary
    let url = http_operation.url.clone();
    Some((http_operation, url))
}

pub fn translate_operation(
    config: &HTTPConfiguration,
    gen_op: &GeneratedOperation,
    op_meta: &Option<OperationMetaData>,
    amos_op: &Operation,
    results: &[InvokeResult],
) -> Option<HTTPCall> {
    match op_meta.clone().unwrap() {
        OperationMetaData::HTTP { url, method } => {
            if let Some(call) =
                translate_parameters(&gen_op.parameters, &amos_op.parameters, results, &url)
            {
                let url = format!(
                    "{}{}:{}{}",
                    config.protocol, config.base_url, config.port, call.url
                );
                Some(HTTPCall {
                    url,
                    method,
                    parameters: call,
                })
            } else {
                // Could not create a valid URL, consider the SEQ as broken
                warn!(gen_op.name, "Disscarded operation");
                None
            }
        }
    }
}

pub fn translate_parameters(
    params: &[GeneratedParameter],
    amos_params: &[Parameter],
    results: &[InvokeResult],
    url: &str,
) -> Option<HTTPParameters> {
    let mut translated_url = url.to_owned();

    // sort up the params that will go into the path and into the body
    let mut form_params = vec![];
    let mut query_params = vec![];
    let mut body_params = vec![];
    let mut file_params = vec![];

    // TODO: make this nicer..
    for p in params {
        let amos_param = amos_params.iter().find(|ap| ap.name == p.name);
        debug!("AMOS param: {:#?}", amos_param);
        match amos_param.unwrap().meta_data.clone().unwrap() {
            ParameterMetaData::HTTP { target } => match target {
                HTTPParameterTarget::Body => {
                    debug!("Body param: {:?}", p);
                    match &p.value {
                        ParameterValue::Reference{
                            fallback,
                            relation,
                            ..
                        } => {
                            match relation {
                                Relation::Response(info) => {
                                    let ref_result = &results[info.op_idx];
                                    let ref_parameter = parse_response(amos_param.unwrap(), ref_result, info, fallback);
                                    body_params.push(
                                        GeneratedParameter { name: p.name.clone(), value: ref_parameter, ref_path: p.ref_path.clone() }
                                    )
                                }
                                Relation::Parameter(info) => panic!("Parameter relations should be resolved before runtime translation. Offending reference: {info:#?}")
                            }
                        }
                        _ => body_params.push(p.clone())
                    }
                }
                HTTPParameterTarget::Path => {
                    debug!("Parameter value: {0:#?}", p.value);
                    match &p.value {
                        ParameterValue::StringValue { value, .. } => {
                            // If any path string parameter is empty (""), we cannot build
                            //  a valid URL
                            if value.is_empty() {
                                return None;
                            }

                            translated_url =
                                translated_url.replace(&("{".to_owned() + &p.name + "}"), value)
                        }
                        ParameterValue::BoolValue { value, .. } => {
                            translated_url = translated_url
                                .replace(&("{".to_owned() + &p.name + "}"), &value.to_string())
                        }
                        ParameterValue::DoubleValue { value, .. } => {
                            translated_url = translated_url
                                .replace(&("{".to_owned() + &p.name + "}"), &value.to_string())
                        }
                        ParameterValue::ArrayOfString { value, .. } => {
                            translated_url = translated_url
                                .replace(&("{".to_owned() + &p.name + "}"), &value.join(","))
                        }
                        ParameterValue::IntValue { value, .. } => {
                            translated_url = translated_url
                                .replace(&("{".to_owned() + &p.name + "}"), &value.to_string())
                        }
                        ParameterValue::IPV4Value { value, .. } => {
                            let (a, b, c, d) = value;
                            let ip_str = format!("{}.{}.{}.{}", a, b, c, d);
                            translated_url =
                                translated_url.replace(&("{".to_owned() + &p.name + "}"), &ip_str)
                        }
                        ParameterValue::Empty => {
                            return None;
                        }
                        ParameterValue::Reference {
                            fallback, relation, ..
                        } => {
                            // a reference at this point means a reference to a response,
                            //  a ref to a parameter should have been resolved already
                            match relation {
                                Relation::Response(info) => {
                                    debug!("translate response reference: {info:#?}");
                                    // a reference to a response, is a ref to a value in
                                    //  the 'results'
                                    let ref_result = &results[info.op_idx];
                                    //info!("Find a ref value from: {ref_result:#?}");
                                    let ref_parameter = parse_response(amos_param.unwrap(), ref_result, info, fallback);
                                    match &ref_parameter {
                                       ParameterValue::StringValue { value, .. } => {
                                           // If any path string parameter is empty (""), we cannot build
                                           //  a valid URL
                                           if value.is_empty() {
                                               return None;
                                           }

                                           translated_url =
                                               translated_url.replace(&("{".to_owned() + &p.name + "}"), value)
                                       }
                                        ParameterValue::IntValue { value, .. } => {
                                            translated_url = translated_url
                                                .replace(&("{".to_owned() + &p.name + "}"), &value.to_string())
                                        },
                                        _ => panic!("work todo")
                                    }
                                },
                                Relation::Parameter(info) => panic!("Parameter relations should be resolved before runtime translation. Offending reference: {info:#?}")
                            }
                        }
                        _ => panic!("Unsupported parameter value: {:#?}", p),
                    }
                }
                HTTPParameterTarget::FormData => {
                    debug!("Parameter with FormData");
                    match &p.value {
                        ParameterValue::Reference{
                            fallback,
                            relation,
                            ..
                        } => {
                            match relation {
                                Relation::Response(info) => {
                                    let ref_result = &results[info.op_idx];
                                    let ref_parameter = parse_response(amos_param.unwrap(), ref_result, info, fallback);
                                    form_params.push(
                                        GeneratedParameter { name: p.name.clone(), value: ref_parameter, ref_path: p.ref_path.clone() }
                                    )
                                }
                                Relation::Parameter(info) => panic!("Parameter relations should be resolved before runtime translation. Offending reference: {info:#?}")
                            }
                        }
                        ParameterValue::File {..} => {
                            file_params.push(p.clone())
                        }
                        _ => form_params.push(p.clone())
                    }
                }
                HTTPParameterTarget::Query => {
                    match &p.value {
                        ParameterValue::Reference {
                            fallback, relation, ..
                        } => {
                            // a reference at this point means a reference to a response,
                            //  a ref to a parameter should have been resolved already
                            match relation {
                                Relation::Response(info) => {
                                    debug!("translate response reference: {info:#?}");
                                    // a reference to a response, is a ref to a value in
                                    //  the 'results'
                                    let ref_result = &results[info.op_idx];
                                    //info!("Find a ref value from: {ref_result:#?}");
                                    let ref_parameter = parse_response(amos_param.unwrap(), ref_result, info, fallback);
                                    match &ref_parameter {
                                       ParameterValue::StringValue { value, .. } => {
                                           // If any path string parameter is empty (""), we cannot build
                                           //  a valid URL
                                           if value.is_empty() {
                                               return None;
                                           }

                                           translated_url =
                                               translated_url.replace(&("{".to_owned() + &p.name + "}"), value)
                                       }
                                        ParameterValue::IntValue { value, .. } => {
                                            translated_url = translated_url
                                                .replace(&("{".to_owned() + &p.name + "}"), &value.to_string())
                                        },
                                        _ => panic!("work todo")
                                    }
                                },
                                Relation::Parameter(info) => panic!("Parameter relations should be resolved before runtime translation. Offending reference: {info:#?}")
                            }
                        }
                        _ => match &p.value {
                            ParameterValue::StringValue { value, .. } => {
                                query_params.push(format!("{}={}", p.name, value))
                            }
                            ParameterValue::IntValue { value, .. } => {
                                query_params.push(format!("{}={}", p.name, &value.to_string()))
                            }
                            _ => todo!(),
                        },
                    }
                }
                HTTPParameterTarget::Unsupported => {
                    error!("Unsupported HTTP Parameter target")
                }
            },
        };
    }

    let form_data = if form_params.is_empty() {
        None
    } else {
        Some(parameters_to_form_data(&form_params))
    };

    let body_data = if body_params.is_empty() {
        None
    } else {
        Some(parameters_to_form_data(&body_params))
    };

    let file_data = if file_params.is_empty() {
        None
    } else {
        // TODO: this could be specialized and warn on non-file type
        Some(parameters_to_form_data(&file_params))
    };

    let translated_url = if query_params.is_empty() {
        translated_url
    } else {
        let params = query_params.join("&");
        translated_url.push('?');
        translated_url.push_str(&params);
        translated_url
    };

    Some(HTTPParameters {
        url: translated_url,
        form_data,
        body: body_data,
        file_data,
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::amos::OperationMetaData::HTTP;
    use crate::amos::{Parameter, ParameterMetaData, ParameterOwnership, Schema};
    use crate::amos_generation::ParameterValue;
    use crate::amos_generation::{GeneratedOperation, GeneratedParameter};
    use crate::http_translation::{parameters_to_json_str, translate_parameters};

    use qr_http_resource::http::HTTPMethod::DELETE;
    use qr_http_resource::http::HTTPParameterTarget::{Body, FormData, Path, Query};

    #[test]
    fn parameters_to_json_str_conversion() {
        let gen_op = GeneratedOperation {
            name: "deleteFeature".to_string(),
            parameters: vec![
                GeneratedParameter {
                    name: "productName".to_string(),
                    value: ParameterValue::StringValue {
                        value: "foo".to_string(),
                        seed: 1,
                        active: true,
                    },
                    ref_path: None,
                },
                GeneratedParameter {
                    name: "configurationName".to_string(),
                    value: ParameterValue::IntValue {
                        value: 123,
                        seed: 2,
                        active: true,
                    },
                    ref_path: None,
                },
            ],
        };

        let result = parameters_to_json_str(&gen_op.parameters);

        assert_eq!("{\"productName\":\"foo\"\"configurationName\":123}", result)
    }

    #[test]
    fn translate_params_form_data() {
        // Generated operation with parameters
        let gen_op = GeneratedOperation {
            name: "deleteFeature".to_string(),
            parameters: vec![
                GeneratedParameter {
                    name: "productName".to_string(),
                    value: ParameterValue::StringValue {
                        value: "foo".to_string(),
                        seed: 1,
                        active: true,
                    },
                    ref_path: None,
                },
                GeneratedParameter {
                    name: "configurationName".to_string(),
                    value: ParameterValue::IntValue {
                        value: 123,
                        seed: 2,
                        active: true,
                    },
                    ref_path: None,
                },
            ],
        };

        // AMOS of the generated operation
        let amos_params = vec![
            Parameter {
                name: "productName".to_string(),
                schema: Schema::String,
                required: true,
                ownership: ParameterOwnership::Owned,
                meta_data: Some(ParameterMetaData::HTTP { target: FormData }),
            },
            Parameter {
                name: "configurationName".to_string(),
                schema: Schema::Int,
                required: true,
                ownership: ParameterOwnership::Owned,
                meta_data: Some(ParameterMetaData::HTTP { target: Path }),
            },
        ];

        let translation = translate_parameters(
            &gen_op.parameters,
            &amos_params,
            &[],
            "/configurations/{configurationName}",
        );

        let url = translation.as_ref().unwrap().url.clone();
        let form = translation.unwrap().form_data.unwrap();

        assert_eq!(url, "/configurations/123");

        assert_eq!(
            form,
            HashMap::from([("productName".to_owned(), "foo".to_owned())])
        )
    }

    #[test]
    fn translate_params_path() {
        // Generated operation with parameters
        let gen_op = GeneratedOperation {
            name: "deleteFeature".to_string(),
            parameters: vec![
                GeneratedParameter {
                    name: "productName".to_string(),
                    value: ParameterValue::StringValue {
                        value: "foo".to_string(),
                        seed: 1,
                        active: true,
                    },
                    ref_path: None,
                },
                GeneratedParameter {
                    name: "configurationName".to_string(),
                    value: ParameterValue::StringValue {
                        value: "bar".to_string(),
                        seed: 2,
                        active: true,
                    },
                    ref_path: None,
                },
            ],
        };

        let _op_meta = Some(HTTP {
            url: "/products/{productName}/configurations/{configurationName}/features/".to_string(),
            method: DELETE,
        });

        // AMOS of the generated operation
        let amos_params = vec![
            Parameter {
                name: "productName".to_string(),
                schema: Schema::String,
                required: true,
                ownership: ParameterOwnership::Owned,
                meta_data: Some(ParameterMetaData::HTTP { target: Path }),
            },
            Parameter {
                name: "configurationName".to_string(),
                schema: Schema::String,
                required: true,
                ownership: ParameterOwnership::Owned,
                meta_data: Some(ParameterMetaData::HTTP { target: Path }),
            },
        ];

        let translation = translate_parameters(
            &gen_op.parameters,
            &amos_params,
            &[],
            "/products/{productName}/configurations/{configurationName}/features/",
        );
        // build the url and body
        assert_eq!(
            translation.unwrap().url,
            "/products/foo/configurations/bar/features/"
        )
    }

    #[test]
    fn translate_query_params() {
        // Generated operation with parameters
        let gen_op = GeneratedOperation {
            name: "deleteFeature".to_string(),
            parameters: vec![
                GeneratedParameter {
                    name: "productName".to_string(),
                    value: ParameterValue::StringValue {
                        value: "foo".to_string(),
                        seed: 1,
                        active: true,
                    },
                    ref_path: None,
                },
                GeneratedParameter {
                    name: "configurationName".to_string(),
                    value: ParameterValue::IntValue {
                        value: 123,
                        seed: 2,
                        active: true,
                    },
                    ref_path: None,
                },
            ],
        };

        // AMOS of the generated operation
        let amos_params = vec![
            Parameter {
                name: "productName".to_string(),
                schema: Schema::String,
                required: true,
                ownership: ParameterOwnership::Owned,
                meta_data: Some(ParameterMetaData::HTTP { target: Query }),
            },
            Parameter {
                name: "configurationName".to_string(),
                schema: Schema::Int,
                required: true,
                ownership: ParameterOwnership::Owned,
                meta_data: Some(ParameterMetaData::HTTP { target: Query }),
            },
        ];

        let translation =
            translate_parameters(&gen_op.parameters, &amos_params, &[], "/configurations");

        let url = translation.as_ref().unwrap().url.clone();
        //let form = translation.unwrap().form_data.unwrap();

        assert_eq!(url, "/configurations?productName=foo&configurationName=123");

        // assert_eq!(
        //     form,
        //     HashMap::from([("productName".to_owned(), "foo".to_owned())])
        // )
    }

    #[test]
    fn translate_body_params() {
        // Generated operation with parameters
        let gen_op = GeneratedOperation {
            name: "deleteFeature".to_string(),
            parameters: vec![
                GeneratedParameter {
                    name: "productName".to_string(),
                    value: ParameterValue::StringValue {
                        value: "foo".to_string(),
                        seed: 1,
                        active: true,
                    },
                    ref_path: None,
                },
                GeneratedParameter {
                    name: "configurationName".to_string(),
                    value: ParameterValue::IntValue {
                        value: 123,
                        seed: 2,
                        active: true,
                    },
                    ref_path: None,
                },
            ],
        };

        // AMOS of the generated operation
        let amos_params = vec![
            Parameter {
                name: "productName".to_string(),
                schema: Schema::String,
                required: true,
                ownership: ParameterOwnership::Owned,
                meta_data: Some(ParameterMetaData::HTTP { target: Body }),
            },
            Parameter {
                name: "configurationName".to_string(),
                schema: Schema::Int,
                required: true,
                ownership: ParameterOwnership::Owned,
                meta_data: Some(ParameterMetaData::HTTP { target: Body }),
            },
        ];

        let translation =
            translate_parameters(&gen_op.parameters, &amos_params, &[], "/configurations");

        let url = translation.as_ref().unwrap().url.clone();
        let body = translation.unwrap().body.unwrap();

        assert_eq!(url, "/configurations");

        assert_eq!(
            body,
            HashMap::from([
                ("configurationName".to_owned(), "123".to_owned()),
                ("productName".to_owned(), "foo".to_owned())
            ])
        )
    }
}
