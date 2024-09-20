use crate::error::Result;
use qr_http_resource::http::{
    CharacterSet, HTTPMethod, HTTPStatus, MimeData, MimeSubType, MimeType, MultipartSubType,
};
use serde_json::{Map, Value};

#[derive(Debug, Clone, PartialEq)]
pub enum DataType {
    // Ints
    Integer32,
    Integer64,

    // Floats
    Double,
    Float,

    // Ints & floats
    Number,

    // Strings
    String,
    StringDateTime,

    // IP
    IPV4,

    // Arrays
    ArrayOfStrings,
    ArrayOfRefItems(String),
    ArrayOfUniqueRefItems(String),

    // Misc
    Boolean,
    File,
    Schema(Schema),

    // Mark unsupported, to track it down
    Unsupported,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParameterIn {
    Path,
    FormData,
    Body,
    Query,
    Header,
    Unsupported(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct OperationParameter {
    pub name: String,
    pub kind: DataType,
    pub target: ParameterIn,
    pub required: bool,
}

#[derive(Debug, PartialEq)]
pub struct DefinitionPath {
    pub path: String,
}

#[derive(Debug)]
pub enum SchemaKind {
    Array,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Schema {
    Inline { properties: Vec<Property> },
    Ref(String),
    ArrayOfString,
    ArrayOfUniqueRefItems(String),
    ArrayOfRefItems(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct OperationResponse {
    pub status: HTTPStatus,
    pub description: String,
    pub schema: Option<DataType>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Operation {
    pub url: String,
    pub method: HTTPMethod,
    pub id: String,
    pub produces: Vec<MimeData>,
    pub consumes: Vec<MimeData>,
    pub parameters: Vec<OperationParameter>,
    pub responses: Vec<OperationResponse>,
}

#[derive(Debug, PartialEq)]
pub struct ParseMessage {
    pub message: String,
    pub path: Option<String>,
    pub operation: Option<String>,
    pub method: Option<String>,
}

impl ParseMessage {
    pub fn new(message: String) -> Self {
        ParseMessage {
            message,
            path: None,
            operation: None,
            method: None,
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct ParseContext {
    warnings: Vec<ParseMessage>,
    current_path: Option<String>,
    current_operation: Option<String>,
    current_method: Option<String>,
}

impl ParseContext {
    fn new() -> Self {
        Self {
            warnings: vec![],
            current_path: None,
            current_operation: None,
            current_method: None,
        }
    }

    fn push_warning(&mut self, mut message: ParseMessage) {
        message.path.clone_from(&self.current_path);
        message.operation.clone_from(&self.current_operation);
        message.method.clone_from(&self.current_method);
        self.warnings.push(message)
    }
}

fn parse_parameter_array(context: &mut ParseContext, param: &Map<String, Value>) -> DataType {
    // TODO: merge/inspire by:
    // parse_property_type_array(context, param);

    if let Some(items_object_raw) = param.get("items") {
        let items_type_object = items_object_raw
            .as_object()
            .expect("Could not parse array items");

        let is_unique_items = match param.get("uniqueItems") {
            Some(unique_items) => unique_items
                .as_bool()
                .expect("Could not parse uniqueItems of array"),
            None => false,
        };

        if let Some(reference) = items_type_object.get("$ref") {
            let ref_str = reference
                .as_str()
                .expect("Could not parse property type of $ref");
            if is_unique_items {
                DataType::ArrayOfUniqueRefItems(ref_str.to_string())
            } else {
                DataType::ArrayOfRefItems(ref_str.to_string())
            }
        } else {
            let items_type_str = items_type_object["type"]
                .as_str()
                .expect("Could not parse array item type");
            // TODO: strings can be an enum
            match items_type_str {
                "string" => DataType::ArrayOfStrings,
                _ => {
                    context.push_warning(ParseMessage::new(format!(
                        "Unsupported array item type : {}",
                        items_type_str
                    )));
                    DataType::Unsupported
                }
            }
        }
    } else {
        context.push_warning(ParseMessage::new(format!(
            "Could not parse array items : {:?} at {}:{}",
            param,
            file!(),
            line!(),
        )));
        DataType::Unsupported
    }
}

fn match_data_type(
    ctx: &mut ParseContext,
    param: &Map<String, Value>,
    type_str: &str,
    format: Option<&str>,
) -> DataType {
    // https://swagger.io/docs/specification/data-models/data-types/#string
    match (type_str, format) {
        ("string", None) => DataType::String,
        // "the date-time notation as defined by RFC 3339, section 5.6, for example, 2017-07-21T17:32:28Z"
        ("string", Some("date-time")) => DataType::StringDateTime,
        ("string", Some("ipv4")) => DataType::IPV4,
        ("string", Some(format)) => {
            ctx.push_warning(ParseMessage::new(format!(
                "Unsupported string format type : {} - Defaults to 'String'",
                format,
            )));
            DataType::String
        }
        ("number", None) => DataType::Number,
        ("number", Some("double")) => DataType::Double,
        ("number", Some("float")) => DataType::Float,
        ("integer", None) => DataType::Integer64,
        ("integer", Some("int32")) => DataType::Integer32,
        ("integer", Some("int64")) => DataType::Integer64,
        ("boolean", None) => DataType::Boolean,
        ("file", None) => DataType::File,
        ("object", None) => {
            let properties_obj = param.get("properties");
            //.expect("Could not find properties on object");

            match properties_obj {
                None => {
                    ctx.push_warning(ParseMessage::new(format!(
                        "No properties defined on object: {:?}",
                        param,
                    )));
                    DataType::Unsupported
                }
                Some(properties_content) => {
                    let properties_obj = properties_content.as_object();

                    match properties_obj {
                        None => {
                            ctx.push_warning(ParseMessage::new(format!(
                                "Could not parse properties as an object: {:?}",
                                param,
                            )));
                            DataType::Unsupported
                        }
                        Some(props) => DataType::Schema(Schema::Inline {
                            properties: props
                                .iter()
                                .map(|p| parse_defintion_property(ctx, p))
                                .collect(),
                        }),
                    }
                }
            }
        }
        ("array", _) => parse_parameter_array(ctx, param),
        _ => {
            ctx.push_warning(ParseMessage::new(format!(
                "Unsupported data type : {}/{:?} - {}:{}",
                type_str,
                format,
                file!(),
                line!(),
            )));
            DataType::Unsupported
        }
    }
}

fn parse_parameter_type(ctx: &mut ParseContext, param: &Map<String, Value>) -> DataType {
    if let Some(param_type) = param.get("type") {
        let type_str = param_type.as_str().expect("Could not parse parameter type");

        let param_format = if let Some(format) = param.get("format") {
            let format_str = format.as_str().expect("Could not parse parameter format");
            Some(format_str)
        } else {
            None
        };

        match_data_type(ctx, param, type_str, param_format)
    } else {
        // If there is no type, the parameter might contain a schema
        if let Some(schema) = parse_schema(ctx, param) {
            schema
        } else {
            ctx.push_warning(ParseMessage::new(format!(
                "Unsupported parameter type : {:?}",
                param
            )));
            DataType::Unsupported
        }
    }
}

fn parse_parmeter_in(context: &mut ParseContext, param: &Map<String, Value>) -> ParameterIn {
    let in_str = param["in"].as_str().expect("Could not parse parameter in");

    match in_str {
        "path" => ParameterIn::Path,
        "formData" => ParameterIn::FormData,
        "body" => ParameterIn::Body,
        "query" => ParameterIn::Query,
        "header" => ParameterIn::Header,
        &_ => {
            context.push_warning(ParseMessage::new(format!(
                "Unsupported parameter 'in' : {}",
                in_str
            )));
            ParameterIn::Unsupported(in_str.to_string())
        }
    }
}

fn parse_method_parameter(context: &mut ParseContext, param: &Value) -> OperationParameter {
    let param_object = param.as_object().expect("Could not parse parameter");

    OperationParameter {
        name: param_object["name"].as_str().unwrap().to_string(),
        kind: parse_parameter_type(context, param_object),
        target: parse_parmeter_in(context, param_object),
        required: param_object["required"].as_bool().unwrap(),
    }
}

fn parse_status_code(context: &mut ParseContext, status_code: &str) -> HTTPStatus {
    match status_code {
        "200" => HTTPStatus::OK,
        "201" => HTTPStatus::Created,
        "204" => HTTPStatus::NoContent,
        "400" => HTTPStatus::BadRequest,
        "401" => HTTPStatus::Unauthorized,
        "403" => HTTPStatus::Forbidden,
        "404" => HTTPStatus::NotFound,
        "405" => HTTPStatus::MethodNotAllowed,
        "default" => HTTPStatus::Default,
        _ => {
            context.push_warning(ParseMessage::new(format!(
                "Unsupported status : {}",
                status_code
            )));
            HTTPStatus::Unsupported
        }
    }
}

fn parse_schema(ctx: &mut ParseContext, response_obj: &Map<String, Value>) -> Option<DataType> {
    if let Some(schema) = response_obj.get("schema") {
        let schema_obj = schema.as_object().expect("Could not parse schema");

        if let Some(schema_ref) = &schema_obj.get("$ref") {
            let ref_s = schema_ref.as_str().expect("Could not parse schema ref");
            return Some(DataType::Schema(Schema::Ref(ref_s.to_string())));
        }

        let schema_type = schema_obj
            .get("type")
            .expect("Could not parse schema type")
            .as_str()
            .expect("Could not parse schema type");

        let format = if let Some(format) = response_obj.get("format") {
            let format_str = format.as_str().expect("Could not parse parameter format");
            Some(format_str)
        } else {
            None
        };
        Some(match_data_type(ctx, schema_obj, schema_type, format))
    } else {
        None
    }
}

fn parse_response(
    context: &mut ParseContext,
    (status_code, response_value): (&String, &Value),
) -> OperationResponse {
    let response_obj = response_value
        .as_object()
        .expect("Could not parse response");

    OperationResponse {
        status: parse_status_code(context, status_code),
        description: response_obj["description"]
            .as_str()
            .expect("Could not parse response description")
            .to_string(),
        schema: parse_schema(context, response_obj),
    }
}

fn parse_method_responses(
    context: &mut ParseContext,
    response: &Map<String, Value>,
) -> Vec<OperationResponse> {
    // need to map for each key(status-code) -> response
    response
        .iter()
        .map(|r| parse_response(context, r))
        .collect()
}

fn parse_method_str(context: &mut ParseContext, method: &str) -> HTTPMethod {
    context.current_method = Some(method.to_string());
    match method {
        "get" => HTTPMethod::GET,
        "delete" => HTTPMethod::DELETE,
        "post" => HTTPMethod::POST,
        "put" => HTTPMethod::PUT,
        _ => {
            context.push_warning(ParseMessage::new(format!(
                "Unsupported method : {}",
                method
            )));
            HTTPMethod::Unsupported
        }
    }
}

fn parse_mime_types_string(context: &mut ParseContext, produces: &str) -> MimeData {
    let parts: Vec<&str> = produces.split(';').collect();

    let kind = match parts[0] {
        "*/*" => {
            context.push_warning(ParseMessage::new(format!(
                "Unspecified MIME type : {}",
                produces
            )));
            MimeType::Unspecified
        }
        "application/json" => MimeType::Application(MimeSubType::Json),
        "application/xml" => MimeType::Application(MimeSubType::XML),
        "multipart/form-data" => MimeType::Multipart(MultipartSubType::FormData),
        _ => {
            // vendor is not the common case
            let kind_parts: Vec<&str> = parts[0].split('/').collect();
            if kind_parts.len() == 2 && kind_parts[1].contains("vnd") {
                MimeType::Application(MimeSubType::Vendor)
            } else {
                context.push_warning(ParseMessage::new(format!(
                    "Unsupported MIME type : {}",
                    produces
                )));
                MimeType::Unsupported
            }
        }
    };

    // can be an optional parameter
    let char_set = if let Some(charset) = parts.iter().find(|part| part.contains("charset=")) {
        if charset.contains("UTF-8") {
            Some(CharacterSet::UTF_8)
        } else {
            None
        }
    } else {
        None
    };

    MimeData { kind, char_set }
}

fn parse_method_parameters(
    ctx: &mut ParseContext,
    method: &Map<String, Value>,
) -> Vec<OperationParameter> {
    if let Some(param_value) = method.get("parameters") {
        if let Some(params) = param_value.as_array() {
            params
                .iter()
                .map(|p| parse_method_parameter(ctx, p))
                .collect()
        } else {
            vec![]
        }
    } else {
        vec![]
    }
}

fn parse_operation_method(
    context: &mut ParseContext,
    path: &String,
    (method, v): (&String, &Value),
) -> Operation {
    let method_info = v.as_object().expect("Could not parse method");

    let operation_id = match method_info.get("operationId") {
        Some(op_id_value) => {
            if let Some(op_id_str) = op_id_value.as_str() {
                op_id_str.to_string()
            } else {
                format!("{}-{}", path, method).to_string()
            }
        }
        None => format!("{}-{}", path, method).to_string(),
    };

    context.current_operation = Some(operation_id.clone());

    let parameters = parse_method_parameters(context, method_info);

    let consumes = match method_info.get("consumes") {
        Some(consumes_entry) => {
            if let Some(consumes) = consumes_entry.as_array() {
                consumes
                    .iter()
                    .map(|x| parse_mime_types_string(context, x.as_str().unwrap()))
                    .collect()
            } else {
                context.push_warning(ParseMessage::new(format!(
                    "Unsupported format for 'consumes' entry : {}/{}",
                    path, method
                )));
                vec![]
            }
        }
        None => vec![],
    };

    Operation {
        url: path.to_string(),
        method: parse_method_str(context, method),
        id: operation_id,
        produces: method_info["produces"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| parse_mime_types_string(context, x.as_str().unwrap()))
            .collect(),
        consumes,
        parameters,
        responses: parse_method_responses(
            context,
            method_info["responses"]
                .as_object()
                .expect("Could not parse method responses"),
        ),
    }
}

fn parse_path(context: &mut ParseContext, (path, value): (&String, &Value)) -> Vec<Operation> {
    let methods = value.as_object();
    context.current_path = Some(path.clone());

    match methods {
        Some(ms) => ms
            .iter()
            .map(|m| parse_operation_method(context, path, m))
            .collect(),
        None => {
            context.push_warning(ParseMessage::new(format!(
                "Could not parse {} as object",
                value
            )));
            Vec::new()
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct Property {
    pub name: String,
    pub kind: DataType,
}

#[derive(Debug, PartialEq)]
pub enum DefinitionKind {
    Object,
    Unsupported,
}

#[derive(Debug, PartialEq)]
pub struct Definition {
    pub name: String,
    pub path: DefinitionPath,
    pub kind: DefinitionKind,
    pub properties: Option<Vec<Property>>,
}

fn parse_defintion_type(context: &mut ParseContext, value: &Value) -> DefinitionKind {
    let type_str = value
        .get("type")
        .expect("Could not find defintion type")
        .as_str()
        .expect("Could not parse type");

    match type_str {
        "object" => DefinitionKind::Object,
        _ => {
            context.push_warning(ParseMessage::new(format!(
                "Unsupported definition type : {}",
                type_str
            )));
            DefinitionKind::Unsupported
        }
    }
}

fn parse_property_type_and_format(
    ctx: &mut ParseContext,
    property_obj: &Map<String, Value>,
) -> DataType {
    if let Some(param_type) = property_obj.get("type") {
        let type_str = param_type.as_str().expect("Could not parse parameter type");

        let param_format = if let Some(format) = property_obj.get("format") {
            let format_str = format.as_str().expect("Could not parse parameter format");
            Some(format_str)
        } else {
            None
        };

        match_data_type(ctx, property_obj, type_str, param_format)
    } else {
        ctx.push_warning(ParseMessage::new(format!(
            "Unsupported property type : {:#?}",
            property_obj
        )));
        DataType::Unsupported
    }
}

fn parse_defintion_property(
    context: &mut ParseContext,
    (name, value): (&String, &Value),
) -> Property {
    let property_obj = value.as_object().expect("Could not parse property object");

    Property {
        name: name.to_string(),
        kind: parse_property_type_and_format(context, property_obj),
    }
}

fn parse_defintion_properties(context: &mut ParseContext, value: &Value) -> Option<Vec<Property>> {
    let properties_obj = value
        .get("properties")?
        .as_object()
        .expect("Could not parse property");

    Some(
        properties_obj
            .iter()
            .map(|p| parse_defintion_property(context, p))
            .collect(),
    )
}

fn parse_definition(context: &mut ParseContext, (name, value): (&String, &Value)) -> Definition {
    Definition {
        name: name.to_string(),
        path: DefinitionPath {
            path: format!("#/definition/{}", name),
        },
        kind: parse_defintion_type(context, value),
        properties: parse_defintion_properties(context, value),
    }
}

#[derive(Debug)]
pub struct ParseResult {
    pub operations: Vec<Operation>,
    pub definitions: Vec<Definition>,
    pub warnings: Vec<ParseMessage>,
}

pub fn parse_definitions(ctx: &mut ParseContext, definitions_json: &Value) -> Vec<Definition> {
    definitions_json
        .as_object()
        .unwrap()
        .iter()
        .map(|d| parse_definition(ctx, d))
        .collect()
}

pub fn parse_json_object(open_api_object: &serde_json::Map<String, Value>) -> Result<ParseResult> {
    let mut ctx = ParseContext::new();

    // Parse the operations defined in paths
    let mut operations: Vec<Operation> = vec![];
    if let Some(paths) = &open_api_object.get("paths") {
        if let Some(paths_obj) = paths.as_object() {
            operations = paths_obj
                .iter()
                .flat_map(|o| parse_path(&mut ctx, o))
                .collect();
        } else {
            ctx.push_warning(ParseMessage::new(
                "Could not parse 'paths' as an object".to_string(),
            ));
        }
    } else {
        ctx.push_warning(ParseMessage::new("Could not parse 'paths'".to_string()));
    }

    // parse definitions if any are definined
    let definitions = if let Some(definitions_json) = &open_api_object.get("definitions") {
        parse_definitions(&mut ctx, definitions_json)
    } else {
        vec![]
    };

    // Semantic checks and sanitize

    // Operations should not include copies of the same parameter
    for op in &mut operations {
        let mut parsed_params: Vec<OperationParameter> = vec![];

        for param in &op.parameters {
            if parsed_params.contains(param) {
                ctx.push_warning(ParseMessage::new(format!(
                    "Duplicated operation parameter: {}/{}",
                    op.id, param.name
                )));
            } else {
                parsed_params.push(param.clone())
            }
        }

        // Only keep the sanitized parameters
        op.parameters = parsed_params;
    }

    Ok(ParseResult {
        operations,
        definitions,
        warnings: ctx.warnings,
    })
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::*;

    #[test]
    fn parse_operation_with_consume_produces_mime_types() {
        let data = r##"
        {"get": {
           "summary": "Foo",
           "operationId": "OpId",
           "produces": [
               "application/json"
           ],
           "consumes": [
               "multipart/form-data"
           ],
           "responses": {
               "200": {
                   "description": "OK",
                   "schema": {
                       "type": "array",
                       "items": {
                           "type": "string"
                       }
                   }
               }
           }
        }
       }
        "##;

        let json_value = serde_json::from_str::<Value>(data).unwrap();
        let json_object = &json_value["get"];

        let mut ctx = ParseContext::new();
        let parsed = parse_operation_method(
            &mut ctx,
            &"foo-path".to_string(),
            (&"get".to_string(), &json_object),
        );

        assert_eq!(
            parsed,
            Operation {
                url: "foo-path".to_string(),
                method: HTTPMethod::GET,
                id: "OpId".to_string(),
                produces: vec![MimeData {
                    kind: MimeType::Application(MimeSubType::Json),
                    char_set: None
                }],
                consumes: vec![MimeData {
                    kind: MimeType::Multipart(MultipartSubType::FormData),
                    char_set: None
                }],
                parameters: vec![],
                responses: vec![OperationResponse {
                    status: HTTPStatus::OK,
                    description: "OK".to_string(),
                    schema: Some(DataType::ArrayOfStrings),
                },],
            }
        )
    }

    #[test]
    fn parse_mime_types() {
        let mut ctx = ParseContext::new();

        let unsupported = parse_mime_types_string(&mut ctx, "foo");
        let json = parse_mime_types_string(&mut ctx, "application/json");
        let xml = parse_mime_types_string(&mut ctx, "application/xml");
        let multipart_form_data = parse_mime_types_string(&mut ctx, "multipart/form-data");
        let vendor = parse_mime_types_string(
            &mut ctx,
            "application/vnd.tsdes.news+json;charset=UTF-8;version=2",
        );
        let json_utf_8 = parse_mime_types_string(&mut ctx, "application/json;charset=UTF-8");

        assert_eq!(unsupported, MimeData::new(MimeType::Unsupported, None));
        assert_eq!(
            json,
            MimeData::new(MimeType::Application(MimeSubType::Json), None)
        );
        assert_eq!(
            json_utf_8,
            MimeData::new(
                MimeType::Application(MimeSubType::Json),
                Some(CharacterSet::UTF_8)
            )
        );
        assert_eq!(
            xml,
            MimeData::new(MimeType::Application(MimeSubType::XML), None)
        );
        assert_eq!(
            vendor,
            MimeData::new(
                MimeType::Application(MimeSubType::Vendor),
                Some(CharacterSet::UTF_8)
            )
        );
        assert_eq!(
            multipart_form_data,
            MimeData::new(MimeType::Multipart(MultipartSubType::FormData), None)
        );
    }

    #[test]
    fn parse_method_with_no_operation_id() {
        let data = r##"
        {"get": {
           "summary": "Foo",
           "produces": [
               "application/json"
           ],
           "responses": {
               "200": {
                   "description": "OK",
                   "schema": {
                       "type": "array",
                       "items": {
                           "type": "string"
                       }
                   }
               }
           }
        }
       }
        "##;

        let json_value = serde_json::from_str::<Value>(data).unwrap();
        let json_object = &json_value["get"];

        let mut ctx = ParseContext::new();
        let parsed = parse_operation_method(
            &mut ctx,
            &"foo-path".to_string(),
            (&"get".to_string(), &json_object),
        );

        assert_eq!(
            parsed,
            Operation {
                url: "foo-path".to_string(),
                method: HTTPMethod::GET,
                id: "foo-path-get".to_string(),
                produces: vec![MimeData {
                    kind: MimeType::Application(MimeSubType::Json),
                    char_set: None
                }],
                consumes: vec![],
                parameters: vec![],
                responses: vec![OperationResponse {
                    status: HTTPStatus::OK,
                    description: "OK".to_string(),
                    schema: Some(DataType::ArrayOfStrings),
                },],
            }
        )
    }

    #[test]
    fn parse_method_with_no_parameters() {
        let data = r##"
        {"get": {
           "tags": [
               "country-api"
           ],
           "summary": "Retrieve list of country names",
           "operationId": "getUsingGET",
           "produces": [
               "application/json"
           ],
           "responses": {
               "200": {
                   "description": "OK",
                   "schema": {
                       "type": "array",
                       "items": {
                           "type": "string"
                       }
                   }
               },
               "401": {
                   "description": "Unauthorized"
               },
               "403": {
                   "description": "Forbidden"
               },
               "404": {
                   "description": "Not Found"
               }
           },
           "deprecated": false
        }
       }
        "##;

        let json_value = serde_json::from_str::<Value>(data).unwrap();
        let json_object = &json_value["get"];

        let mut ctx = ParseContext::new();
        let parsed = parse_operation_method(
            &mut ctx,
            &"foo-path".to_string(),
            (&"get".to_string(), &json_object),
        );

        assert_eq!(
            parsed,
            Operation {
                url: "foo-path".to_string(),
                method: HTTPMethod::GET,
                id: "getUsingGET".to_string(),
                produces: vec![MimeData {
                    kind: MimeType::Application(MimeSubType::Json),
                    char_set: None
                }],
                consumes: vec![],
                parameters: vec![],
                responses: vec![
                    OperationResponse {
                        status: HTTPStatus::OK,
                        description: "OK".to_string(),
                        schema: Some(DataType::ArrayOfStrings),
                    },
                    OperationResponse {
                        status: HTTPStatus::Unauthorized,
                        description: "Unauthorized".to_string(),
                        schema: None,
                    },
                    OperationResponse {
                        status: HTTPStatus::Forbidden,
                        description: "Forbidden".to_string(),
                        schema: None,
                    },
                    OperationResponse {
                        status: HTTPStatus::NotFound,
                        description: "Not Found".to_string(),
                        schema: None,
                    },
                ],
            }
        )
    }

    #[test]
    fn parse_response_schema_ref_test() {
        let data = r##"
        {"responses" : {
           "200" : {
             "description" : "successful operation",
             "schema" : {
               "$ref" : "#/definitions/ProductConfiguration"
             },
             "headers" : { }
           }
         }
        }"##;

        let json_value = serde_json::from_str::<Value>(data).unwrap();
        let json_object = json_value["responses"].as_object().unwrap();

        let mut ctx = ParseContext::new();
        let parsed = parse_method_responses(&mut ctx, &json_object);

        assert_eq!(
            parsed,
            vec![OperationResponse {
                status: HTTPStatus::OK,
                description: "successful operation".to_string(),
                schema: Some(DataType::Schema(Schema::Ref(
                    "#/definitions/ProductConfiguration".to_string()
                ))),
            }]
        )
    }

    #[test]
    fn parse_response_schema_array_of_string_test() {
        let data = r##"
        {"responses" : {
           "200" : {
             "description" : "successful operation",
             "schema" : {
               "type" : "array",
               "items" : {
                 "type" : "string"
               }
             },
             "headers" : { }
           }
         }
        }"##;

        let json_value = serde_json::from_str::<Value>(data).unwrap();
        let json_object = json_value["responses"].as_object().unwrap();

        let mut ctx = ParseContext::new();
        let parsed = parse_method_responses(&mut ctx, &json_object);

        assert_eq!(
            parsed,
            vec![OperationResponse {
                status: HTTPStatus::OK,
                description: "successful operation".to_string(),
                schema: Some(DataType::ArrayOfStrings),
            }]
        )
    }

    #[test]
    fn parse_response_schema_array_of_ref_test() {
        let data = r##"
        {"responses" : {
           "200" : {
             "description" : "successful operation",
             "schema" : {
               "type" : "array",
               "uniqueItems" : true,
               "items" : {
                 "$ref" : "#/definitions/Feature"
               }
             },
             "headers" : { }
           }
         }
        }"##;

        let json_value = serde_json::from_str::<Value>(data).unwrap();
        let json_object = json_value["responses"].as_object().unwrap();

        let mut ctx = ParseContext::new();
        let parsed = parse_method_responses(&mut ctx, &json_object);

        assert_eq!(
            parsed,
            vec![OperationResponse {
                status: HTTPStatus::OK,
                description: "successful operation".to_string(),
                schema: Some(DataType::ArrayOfUniqueRefItems(
                    "#/definitions/Feature".to_string()
                )),
            }]
        )
    }

    #[test]
    fn parse_parameter_array_ref() {
        let data = r##"
        {"parameters" : [{
           "name" : "productName",
           "in" : "path",
           "required" : true,
           "type" : "array",
           "items": {
             "$ref": "#/definitions/Foo"
             }
         }]
        }"##;

        let json_value = serde_json::from_str::<Value>(data).unwrap();
        let json_object = json_value.as_object().unwrap();

        let mut ctx = ParseContext::new();
        let parsed = parse_method_parameters(&mut ctx, &json_object);

        assert_eq!(
            parsed,
            vec![OperationParameter {
                name: "productName".to_string(),
                kind: DataType::ArrayOfRefItems("#/definitions/Foo".to_string()),
                target: ParameterIn::Path,
                required: true
            }]
        )
    }

    #[test]
    fn parse_parameter_array_no_items() {
        let data = r##"
        {"parameters" : [{
           "name" : "productName",
           "in" : "path",
           "required" : true,
           "type" : "array",
           "foo": "bar"             
         }]
        }"##;

        let json_value = serde_json::from_str::<Value>(data).unwrap();
        let json_object = json_value.as_object().unwrap();

        let mut ctx = ParseContext::new();
        let parsed = parse_method_parameters(&mut ctx, &json_object);

        assert_eq!(
            parsed,
            vec![OperationParameter {
                name: "productName".to_string(),
                kind: DataType::Unsupported,
                target: ParameterIn::Path,
                required: true
            }]
        )
    }

    #[test]
    fn parse_parameter_array_ref_with_unsupported_type() {
        let data = r##"
        {"parameters" : [{
           "name" : "productName",
           "in" : "path",
           "required" : true,
           "type" : "array",
           "items": {
             "type": "foo"
             }
         }]
        }"##;

        let json_value = serde_json::from_str::<Value>(data).unwrap();
        let json_object = json_value.as_object().unwrap();

        let mut ctx = ParseContext::new();
        let parsed = parse_method_parameters(&mut ctx, &json_object);

        assert_eq!(
            parsed,
            vec![OperationParameter {
                name: "productName".to_string(),
                kind: DataType::Unsupported,
                target: ParameterIn::Path,
                required: true
            }]
        )
    }

    #[test]
    fn parse_parameter_string_types() {
        let data = r##"
        {"parameters" : [{
           "name" : "foo",
           "in" : "path",
           "required" : true,
           "type" : "string",
           "format": "ipv4"
         },
         {
           "name" : "bar",
           "in" : "path",
           "required" : true,
           "type" : "string",
           "format": "date-time"
         },
         {
           "name" : "baz",
           "in" : "path",
           "required" : true,
           "type" : "string",
           "format": "crap"
         },
         {
           "name" : "gizmo",
           "in" : "path",
           "required" : true,
           "type" : "string"
         }]
        }"##;

        let json_value = serde_json::from_str::<Value>(data).unwrap();
        let json_object = json_value.as_object().unwrap();

        let mut ctx = ParseContext::new();
        let parsed = parse_method_parameters(&mut ctx, &json_object);

        assert_eq!(ctx.warnings.len(), 1);
        assert_eq!(
            parsed,
            vec![
                OperationParameter {
                    name: "foo".to_string(),
                    kind: DataType::IPV4,
                    target: ParameterIn::Path,
                    required: true
                },
                OperationParameter {
                    name: "bar".to_string(),
                    kind: DataType::StringDateTime,
                    target: ParameterIn::Path,
                    required: true
                },
                OperationParameter {
                    name: "baz".to_string(),
                    kind: DataType::String,
                    target: ParameterIn::Path,
                    required: true
                },
                OperationParameter {
                    name: "gizmo".to_string(),
                    kind: DataType::String,
                    target: ParameterIn::Path,
                    required: true
                }
            ]
        )
    }

    #[test]
    fn parse_parameter_number_types() {
        let data = r##"
        {"parameters" : [{
           "name" : "foo",
           "in" : "path",
           "required" : true,
           "type" : "number"
         },
         {
           "name" : "bar",
           "in" : "path",
           "required" : true,
           "type" : "number",
           "format": "double"
         },
         {
           "name" : "baz",
           "in" : "path",
           "required" : true,
           "type" : "number",
           "format": "float"
         }]
        }"##;

        let json_value = serde_json::from_str::<Value>(data).unwrap();
        let json_object = json_value.as_object().unwrap();

        let mut ctx = ParseContext::new();
        let parsed = parse_method_parameters(&mut ctx, &json_object);

        assert_eq!(
            parsed,
            vec![
                OperationParameter {
                    name: "foo".to_string(),
                    kind: DataType::Number,
                    target: ParameterIn::Path,
                    required: true
                },
                OperationParameter {
                    name: "bar".to_string(),
                    kind: DataType::Double,
                    target: ParameterIn::Path,
                    required: true
                },
                OperationParameter {
                    name: "baz".to_string(),
                    kind: DataType::Float,
                    target: ParameterIn::Path,
                    required: true
                },
            ]
        )
    }

    #[test]
    fn parse_parameter_file_type() {
        let data = r##"
        {"parameters" : [{
           "name" : "foo",
           "in" : "path",
           "required" : true,
           "type" : "file"
         }]
        }"##;

        let json_value = serde_json::from_str::<Value>(data).unwrap();
        let json_object = json_value.as_object().unwrap();

        let mut ctx = ParseContext::new();
        let parsed = parse_method_parameters(&mut ctx, &json_object);

        assert_eq!(
            parsed,
            vec![OperationParameter {
                name: "foo".to_string(),
                kind: DataType::File,
                target: ParameterIn::Path,
                required: true
            },]
        )
    }

    #[test]
    fn parse_parameter_object_type_no_properties() {
        let data = r##"
        {"parameters" : [{
           "name" : "foo",
           "in" : "path",
           "required" : true,
           "type" : "object",
           "properties" : "foo"
         }]
        }"##;

        let json_value = serde_json::from_str::<Value>(data).unwrap();
        let json_object = json_value.as_object().unwrap();

        let mut ctx = ParseContext::new();
        let parsed = parse_method_parameters(&mut ctx, &json_object);

        assert_eq!(ctx.warnings,
                   vec![
                       ParseMessage { message: "Could not parse properties as an object: {\"in\": String(\"path\"), \"name\": String(\"foo\"), \"properties\": String(\"foo\"), \"required\": Bool(true), \"type\": String(\"object\")}".to_string(), path: None, operation: None, method: None }
                   ]);
        assert_eq!(
            parsed,
            vec![OperationParameter {
                name: "foo".to_string(),
                kind: DataType::Unsupported,
                target: ParameterIn::Path,
                required: true
            },]
        )
    }

    #[test]
    fn parse_parameter_unsupported_type() {
        let data = r##"
        {"parameters" : [{
           "name" : "foo",
           "in" : "path",
           "required" : true,
           "type" : "crap"
         }]
        }"##;

        let json_value = serde_json::from_str::<Value>(data).unwrap();
        let json_object = json_value.as_object().unwrap();

        let mut ctx = ParseContext::new();
        let parsed = parse_method_parameters(&mut ctx, &json_object);

        assert_eq!(ctx.warnings.len(), 1);
        assert_eq!(
            parsed,
            vec![OperationParameter {
                name: "foo".to_string(),
                kind: DataType::Unsupported,
                target: ParameterIn::Path,
                required: true
            },]
        )
    }

    #[test]
    fn parse_parameter_no_type() {
        let data = r##"
        {"parameters" : [{
           "name" : "foo",
           "in" : "path",
           "required" : true
         }]
        }"##;

        let json_value = serde_json::from_str::<Value>(data).unwrap();
        let json_object = json_value.as_object().unwrap();

        let mut ctx = ParseContext::new();
        let parsed = parse_method_parameters(&mut ctx, &json_object);

        assert_eq!(
            ctx.warnings,
            vec![ParseMessage {
                message:
                    "Unsupported parameter type : {\"in\": String(\"path\"), \"name\": String(\"foo\"), \"required\": Bool(true)}".to_string(),
                path: None,
                operation: None,
                method: None
            }]
        );
        assert_eq!(
            parsed,
            vec![OperationParameter {
                name: "foo".to_string(),
                kind: DataType::Unsupported,
                target: ParameterIn::Path,
                required: true
            },]
        )
    }

    #[test]
    fn parse_parameter_in() {
        let data = r##"
        {"parameters" : [{
           "name" : "foo",
           "in" : "header",
           "required" : true,
           "type" : "number"
         },
         {
           "name" : "bar",
           "in" : "crap",
           "required" : true,
           "type": "string"
         }]
        }"##;

        let json_value = serde_json::from_str::<Value>(data).unwrap();
        let json_object = json_value.as_object().unwrap();

        let mut ctx = ParseContext::new();
        let parsed = parse_method_parameters(&mut ctx, &json_object);

        assert_eq!(
            ctx.warnings,
            vec![ParseMessage {
                message: "Unsupported parameter 'in' : crap".to_string(),
                path: None,
                operation: None,
                method: None
            }]
        );
        assert_eq!(
            parsed,
            vec![
                OperationParameter {
                    name: "foo".to_string(),
                    kind: DataType::Number,
                    target: ParameterIn::Header,
                    required: true
                },
                OperationParameter {
                    name: "bar".to_string(),
                    kind: DataType::String,
                    target: ParameterIn::Unsupported("crap".to_string()),
                    required: true
                }
            ]
        )
    }

    #[test]
    fn parse_definition_object() {
        let data = r##"
            {"definitions" : {
               "Feature" : {
                 "type" : "object",
                 "properties" : {
                   "id" : {
                     "type" : "integer",
                     "format" : "int64"
                   },
                   "name" : {
                     "type" : "string"
                   },
                   "description" : {
                     "type" : "string"
                   }
                 }
               }
           }}"##;

        let json_value = serde_json::from_str::<Value>(data).unwrap();
        let json_object = &json_value["definitions"];

        let mut ctx = ParseContext::new();

        let parsed = parse_definitions(&mut ctx, &json_object);

        assert_eq!(
            parsed,
            vec![Definition {
                name: "Feature".to_string(),
                path: DefinitionPath {
                    path: "#/definition/Feature".to_string()
                },
                kind: DefinitionKind::Object,
                properties: Some(vec![
                    Property {
                        name: "description".to_string(),
                        kind: DataType::String
                    },
                    Property {
                        name: "id".to_string(),
                        kind: DataType::Integer64
                    },
                    Property {
                        name: "name".to_string(),
                        kind: DataType::String
                    }
                ])
            }]
        )
    }

    #[test]
    fn parse_definition_object_with_nested_objects() {
        let data = r##"
            {"definitions" : {
               "Feature" : {
                 "type" : "object",
                 "properties" : {
                   "Foo" : {
                     "type" : "object",
                     "properties" : {
                       "Name": {
                         "type": "string"
                       }
                     }
                   }               
               }
             }
           }}"##;

        let json_value = serde_json::from_str::<Value>(data).unwrap();
        let json_object = &json_value["definitions"];

        let mut ctx = ParseContext::new();

        let parsed = parse_definitions(&mut ctx, &json_object);

        assert_eq!(
            parsed,
            vec![Definition {
                name: "Feature".to_string(),
                path: DefinitionPath {
                    path: "#/definition/Feature".to_string()
                },
                kind: DefinitionKind::Object,
                properties: Some(vec![Property {
                    name: "Foo".to_string(),
                    kind: DataType::Schema(Schema::Inline {
                        properties: vec![Property {
                            name: "Name".to_string(),
                            kind: DataType::String
                        }]
                    })
                },])
            }]
        )
    }
}
