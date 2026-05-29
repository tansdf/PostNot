use std::{
    collections::{HashMap, VecDeque},
    path::Path,
    sync::{Arc, LazyLock, Mutex},
    time::{Duration, Instant},
};

use chrono::Utc;
use reqwest::header::{HeaderName, HeaderValue};
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

#[derive(Clone, Debug)]
pub struct ResponseDownloadProgress {
    pub downloaded_bytes: usize,
    pub content_length: Option<u64>,
    pub finished: bool,
}

pub type ResponseProgressSink = Arc<dyn Fn(ResponseDownloadProgress) + Send + Sync>;

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
    progress_sink: Option<ResponseProgressSink>,
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

    if let Some(progress_sink) = progress_sink.as_ref() {
        progress_sink(ResponseDownloadProgress {
            downloaded_bytes: 0,
            content_length,
            finished: false,
        });
    }

    let body_bytes = read_full_body(&mut response, &mut cancel_rx, progress_sink.as_ref()).await?;
    let body_size = content_length
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or(body_bytes.len());
    let body_text = String::from_utf8_lossy(&body_bytes).to_string();

    if let Some(progress_sink) = progress_sink.as_ref() {
        progress_sink(ResponseDownloadProgress {
            downloaded_bytes: body_bytes.len(),
            content_length,
            finished: true,
        });
    }

    Ok(ResponsePayload {
        status_code: Some(status.as_u16()),
        status_text: status.canonical_reason().unwrap_or_default().to_string(),
        duration_ms: started_at.elapsed().as_millis(),
        size_bytes: body_size,
        headers,
        body_text,
        error_text: String::new(),
        executed_at: Utc::now().to_rfc3339(),
    })
}

async fn read_full_body(
    response: &mut Response,
    cancel_rx: &mut watch::Receiver<bool>,
    progress_sink: Option<&ResponseProgressSink>,
) -> AppResult<Vec<u8>> {
    let mut bytes = Vec::new();

    loop {
        let chunk = tokio::select! {
            chunk = response.chunk() => chunk?,
            _ = wait_for_cancellation(cancel_rx) => return Err(AppError::Cancelled),
        };

        let Some(chunk) = chunk else {
            return Ok(bytes);
        };

        bytes.extend_from_slice(&chunk);

        if let Some(progress_sink) = progress_sink {
            progress_sink(ResponseDownloadProgress {
                downloaded_bytes: bytes.len(),
                content_length: response.content_length(),
                finished: false,
            });
        }
    }
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
    use super::send_request;
    use crate::domain::{
        requests::{KeyValueRow, RequestAuth, RequestBody, SendRequestPayload},
        settings::AppSettings,
    };
    use std::{
        io::{Read, Write},
        net::{TcpListener, TcpStream},
        sync::mpsc,
        thread,
        time::Duration,
    };
    use tokio::sync::watch;

    struct CapturedRequest {
        head: String,
        body: Vec<u8>,
    }

    #[tokio::test]
    async fn send_request_writes_method_query_headers_and_body() {
        let (url, captured_rx) = spawn_test_server(
            "HTTP/1.1 201 Created\r\ncontent-type: application/json\r\ncontent-length: 11\r\n\r\n{\"ok\":true}",
        );
        let (_cancel_tx, cancel_rx) = watch::channel(false);

        let response = send_request(
            &SendRequestPayload {
                name: "Create thing".to_string(),
                method: "POST".to_string(),
                url: format!("{url}/submit?existing=1"),
                query_params: vec![row("query-1", "alpha", "a b")],
                headers: vec![row("header-1", "x-postnot-test", "yes")],
                body: RequestBody {
                    mode: "json".to_string(),
                    raw: "{\"name\":\"demo\"}".to_string(),
                    form: Vec::new(),
                    files: Vec::new(),
                },
                auth: RequestAuth {
                    auth_type: "bearer".to_string(),
                    bearer_token: "token-123".to_string(),
                    ..empty_auth()
                },
                pre_request_script: String::new(),
                test_script: String::new(),
            },
            &default_settings(),
            cancel_rx,
            None,
        )
        .await
        .expect("request should succeed");

        let captured = captured_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("server captured request");

        assert_eq!(response.status_code, Some(201));
        assert_eq!(response.status_text, "Created");
        assert_eq!(response.body_text, "{\"ok\":true}");
        assert_eq!(response.size_bytes, 11);
        assert!(captured
            .head
            .starts_with("POST /submit?existing=1&alpha=a+b HTTP/1.1"));
        assert!(captured.head.contains("\r\nx-postnot-test: yes\r\n"));
        assert!(captured
            .head
            .contains("\r\nauthorization: Bearer token-123\r\n"));
        assert!(captured
            .head
            .contains("\r\ncontent-type: application/json\r\n"));
        assert_eq!(
            String::from_utf8(captured.body).unwrap(),
            "{\"name\":\"demo\"}"
        );
    }

    #[tokio::test]
    async fn send_request_decodes_large_json_body_even_with_binary_content_type() {
        let body = format!("{{\"data\":\"{}\"}}", "x".repeat(5 * 1024 * 1024 + 1024));
        let response_message = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/octet-stream\r\ncontent-length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        let (url, _captured_rx) = spawn_test_server(&response_message);
        let (_cancel_tx, cancel_rx) = watch::channel(false);

        let response = send_request(
            &SendRequestPayload {
                name: "Large JSON".to_string(),
                method: "GET".to_string(),
                url: format!("{url}/large-json"),
                query_params: Vec::new(),
                headers: Vec::new(),
                body: RequestBody {
                    mode: "none".to_string(),
                    raw: String::new(),
                    form: Vec::new(),
                    files: Vec::new(),
                },
                auth: empty_auth(),
                pre_request_script: String::new(),
                test_script: String::new(),
            },
            &default_settings(),
            cancel_rx,
            None,
        )
        .await
        .expect("large response should succeed");

        assert_eq!(response.status_code, Some(200));
        assert_eq!(response.size_bytes, body.len());
        assert_eq!(response.body_text.len(), body.len());
        assert!(response.body_text.starts_with("{\"data\":\"xxx"));
        assert!(response.body_text.ends_with("\"}"));
    }

    #[tokio::test]
    async fn send_request_decodes_gzip_response_when_accept_encoding_is_forwarded() {
        let compressed_body = [
            31, 139, 8, 0, 28, 81, 243, 105, 2, 255, 171, 86, 74, 206, 207, 45, 40, 74, 45, 46, 78,
            77, 81, 178, 42, 41, 42, 77, 213, 81, 74, 73, 44, 73, 84, 178, 82, 170, 24, 96, 160,
            84, 11, 0, 103, 137, 26, 254, 157, 0, 0, 0,
        ];
        let response_message = http_response_bytes(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-encoding: gzip\r\ncontent-length: 52\r\n\r\n",
            &compressed_body,
        );
        let (url, captured_rx) = spawn_test_server_bytes(response_message);
        let (_cancel_tx, cancel_rx) = watch::channel(false);

        let response = send_request(
            &SendRequestPayload {
                name: "Compressed JSON".to_string(),
                method: "GET".to_string(),
                url: format!("{url}/compressed-json"),
                query_params: Vec::new(),
                headers: vec![row("header-1", "Accept-Encoding", "gzip, deflate, br")],
                body: RequestBody {
                    mode: "none".to_string(),
                    raw: String::new(),
                    form: Vec::new(),
                    files: Vec::new(),
                },
                auth: empty_auth(),
                pre_request_script: String::new(),
                test_script: String::new(),
            },
            &default_settings(),
            cancel_rx,
            None,
        )
        .await
        .expect("compressed response should decode");

        let captured = captured_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("server captured request");

        assert!(captured
            .head
            .contains("\r\naccept-encoding: gzip, deflate, br\r\n"));
        assert_eq!(
            response.body_text,
            format!("{{\"compressed\":true,\"data\":\"{}\"}}", "x".repeat(128))
        );
        assert_eq!(response.size_bytes, response.body_text.len());
    }

    fn spawn_test_server(response: &str) -> (String, mpsc::Receiver<CapturedRequest>) {
        spawn_test_server_bytes(response.as_bytes().to_vec())
    }

    fn spawn_test_server_bytes(response: Vec<u8>) -> (String, mpsc::Receiver<CapturedRequest>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
        let address = listener.local_addr().expect("read test server address");
        let (captured_tx, captured_rx) = mpsc::channel();

        thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept request");
            let captured = read_http_request(&mut stream);
            captured_tx.send(captured).expect("send captured request");
            stream.write_all(&response).expect("write response");
            stream.flush().expect("flush response");
        });

        (format!("http://{address}"), captured_rx)
    }

    fn http_response_bytes(head: &str, body: &[u8]) -> Vec<u8> {
        let mut response = head.as_bytes().to_vec();
        response.extend_from_slice(body);
        response
    }

    fn read_http_request(stream: &mut TcpStream) -> CapturedRequest {
        let mut buffer = Vec::new();
        let mut chunk = [0_u8; 4096];

        loop {
            let read = stream.read(&mut chunk).expect("read request");
            assert!(
                read > 0,
                "connection closed before request headers finished"
            );
            buffer.extend_from_slice(&chunk[..read]);

            if buffer.windows(4).any(|window| window == b"\r\n\r\n") {
                break;
            }
        }

        let header_end = buffer
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .expect("header terminator")
            + 4;
        let head = String::from_utf8(buffer[..header_end].to_vec()).expect("utf8 headers");
        let content_length = content_length(&head);
        let mut body = buffer[header_end..].to_vec();

        while body.len() < content_length {
            let read = stream.read(&mut chunk).expect("read request body");
            assert!(read > 0, "connection closed before request body finished");
            body.extend_from_slice(&chunk[..read]);
        }

        body.truncate(content_length);
        CapturedRequest { head, body }
    }

    fn content_length(head: &str) -> usize {
        head.lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse().expect("valid content-length"))
            })
            .unwrap_or(0)
    }

    fn default_settings() -> AppSettings {
        AppSettings {
            theme: "system".to_string(),
            ui_scale: 1.0,
            request_timeout_ms: 30_000,
            follow_redirects: true,
            validate_tls: true,
            history_limit: 200,
            is_history_collapsed: false,
            environment_autosave: true,
            notification_timeout_ms: 5_000,
            last_update_checked_at: None,
        }
    }

    fn empty_auth() -> RequestAuth {
        RequestAuth {
            auth_type: "none".to_string(),
            basic_username: String::new(),
            basic_password: String::new(),
            bearer_token: String::new(),
            api_key_name: String::new(),
            api_key_value: String::new(),
            api_key_in: "header".to_string(),
            oauth2_access_token: String::new(),
            oauth2_token_url: String::new(),
            oauth2_client_id: String::new(),
            oauth2_client_secret: String::new(),
            oauth2_scope: String::new(),
        }
    }

    fn row(id: &str, key: &str, value: &str) -> KeyValueRow {
        KeyValueRow {
            id: id.to_string(),
            key: key.to_string(),
            value: value.to_string(),
            enabled: true,
        }
    }
}
