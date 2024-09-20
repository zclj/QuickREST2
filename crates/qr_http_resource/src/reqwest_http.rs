use crate::http::{HTTPCall, HTTPMethod};

pub fn build_reqwest_request(
    client: &reqwest::blocking::Client,
    http_operation: &HTTPCall,
) -> reqwest::blocking::RequestBuilder {
    let con_method = match http_operation.method {
        HTTPMethod::GET => reqwest::Method::GET,
        HTTPMethod::POST => reqwest::Method::POST,
        HTTPMethod::DELETE => reqwest::Method::DELETE,
        HTTPMethod::PUT => reqwest::Method::PUT,
        _ => todo!(),
    };
    let init_request = client.request(con_method, http_operation.url.clone());

    let request_with_form_data = if let Some(payload) = &http_operation.parameters.form_data {
        init_request.form(&payload)
    } else {
        init_request
    };

    // With form-data present and no other evidence, we default to
    // 'application/x-www-form-urlencoded'. 'multipart/form-data' should
    // be used for binary data or data of 'significant' size. Thus, currently,
    // the operation need to state that specific mime-type in 'consumes', or
    // the type of the parameter is 'file'.
    let request_with_form_and_file = if let Some(file) = &http_operation.parameters.file_data {
        let file_form = reqwest::blocking::multipart::Form::new();
        let mut payload = reqwest::blocking::multipart::Part::bytes("Uninitialized".as_bytes());
        let mut param_name = "Uninitialized".to_string();
        // Currently, only support one file parameter
        for (k, v) in file {
            payload = reqwest::blocking::multipart::Part::bytes(v.as_bytes().to_owned())
                .file_name("foo.bar")
                .mime_str("application/octet-stream")
                .unwrap();
            param_name = k.to_string();
        }

        let final_form = file_form.part(param_name, payload);
        request_with_form_data.multipart(final_form)
    } else {
        request_with_form_data
    };

    if let Some(body) = &http_operation.parameters.body {
        // TODO: respect the operations "consumes" mime type
        request_with_form_and_file.json(&body)
    } else {
        request_with_form_and_file
    }
}
