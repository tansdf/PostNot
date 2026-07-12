use sqlx::SqlitePool;
use url::Url;
use uuid::Uuid;

use crate::{
    domain::{
        collections::CreateCollectionInput,
        imports::{ImportDetails, ImportResult},
        requests::{FileRow, KeyValueRow, RequestAuth, RequestBody, SendRequestPayload},
    },
    error::{AppError, AppResult},
    services::{collections_service, request_url_service::normalize_request_url},
};

use super::shared::{empty_auth, empty_kv, normalize_method};

pub(super) async fn import_curl_request(
    pool: &SqlitePool,
    source: &str,
    target_collection_id: Option<&str>,
) -> AppResult<ImportResult> {
    let request = parse_curl_command(source)?;

    if let Some(collection_id) = target_collection_id {
        let collection = collections_service::list_collections(pool)
            .await?
            .into_iter()
            .find(|item| item.id == collection_id)
            .ok_or_else(|| AppError::Message("Target collection not found.".to_string()))?;

        collections_service::save_request(pool, &collection.id, None, &request).await?;

        return Ok(ImportResult {
            collection_id: collection.id,
            collection_name: collection.name,
            imported_request_count: 1,
            created_collection: false,
            details: Some(ImportDetails {
                format: "curl".to_string(),
                summary: "1 request imported from cURL.".to_string(),
                imported_items: vec![request.name.clone()],
                warnings: Vec::new(),
                errors: Vec::new(),
            }),
        });
    }

    let created = collections_service::import_collection_atomic(
        pool,
        &CreateCollectionInput {
            name: "Imported cURL".to_string(),
            description: "Requests imported from cURL.".to_string(),
            pre_request_script: String::new(),
            test_script: String::new(),
        },
        &[],
        &[collections_service::ImportCollectionRequest {
            parent_id: None,
            sort_order: 0,
            request: request.clone(),
        }],
    )
    .await?;

    Ok(ImportResult {
        collection_id: created.id,
        collection_name: created.name,
        imported_request_count: 1,
        created_collection: true,
        details: Some(ImportDetails {
            format: "curl".to_string(),
            summary: "1 request imported from cURL.".to_string(),
            imported_items: vec![request.name],
            warnings: Vec::new(),
            errors: Vec::new(),
        }),
    })
}

pub(super) fn parse_curl_command(source: &str) -> AppResult<SendRequestPayload> {
    let normalized_source = normalize_shell_continuations(source);
    let parts = shlex::split(&normalized_source)
        .ok_or_else(|| AppError::Message("Invalid cURL command.".to_string()))?;
    if parts.is_empty() || parts[0] != "curl" {
        return Err(AppError::Message(
            "Paste a complete cURL command starting with `curl`.".to_string(),
        ));
    }

    let mut method = "GET".to_string();
    let mut url = String::new();
    let mut headers = Vec::new();
    let mut data_parts: Vec<String> = Vec::new();
    let mut body_mode = "none".to_string();
    let mut form_rows: Vec<KeyValueRow> = Vec::new();
    let mut file_rows: Vec<FileRow> = Vec::new();
    let mut force_get = false;
    let mut compressed = false;
    let mut auth = empty_auth();
    let mut i = 1usize;

    while i < parts.len() {
        let (flag, inline_value) = split_long_flag(&parts[i]);
        match flag.as_str() {
            "-X" | "--request" => {
                if let Some(value) = inline_value.or_else(|| next_value(&parts, &mut i)) {
                    method = normalize_method(value);
                }
            }
            "--url" => {
                if let Some(value) = inline_value.or_else(|| next_value(&parts, &mut i)) {
                    url = value.to_string();
                }
            }
            "-G" | "--get" => {
                force_get = true;
                method = "GET".to_string();
            }
            "-L" | "--location" | "--location-trusted" => {}
            "--compressed" => {
                compressed = true;
            }
            "-H" | "--header" => {
                if let Some(value) = inline_value.or_else(|| next_value(&parts, &mut i)) {
                    apply_header(value, &mut headers, &mut auth);
                }
            }
            "-A" | "--user-agent" => {
                if let Some(value) = inline_value.or_else(|| next_value(&parts, &mut i)) {
                    upsert_header(&mut headers, "User-Agent", value);
                }
            }
            "-e" | "--referer" => {
                if let Some(value) = inline_value.or_else(|| next_value(&parts, &mut i)) {
                    upsert_header(&mut headers, "Referer", value);
                }
            }
            "-b" | "--cookie" => {
                if let Some(value) = inline_value.or_else(|| next_value(&parts, &mut i)) {
                    if !value.trim_start().starts_with('@') {
                        upsert_header(&mut headers, "Cookie", value);
                    }
                }
            }
            "-c" | "--cookie-jar" => {
                let _ = inline_value.or_else(|| next_value(&parts, &mut i));
            }
            "-d" | "--data" | "--data-raw" | "--data-binary" | "--data-urlencode"
            | "--data-ascii" => {
                if let Some(value) = inline_value.or_else(|| next_value(&parts, &mut i)) {
                    data_parts.push(value.to_string());
                    if body_mode == "none" {
                        body_mode = if looks_like_json(value) {
                            "json".to_string()
                        } else {
                            "raw".to_string()
                        };
                    }
                    if method == "GET" && !force_get {
                        method = "POST".to_string();
                    }
                }
            }
            "-F" | "--form" | "--form-string" => {
                if let Some(value) = inline_value.or_else(|| next_value(&parts, &mut i)) {
                    apply_form_part(value, &mut form_rows, &mut file_rows);
                    body_mode = "multipart".to_string();
                    if method == "GET" && !force_get {
                        method = "POST".to_string();
                    }
                }
            }
            "-u" | "--user" => {
                if let Some(value) = inline_value.or_else(|| next_value(&parts, &mut i)) {
                    let (username, password) = value.split_once(':').unwrap_or((value, ""));
                    auth = RequestAuth {
                        auth_type: "basic".to_string(),
                        basic_username: username.to_string(),
                        basic_password: password.to_string(),
                        ..empty_auth()
                    };
                }
            }
            value if is_request_url(value) => {
                url = value.to_string();
            }
            _ => {}
        }

        i += 1;
    }

    if url.is_empty() {
        return Err(AppError::Message(
            "Could not find a request URL in the cURL command.".to_string(),
        ));
    }

    let (base_url, mut query_params) = split_url_query_params(&url);
    if force_get {
        for part in &data_parts {
            append_query_data(part, &mut query_params);
        }
        body_mode = "none".to_string();
        data_parts.clear();
    }

    if compressed && !has_header(&headers, "Accept-Encoding") {
        headers.push(kv("Accept-Encoding", "gzip, deflate, br"));
    }
    let body_raw = data_parts.join("&");

    Ok(SendRequestPayload {
        name: format!("{} {}", method, base_url),
        method,
        url: base_url,
        query_params: if query_params.is_empty() {
            vec![empty_kv()]
        } else {
            query_params
        },
        headers: if headers.is_empty() {
            vec![empty_kv()]
        } else {
            headers
        },
        body: RequestBody {
            mode: body_mode,
            raw: body_raw,
            form: if form_rows.is_empty() {
                vec![empty_kv()]
            } else {
                form_rows
            },
            files: file_rows,
        },
        auth,
        pre_request_script: String::new(),
        test_script: String::new(),
    })
}

fn normalize_shell_continuations(source: &str) -> String {
    let source = source.replace("\\\r\n", " ");
    let source = source.replace("\\\n", " ");
    let source = source.replace("`\r\n", " ");
    let source = source.replace("`\n", " ");
    source.replace("^\r\n", " ").replace("^\n", " ")
}

fn split_long_flag(part: &str) -> (String, Option<&str>) {
    if part.starts_with("--") {
        if let Some((flag, value)) = part.split_once('=') {
            return (flag.to_string(), Some(value));
        }
    }

    (part.to_string(), None)
}

fn next_value<'a>(parts: &'a [String], index: &mut usize) -> Option<&'a str> {
    *index += 1;
    parts.get(*index).map(String::as_str)
}

fn is_request_url(value: &str) -> bool {
    value.starts_with("http://")
        || value.starts_with("https://")
        || normalize_request_url(value) != value.trim()
}

fn split_url_query_params(url: &str) -> (String, Vec<KeyValueRow>) {
    let normalized_url = normalize_request_url(url);
    let Ok(mut parsed_url) = Url::parse(&normalized_url) else {
        return (url.to_string(), Vec::new());
    };

    let query_params = parsed_url
        .query_pairs()
        .map(|(key, value)| kv(&key, &value))
        .collect();
    parsed_url.set_query(None);

    (parsed_url.to_string(), query_params)
}

fn append_query_data(value: &str, query_params: &mut Vec<KeyValueRow>) {
    for segment in value.split('&').filter(|segment| !segment.is_empty()) {
        let (key, value) = segment.split_once('=').unwrap_or((segment, ""));
        query_params.push(kv(key, value));
    }
}

fn apply_header(value: &str, headers: &mut Vec<KeyValueRow>, auth: &mut RequestAuth) {
    let Some((key, header_value)) = value.split_once(':') else {
        return;
    };
    let key = key.trim();
    let header_value = header_value.trim();

    if key.eq_ignore_ascii_case("authorization") {
        if let Some(token) = header_value.strip_prefix("Bearer ") {
            *auth = RequestAuth {
                auth_type: "bearer".to_string(),
                bearer_token: token.trim().to_string(),
                ..empty_auth()
            };
            return;
        }
    }

    headers.push(kv(key, header_value));
}

fn apply_form_part(value: &str, form_rows: &mut Vec<KeyValueRow>, file_rows: &mut Vec<FileRow>) {
    let (name, value) = value.split_once('=').unwrap_or((value, ""));
    if let Some(path) = value.strip_prefix('@') {
        let path = path.split(';').next().unwrap_or(path);
        file_rows.push(FileRow {
            id: Uuid::new_v4().to_string(),
            name: name.trim().to_string(),
            path: path.trim().to_string(),
            enabled: true,
        });
    } else {
        form_rows.push(kv(name.trim(), value));
    }
}

fn upsert_header(headers: &mut Vec<KeyValueRow>, key: &str, value: &str) {
    if let Some(header) = headers
        .iter_mut()
        .find(|header| header.key.eq_ignore_ascii_case(key))
    {
        header.value = value.to_string();
        header.enabled = true;
    } else {
        headers.push(kv(key, value));
    }
}

fn has_header(headers: &[KeyValueRow], key: &str) -> bool {
    headers
        .iter()
        .any(|header| header.enabled && header.key.eq_ignore_ascii_case(key))
}

fn kv(key: &str, value: &str) -> KeyValueRow {
    KeyValueRow {
        id: Uuid::new_v4().to_string(),
        key: key.to_string(),
        value: value.to_string(),
        enabled: true,
    }
}

fn looks_like_json(value: &str) -> bool {
    let trimmed = value.trim();
    (trimmed.starts_with('{') && trimmed.ends_with('}'))
        || (trimmed.starts_with('[') && trimmed.ends_with(']'))
}

#[cfg(test)]
mod tests {
    use super::parse_curl_command;

    #[test]
    fn parses_url_flag_and_splits_query_params() {
        let request =
            parse_curl_command("curl --url 'https://api.example.com/items?limit=10&q=rust'")
                .unwrap();

        assert_eq!(request.method, "GET");
        assert_eq!(request.url, "https://api.example.com/items");
        assert_eq!(request.query_params.len(), 2);
        assert_eq!(request.query_params[0].key, "limit");
        assert_eq!(request.query_params[0].value, "10");
        assert_eq!(request.query_params[1].key, "q");
        assert_eq!(request.query_params[1].value, "rust");
    }

    #[test]
    fn parses_bare_localhost_url_and_splits_query_params() {
        let request = parse_curl_command("curl localhost:3000/health?ready=true").unwrap();

        assert_eq!(request.method, "GET");
        assert_eq!(request.url, "http://localhost:3000/health");
        assert_eq!(request.query_params.len(), 1);
        assert_eq!(request.query_params[0].key, "ready");
        assert_eq!(request.query_params[0].value, "true");
    }

    #[test]
    fn parses_bare_localhost_url_flag() {
        let request = parse_curl_command("curl --url localhost/status").unwrap();

        assert_eq!(request.method, "GET");
        assert_eq!(request.url, "http://localhost/status");
    }

    #[test]
    fn keeps_scheme_relaxation_limited_to_localhost() {
        let error = parse_curl_command("curl api.example.com/items").unwrap_err();

        assert!(error
            .to_string()
            .contains("Could not find a request URL in the cURL command."));

        let error = parse_curl_command("curl localhost.example.com/items").unwrap_err();

        assert!(error
            .to_string()
            .contains("Could not find a request URL in the cURL command."));
    }

    #[test]
    fn combines_repeated_data_and_allows_get_query_data() {
        let post =
            parse_curl_command("curl https://api.example.com/items --data 'a=1' --data 'b=two'")
                .unwrap();
        assert_eq!(post.method, "POST");
        assert_eq!(post.body.mode, "raw");
        assert_eq!(post.body.raw, "a=1&b=two");

        let get = parse_curl_command(
            "curl --get https://api.example.com/items --data 'a=1' --data 'b=two'",
        )
        .unwrap();
        assert_eq!(get.method, "GET");
        assert_eq!(get.body.mode, "none");
        assert_eq!(get.query_params.len(), 2);
        assert_eq!(get.query_params[1].key, "b");
        assert_eq!(get.query_params[1].value, "two");
    }

    #[test]
    fn parses_multipart_form_fields_and_files() {
        let request = parse_curl_command(
            "curl -F 'description=hello' -F 'upload=@/tmp/report.pdf;type=application/pdf' https://api.example.com/upload",
        )
        .unwrap();

        assert_eq!(request.method, "POST");
        assert_eq!(request.body.mode, "multipart");
        assert_eq!(request.body.form[0].key, "description");
        assert_eq!(request.body.form[0].value, "hello");
        assert_eq!(request.body.files[0].name, "upload");
        assert_eq!(request.body.files[0].path, "/tmp/report.pdf");
    }

    #[test]
    fn maps_common_headers_and_continuations() {
        let request = parse_curl_command(
            r#"curl \
--location \
--compressed \
--cookie 'sid=abc; theme=dark' \
-H 'Authorization: Bearer {{api_token}}' \
https://api.example.com/me"#,
        )
        .unwrap();

        assert_eq!(request.auth.auth_type, "bearer");
        assert_eq!(request.auth.bearer_token, "{{api_token}}");
        assert!(request
            .headers
            .iter()
            .any(|header| header.key == "Cookie" && header.value == "sid=abc; theme=dark"));
        assert!(request.headers.iter().any(|header| {
            header.key == "Accept-Encoding" && header.value == "gzip, deflate, br"
        }));
    }
}
