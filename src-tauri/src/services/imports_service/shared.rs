use uuid::Uuid;

use crate::domain::requests::{KeyValueRow, RequestAuth, RequestBody, SendRequestPayload};

pub(super) fn imported_request_name(item_name: &str) -> String {
    let trimmed_name = item_name.trim();
    if trimmed_name.is_empty() {
        "Imported request".to_string()
    } else {
        trimmed_name.to_string()
    }
}

pub(super) fn normalized_folder_name(folder_name: &str) -> String {
    let trimmed_name = folder_name.trim();
    if trimmed_name.is_empty() {
        "Imported folder".to_string()
    } else {
        trimmed_name.to_string()
    }
}

pub(super) fn normalize_method(method: &str) -> String {
    let uppercase = method.trim().to_uppercase();
    match uppercase.as_str() {
        "GET" | "QUERY" | "POST" | "PUT" | "PATCH" | "DELETE" | "HEAD" | "OPTIONS" => uppercase,
        _ => "GET".to_string(),
    }
}

pub(super) fn stringify_postman_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => String::new(),
        serde_json::Value::String(value) => value.clone(),
        serde_json::Value::Bool(value) => value.to_string(),
        serde_json::Value::Number(value) => value.to_string(),
        _ => value.to_string(),
    }
}

pub(super) fn json_value_to_input_string(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => String::new(),
        serde_json::Value::String(value) => value.clone(),
        serde_json::Value::Bool(value) => value.to_string(),
        serde_json::Value::Number(value) => value.to_string(),
        _ => serde_json::to_string(value).unwrap_or_default(),
    }
}

pub(super) fn empty_body() -> RequestBody {
    RequestBody {
        mode: "none".to_string(),
        raw: String::new(),
        form: vec![empty_kv()],
        files: vec![],
    }
}

pub(super) fn empty_auth() -> RequestAuth {
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

pub(super) fn empty_kv() -> KeyValueRow {
    KeyValueRow {
        id: Uuid::new_v4().to_string(),
        key: String::new(),
        value: String::new(),
        enabled: true,
    }
}

pub(super) fn create_empty_request_payload() -> SendRequestPayload {
    SendRequestPayload {
        name: "Imported request".to_string(),
        method: "GET".to_string(),
        url: "https://example.com".to_string(),
        query_params: vec![empty_kv()],
        headers: vec![empty_kv()],
        body: empty_body(),
        auth: empty_auth(),
        pre_request_script: String::new(),
        test_script: String::new(),
    }
}
