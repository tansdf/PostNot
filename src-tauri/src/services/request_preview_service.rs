use std::{collections::BTreeSet, path::Path};

use reqwest::header::{HeaderName, HeaderValue};
use serde_json::Value;
use url::Url;

use crate::{
    domain::{
        environments::EnvironmentDetail,
        requests::{
            KeyValueRow, RequestAuth, RequestBody, RequestPreview, RequestPreviewSettings,
            SendRequestPayload,
        },
        settings::AppSettings,
    },
    services::{
        environments_service::RequestSecretUsage, request_url_service::normalize_request_url,
    },
};

const REDACTED_VALUE: &str = "{{redacted}}";
const REDACTED_URL_VALUE: &str = "redacted";

pub fn build_request_preview(
    original: &SendRequestPayload,
    resolved: &SendRequestPayload,
    secret_usage: &RequestSecretUsage,
    settings: &AppSettings,
    active_environment: Option<&EnvironmentDetail>,
) -> RequestPreview {
    let mut warnings = Vec::new();
    let query_params = preview_query_params(original, resolved, secret_usage);
    let final_url = preview_final_url(resolved, &query_params, secret_usage.url, &mut warnings);
    let headers = preview_headers(original, resolved, secret_usage, &mut warnings);
    let body = preview_body(original, resolved, secret_usage, &mut warnings);
    let auth = preview_auth(original, resolved, secret_usage);

    append_request_warnings(resolved, &mut warnings);

    RequestPreview {
        name: resolved.name.clone(),
        method: resolved.method.clone(),
        final_url,
        query_params,
        headers,
        body,
        auth,
        settings: RequestPreviewSettings {
            request_timeout_ms: settings.request_timeout_ms,
            follow_redirects: settings.follow_redirects,
            validate_tls: settings.validate_tls,
            active_environment_name: active_environment.map(|environment| environment.name.clone()),
        },
        warnings,
        notes: preview_notes(original),
    }
}

fn preview_query_params(
    original: &SendRequestPayload,
    resolved: &SendRequestPayload,
    secret_usage: &RequestSecretUsage,
) -> Vec<KeyValueRow> {
    let mut rows: Vec<KeyValueRow> = resolved
        .query_params
        .iter()
        .filter(|row| row.enabled && !row.key.trim().is_empty())
        .map(|row| {
            let original_row = original.query_params.iter().find(|item| item.id == row.id);
            let used_secret = secret_usage.query_param_ids.contains(&row.id);
            redact_key_value_row(original_row, row, used_secret, is_sensitive_key(&row.key))
        })
        .collect();

    if resolved.auth.auth_type == "api-key"
        && resolved.auth.api_key_in == "query"
        && !resolved.auth.api_key_name.trim().is_empty()
    {
        rows.push(KeyValueRow {
            id: "preview-auth-api-key-query".to_string(),
            key: resolved.auth.api_key_name.clone(),
            value: redact_if_present(&resolved.auth.api_key_value),
            enabled: true,
        });
    }

    rows
}

fn preview_final_url(
    resolved: &SendRequestPayload,
    query_params: &[KeyValueRow],
    url_used_secret: bool,
    warnings: &mut Vec<String>,
) -> String {
    let Ok(mut url) = Url::parse(&normalize_request_url(&resolved.url)) else {
        warnings.push(
            "URL is invalid and cannot be sent until it parses as an absolute URL.".to_string(),
        );
        return if url_used_secret {
            REDACTED_VALUE.to_string()
        } else {
            resolved.url.clone()
        };
    };

    for row in query_params
        .iter()
        .filter(|row| row.enabled && !row.key.trim().is_empty())
    {
        url.query_pairs_mut()
            .append_pair(&row.key, &url_safe_preview_value(&row.value));
    }

    redact_url(url, url_used_secret)
}

fn preview_headers(
    original: &SendRequestPayload,
    resolved: &SendRequestPayload,
    secret_usage: &RequestSecretUsage,
    warnings: &mut Vec<String>,
) -> Vec<KeyValueRow> {
    let mut rows: Vec<KeyValueRow> = resolved
        .headers
        .iter()
        .filter(|row| row.enabled && !row.key.trim().is_empty())
        .map(|row| {
            let original_row = original.headers.iter().find(|item| item.id == row.id);
            let used_secret = secret_usage.header_ids.contains(&row.id);
            redact_key_value_row(original_row, row, used_secret, is_sensitive_key(&row.key))
        })
        .collect();

    match resolved.auth.auth_type.as_str() {
        "basic" => rows.push(KeyValueRow {
            id: "preview-auth-basic".to_string(),
            key: "Authorization".to_string(),
            value: redact_if_present(&format!(
                "Basic {}:{}",
                resolved.auth.basic_username, resolved.auth.basic_password
            )),
            enabled: true,
        }),
        "bearer" if !resolved.auth.bearer_token.trim().is_empty() => rows.push(KeyValueRow {
            id: "preview-auth-bearer".to_string(),
            key: "Authorization".to_string(),
            value: "Bearer {{redacted}}".to_string(),
            enabled: true,
        }),
        "oauth2" => {
            let token = if resolved.auth.oauth2_access_token.trim().is_empty() {
                &resolved.auth.bearer_token
            } else {
                &resolved.auth.oauth2_access_token
            };

            if !token.trim().is_empty() {
                rows.push(KeyValueRow {
                    id: "preview-auth-oauth2".to_string(),
                    key: "Authorization".to_string(),
                    value: "Bearer {{redacted}}".to_string(),
                    enabled: true,
                });
            }
        }
        "api-key"
            if resolved.auth.api_key_in == "header"
                && !resolved.auth.api_key_name.trim().is_empty() =>
        {
            rows.push(KeyValueRow {
                id: "preview-auth-api-key-header".to_string(),
                key: resolved.auth.api_key_name.clone(),
                value: redact_if_present(&resolved.auth.api_key_value),
                enabled: true,
            });
        }
        _ => {}
    }

    match resolved.body.mode.as_str() {
        "json" => rows.push(KeyValueRow {
            id: "preview-content-type-json".to_string(),
            key: "content-type".to_string(),
            value: "application/json".to_string(),
            enabled: true,
        }),
        "form-urlencoded" => rows.push(KeyValueRow {
            id: "preview-content-type-form".to_string(),
            key: "content-type".to_string(),
            value: "application/x-www-form-urlencoded".to_string(),
            enabled: true,
        }),
        "multipart" => rows.push(KeyValueRow {
            id: "preview-content-type-multipart".to_string(),
            key: "content-type".to_string(),
            value: "multipart/form-data; boundary=(generated by reqwest)".to_string(),
            enabled: true,
        }),
        _ => {}
    }

    for row in &rows {
        validate_header_row(row, warnings);
    }

    rows
}

fn preview_body(
    original: &SendRequestPayload,
    resolved: &SendRequestPayload,
    secret_usage: &RequestSecretUsage,
    warnings: &mut Vec<String>,
) -> RequestBody {
    let raw_source = if secret_usage.body_raw {
        &original.body.raw
    } else {
        &resolved.body.raw
    };
    let raw = if resolved.body.mode == "json" || resolved.body.mode == "raw" {
        redact_raw_body(raw_source)
    } else {
        raw_source.clone()
    };

    if resolved.body.mode == "json" {
        validate_json_body(&resolved.body.raw, warnings);
    }

    RequestBody {
        mode: resolved.body.mode.clone(),
        raw,
        form: resolved
            .body
            .form
            .iter()
            .filter(|row| row.enabled && !row.key.trim().is_empty())
            .map(|row| {
                let original_row = original.body.form.iter().find(|item| item.id == row.id);
                let used_secret = secret_usage.body_form_ids.contains(&row.id);
                redact_key_value_row(original_row, row, used_secret, is_sensitive_key(&row.key))
            })
            .collect(),
        files: resolved
            .body
            .files
            .iter()
            .filter(|file| {
                file.enabled && (!file.name.trim().is_empty() || !file.path.trim().is_empty())
            })
            .map(|file| {
                let mut next = file.clone();
                if secret_usage.body_file_ids.contains(&file.id) {
                    if !next.path.trim().is_empty() {
                        next.path = REDACTED_VALUE.to_string();
                    }
                    if !next.name.trim().is_empty()
                        && original
                            .body
                            .files
                            .iter()
                            .any(|item| item.id == file.id && item.name.contains("{{"))
                    {
                        if let Some(original_file) =
                            original.body.files.iter().find(|item| item.id == file.id)
                        {
                            next.name = original_file.name.clone();
                        }
                    }
                }
                next
            })
            .collect(),
    }
}

fn preview_auth(
    original: &SendRequestPayload,
    resolved: &SendRequestPayload,
    secret_usage: &RequestSecretUsage,
) -> RequestAuth {
    RequestAuth {
        auth_type: resolved.auth.auth_type.clone(),
        basic_username: redact_auth_value(
            &original.auth.basic_username,
            &resolved.auth.basic_username,
            secret_usage.auth_basic_username,
            false,
        ),
        basic_password: redact_auth_value(
            &original.auth.basic_password,
            &resolved.auth.basic_password,
            secret_usage.auth_basic_password,
            true,
        ),
        bearer_token: redact_auth_value(
            &original.auth.bearer_token,
            &resolved.auth.bearer_token,
            secret_usage.auth_bearer_token,
            true,
        ),
        api_key_name: redact_auth_value(
            &original.auth.api_key_name,
            &resolved.auth.api_key_name,
            secret_usage.auth_api_key_name,
            false,
        ),
        api_key_value: redact_auth_value(
            &original.auth.api_key_value,
            &resolved.auth.api_key_value,
            secret_usage.auth_api_key_value,
            true,
        ),
        api_key_in: resolved.auth.api_key_in.clone(),
        oauth2_access_token: redact_auth_value(
            &original.auth.oauth2_access_token,
            &resolved.auth.oauth2_access_token,
            secret_usage.auth_oauth2_access_token,
            true,
        ),
        oauth2_token_url: redact_auth_value(
            &original.auth.oauth2_token_url,
            &resolved.auth.oauth2_token_url,
            secret_usage.auth_oauth2_token_url,
            false,
        ),
        oauth2_client_id: redact_auth_value(
            &original.auth.oauth2_client_id,
            &resolved.auth.oauth2_client_id,
            secret_usage.auth_oauth2_client_id,
            false,
        ),
        oauth2_client_secret: redact_auth_value(
            &original.auth.oauth2_client_secret,
            &resolved.auth.oauth2_client_secret,
            secret_usage.auth_oauth2_client_secret,
            true,
        ),
        oauth2_scope: redact_auth_value(
            &original.auth.oauth2_scope,
            &resolved.auth.oauth2_scope,
            secret_usage.auth_oauth2_scope,
            false,
        ),
    }
}

fn redact_key_value_row(
    original: Option<&KeyValueRow>,
    resolved: &KeyValueRow,
    used_secret: bool,
    sensitive_key: bool,
) -> KeyValueRow {
    let mut row = resolved.clone();

    if used_secret {
        if original.is_some_and(|item| item.key.contains("{{")) {
            if let Some(original) = original {
                row.key = original.key.clone();
            }
        }

        if !row.value.trim().is_empty() {
            row.value = REDACTED_VALUE.to_string();
        }
    } else if sensitive_key {
        row.value = redact_if_present(&row.value);
    }

    row
}

fn redact_auth_value(
    original: &str,
    resolved: &str,
    used_secret: bool,
    always_redact: bool,
) -> String {
    if resolved.trim().is_empty() {
        return String::new();
    }

    if used_secret && original.contains("{{") {
        return original.to_string();
    }

    if used_secret || always_redact {
        return REDACTED_VALUE.to_string();
    }

    resolved.to_string()
}

fn redact_if_present(value: &str) -> String {
    if value.trim().is_empty() {
        String::new()
    } else {
        REDACTED_VALUE.to_string()
    }
}

fn url_safe_preview_value(value: &str) -> String {
    if value == REDACTED_VALUE {
        REDACTED_URL_VALUE.to_string()
    } else {
        value.to_string()
    }
}

fn redact_url(mut url: Url, force_redact_path: bool) -> String {
    if !url.username().is_empty() {
        let _ = url.set_username(REDACTED_URL_VALUE);
    }

    if url.password().is_some() {
        let _ = url.set_password(Some(REDACTED_URL_VALUE));
    }

    if force_redact_path {
        url.set_path("/redacted");
    } else {
        redact_sensitive_path_segments(&mut url);
    }

    let query_pairs = url
        .query_pairs()
        .map(|(key, value)| {
            let value = if !value.is_empty() && is_sensitive_key(&key) {
                REDACTED_URL_VALUE.to_string()
            } else {
                value.to_string()
            };
            (key.to_string(), value)
        })
        .collect::<Vec<_>>();

    if !query_pairs.is_empty() {
        url.set_query(None);
        let mut pairs = url.query_pairs_mut();
        for (key, value) in query_pairs {
            pairs.append_pair(&key, &value);
        }
    }

    url.to_string()
}

fn redact_sensitive_path_segments(url: &mut Url) {
    let Some(segments) = url.path_segments() else {
        return;
    };

    let next_segments = segments
        .map(|segment| {
            if looks_like_sensitive_url_segment(segment) {
                if segment.to_lowercase().starts_with("bot") {
                    "bot-redacted".to_string()
                } else {
                    REDACTED_URL_VALUE.to_string()
                }
            } else {
                segment.to_string()
            }
        })
        .collect::<Vec<_>>();

    if let Ok(mut target) = url.path_segments_mut() {
        target.clear();
        for segment in next_segments {
            target.push(&segment);
        }
    }
}

fn looks_like_sensitive_url_segment(segment: &str) -> bool {
    let trimmed = segment.trim();
    if trimmed.len() < 20 {
        return false;
    }

    let lower = trimmed.to_lowercase();
    if lower.starts_with("bot") && trimmed.contains(':') {
        return true;
    }

    if lower.starts_with("xox") || lower.starts_with("ghp_") || lower.starts_with("github_pat_") {
        return true;
    }

    trimmed.contains(':')
        && trimmed
            .chars()
            .filter(|ch| ch.is_ascii_alphanumeric())
            .count()
            >= 20
}

fn redact_raw_body(raw: &str) -> String {
    if raw.trim().is_empty() {
        return raw.to_string();
    }

    if let Ok(mut value) = serde_json::from_str::<Value>(raw) {
        if redact_json_secrets(&mut value) {
            return serde_json::to_string_pretty(&value).unwrap_or_else(|_| raw.to_string());
        }
        return raw.to_string();
    }

    redact_urlencoded_body(raw)
}

fn redact_json_secrets(value: &mut Value) -> bool {
    match value {
        Value::Array(items) => items.iter_mut().any(redact_json_secrets),
        Value::Object(map) => {
            let mut changed = false;
            for (key, item) in map {
                if is_sensitive_key(key) && !item.is_null() {
                    *item = Value::String(REDACTED_VALUE.to_string());
                    changed = true;
                } else if redact_json_secrets(item) {
                    changed = true;
                }
            }
            changed
        }
        _ => false,
    }
}

fn redact_urlencoded_body(raw: &str) -> String {
    let trimmed = raw.trim();
    if !trimmed.contains('=') || trimmed.contains('\n') || trimmed.contains('{') {
        return raw.to_string();
    }

    raw.split('&')
        .map(|part| {
            let Some((key, value)) = part.split_once('=') else {
                return part.to_string();
            };

            if is_sensitive_key(key) && !value.is_empty() {
                format!("{key}={REDACTED_VALUE}")
            } else {
                part.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("&")
}

fn append_request_warnings(resolved: &SendRequestPayload, warnings: &mut Vec<String>) {
    let unresolved = collect_unresolved_variables(resolved);
    if !unresolved.is_empty() {
        warnings.push(format!(
            "Unresolved variable{}: {}",
            if unresolved.len() == 1 { "" } else { "s" },
            unresolved.into_iter().collect::<Vec<_>>().join(", ")
        ));
    }

    if resolved.auth.auth_type == "oauth2"
        && resolved.auth.oauth2_access_token.trim().is_empty()
        && resolved.auth.bearer_token.trim().is_empty()
    {
        warnings.push("OAuth2 auth is selected, but no access token is currently set.".to_string());
    }

    for file in
        resolved.body.files.iter().filter(|file| {
            file.enabled && !file.path.trim().is_empty() && !file.path.contains("{{")
        })
    {
        if !Path::new(&file.path).is_file() {
            warnings.push(format!(
                "Multipart file path does not point to a readable file: {}",
                file.path
            ));
        }
    }
}

fn validate_header_row(row: &KeyValueRow, warnings: &mut Vec<String>) {
    if HeaderName::from_bytes(row.key.as_bytes()).is_err() {
        warnings.push(format!("Header name is invalid: {}", row.key));
    }

    if HeaderValue::from_str(&row.value).is_err() {
        warnings.push(format!(
            "Header value contains characters that cannot be sent: {}",
            row.key
        ));
    }
}

fn validate_json_body(raw: &str, warnings: &mut Vec<String>) {
    if raw.trim().is_empty() {
        return;
    }

    if let Err(error) = serde_json::from_str::<Value>(raw) {
        warnings.push(format!("JSON body is not valid JSON: {error}"));
    }
}

fn collect_unresolved_variables(request: &SendRequestPayload) -> BTreeSet<String> {
    let mut variables = BTreeSet::new();
    collect_variables_from_string(&request.name, &mut variables);
    collect_variables_from_string(&request.url, &mut variables);

    for row in request.query_params.iter().chain(request.headers.iter()) {
        collect_variables_from_string(&row.key, &mut variables);
        collect_variables_from_string(&row.value, &mut variables);
    }

    collect_variables_from_string(&request.body.raw, &mut variables);
    for row in &request.body.form {
        collect_variables_from_string(&row.key, &mut variables);
        collect_variables_from_string(&row.value, &mut variables);
    }
    for file in &request.body.files {
        collect_variables_from_string(&file.name, &mut variables);
        collect_variables_from_string(&file.path, &mut variables);
    }

    collect_variables_from_string(&request.auth.basic_username, &mut variables);
    collect_variables_from_string(&request.auth.basic_password, &mut variables);
    collect_variables_from_string(&request.auth.bearer_token, &mut variables);
    collect_variables_from_string(&request.auth.api_key_name, &mut variables);
    collect_variables_from_string(&request.auth.api_key_value, &mut variables);
    collect_variables_from_string(&request.auth.oauth2_access_token, &mut variables);
    collect_variables_from_string(&request.auth.oauth2_token_url, &mut variables);
    collect_variables_from_string(&request.auth.oauth2_client_id, &mut variables);
    collect_variables_from_string(&request.auth.oauth2_client_secret, &mut variables);
    collect_variables_from_string(&request.auth.oauth2_scope, &mut variables);

    variables
}

fn preview_notes(original: &SendRequestPayload) -> Vec<String> {
    let mut notes = vec![
        "Preview is read-only and does not execute pre-request scripts, helper HTTP calls, or environment writes.".to_string(),
        "Transport-generated headers such as Host and Content-Length may still be added by the native HTTP client.".to_string(),
    ];

    let dynamic_variables = collect_dynamic_variables(original);
    if !dynamic_variables.is_empty() {
        notes.push(format!(
            "Dynamic variable{} sampled for preview may resolve differently when sent: {}",
            if dynamic_variables.len() == 1 {
                ""
            } else {
                "s"
            },
            dynamic_variables.into_iter().collect::<Vec<_>>().join(", ")
        ));
    }

    notes
}

fn collect_dynamic_variables(request: &SendRequestPayload) -> BTreeSet<String> {
    collect_variables_from_request(request)
        .into_iter()
        .filter(|variable| variable.trim_start_matches("{{").trim().starts_with('$'))
        .collect()
}

fn collect_variables_from_request(request: &SendRequestPayload) -> BTreeSet<String> {
    let mut variables = BTreeSet::new();
    collect_variables_from_string(&request.name, &mut variables);
    collect_variables_from_string(&request.url, &mut variables);
    for row in request.query_params.iter().chain(request.headers.iter()) {
        collect_variables_from_string(&row.key, &mut variables);
        collect_variables_from_string(&row.value, &mut variables);
    }
    collect_variables_from_string(&request.body.raw, &mut variables);
    for row in &request.body.form {
        collect_variables_from_string(&row.key, &mut variables);
        collect_variables_from_string(&row.value, &mut variables);
    }
    for file in &request.body.files {
        collect_variables_from_string(&file.name, &mut variables);
        collect_variables_from_string(&file.path, &mut variables);
    }
    collect_variables_from_string(&request.auth.basic_username, &mut variables);
    collect_variables_from_string(&request.auth.basic_password, &mut variables);
    collect_variables_from_string(&request.auth.bearer_token, &mut variables);
    collect_variables_from_string(&request.auth.api_key_name, &mut variables);
    collect_variables_from_string(&request.auth.api_key_value, &mut variables);
    collect_variables_from_string(&request.auth.oauth2_access_token, &mut variables);
    collect_variables_from_string(&request.auth.oauth2_token_url, &mut variables);
    collect_variables_from_string(&request.auth.oauth2_client_id, &mut variables);
    collect_variables_from_string(&request.auth.oauth2_client_secret, &mut variables);
    collect_variables_from_string(&request.auth.oauth2_scope, &mut variables);
    variables
}

fn collect_variables_from_string(value: &str, target: &mut BTreeSet<String>) {
    let mut rest = value;

    loop {
        let Some(start) = rest.find("{{") else {
            break;
        };
        let after_start = &rest[start + 2..];
        let Some(end) = after_start.find("}}") else {
            break;
        };
        let key = after_start[..end].trim();
        if !key.is_empty() {
            target.insert(format!("{{{{{key}}}}}"));
        }
        rest = &after_start[end + 2..];
    }
}

fn is_sensitive_key(value: &str) -> bool {
    let normalized = normalized_secret_name(value);
    if normalized.is_empty() {
        return false;
    }

    normalized == "authorization"
        || normalized == "proxyauthorization"
        || normalized == "cookie"
        || normalized == "setcookie"
        || normalized == "apikey"
        || normalized == "xapikey"
        || normalized == "clientsecret"
        || normalized == "accesstoken"
        || normalized.contains("accesstoken")
        || normalized.contains("apikey")
        || normalized.contains("secret")
        || normalized.contains("token")
        || normalized.contains("password")
        || normalized.contains("passwd")
}

fn normalized_secret_name(value: &str) -> String {
    value
        .trim()
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(|ch| ch.to_lowercase())
        .collect()
}
