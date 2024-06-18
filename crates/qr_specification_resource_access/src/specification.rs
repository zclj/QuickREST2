use std::fs;

use crate::error::{Error, Result};

fn open_api_as_json(content: &str) -> Result<serde_json::Map<String, serde_json::Value>> {
    let open_api_doc: serde_json::Value = match serde_json::from_str(content) {
        Ok(json) => json,
        Err(e) => return Err(Error::OpenAPIInvalidJSON(e)),
    };

    // parse the OAS document as on object, or bail if we cannot
    let open_api_object = if let Some(oas_obj) = open_api_doc.as_object() {
        oas_obj
    } else {
        return Err(Error::OpenAPIInvalidFormat(
            "Could not parse Open API document as JSON object".to_string(),
        ));
    };

    Ok(open_api_object.clone())
}
pub fn open_api_from_url(url: &reqwest::Url) -> Result<serde_json::Map<String, serde_json::Value>> {
    let client = reqwest::blocking::Client::new();
    let request = client.request(reqwest::Method::GET, url.clone());

    let response = request.send()?;

    open_api_as_json(&response.text()?)
}

pub fn open_api_from_file(file_path: &str) -> Result<serde_json::Map<String, serde_json::Value>> {
    let contents = match fs::read_to_string(file_path) {
        Ok(content) => content,
        Err(e) => {
            return Err(Error::OpenAPIReadFileFailure(e));
        }
    };

    let open_api_doc: serde_json::Value = match serde_json::from_str(&contents) {
        Ok(json) => json,
        Err(e) => return Err(Error::OpenAPIInvalidJSON(e)),
    };

    // parse the OAS document as on object, or bail if we cannot
    let open_api_object = if let Some(oas_obj) = open_api_doc.as_object() {
        oas_obj
    } else {
        return Err(Error::OpenAPIInvalidFormat(
            "Could not parse Open API document as JSON object".to_string(),
        ));
    };

    Ok(open_api_object.clone())
}
