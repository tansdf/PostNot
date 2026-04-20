use std::{
    collections::{HashMap, VecDeque},
    path::Path,
    sync::{LazyLock, Mutex},
    time::{Duration, Instant},
};

use chrono::Utc;
use reqwest::header::{HeaderName, HeaderValue};
use reqwest::redirect::Policy;
use reqwest::{multipart, Client, Method};
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
    let response = tokio::select! {
        response = request.send() => response?,
        _ = wait_for_cancellation(&mut cancel_rx) => return Err(AppError::Cancelled),
    };
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

    let body_bytes = tokio::select! {
        body = response.bytes() => body?,
        _ = wait_for_cancellation(&mut cancel_rx) => return Err(AppError::Cancelled),
    };

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
