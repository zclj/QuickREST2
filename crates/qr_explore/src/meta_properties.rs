use crate::amos::{InvokeResult, ResultMetaData};

use qr_http_resource::http::HTTPStatus;

pub fn check_response_inequality(invocation_result: &[InvokeResult]) -> bool {
    invocation_result
        .iter()
        .all(|res| *res == invocation_result[0])
}

pub fn check_response_equality(invocation_result: &[InvokeResult]) -> bool {
    !check_response_inequality(invocation_result)
}

pub fn check_state_mutation(invocation_result: &[InvokeResult]) -> bool {
    // TODO: query op should be inserted first and last

    invocation_result
        .iter()
        .all(|res| res == &invocation_result[0])
}

pub fn check_state_identity_with_observation(invocation_result: &[InvokeResult]) -> bool {
    // first and last should be equal, the 'identity'
    if invocation_result.len() > 1
        && invocation_result[0] == invocation_result[invocation_result.len() - 1]
    {
        // check for an observation of a state change, 'mutation'
        check_state_mutation(invocation_result)
    } else {
        true
    }
}

////////////////////////////////////////
// TODO: Do these belong as MPs?

pub fn check_response(invocation_result: &[InvokeResult]) -> bool {
    // Check for HTTP 500
    for result in invocation_result {
        if let Some(meta) = &result.meta_data {
            match meta {
                ResultMetaData::HTTP { status, .. } => {
                    return status != &HTTPStatus::InternalServerError;
                }
            }
        }
    }

    true
}

#[cfg(test)]
mod tests {

    use crate::amos_generation::GeneratedOperation;
    use crate::{
        amos::{InvokeResult, ResultMetaData},
        meta_properties as sut,
    };
    use qr_http_resource::http;

    #[test]
    fn check_response_equality_pos() {
        let res_1 = InvokeResult::new(
            GeneratedOperation {
                name: "get_persons".to_string(),
                parameters: vec![],
            },
            "[\"Fake result\"]".to_string(),
            true,
            Some(ResultMetaData::HTTP {
                url: "udddrl".to_string(),
                status: http::HTTPStatus::OK,
            }),
        );
        let res_2 = InvokeResult::new(
            GeneratedOperation {
                name: "get_persons".to_string(),
                parameters: vec![],
            },
            "[\"Fake result\"]".to_string(),
            true,
            Some(ResultMetaData::HTTP {
                url: "udddrl".to_string(),
                status: http::HTTPStatus::OK,
            }),
        );

        let results = vec![res_1, res_2];

        let res = sut::check_response_equality(&results);

        assert_eq!(res, false)
    }

    #[test]
    fn check_response_equality_neg() {
        let res_1 = InvokeResult::new(
            GeneratedOperation {
                name: "get_persons".to_string(),
                parameters: vec![],
            },
            "[\"Fake result\"]".to_string(),
            true,
            Some(ResultMetaData::HTTP {
                url: "udddrl".to_string(),
                status: http::HTTPStatus::OK,
            }),
        );
        let res_2 = InvokeResult::new(
            GeneratedOperation {
                name: "get_persons".to_string(),
                parameters: vec![],
            },
            "[\"Fake result0000000000000\"]".to_string(),
            true,
            Some(ResultMetaData::HTTP {
                url: "udddrl".to_string(),
                status: http::HTTPStatus::OK,
            }),
        );

        let results = vec![res_1, res_2];

        let res = sut::check_response_equality(&results);

        assert_eq!(res, true)
    }
}
