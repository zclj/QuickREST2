use crate::amos_generation;
use qr_http_resource::http::HTTPParameterTarget;
use qr_http_resource::http::{HTTPMethod, HTTPStatus};
use qr_open_api::open_api::DataType;
use qr_open_api::open_api::{
    DataType as OpenAPIDataType, Definition as OpenAPIDefinition, DefinitionKind,
    Operation as OpenAPIOperation, OperationParameter, OperationResponse, ParameterIn,
    Property as OpenAPIProperty, Schema as OpenAPISchema,
};
use serde;
use serde_json;
use tracing::error;

pub type Result<T> = core::result::Result<T, Error>;

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub enum Error {
    LoadFileFailure,
}

#[derive(PartialEq, Debug, Clone)]
pub enum ResultMetaData {
    HTTP { url: String, status: HTTPStatus },
}

/// Results of invoking AMOS operations
// TODO: should probably have a way of expressing a failed result
// TODO: Fix the cycle between amos/amos_generation
#[derive(PartialEq, Debug, Clone)]
pub struct InvokeResult {
    pub operation: amos_generation::GeneratedOperation,
    pub result: String,
    pub success: bool,
    pub meta_data: Option<ResultMetaData>,
}

impl InvokeResult {
    pub fn new(
        operation: amos_generation::GeneratedOperation,
        result: String,
        success: bool,
        meta_data: Option<ResultMetaData>,
    ) -> Self {
        InvokeResult {
            operation,
            result,
            success,
            meta_data,
        }
    }
}

#[derive(Debug, PartialEq, Clone, serde::Deserialize, serde::Serialize)]
pub struct Property {
    pub name: String,
    pub schema: Schema,
}

#[derive(Debug, PartialEq, Clone, serde::Deserialize, serde::Serialize)]
pub enum Schema {
    Reference(String),
    Object { properties: Vec<Property> },
    //Array,
    ArrayOfUniqueRefItems(String),
    ArrayOfString,
    ArrayOfRefItems(String),
    DateTime,
    IPV4,
    String,
    StringNonEmpty,
    StringDateTime,
    StringRegex { regex: String },
    Number,
    Double,
    Float,
    Int,
    Int8,
    Int32,
    Bool,
    File,
    Unsupported,
}

impl std::fmt::Display for Schema {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Debug, PartialEq, Clone, serde::Deserialize, serde::Serialize)]
pub enum ParameterMetaData {
    HTTP { target: HTTPParameterTarget },
}

#[derive(Debug, PartialEq, Clone, serde::Deserialize, serde::Serialize)]
pub enum ParameterOwnership {
    Owned,
    Dependency,
    Unknown,
}

#[derive(Debug, PartialEq, Clone, serde::Deserialize, serde::Serialize)]
pub struct Parameter {
    pub name: String,
    pub schema: Schema,
    pub required: bool,
    pub ownership: ParameterOwnership,
    pub meta_data: Option<ParameterMetaData>,
}

#[derive(Debug, PartialEq, Clone, serde::Deserialize, serde::Serialize)]
pub struct Response {
    pub name: String,
    pub schema: Schema,
}

#[derive(Debug, PartialEq, Clone, serde::Deserialize, serde::Serialize)]
pub struct OperationInfo {
    pub name: String,
    // TODO: needed?
    pub key: String,
}

#[derive(Debug, PartialEq, Clone, serde::Deserialize, serde::Serialize)]
pub enum OperationMetaData {
    HTTP { url: String, method: HTTPMethod },
}

#[derive(Debug, PartialEq, Clone, serde::Deserialize, serde::Serialize)]
pub struct Operation {
    pub info: OperationInfo,
    pub parameters: Vec<Parameter>,
    pub responses: Vec<Response>,
    pub meta_data: Option<OperationMetaData>,
}

#[derive(Debug, PartialEq, serde::Deserialize, serde::Serialize, Clone)]
pub struct Definition {
    pub name: String,
    pub key: String,
    pub schema: Schema,
}

#[derive(Debug, PartialEq, serde::Deserialize, serde::Serialize, Clone)]
pub struct Domain {
    // TODO: prob a hash map
    pub data: String,
}

#[derive(Debug, PartialEq, serde::Deserialize, serde::Serialize, Clone)]
pub struct AMOS {
    pub name: String,
    pub domain: Domain,
    pub definitions: Vec<Definition>,
    pub operations: Vec<Operation>,
}

impl AMOS {
    pub fn new() -> Self {
        AMOS {
            name: "New AMOS".to_string(),
            domain: Domain {
                data: "TODO".to_string(),
            },
            definitions: vec![],
            operations: vec![],
        }
    }

    pub fn push_operation(&mut self, operation: Operation) {
        self.operations.push(operation)
    }

    pub fn find_operation(&self, name: &str) -> Option<&Operation> {
        self.operations.iter().find(|o| o.info.name == name)
    }

    pub fn find_definition(&self, name: &str) -> Option<&Definition> {
        self.definitions.iter().find(|d| d.name == name)
    }

    pub fn resolve_operation(&self, name: &str) -> Option<Operation> {
        if let Some(op) = self.find_operation(name) {
            let mut resolved = op.clone();

            let mut new_params = vec![];

            for param in resolved.parameters {
                match param.schema {
                    Schema::Reference(ref r) => {
                        if let Some(name) = r.split('/').last() {
                            if let Some(definition) = self.find_definition(name) {
                                // the definition contains an object/properties schema
                                //  which need to be resolved into a [properties] format
                                match &definition.schema {
                                    Schema::Object { properties } => {
                                        for def_prop in properties {
                                            new_params.push(Parameter {
                                                name: def_prop.name.clone(),
                                                schema: def_prop.schema.clone(),
                                                required: param.required,
                                                ownership: param.ownership.clone(),
                                                meta_data: param.meta_data.clone(),
                                            })
                                        }
                                    }
                                    _ => new_params.push(param.clone()),
                                }
                                //param.schema = definition.schema.clone();
                            }
                        }
                    }
                    _ => new_params.push(param),
                }
            }

            resolved.parameters = new_params;
            Some(resolved)
        } else {
            None
        }
    }

    // TODO: error handling
    pub fn save(&self, path: &std::path::Path) {
        let payload = serde_json::to_string_pretty(self);

        match payload {
            Ok(p) => {
                let _ = std::fs::write(path, p);
            }
            Err(e) => panic!("Failed to serialize: {}", e),
        }
    }

    // TODO: error handling
    pub fn load_or_default(path: &std::path::Path) -> Self {
        let content_result = std::fs::read(path);

        match content_result {
            Ok(content) => {
                let deserialize_result = serde_json::from_slice(&content);
                match deserialize_result {
                    Ok(amos) => amos,
                    Err(e) => {
                        tracing::error!("Could not load AMOS file: {}", e);
                        AMOS::new()
                    }
                }
            }
            Err(e) => {
                error!("Could not load AMOS: {}", e);
                AMOS::new()
            }
        }
    }

    pub fn load(path: &std::path::Path) -> Result<Self> {
        let content_result = std::fs::read(path);

        match content_result {
            Ok(content) => {
                let deserialize_result = serde_json::from_slice(&content);
                match deserialize_result {
                    Ok(amos) => Ok(amos),
                    Err(e) => {
                        tracing::error!("Could not load AMOS file: {}", e);
                        Err(Error::LoadFileFailure)
                    }
                }
            }
            Err(e) => {
                error!("Could not load AMOS: {}", e);
                Err(Error::LoadFileFailure)
            }
        }
    }
}

impl Default for AMOS {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
pub struct TranslationResult {
    pub amos: AMOS,
    pub warnings: Vec<TranslationMessage>,
    pub errors: Vec<TranslationMessage>,
}

#[derive(Debug)]
pub struct TranslationMessage {
    pub message: String,
}

impl TranslationMessage {
    pub fn new(message: String) -> Self {
        TranslationMessage { message }
    }
}

#[derive(Debug)]
struct TranslationContext {
    warnings: Vec<TranslationMessage>,
    errors: Vec<TranslationMessage>,
}

impl TranslationContext {
    pub fn new() -> Self {
        TranslationContext {
            warnings: vec![],
            errors: vec![],
        }
    }

    fn push_warning(&mut self, message: TranslationMessage) {
        self.warnings.push(message)
    }

    fn push_error(&mut self, message: TranslationMessage) {
        self.errors.push(message)
    }
}

fn definition_object_to_schema(
    ctx: &mut TranslationContext,
    properties: &[OpenAPIProperty],
) -> Schema {
    let props = properties
        .iter()
        .map(|p| match &p.kind {
            OpenAPIDataType::ArrayOfUniqueRefItems(refstr) => Property {
                name: p.name.clone(),
                schema: Schema::ArrayOfUniqueRefItems(refstr.clone()),
            },
            OpenAPIDataType::ArrayOfRefItems(refstr) => Property {
                name: p.name.clone(),
                schema: Schema::ArrayOfRefItems(refstr.clone()),
            },
            OpenAPIDataType::ArrayOfStrings => Property {
                name: p.name.clone(),
                schema: Schema::ArrayOfString,
            },
            OpenAPIDataType::File => Property {
                name: p.name.clone(),
                schema: Schema::String,
            },
            OpenAPIDataType::Integer64 => Property {
                name: p.name.clone(),
                schema: Schema::Int,
            },
            OpenAPIDataType::Integer32 => Property {
                name: p.name.clone(),
                schema: Schema::Int32,
            },
            OpenAPIDataType::String => Property {
                name: p.name.clone(),
                schema: Schema::String,
            },
            OpenAPIDataType::Boolean => Property {
                name: p.name.clone(),
                schema: Schema::Bool,
            },
            OpenAPIDataType::StringDateTime => Property {
                name: p.name.clone(),
                schema: Schema::StringDateTime,
            },
            OpenAPIDataType::Double => Property {
                name: p.name.clone(),
                schema: Schema::Double,
            },
            OpenAPIDataType::Float => Property {
                name: p.name.clone(),
                schema: Schema::Float,
            },
            OpenAPIDataType::Number => Property {
                name: p.name.clone(),
                schema: Schema::Number,
            },
            OpenAPIDataType::IPV4 => Property {
                name: p.name.clone(),
                schema: Schema::IPV4,
            },
            OpenAPIDataType::Schema(schema) => Property {
                name: p.name.clone(),
                schema: match_schema(ctx, schema),
            },
            OpenAPIDataType::Unsupported => {
                ctx.push_warning(TranslationMessage::new(format!(
                    "Unsupported Property kind: {:?} at {}:{}",
                    p.kind,
                    file!(),
                    line!(),
                )));

                Property {
                    name: p.name.clone(),
                    schema: Schema::Unsupported,
                }
            } //_ => todo!(),
        })
        .collect();

    Schema::Object { properties: props }
}

fn open_api_definition_to_amos(
    ctx: &mut TranslationContext,
    definition: &OpenAPIDefinition,
) -> Definition {
    let mut key = "definition/".to_owned();
    key.push_str(&definition.name);

    let schema = match definition.kind {
        DefinitionKind::Object => {
            if let Some(ps) = &definition.properties {
                definition_object_to_schema(ctx, ps)
            } else {
                Schema::Object { properties: vec![] }
            }
        }
        _ => todo!("Add definitions"),
    };

    Definition {
        name: definition.name.clone(),
        key,
        schema,
    }
}

fn match_schema(ctx: &mut TranslationContext, schema: &OpenAPISchema) -> Schema {
    match schema {
        OpenAPISchema::Ref(r) => Schema::Reference(r.clone()),
        OpenAPISchema::Inline { properties } => definition_object_to_schema(ctx, properties),

        OpenAPISchema::ArrayOfString => Schema::ArrayOfString,
        OpenAPISchema::ArrayOfUniqueRefItems(r) => Schema::ArrayOfUniqueRefItems(r.clone()),
        OpenAPISchema::ArrayOfRefItems(r) => Schema::ArrayOfRefItems(r.clone()),
    }
}

fn open_api_response_to_amos(
    ctx: &mut TranslationContext,
    response: &OperationResponse,
) -> Response {
    let schema = match (&response.status, &response.schema) {
        (HTTPStatus::OK, None) => Schema::Int,
        (HTTPStatus::OK, Some(s)) => match_data_type_to_schema(ctx, s, None),
        (HTTPStatus::Created, _) => Schema::Int,
        (HTTPStatus::NoContent, _) => Schema::Int,
        (HTTPStatus::BadRequest, _) => Schema::Int,
        (HTTPStatus::Unauthorized, _) => Schema::Int,
        (HTTPStatus::Forbidden, _) => Schema::Int,
        (HTTPStatus::MethodNotAllowed, _) => Schema::Int,
        (HTTPStatus::UnsupportedMediaType, _) => Schema::Int,
        (HTTPStatus::Default, _) => Schema::Int,
        (HTTPStatus::NotFound, _) => Schema::Int,
        (HTTPStatus::InternalServerError, _) => Schema::Int,
        (HTTPStatus::Unsupported, _) => {
            ctx.push_warning(TranslationMessage::new(format!(
                "Unsupported HTTP Status: {:?}",
                response.status
            )));
            Schema::Unsupported
        }
    };

    Response {
        name: response.description.clone(),
        schema,
    }
}

fn match_data_type_to_schema(
    ctx: &mut TranslationContext,
    data_type: &DataType,
    target: Option<HTTPParameterTarget>,
) -> Schema {
    match data_type {
        DataType::String => {
            // Empty strings are not allowed in URLs (result in '//').
            // Thus, strings in 'Path' should have a non-empty schema
            if let Some(t) = target {
                if t == HTTPParameterTarget::Path {
                    Schema::StringNonEmpty
                } else {
                    Schema::String
                }
            } else {
                Schema::String
            }
        }
        DataType::Number => Schema::Number,
        DataType::Float => Schema::Float,
        DataType::Double => Schema::Double,
        DataType::Integer64 => Schema::Int,
        DataType::Integer32 => Schema::Int32,
        DataType::Boolean => Schema::Bool,
        DataType::IPV4 => Schema::IPV4,
        DataType::File => {
            ctx.push_warning(TranslationMessage::new(format!(
                "Unsupported data kind: {:?}",
                data_type,
            )));
            Schema::Unsupported
        }

        DataType::Schema(s) => {
            match_schema(ctx, s)
            // ctx.push_warning(TranslationMessage::new(format!(
            //     "Unsupported data kind: {:?}",
            //     data_type,
            // )));
            // Schema::Unsupported
        }
        DataType::ArrayOfStrings => Schema::ArrayOfString,
        DataType::ArrayOfRefItems(s) => Schema::ArrayOfRefItems(s.clone()),
        DataType::ArrayOfUniqueRefItems(s) => Schema::ArrayOfUniqueRefItems(s.clone()),
        DataType::StringDateTime => Schema::DateTime,
        DataType::Unsupported => {
            ctx.push_warning(TranslationMessage::new(format!(
                "Unsupported data kind: {:?}",
                data_type,
            )));
            Schema::Unsupported
        } //_ => panic!("Unsupported parameter: {:?}", parameter.kind),
    }
}

fn open_api_parameter_to_amos(
    ctx: &mut TranslationContext,
    parameter: &OperationParameter,
    url: &str,
    url_parts: &[&str],
    method: &HTTPMethod,
) -> Parameter {
    let target = match parameter.target {
        ParameterIn::Path => HTTPParameterTarget::Path,
        ParameterIn::FormData => HTTPParameterTarget::FormData,
        ParameterIn::Query => HTTPParameterTarget::Query,
        ParameterIn::Body => HTTPParameterTarget::Body,
        _ => {
            ctx.push_error(TranslationMessage::new(format!(
                "Missing parameter support ({:?}) at: {}:{}",
                parameter.target,
                file!(),
                line!()
            )));
            HTTPParameterTarget::Unsupported
        }
    };

    let schema = match &parameter.kind {
        DataType::String => {
            // Empty strings are not allowed in URLs (result in '//').
            // Thus, strings in 'Path' should have a non-empty schema
            if target == HTTPParameterTarget::Path {
                Schema::StringNonEmpty
            } else {
                Schema::String
            }
        }
        DataType::Number => Schema::Number,
        DataType::Float => Schema::Float,
        DataType::Double => Schema::Double,
        DataType::Integer64 => Schema::Int,
        DataType::Integer32 => Schema::Int32,
        DataType::Boolean => Schema::Bool,
        DataType::IPV4 => Schema::IPV4,
        DataType::File => Schema::File,

        DataType::Schema(schema) => match schema {
            OpenAPISchema::ArrayOfString => Schema::ArrayOfString,
            OpenAPISchema::ArrayOfRefItems(r) => Schema::ArrayOfRefItems(r.to_string()),
            OpenAPISchema::ArrayOfUniqueRefItems(r) => Schema::ArrayOfUniqueRefItems(r.to_string()),
            OpenAPISchema::Inline { properties } => definition_object_to_schema(ctx, properties),
            OpenAPISchema::Ref(r) => Schema::Reference(r.to_string()),
        },
        DataType::ArrayOfStrings => Schema::ArrayOfString,
        DataType::ArrayOfRefItems(s) => Schema::ArrayOfRefItems(s.clone()),
        DataType::ArrayOfUniqueRefItems(s) => Schema::ArrayOfUniqueRefItems(s.clone()),
        DataType::StringDateTime => Schema::DateTime,
        DataType::Unsupported => {
            ctx.push_warning(TranslationMessage::new(format!(
                "Unsupported parameter kind: {:?}",
                parameter.kind,
            )));
            Schema::Unsupported
        } //_ => panic!("Unsupported parameter: {:?}", parameter.kind),
    };

    let meta_data = Some(ParameterMetaData::HTTP { target });

    let ownership = match method {
        HTTPMethod::POST | HTTPMethod::PUT => {
            match &parameter.target {
                ParameterIn::Path => {
                    // URL positions except the last is a dependecy, otherwise owned
                    let pos = url_parts
                        .iter()
                        .position(|part| *part == format!("{{{}}}", parameter.name));
                    if let Some(idx) = pos {
                        if idx == url_parts.len() - 1 && url.ends_with('}') {
                            // last position (both in params and URL) , it's owned
                            ParameterOwnership::Owned
                        } else {
                            ParameterOwnership::Dependency
                        }
                    } else {
                        // TODO: should probably warn about this
                        ParameterOwnership::Unknown
                    }
                }
                ParameterIn::FormData => ParameterOwnership::Owned, // TODO: revisit, this depends
                ParameterIn::Body | ParameterIn::Query | ParameterIn::Header => {
                    ParameterOwnership::Unknown
                }
                ParameterIn::Unsupported(s) => {
                    ctx.push_warning(TranslationMessage::new(format!(
                        "Unsupported ownership support: {}",
                        s
                    )));

                    ParameterOwnership::Unknown
                }
            }
        }
        // Makes no sense for GET and DELETE to 'make up' values
        HTTPMethod::GET | HTTPMethod::DELETE => ParameterOwnership::Dependency,
        _ => panic!("Add support"),
    };

    Parameter {
        name: parameter.name.clone(),
        schema,
        required: parameter.required,
        ownership,
        meta_data,
    }
}

fn open_api_operation_to_amos(
    ctx: &mut TranslationContext,
    operation: &OpenAPIOperation,
) -> Operation {
    let mut key = "operation/".to_owned();
    key.push_str(&operation.id);

    let url_parts = operation
        .url
        .split('/')
        .filter(|s| s.starts_with('{'))
        .collect::<Vec<&str>>();

    Operation {
        //id: 1,
        info: OperationInfo {
            name: operation.id.clone(),
            key,
        },
        parameters: operation
            .parameters
            .iter()
            .map(|p| {
                open_api_parameter_to_amos(ctx, p, &operation.url, &url_parts, &operation.method)
            })
            .collect(),
        responses: operation
            .responses
            .iter()
            .map(|r| open_api_response_to_amos(ctx, r))
            .collect(),
        meta_data: Some(OperationMetaData::HTTP {
            url: operation.url.clone(),
            method: operation.method.clone(),
        }),
    }
}

pub fn open_api_v2_to_amos(
    operations: &[OpenAPIOperation],
    definitions: &[OpenAPIDefinition],
) -> TranslationResult {
    let mut ctx = TranslationContext::new();

    let defs = definitions
        .iter()
        .map(|d| open_api_definition_to_amos(&mut ctx, d))
        .collect();

    let ops = operations
        .iter()
        .map(|op| open_api_operation_to_amos(&mut ctx, op))
        .collect();

    let amos = AMOS {
        name: "New AMOS".to_string(),
        domain: Domain {
            data: "todo".to_string(),
        },
        definitions: defs,
        operations: ops,
    };

    TranslationResult {
        amos,
        warnings: ctx.warnings,
        errors: ctx.errors,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::OnceLock;

    use crate::amos::OperationMetaData::HTTP;
    use crate::amos::Schema::*;
    use crate::amos::*;
    use qr_http_resource::http::HTTPMethod::{GET, POST};
    use qr_http_resource::http::HTTPParameterTarget;
    use qr_open_api::open_api;
    use qr_open_api::open_api::{
        DataType as OpenAPIDataType, Definition as OpenAPIDefinition, DefinitionKind,
        DefinitionPath, ParseResult, Property as OpenAPIProperty,
    };
    use qr_specification_resource_access::specification as spec_ra;

    fn parse_open_api(path: &str) -> ParseResult {
        let oas_json = spec_ra::open_api_from_file(path).unwrap();

        open_api::parse_json_object(&oas_json).unwrap()
    }

    fn oas_file_1() -> &'static ParseResult {
        static INSTANCE: OnceLock<ParseResult> = OnceLock::new();

        INSTANCE.get_or_init(|| parse_open_api("./test/resources/feature-service.json"))
    }

    #[test]
    fn get_operation_to_amos() {
        let parse_result = oas_file_1();
        let op = &parse_result.operations[0];

        let amos = open_api_v2_to_amos(&vec![op.clone()], &vec![]).amos;

        assert_eq!(
            amos.operations,
            vec![Operation {
                info: OperationInfo {
                    name: "getAllProducts".to_string(),
                    key: "operation/getAllProducts".to_string()
                },
                parameters: vec![],
                responses: vec![Response {
                    name: "successful operation".to_string(),
                    schema: ArrayOfString
                }],
                meta_data: Some(HTTP {
                    url: "/products".to_string(),
                    method: GET
                })
            }]
        )
    }

    #[test]
    fn post_operation_to_amos() {
        let parse_result = oas_file_1();
        let op = &parse_result.operations[3];

        let amos = open_api_v2_to_amos(&vec![op.clone()], &vec![]).amos;

        assert_eq!(
            amos.operations,
            vec![Operation {
                info: OperationInfo {
                    name: "addProduct".to_string(),
                    key: "operation/addProduct".to_string()
                },
                parameters: vec![Parameter {
                    name: "productName".to_string(),
                    schema: Schema::StringNonEmpty,
                    ownership: ParameterOwnership::Owned,
                    required: true,
                    meta_data: Some(ParameterMetaData::HTTP {
                        target: HTTPParameterTarget::Path
                    })
                }],
                responses: vec![Response {
                    name: "successful operation".to_string(),
                    schema: Int
                }],
                meta_data: Some(HTTP {
                    url: "/products/{productName}".to_string(),
                    method: POST,
                })
            }]
        )
    }

    #[test]
    fn post_operation_with_path_and_form_data() {
        let parse_result = oas_file_1();
        let op = parse_result
            .operations
            .iter()
            .find(|op| op.id == "addRequiresConstraintToProduct")
            .expect("Open API operation missing");

        let amos = open_api_v2_to_amos(&vec![op.clone()], &[]).amos;

        assert_eq!(
            amos.operations,
            vec![Operation {
                info: OperationInfo {
                    name: "addRequiresConstraintToProduct".to_string(),
                    key: "operation/addRequiresConstraintToProduct".to_string()
                },
                parameters: vec![
                    Parameter {
                        name: "productName".to_string(),
                        schema: StringNonEmpty,
                        required: true,
                        ownership: ParameterOwnership::Dependency,
                        meta_data: Some(ParameterMetaData::HTTP {
                            target: HTTPParameterTarget::Path
                        })
                    },
                    Parameter {
                        name: "sourceFeature".to_string(),
                        schema: String,
                        required: false,
                        ownership: ParameterOwnership::Owned,
                        meta_data: Some(ParameterMetaData::HTTP {
                            target: HTTPParameterTarget::FormData
                        })
                    },
                    Parameter {
                        name: "requiredFeature".to_string(),
                        schema: String,
                        required: false,
                        ownership: ParameterOwnership::Owned,
                        meta_data: Some(ParameterMetaData::HTTP {
                            target: HTTPParameterTarget::FormData
                        })
                    }
                ],
                responses: vec![Response {
                    name: "successful operation".to_string(),
                    schema: Int
                }],
                meta_data: Some(HTTP {
                    url: "/products/{productName}/constraints/requires".to_string(),
                    method: POST
                })
            }]
        )
    }

    #[test]
    fn response_with_definition() {
        let parse_result = oas_file_1();
        let op = &parse_result.operations[2];

        let amos = open_api_v2_to_amos(&vec![op.clone()], &vec![]).amos;

        assert_eq!(
            amos.operations,
            vec![Operation {
                info: OperationInfo {
                    name: "getProductByName".to_string(),
                    key: "operation/getProductByName".to_string()
                },
                parameters: vec![Parameter {
                    name: "productName".to_string(),
                    schema: Schema::StringNonEmpty,
                    required: true,
                    ownership: ParameterOwnership::Dependency,
                    meta_data: Some(ParameterMetaData::HTTP {
                        target: HTTPParameterTarget::Path
                    })
                }],
                responses: vec![Response {
                    name: "successful operation".to_string(),
                    schema: Reference("#/definitions/Product".to_string())
                }],
                meta_data: Some(HTTP {
                    url: "/products/{productName}".to_string(),
                    method: GET,
                })
            }]
        )
    }

    #[test]
    fn definition_to_amos() {
        let definition = OpenAPIDefinition {
            name: "Product".to_string(),
            path: DefinitionPath {
                path: "#/definition/Product".to_string(),
            },
            kind: DefinitionKind::Object,
            properties: Some(vec![
                OpenAPIProperty {
                    name: "constraints".to_string(),
                    kind: OpenAPIDataType::ArrayOfUniqueRefItems(
                        "#/definitions/FeatureConstraint".to_string(),
                    ),
                },
                OpenAPIProperty {
                    name: "features".to_string(),
                    kind: OpenAPIDataType::ArrayOfUniqueRefItems(
                        "#/definitions/Feature".to_string(),
                    ),
                },
                OpenAPIProperty {
                    name: "id".to_string(),
                    kind: OpenAPIDataType::Integer64,
                },
                OpenAPIProperty {
                    name: "name".to_string(),
                    kind: OpenAPIDataType::String,
                },
            ]),
        };

        let amos = open_api_v2_to_amos(&vec![], &vec![definition]).amos;

        assert_eq!(
            amos.definitions,
            vec![Definition {
                name: "Product".to_string(),
                key: "definition/Product".to_string(),
                schema: Object {
                    properties: vec![
                        Property {
                            name: "constraints".to_string(),
                            schema: ArrayOfUniqueRefItems(
                                "#/definitions/FeatureConstraint".to_string()
                            )
                        },
                        Property {
                            name: "features".to_string(),
                            schema: ArrayOfUniqueRefItems("#/definitions/Feature".to_string())
                        },
                        Property {
                            name: "id".to_string(),
                            schema: Int
                        },
                        Property {
                            name: "name".to_string(),
                            schema: String
                        }
                    ]
                }
            }]
        )
    }

    #[test]
    fn parse_rest_ncs() {
        let parse_result = parse_open_api("./test/resources/rest-ncs.json");
        let amos = open_api_v2_to_amos(&parse_result.operations, &parse_result.definitions).amos;

        assert_eq!(amos.operations.len(), 6);
        assert_eq!(amos.definitions.len(), 1);
    }

    #[test]
    fn parse_rest_news() {
        let parse_result = parse_open_api("./test/resources/rest-news.json");
        let amos = open_api_v2_to_amos(&parse_result.operations, &parse_result.definitions).amos;

        assert_eq!(amos.operations.len(), 7);
        assert_eq!(amos.definitions.len(), 1);
    }

    #[test]
    fn resolve_parameter_with_definition_reference() {
        let parse_result = parse_open_api("./test/resources/rest-news.json");
        let amos = open_api_v2_to_amos(&parse_result.operations, &parse_result.definitions).amos;

        let op_resolved = amos.resolve_operation("createNewsUsingPOST");

        println!("{:#?}", op_resolved);

        assert_eq!(
            op_resolved,
            Some(Operation {
                info: OperationInfo {
                    name: "createNewsUsingPOST".to_string(),
                    key: "operation/createNewsUsingPOST".to_string(),
                },
                parameters: vec![
                    Parameter {
                        name: "authorId".to_string(),
                        schema: String,
                        required: false,
                        ownership: ParameterOwnership::Unknown,
                        meta_data: Some(ParameterMetaData::HTTP {
                            target: HTTPParameterTarget::Body,
                        },),
                    },
                    Parameter {
                        name: "country".to_string(),
                        schema: String,
                        required: false,
                        ownership: ParameterOwnership::Unknown,
                        meta_data: Some(ParameterMetaData::HTTP {
                            target: HTTPParameterTarget::Body,
                        },),
                    },
                    Parameter {
                        name: "creationTime".to_string(),
                        schema: StringDateTime,
                        required: false,
                        ownership: ParameterOwnership::Unknown,
                        meta_data: Some(ParameterMetaData::HTTP {
                            target: HTTPParameterTarget::Body,
                        },),
                    },
                    Parameter {
                        name: "id".to_string(),
                        schema: String,
                        required: false,
                        ownership: ParameterOwnership::Unknown,
                        meta_data: Some(ParameterMetaData::HTTP {
                            target: HTTPParameterTarget::Body,
                        },),
                    },
                    Parameter {
                        name: "newsId".to_string(),
                        schema: String,
                        required: false,
                        ownership: ParameterOwnership::Unknown,
                        meta_data: Some(ParameterMetaData::HTTP {
                            target: HTTPParameterTarget::Body,
                        },),
                    },
                    Parameter {
                        name: "text".to_string(),
                        schema: String,
                        required: false,
                        ownership: ParameterOwnership::Unknown,
                        meta_data: Some(ParameterMetaData::HTTP {
                            target: HTTPParameterTarget::Body,
                        },),
                    },
                ],
                responses: vec![
                    Response {
                        name: "OK".to_string(),
                        schema: Int,
                    },
                    Response {
                        name: "Created".to_string(),
                        schema: Int,
                    },
                    Response {
                        name: "Unauthorized".to_string(),
                        schema: Int,
                    },
                    Response {
                        name: "Forbidden".to_string(),
                        schema: Int,
                    },
                    Response {
                        name: "Not Found".to_string(),
                        schema: Int,
                    },
                ],
                meta_data: Some(HTTP {
                    url: "/news".to_string(),
                    method: POST,
                },),
            },)
        )
    }
}
