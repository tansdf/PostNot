use std::{
    collections::{HashMap, VecDeque},
    path::Path,
    sync::{Arc, LazyLock, Mutex},
    time::{Duration, Instant},
};

use chrono::Utc;
use reqwest::header::{HeaderName, HeaderValue};
use reqwest::redirect::Policy;
use reqwest::{multipart, Body, Client, Method, Response};
use tokio::io::AsyncWriteExt;
use tokio::sync::watch;
use tokio_util::io::ReaderStream;
use url::Url;

use crate::domain::{
    requests::{KeyValueRow, ResponseBody, ResponsePayload, SendRequestPayload},
    settings::AppSettings,
};
use crate::error::{AppError, AppResult};
use crate::services::request_url_service::normalize_request_url;
use crate::services::response_body_service::{
    decode_text, describe_inline, ResponseBodyStore, ResponseRowIndexBuilder, BODY_PREVIEW_LIMIT,
    INLINE_BODY_LIMIT,
};

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

struct TemporaryBodyCleanup(Option<std::path::PathBuf>);

impl TemporaryBodyCleanup {
    fn new() -> Self {
        Self(None)
    }
    fn track(&mut self, path: std::path::PathBuf) {
        self.0 = Some(path);
    }
    fn disarm(&mut self) {
        self.0 = None;
    }
}

impl Drop for TemporaryBodyCleanup {
    fn drop(&mut self) {
        if let Some(path) = self.0.take() {
            let _ = std::fs::remove_file(path);
        }
    }
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
    send_request_with_store(payload, settings, cancel_rx, progress_sink, None).await
}

pub async fn send_request_with_store(
    payload: &SendRequestPayload,
    settings: &AppSettings,
    cancel_rx: watch::Receiver<bool>,
    progress_sink: Option<ResponseProgressSink>,
    body_store: Option<&ResponseBodyStore>,
) -> AppResult<ResponsePayload> {
    let mut cancel_rx = cancel_rx;
    let client = client_for_settings(settings)?;
    let mut url = Url::parse(&normalize_request_url(&payload.url))?;

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

                let source_file = tokio::select! {
                    file = tokio::fs::File::open(&file.path) => file?,
                    _ = wait_for_cancellation(&mut cancel_rx) => return Err(AppError::Cancelled),
                };
                let metadata = source_file.metadata().await?;
                let file_name = Path::new(&file.path)
                    .file_name()
                    .and_then(|name| name.to_str())
                    .filter(|name| !name.trim().is_empty())
                    .unwrap_or(field_name)
                    .to_string();
                let stream = ReaderStream::new(source_file);
                let part =
                    multipart::Part::stream_with_length(Body::wrap_stream(stream), metadata.len())
                        .file_name(file_name);
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
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
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

    let downloaded = read_response_body(
        &mut response,
        &mut cancel_rx,
        progress_sink.as_ref(),
        content_length,
        body_store,
        content_type.clone(),
    )
    .await?;
    let body_size = downloaded.size_bytes();

    if let Some(progress_sink) = progress_sink.as_ref() {
        progress_sink(ResponseDownloadProgress {
            downloaded_bytes: usize::try_from(body_size).unwrap_or(usize::MAX),
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
        body: downloaded,
        error_text: String::new(),
        executed_at: Utc::now().to_rfc3339(),
    })
}

async fn read_response_body(
    response: &mut Response,
    cancel_rx: &mut watch::Receiver<bool>,
    progress_sink: Option<&ResponseProgressSink>,
    content_length: Option<u64>,
    body_store: Option<&ResponseBodyStore>,
    content_type: Option<String>,
) -> AppResult<ResponseBody> {
    let mut bytes = Vec::new();
    if let Some(length) = content_length.and_then(|value| usize::try_from(value).ok()) {
        bytes.reserve(length.min(INLINE_BODY_LIMIT + 1));
    }
    let mut preview = Vec::with_capacity(BODY_PREVIEW_LIMIT);
    let mut spill: Option<(String, std::path::PathBuf, tokio::fs::File)> = None;
    let mut downloaded_bytes = 0usize;
    let mut last_progress_emit = Instant::now();
    let mut row_index = body_store.map(|_| ResponseRowIndexBuilder::new());
    let mut cleanup = TemporaryBodyCleanup::new();

    loop {
        let chunk = tokio::select! {
            chunk = response.chunk() => chunk?,
            _ = wait_for_cancellation(cancel_rx) => return Err(AppError::Cancelled),
        };

        let Some(chunk) = chunk else {
            if let Some((handle_id, path, mut file)) = spill {
                file.flush().await?;
                drop(file);
                let store = body_store.expect("spill requires body store");
                let response_body = store.register_temporary_with_index(
                    handle_id,
                    path,
                    content_type,
                    &preview,
                    downloaded_bytes as u64,
                    row_index
                        .take()
                        .expect("file-backed response has row index")
                        .finish(),
                )?;
                cleanup.disarm();
                return Ok(response_body.into());
            }
            let descriptor = describe_inline(&bytes, content_type);
            if body_store.is_some()
                && matches!(
                    descriptor.presentation,
                    crate::services::response_body_service::ResponsePresentation::Image
                        | crate::services::response_body_service::ResponsePresentation::Binary
                )
            {
                return Ok(body_store
                    .expect("checked body store")
                    .store_bytes(&bytes, descriptor.content_type.as_deref())
                    .await?
                    .into());
            }
            return Ok(ResponseBody::Inline {
                text: decode_text(&bytes, descriptor.charset.as_deref()),
                size_bytes: downloaded_bytes as u64,
                content_type: descriptor.content_type,
                charset: descriptor.charset,
                presentation: descriptor.presentation,
            });
        };

        downloaded_bytes = downloaded_bytes.saturating_add(chunk.len());
        if let Some(index) = row_index.as_mut() {
            index.push(&chunk);
        }
        if preview.len() < BODY_PREVIEW_LIMIT {
            let remaining = BODY_PREVIEW_LIMIT - preview.len();
            preview.extend_from_slice(&chunk[..chunk.len().min(remaining)]);
        }

        if spill.is_none()
            && body_store.is_some()
            && bytes.len().saturating_add(chunk.len()) > INLINE_BODY_LIMIT
        {
            let store = body_store.expect("checked body store");
            tokio::fs::create_dir_all(store.root()).await?;
            let handle_id = uuid::Uuid::new_v4().to_string();
            let path = store.root().join(format!("{handle_id}.body"));
            let mut file = tokio::fs::File::create(&path).await?;
            cleanup.track(path.clone());
            file.write_all(&bytes).await?;
            bytes.clear();
            spill = Some((handle_id, path, file));
        }

        if let Some((_, _, file)) = spill.as_mut() {
            file.write_all(&chunk).await?;
        } else {
            bytes.extend_from_slice(&chunk);
        }

        if let Some(progress_sink) =
            progress_sink.filter(|_| last_progress_emit.elapsed() >= Duration::from_millis(100))
        {
            progress_sink(ResponseDownloadProgress {
                downloaded_bytes,
                content_length,
                finished: false,
            });
            last_progress_emit = Instant::now();
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
    use super::{
        send_request, send_request_with_store, ResponseDownloadProgress, ResponseProgressSink,
    };
    use crate::domain::{
        requests::{KeyValueRow, RequestAuth, RequestBody, SendRequestPayload},
        settings::AppSettings,
    };
    use crate::services::response_body_service::ResponseBodyStore;
    use std::{
        io::{Read, Write},
        net::{TcpListener, TcpStream},
        sync::{mpsc, Arc, Mutex},
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
        assert_eq!(response.body.inline_text(), Some("{\"ok\":true}"));
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
    async fn send_request_allows_localhost_without_scheme() {
        let (url, captured_rx) =
            spawn_test_server("HTTP/1.1 200 OK\r\ncontent-length: 2\r\n\r\nok");
        let port = url.rsplit_once(':').expect("test server url has port").1;
        let (_cancel_tx, cancel_rx) = watch::channel(false);

        let response = send_request(
            &SendRequestPayload {
                name: "Localhost".to_string(),
                method: "GET".to_string(),
                url: format!("localhost:{port}/health"),
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
        .expect("localhost without scheme should send");

        let captured = captured_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("server captured request");

        assert_eq!(response.status_code, Some(200));
        assert_eq!(response.body.inline_text(), Some("ok"));
        assert!(captured.head.starts_with("GET /health HTTP/1.1"));
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
        let body_text = response
            .body
            .inline_text()
            .expect("test response is inline");
        assert_eq!(response.size_bytes, body.len() as u64);
        assert_eq!(body_text.len(), body.len());
        assert!(body_text.starts_with("{\"data\":\"xxx"));
        assert!(body_text.ends_with("\"}"));
    }

    #[tokio::test]
    async fn send_request_spills_large_response_to_managed_file() {
        let body = format!("{{\"data\":\"{}\"}}", "x".repeat(2 * 1024 * 1024));
        let response_message = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        let (url, _captured_rx) = spawn_test_server(&response_message);
        let (_cancel_tx, cancel_rx) = watch::channel(false);
        let root = std::env::temp_dir().join(format!("postnot-http-body-{}", uuid::Uuid::new_v4()));
        let store = ResponseBodyStore::new(root.clone());
        let response = send_request_with_store(
            &SendRequestPayload {
                name: "Large file-backed JSON".into(),
                method: "GET".into(),
                url: format!("{url}/large-file"),
                query_params: Vec::new(),
                headers: Vec::new(),
                body: RequestBody {
                    mode: "none".into(),
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
            Some(&store),
        )
        .await
        .expect("large response should succeed");
        let handle = response
            .body
            .handle_id()
            .expect("large response is file-backed");
        assert_eq!(response.size_bytes, body.len() as u64);
        assert_eq!(store.read_all_text(handle).await.unwrap().len(), body.len());
        store.release(handle).unwrap();
        std::fs::remove_dir_all(root).ok();
    }

    #[tokio::test]
    async fn send_request_keeps_small_binary_response_file_backed() {
        let response_message =
            "HTTP/1.1 200 OK\r\ncontent-type: image/png\r\ncontent-length: 4\r\n\r\nPNG!";
        let (url, _captured_rx) = spawn_test_server(response_message);
        let (_cancel_tx, cancel_rx) = watch::channel(false);
        let root =
            std::env::temp_dir().join(format!("postnot-http-binary-{}", uuid::Uuid::new_v4()));
        let store = ResponseBodyStore::new(root.clone());
        let response = send_request_with_store(
            &SendRequestPayload {
                name: "Small image".into(),
                method: "GET".into(),
                url,
                query_params: Vec::new(),
                headers: Vec::new(),
                body: RequestBody {
                    mode: "none".into(),
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
            Some(&store),
        )
        .await
        .expect("small image response");
        let handle = response
            .body
            .handle_id()
            .expect("binary response is file-backed");
        assert_eq!(
            tokio::fs::read(store.path_for(handle).unwrap())
                .await
                .unwrap(),
            b"PNG!"
        );
        store.release(handle).unwrap();
        std::fs::remove_dir_all(root).ok();
    }

    #[tokio::test]
    async fn send_request_keeps_response_progress_total_stable() {
        let body = "x".repeat(512 * 1024);
        let response_message = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: text/plain\r\ncontent-length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        let (url, _captured_rx) = spawn_test_server(&response_message);
        let (_cancel_tx, cancel_rx) = watch::channel(false);
        let progress_events = Arc::new(Mutex::new(Vec::<ResponseDownloadProgress>::new()));
        let captured_progress_events = Arc::clone(&progress_events);
        let progress_sink: ResponseProgressSink = Arc::new(move |progress| {
            captured_progress_events
                .lock()
                .expect("progress events lock")
                .push(progress);
        });

        let response = send_request(
            &SendRequestPayload {
                name: "Progress".to_string(),
                method: "GET".to_string(),
                url: format!("{url}/progress"),
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
            Some(progress_sink),
        )
        .await
        .expect("response should succeed");

        let events = progress_events.lock().expect("progress events lock");
        assert_eq!(response.size_bytes, body.len() as u64);
        assert!(
            events.len() >= 2,
            "expected initial and finished progress events"
        );
        assert_eq!(
            events.first().unwrap().content_length,
            Some(body.len() as u64)
        );
        assert!(events.last().unwrap().finished);
        assert!(events
            .iter()
            .all(|event| event.content_length == Some(body.len() as u64)));
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
        let body_text = response
            .body
            .inline_text()
            .expect("test response is inline");
        assert_eq!(
            body_text,
            format!("{{\"compressed\":true,\"data\":\"{}\"}}", "x".repeat(128))
        );
        assert_eq!(response.size_bytes, body_text.len() as u64);
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
