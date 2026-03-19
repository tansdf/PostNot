use std::time::Instant;

use chrono::Utc;
use reqwest::header::{HeaderName, HeaderValue};
use reqwest::{multipart, Client, Method};
use url::Url;

use crate::domain::requests::{KeyValueRow, ResponsePayload, SendRequestPayload};
use crate::error::AppResult;

pub async fn send_request(payload: &SendRequestPayload) -> AppResult<ResponsePayload> {
    let client = Client::builder().build()?;
    let mut url = Url::parse(&payload.url)?;

    for query in payload.query_params.iter().filter(|item| item.enabled && !item.key.trim().is_empty()) {
        url.query_pairs_mut().append_pair(&query.key, &query.value);
    }

    if payload.auth.auth_type == "api-key"
        && payload.auth.api_key_in == "query"
        && !payload.auth.api_key_name.trim().is_empty()
    {
        url.query_pairs_mut()
            .append_pair(&payload.auth.api_key_name, &payload.auth.api_key_value);
    }

    let method = Method::from_bytes(payload.method.as_bytes())?;
    let mut request = client.request(method, url);

    for header in payload.headers.iter().filter(|item| item.enabled && !item.key.trim().is_empty()) {
        let name = HeaderName::from_bytes(header.key.as_bytes())?;
        let value = HeaderValue::from_str(&header.value)?;
        request = request.header(name, value);
    }

    match payload.auth.auth_type.as_str() {
        "basic" => {
            request = request.basic_auth(&payload.auth.basic_username, Some(&payload.auth.basic_password));
        }
        "bearer" => {
            request = request.bearer_auth(&payload.auth.bearer_token);
        }
        "api-key" if payload.auth.api_key_in == "header" && !payload.auth.api_key_name.trim().is_empty() => {
            request = request.header(&payload.auth.api_key_name, &payload.auth.api_key_value);
        }
        _ => {}
    }

    match payload.body.mode.as_str() {
        "json" => {
            request = request.header("content-type", "application/json");
            request = request.body(payload.body.raw.clone());
        }
        "raw" => {
            request = request.body(payload.body.raw.clone());
        }
        "form-urlencoded" => {
            let form_fields: Vec<(String, String)> = payload
                .body
                .form
                .iter()
                .filter(|item| item.enabled && !item.key.trim().is_empty())
                .map(|item| (item.key.clone(), item.value.clone()))
                .collect();
            request = request.form(&form_fields);
        }
        "multipart" => {
            let mut form = multipart::Form::new();

            for item in payload.body.form.iter().filter(|entry| entry.enabled && !entry.key.trim().is_empty()) {
                form = form.text(item.key.clone(), item.value.clone());
            }

            for file in &payload.body.files {
                if file.path.trim().is_empty() {
                    continue;
                }

                let bytes = tokio::fs::read(&file.path).await?;
                let part = multipart::Part::bytes(bytes).file_name(file.name.clone());
                form = form.part(file.name.clone(), part);
            }

            request = request.multipart(form);
        }
        _ => {}
    }

    let started_at = Instant::now();
    let response = request.send().await?;
    let status = response.status();

    let headers = response
        .headers()
        .iter()
        .enumerate()
        .map(|(index, (name, value))| KeyValueRow {
            id: format!("header-{index}"),
            key: name.to_string(),
            value: value.to_str().unwrap_or_default().to_string(),
            enabled: true,
        })
        .collect();

    let body_bytes = response.bytes().await?;

    Ok(ResponsePayload {
        status_code: Some(status.as_u16()),
        status_text: status.canonical_reason().unwrap_or_default().to_string(),
        duration_ms: started_at.elapsed().as_millis(),
        size_bytes: body_bytes.len(),
        headers,
        body_text: String::from_utf8_lossy(&body_bytes).to_string(),
        error_text: String::new(),
        executed_at: Utc::now().to_rfc3339(),
    })
}
