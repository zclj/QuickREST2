use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum HTTPStatus {
    OK = 200,
    Created = 201,
    NoContent = 204,
    BadRequest = 400,
    Unauthorized = 401,
    Forbidden = 403,
    NotFound = 404,
    MethodNotAllowed = 405,
    UnsupportedMediaType = 415,
    InternalServerError = 500,
    Default,
    Unsupported,
}

impl fmt::Display for HTTPStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            HTTPStatus::OK => "200",
            HTTPStatus::Created => "201",
            HTTPStatus::NoContent => "204",
            HTTPStatus::BadRequest => "400",
            HTTPStatus::Unauthorized => "401",
            HTTPStatus::Forbidden => "403",
            HTTPStatus::NotFound => "404",
            HTTPStatus::MethodNotAllowed => "405",
            HTTPStatus::UnsupportedMediaType => "415",
            HTTPStatus::InternalServerError => "500",
            HTTPStatus::Default => "Default",
            HTTPStatus::Unsupported => "Unsupported",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone, PartialEq)]
#[allow(non_camel_case_types)]
pub enum CharacterSet {
    UTF_8,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MimeSubType {
    Json,
    XML,
    Vendor,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MultipartSubType {
    FormData,
}

#[derive(Debug, Clone, PartialEq)]
/// See [MDN](https://developer.mozilla.org/en-US/docs/Web/HTTP/Basics_of_HTTP/MIME_types#structure_of_a_mime_type) for reference
pub enum MimeType {
    Application(MimeSubType),
    Multipart(MultipartSubType),
    Unspecified,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MimeData {
    pub kind: MimeType,
    pub char_set: Option<CharacterSet>,
}

impl MimeData {
    pub fn new(kind: MimeType, char_set: Option<CharacterSet>) -> Self {
        MimeData { kind, char_set }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum HTTPMethod {
    GET,
    DELETE,
    POST,
    PUT,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum HTTPParameterTarget {
    Path,
    FormData,
    Query,
    Body,
    Unsupported,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, PartialEq, Clone)]
pub enum Protocol {
    HTTP,
    HTTPS,
}

impl ToString for Protocol {
    fn to_string(&self) -> String {
        match self {
            Protocol::HTTP => "http://".to_string(),
            Protocol::HTTPS => "https://".to_string(),
        }
    }
}
