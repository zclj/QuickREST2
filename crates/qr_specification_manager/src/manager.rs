use crate::error::{Error, Result};
use qr_explore::amos;
use qr_open_api::open_api;
use qr_open_api::open_api::ParseResult;
use qr_specification_resource_access::specification as spec_ra;
use tracing::info;

pub fn load_open_api_file_path(file_path: &str) -> Result<(ParseResult, amos::TranslationResult)> {
    info!(file_path, "Parse OpenAPI file");

    let oas_json = spec_ra::open_api_from_file(file_path)?;

    let parse_result = match open_api::parse_json_object(&oas_json) {
        Ok(result) => result,
        Err(e) => return Err(Error::OpenAPIParseFailed(e)),
    };

    let translation_result =
        amos::open_api_v2_to_amos(&parse_result.operations, &parse_result.definitions);

    Ok((parse_result, translation_result))
}

pub fn fetch_open_api_from_url(
    url: &reqwest::Url,
) -> Result<(ParseResult, amos::TranslationResult)> {
    info!("Fetch OpenAPI-specification from URL: {}", url);

    let oas_json = spec_ra::open_api_from_url(url)?;

    let parse_result = match open_api::parse_json_object(&oas_json) {
        Ok(result) => result,
        Err(e) => return Err(Error::OpenAPIParseFailed(e)),
    };

    let translation_result =
        amos::open_api_v2_to_amos(&parse_result.operations, &parse_result.definitions);

    Ok((parse_result, translation_result))
}
