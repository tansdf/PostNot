use std::{
    collections::{HashMap, VecDeque},
    path::Path,
    sync::{LazyLock, Mutex},
    time::{Duration, Instant},
};

use base64::{engine::general_purpose, Engine as _};
use chrono::Utc;
use reqwest::header::{HeaderName, HeaderValue, CONTENT_TYPE};
use reqwest::redirect::Policy;
use reqwest::{multipart, Client, Method, Response};
use tokio::sync::watch;
use url::Url;

use crate::domain::{
    requests::{KeyValueRow, ResponsePayload, SendRequestPayload},
    settings::AppSettings,
};
use crate::error::{AppError, AppResult};

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct ClientCacheKey {
    validate_tls: bool,
    follow_redirects: bool,
    timeout_ms: u64,
}

const HTTP_CLIENT_CACHE_MAX: usize = 32;
const RESPONSE_BODY_PREVIEW_LIMIT_BYTES: usize = 5 * 1024 * 1024;

struct ClientCache {
    clients: HashMap<ClientCacheKey, Client>,
    insertion_order: VecDeque<ClientCacheKey>,
}

static HTTP_CLIENT_CACHE: LazyLock<Mutex<ClientCache>> = LazyLock::new(|| {
    Mutex::new(ClientCache {
        clients: HashMap::new(),
        insertion_order: VecDeque::new(),
    })
});

fn client_for_settings(settings: &AppSettings) -> AppResult<Client> {
    let key = ClientCacheKey {
        validate_tls: settings.validate_tls,
        follow_redirects: settings.follow_redirects,
        timeout_ms: settings.request_timeout_ms.max(1),
    };

    let mut guard = HTTP_CLIENT_CACHE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    let ClientCache {
        clients: map,
        insertion_order: order,
    } = &mut *guard;

    if let Some(client) = map.get(&key) {
        return Ok(client.clone());
    }

    let client = Client::builder()
        .danger_accept_invalid_certs(!key.validate_tls)
        .redirect(if key.follow_redirects {
            Policy::limited(10)
        } else {
            Policy::none()
        })
        .timeout(Duration::from_millis(key.timeout_ms))
        .build()?;

    while map.len() >= HTTP_CLIENT_CACHE_MAX {
        if let Some(oldest) = order.pop_front() {
            map.remove(&oldest);
        } else {
            map.clear();
            order.clear();
            break;
        }
    }

    order.push_back(key);
    map.insert(key, client.clone());
    Ok(client)
}

pub async fn send_request(
    payload: &SendRequestPayload,
    settings: &AppSettings,
    cancel_rx: watch::Receiver<bool>,
) -> AppResult<ResponsePayload> {
    let mut cancel_rx = cancel_rx;
    let client = client_for_settings(settings)?;
    let mut url = Url::parse(&payload.url)?;

    for query in payload
        .query_params
        .iter()
        .filter(|item| item.enabled && !item.key.trim().is_empty())
    {
        url.query_pairs_mut().append_pair(&query.key, &query.value);
    }

    if payload.auth.auth_type == "api-key"
        && payload.auth.api_key_in == "query"
        && !payload.auth.api_key_name.trim().is_empty()
    {
        url.query_pairs_mut()
            .append_pair(&payload.auth.api_key_name, &payload.auth.api_key_value);
    }

    let method = Method::from_bytes(payload.method.as_bytes())
        .map_err(|error| AppError::Message(error.to_string()))?;
    let mut request = client.request(method, url);

    for header in payload
        .headers
        .iter()
        .filter(|item| item.enabled && !item.key.trim().is_empty())
    {
        let name = HeaderName::from_bytes(header.key.as_bytes())?;
        let value = HeaderValue::from_str(&header.value)?;
        request = request.header(name, value);
    }

    match payload.auth.auth_type.as_str() {
        "basic" => {
            request = request.basic_auth(
                &payload.auth.basic_username,
                Some(&payload.auth.basic_password),
            );
        }
        "bearer" => {
            request = request.bearer_auth(&payload.auth.bearer_token);
        }
        "oauth2" => {
            let token = if payload.auth.oauth2_access_token.trim().is_empty() {
                &payload.auth.bearer_token
            } else {
                &payload.auth.oauth2_access_token
            };
            request = request.bearer_auth(token);
        }
        "api-key"
            if payload.auth.api_key_in == "header"
                && !payload.auth.api_key_name.trim().is_empty() =>
        {
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

            for item in payload
                .body
                .form
                .iter()
                .filter(|entry| entry.enabled && !entry.key.trim().is_empty())
            {
                form = form.text(item.key.clone(), item.value.clone());
            }

            for file in payload
                .body
                .files
                .iter()
                .filter(|file| file.enabled && !file.path.trim().is_empty())
            {
                let field_name = match file.name.trim() {
                    "" => "file",
                    value => value,
                };

                if file.path.trim().is_empty() {
                    continue;
                }

                let bytes = tokio::select! {
                    bytes = tokio::fs::read(&file.path) => bytes?,
                    _ = wait_for_cancellation(&mut cancel_rx) => return Err(AppError::Cancelled),
                };
                let file_name = Path::new(&file.path)
                    .file_name()
                    .and_then(|name| name.to_str())
                    .filter(|name| !name.trim().is_empty())
                    .unwrap_or(field_name)
                    .to_string();
                let part = multipart::Part::bytes(bytes).file_name(file_name);
                form = form.part(field_name.to_string(), part);
            }

            request = request.multipart(form);
        }
        _ => {}
    }

    let started_at = Instant::now();
    let mut response = tokio::select! {
        response = request.send() => response?,
        _ = wait_for_cancellation(&mut cancel_rx) => return Err(AppError::Cancelled),
    };
    let status = response.status();
    let content_length = response.content_length();
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();

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

    let body_read = read_limited_body(&mut response, &mut cancel_rx).await?;
    let body_size = content_length
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or_else(|| body_read.bytes.len() + usize::from(body_read.is_truncated));
    let utf8_body = std::str::from_utf8(&body_read.bytes);
    let body_is_binary = is_binary_response_body(&body_read.bytes, &content_type);
    let should_decode_body = !body_is_binary || settings.always_decode_binary_response_bodies;
    let (body_text, body_encoding) = if should_decode_body {
        match utf8_body {
            Ok(text) => (text.to_string(), "utf-8".to_string()),
            Err(_) => (
                String::from_utf8_lossy(&body_read.bytes).to_string(),
                "lossy-utf8".to_string(),
            ),
        }
    } else {
        (String::new(), "not-decoded".to_string())
    };
    let body_base64 = if body_is_binary && !body_read.bytes.is_empty() {
        general_purpose::STANDARD.encode(&body_read.bytes)
    } else {
        String::new()
    };

    Ok(ResponsePayload {
        status_code: Some(status.as_u16()),
        status_text: status.canonical_reason().unwrap_or_default().to_string(),
        duration_ms: started_at.elapsed().as_millis(),
        size_bytes: body_size,
        headers,
        body_text,
        body_base64,
        body_content_type: content_type,
        body_is_binary,
        body_is_truncated: body_read.is_truncated,
        body_truncated_at_bytes: body_read
            .is_truncated
            .then_some(RESPONSE_BODY_PREVIEW_LIMIT_BYTES),
        body_encoding,
        error_text: String::new(),
        executed_at: Utc::now().to_rfc3339(),
    })
}

struct LimitedBody {
    bytes: Vec<u8>,
    is_truncated: bool,
}

async fn read_limited_body(
    response: &mut Response,
    cancel_rx: &mut watch::Receiver<bool>,
) -> AppResult<LimitedBody> {
    let mut bytes = Vec::new();
    let mut is_truncated = false;

    while bytes.len() < RESPONSE_BODY_PREVIEW_LIMIT_BYTES {
        let chunk = tokio::select! {
            chunk = response.chunk() => chunk?,
            _ = wait_for_cancellation(cancel_rx) => return Err(AppError::Cancelled),
        };

        let Some(chunk) = chunk else {
            return Ok(LimitedBody {
                bytes,
                is_truncated,
            });
        };

        let remaining = RESPONSE_BODY_PREVIEW_LIMIT_BYTES - bytes.len();
        if chunk.len() > remaining {
            bytes.extend_from_slice(&chunk[..remaining]);
            is_truncated = true;
            break;
        }

        bytes.extend_from_slice(&chunk);
    }

    if !is_truncated {
        let next_chunk = tokio::select! {
            chunk = response.chunk() => chunk?,
            _ = wait_for_cancellation(cancel_rx) => return Err(AppError::Cancelled),
        };
        is_truncated = next_chunk.is_some();
    }

    Ok(LimitedBody {
        bytes,
        is_truncated,
    })
}

fn is_text_content_type(content_type: &str) -> bool {
    let media_type = content_type
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();

    media_type.starts_with("text/")
        || matches!(
            media_type.as_str(),
            "application/json"
                | "application/problem+json"
                | "application/xml"
                | "application/xhtml+xml"
                | "application/javascript"
                | "application/x-javascript"
                | "application/x-www-form-urlencoded"
                | "application/yaml"
                | "application/x-yaml"
                | "image/svg+xml"
        )
        || media_type.ends_with("+json")
        || media_type.ends_with("+xml")
}

fn is_binary_response_body(bytes: &[u8], content_type: &str) -> bool {
    if bytes.is_empty() {
        return false;
    }

    let has_content_type = !content_type.trim().is_empty();
    (has_content_type && !is_text_content_type(content_type)) || std::str::from_utf8(bytes).is_err()
}

async fn wait_for_cancellation(cancel_rx: &mut watch::Receiver<bool>) {
    if *cancel_rx.borrow() {
        return;
    }

    loop {
        if cancel_rx.changed().await.is_err() {
            return;
        }

        if *cancel_rx.borrow() {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{is_binary_response_body, is_text_content_type};

    #[test]
    fn text_content_type_detection_covers_api_formats() {
        assert!(is_text_content_type("text/plain; charset=utf-8"));
        assert!(is_text_content_type("application/json"));
        assert!(is_text_content_type("application/vnd.api+json"));
        assert!(is_text_content_type("application/xml"));
        assert!(is_text_content_type("image/svg+xml"));
        assert!(!is_text_content_type(""));
        assert!(!is_text_content_type("image/png"));
        assert!(!is_text_content_type("application/octet-stream"));
    }

    #[test]
    fn binary_response_detection_respects_content_type_and_utf8() {
        assert!(!is_binary_response_body(b"hello", ""));
        assert!(!is_binary_response_body(
            b"{\"ok\":true}",
            "application/json"
        ));
        assert!(is_binary_response_body(
            b"hello",
            "application/octet-stream"
        ));
        assert!(is_binary_response_body(&[0, 159, 146, 150], ""));
        assert!(is_binary_response_body(&[0, 159, 146, 150], "text/plain"));
        assert!(!is_binary_response_body(&[], "application/octet-stream"));
    }
}
