use serde::Deserialize;
use sqlx::SqlitePool;
use url::Url;
use uuid::Uuid;

use crate::{
    domain::{
        collections::CreateCollectionInput,
        environments::{EnvironmentInput, ImportEnvironmentInput, ImportEnvironmentResult},
        imports::{ImportRequestInput, ImportResult, ImportedRequestDraft},
        requests::{FileRow, KeyValueRow, RequestAuth, RequestBody, SendRequestPayload},
    },
    error::{AppError, AppResult},
    services::{collections_service, environments_service},
};

#[derive(Debug, Deserialize)]
struct PostmanCollection {
    info: PostmanInfo,
    #[serde(default)]
    item: Vec<PostmanItem>,
}

#[derive(Debug, Deserialize)]
struct PostmanInfo {
    #[serde(default)]
    name: String,
    #[serde(default)]
    description: PostmanDescription,
}

#[derive(Debug, Deserialize, Default)]
#[serde(untagged)]
enum PostmanDescription {
    Text(String),
    Detailed {
        content: Option<String>,
    },
    #[default]
    Empty,
}

#[derive(Debug, Deserialize)]
struct PostmanItem {
    #[serde(default)]
    name: String,
    #[serde(default)]
    item: Vec<PostmanItem>,
    request: Option<PostmanRequest>,
}

#[derive(Debug, Deserialize)]
struct PostmanRequest {
    #[serde(default)]
    method: String,
    #[serde(default)]
    header: Vec<PostmanHeader>,
    #[serde(default)]
    auth: Option<PostmanAuth>,
    #[serde(default)]
    body: Option<PostmanBody>,
    #[serde(default)]
    url: PostmanUrl,
}

#[derive(Debug, Deserialize)]
struct PostmanHeader {
    #[serde(default)]
    key: String,
    #[serde(default)]
    value: String,
    disabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct PostmanAuth {
    #[serde(rename = "type", default)]
    auth_type: String,
    #[serde(default)]
    basic: Vec<PostmanAuthValue>,
    #[serde(default)]
    bearer: Vec<PostmanAuthValue>,
    #[serde(rename = "apikey", default)]
    api_key: Vec<PostmanAuthValue>,
}

#[derive(Debug, Deserialize)]
struct PostmanAuthValue {
    #[serde(default)]
    key: String,
    #[serde(default)]
    value: String,
}

#[derive(Debug, Deserialize)]
struct PostmanBody {
    #[serde(rename = "mode", default)]
    body_mode: String,
    #[serde(default)]
    raw: String,
    #[serde(default)]
    urlencoded: Vec<PostmanFormValue>,
    #[serde(default)]
    formdata: Vec<PostmanFormDataValue>,
    #[serde(default)]
    options: Option<PostmanBodyOptions>,
}

#[derive(Debug, Deserialize)]
struct PostmanFormValue {
    #[serde(default)]
    key: String,
    #[serde(default)]
    value: String,
    disabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct PostmanFormDataValue {
    #[serde(default)]
    key: String,
    #[serde(default)]
    value: String,
    #[serde(default)]
    src: PostmanFileSource,
    #[serde(rename = "type", default)]
    item_type: String,
    disabled: Option<bool>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(untagged)]
enum PostmanFileSource {
    Text(String),
    List(Vec<String>),
    #[default]
    Empty,
}

#[derive(Debug, Deserialize)]
struct PostmanBodyOptions {
    raw: Option<PostmanRawBodyOptions>,
}

#[derive(Debug, Deserialize)]
struct PostmanRawBodyOptions {
    #[serde(default)]
    language: String,
}

#[derive(Debug, Deserialize, Default)]
#[serde(untagged)]
enum PostmanUrl {
    Text(String),
    Structured(PostmanUrlObject),
    #[default]
    Empty,
}

#[derive(Debug, Deserialize, Default)]
struct PostmanUrlObject {
    raw: Option<String>,
    #[serde(default)]
    query: Vec<PostmanQueryParam>,
}

#[derive(Debug, Deserialize)]
struct PostmanQueryParam {
    #[serde(default)]
    key: String,
    #[serde(default)]
    value: String,
    disabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct PostmanEnvironment {
    #[serde(default)]
    name: String,
    #[serde(default)]
    values: Vec<PostmanEnvironmentValue>,
}

#[derive(Debug, Deserialize)]
struct PostmanEnvironmentValue {
    #[serde(default)]
    key: String,
    #[serde(default)]
    value: serde_json::Value,
    enabled: Option<bool>,
    disabled: Option<bool>,
}

pub async fn import_requests(
    pool: &SqlitePool,
    input: &ImportRequestInput,
) -> AppResult<ImportResult> {
    let source = input.source.trim();
    if source.is_empty() {
        return Err(AppError::Message(
            "Import source cannot be empty.".to_string(),
        ));
    }

    match input.format.as_str() {
        "postman" => import_postman_collection(pool, source).await,
        "curl" => import_curl_request(pool, source, input.target_collection_id.as_deref()).await,
        _ => Err(AppError::Message("Unsupported import format.".to_string())),
    }
}

pub fn import_curl_to_draft(source: &str) -> AppResult<ImportedRequestDraft> {
    let source = source.trim();
    if source.is_empty() {
        return Err(AppError::Message(
            "Import source cannot be empty.".to_string(),
        ));
    }

    Ok(ImportedRequestDraft {
        request: parse_curl_command(source)?,
    })
}

pub async fn import_postman_environment(
    pool: &SqlitePool,
    input: &ImportEnvironmentInput,
) -> AppResult<ImportEnvironmentResult> {
    let source = input.source.trim();
    if source.is_empty() {
        return Err(AppError::Message(
            "Import source cannot be empty.".to_string(),
        ));
    }

    let environment: PostmanEnvironment = serde_json::from_str(source).map_err(|error| {
        AppError::Message(format!("Invalid Postman environment JSON: {}", error))
    })?;

    let name = if environment.name.trim().is_empty() {
        "Imported Postman environment".to_string()
    } else {
        environment.name.trim().to_string()
    };

    let variables = environment
        .values
        .into_iter()
        .map(|item| KeyValueRow {
            id: Uuid::new_v4().to_string(),
            key: item.key,
            value: stringify_postman_value(&item.value),
            enabled: !item.disabled.unwrap_or(false) && item.enabled.unwrap_or(true),
        })
        .collect();

    let environment = environments_service::create_environment_from_input(
        pool,
        &EnvironmentInput { name, variables },
        input.set_active,
    )
    .await?;

    Ok(ImportEnvironmentResult {
        environment_id: environment.id,
        environment_name: environment.name,
        imported_variable_count: environment
            .variables
            .iter()
            .filter(|item| !item.key.trim().is_empty())
            .count(),
        activated: environment.is_active,
    })
}

async fn import_postman_collection(pool: &SqlitePool, source: &str) -> AppResult<ImportResult> {
    let collection: PostmanCollection = serde_json::from_str(source).map_err(|error| {
        AppError::Message(format!("Invalid Postman collection JSON: {}", error))
    })?;

    let collection_name = if collection.info.name.trim().is_empty() {
        "Imported Postman collection".to_string()
    } else {
        collection.info.name.trim().to_string()
    };

    let description = match collection.info.description {
        PostmanDescription::Text(value) => value.trim().to_string(),
        PostmanDescription::Detailed { content } => content.unwrap_or_default().trim().to_string(),
        PostmanDescription::Empty => String::new(),
    };

    let created_collection = collections_service::create_collection(
        pool,
        &CreateCollectionInput {
            name: collection_name.clone(),
            description,
        },
    )
    .await?;

    let mut requests = Vec::new();
    collect_postman_requests(&collection.item, &mut Vec::new(), &mut requests)?;

    if requests.is_empty() {
        return Err(AppError::Message(
            "No requests were found in this Postman collection.".to_string(),
        ));
    }

    for request in &requests {
        collections_service::save_request(pool, &created_collection.id, request).await?;
    }

    Ok(ImportResult {
        collection_id: created_collection.id,
        collection_name: created_collection.name,
        imported_request_count: requests.len(),
        created_collection: true,
    })
}

fn collect_postman_requests(
    items: &[PostmanItem],
    path: &mut Vec<String>,
    requests: &mut Vec<SendRequestPayload>,
) -> AppResult<()> {
    for item in items {
        if !item.item.is_empty() {
            if !item.name.trim().is_empty() {
                path.push(item.name.trim().to_string());
            }
            collect_postman_requests(&item.item, path, requests)?;
            if !item.name.trim().is_empty() {
                path.pop();
            }
            continue;
        }

        if let Some(request) = &item.request {
            requests.push(map_postman_request(item, request, path)?);
        }
    }

    Ok(())
}

fn map_postman_request(
    item: &PostmanItem,
    request: &PostmanRequest,
    path: &[String],
) -> AppResult<SendRequestPayload> {
    let mut query_params = Vec::new();
    let url = match &request.url {
        PostmanUrl::Text(value) => value.trim().to_string(),
        PostmanUrl::Structured(value) => {
            for query in &value.query {
                query_params.push(KeyValueRow {
                    id: Uuid::new_v4().to_string(),
                    key: query.key.clone(),
                    value: query.value.clone(),
                    enabled: !query.disabled.unwrap_or(false),
                });
            }

            let raw_url = value.raw.clone().unwrap_or_default();
            strip_query_from_postman_raw_url(&raw_url, !value.query.is_empty())
        }
        PostmanUrl::Empty => String::new(),
    };

    let headers: Vec<KeyValueRow> = request
        .header
        .iter()
        .map(|header| KeyValueRow {
            id: Uuid::new_v4().to_string(),
            key: header.key.clone(),
            value: header.value.clone(),
            enabled: !header.disabled.unwrap_or(false),
        })
        .collect();

    let body = map_postman_body(request.body.as_ref());
    let auth = map_postman_auth(request.auth.as_ref());
    let name = build_imported_request_name(path, &item.name);

    Ok(SendRequestPayload {
        name,
        method: normalize_method(&request.method),
        url,
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
        body,
        auth,
    })
}

fn map_postman_body(body: Option<&PostmanBody>) -> RequestBody {
    let Some(body) = body else {
        return empty_body();
    };

    match body.body_mode.as_str() {
        "raw" => RequestBody {
            mode: match body
                .options
                .as_ref()
                .and_then(|options| options.raw.as_ref())
            {
                Some(raw_options) if raw_options.language.eq_ignore_ascii_case("json") => {
                    "json".to_string()
                }
                _ => "raw".to_string(),
            },
            raw: body.raw.clone(),
            form: vec![empty_kv()],
            files: vec![],
        },
        "urlencoded" => RequestBody {
            mode: "form-urlencoded".to_string(),
            raw: String::new(),
            form: if body.urlencoded.is_empty() {
                vec![empty_kv()]
            } else {
                body.urlencoded
                    .iter()
                    .map(|item| KeyValueRow {
                        id: Uuid::new_v4().to_string(),
                        key: item.key.clone(),
                        value: item.value.clone(),
                        enabled: !item.disabled.unwrap_or(false),
                    })
                    .collect()
            },
            files: vec![],
        },
        "formdata" => {
            let mut form = Vec::new();
            let mut files = Vec::new();

            for item in &body.formdata {
                match item.item_type.as_str() {
                    "file" => files.push(FileRow {
                        id: Uuid::new_v4().to_string(),
                        name: item.key.clone(),
                        path: file_source_to_string(&item.src),
                        enabled: !item.disabled.unwrap_or(false),
                    }),
                    _ => form.push(KeyValueRow {
                        id: Uuid::new_v4().to_string(),
                        key: item.key.clone(),
                        value: item.value.clone(),
                        enabled: !item.disabled.unwrap_or(false),
                    }),
                }
            }

            RequestBody {
                mode: "multipart".to_string(),
                raw: String::new(),
                form: if form.is_empty() {
                    vec![empty_kv()]
                } else {
                    form
                },
                files,
            }
        }
        _ => empty_body(),
    }
}

fn map_postman_auth(auth: Option<&PostmanAuth>) -> RequestAuth {
    let Some(auth) = auth else {
        return empty_auth();
    };

    match auth.auth_type.as_str() {
        "basic" => RequestAuth {
            auth_type: "basic".to_string(),
            basic_username: auth_value(&auth.basic, "username"),
            basic_password: auth_value(&auth.basic, "password"),
            ..empty_auth()
        },
        "bearer" => RequestAuth {
            auth_type: "bearer".to_string(),
            bearer_token: auth_value(&auth.bearer, "token"),
            ..empty_auth()
        },
        "apikey" => RequestAuth {
            auth_type: "api-key".to_string(),
            api_key_name: auth_value(&auth.api_key, "key"),
            api_key_value: auth_value(&auth.api_key, "value"),
            api_key_in: match auth_value(&auth.api_key, "in").as_str() {
                "query" => "query".to_string(),
                _ => "header".to_string(),
            },
            ..empty_auth()
        },
        _ => empty_auth(),
    }
}

fn auth_value(values: &[PostmanAuthValue], key: &str) -> String {
    values
        .iter()
        .find(|item| item.key == key)
        .map(|item| item.value.clone())
        .unwrap_or_default()
}

fn file_source_to_string(source: &PostmanFileSource) -> String {
    match source {
        PostmanFileSource::Text(value) => value.clone(),
        PostmanFileSource::List(values) => values.first().cloned().unwrap_or_default(),
        PostmanFileSource::Empty => String::new(),
    }
}

fn strip_query_from_postman_raw_url(raw_url: &str, has_query_rows: bool) -> String {
    let trimmed = raw_url.trim();
    if !has_query_rows {
        return trimmed.to_string();
    }

    let hash_index = trimmed.find('#').unwrap_or(trimmed.len());
    let before_hash = &trimmed[..hash_index];
    let hash = &trimmed[hash_index..];

    match before_hash.find('?') {
        Some(query_index) => format!("{}{}", &before_hash[..query_index], hash),
        None => trimmed.to_string(),
    }
}

async fn import_curl_request(
    pool: &SqlitePool,
    source: &str,
    target_collection_id: Option<&str>,
) -> AppResult<ImportResult> {
    let request = parse_curl_command(source)?;

    let (collection_id, collection_name, created_collection) =
        if let Some(collection_id) = target_collection_id {
            let collection = collections_service::list_collections(pool)
                .await?
                .into_iter()
                .find(|item| item.id == collection_id)
                .ok_or_else(|| AppError::Message("Target collection not found.".to_string()))?;

            (collection.id, collection.name, false)
        } else {
            let created = collections_service::create_collection(
                pool,
                &CreateCollectionInput {
                    name: "Imported cURL".to_string(),
                    description: "Requests imported from cURL.".to_string(),
                },
            )
            .await?;

            (created.id, created.name, true)
        };

    collections_service::save_request(pool, &collection_id, &request).await?;

    Ok(ImportResult {
        collection_id,
        collection_name,
        imported_request_count: 1,
        created_collection,
    })
}

fn parse_curl_command(source: &str) -> AppResult<SendRequestPayload> {
    let parts = shlex::split(source)
        .ok_or_else(|| AppError::Message("Invalid cURL command.".to_string()))?;
    if parts.is_empty() || parts[0] != "curl" {
        return Err(AppError::Message(
            "Paste a complete cURL command starting with `curl`.".to_string(),
        ));
    }

    let mut method = "GET".to_string();
    let mut url = String::new();
    let mut headers = Vec::new();
    let mut body_raw = String::new();
    let mut body_mode = "none".to_string();
    let mut auth = empty_auth();
    let mut i = 1usize;

    while i < parts.len() {
        match parts[i].as_str() {
            "-X" | "--request" => {
                i += 1;
                if let Some(value) = parts.get(i) {
                    method = normalize_method(value);
                }
            }
            "-H" | "--header" => {
                i += 1;
                if let Some(value) = parts.get(i) {
                    if let Some((key, header_value)) = value.split_once(':') {
                        headers.push(KeyValueRow {
                            id: Uuid::new_v4().to_string(),
                            key: key.trim().to_string(),
                            value: header_value.trim().to_string(),
                            enabled: true,
                        });
                    }
                }
            }
            "-d" | "--data" | "--data-raw" | "--data-binary" | "--data-urlencode" => {
                i += 1;
                if let Some(value) = parts.get(i) {
                    body_raw = value.clone();
                    if body_mode == "none" {
                        body_mode = if looks_like_json(value) {
                            "json".to_string()
                        } else {
                            "raw".to_string()
                        };
                    }
                    if method == "GET" {
                        method = "POST".to_string();
                    }
                }
            }
            "-u" | "--user" => {
                i += 1;
                if let Some(value) = parts.get(i) {
                    let (username, password) =
                        value.split_once(':').unwrap_or((value.as_str(), ""));
                    auth = RequestAuth {
                        auth_type: "basic".to_string(),
                        basic_username: username.to_string(),
                        basic_password: password.to_string(),
                        ..empty_auth()
                    };
                }
            }
            value if value.starts_with("http://") || value.starts_with("https://") => {
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

    let mut query_params = Vec::new();
    if let Ok(parsed_url) = Url::parse(&url) {
        for (key, value) in parsed_url.query_pairs() {
            query_params.push(KeyValueRow {
                id: Uuid::new_v4().to_string(),
                key: key.to_string(),
                value: value.to_string(),
                enabled: true,
            });
        }
    }

    Ok(SendRequestPayload {
        name: format!("{} {}", method, url),
        method,
        url,
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
            form: vec![empty_kv()],
            files: vec![],
        },
        auth,
    })
}

fn looks_like_json(value: &str) -> bool {
    let trimmed = value.trim();
    (trimmed.starts_with('{') && trimmed.ends_with('}'))
        || (trimmed.starts_with('[') && trimmed.ends_with(']'))
}

fn build_imported_request_name(path: &[String], item_name: &str) -> String {
    let trimmed_name = item_name.trim();
    if path.is_empty() {
        return if trimmed_name.is_empty() {
            "Imported request".to_string()
        } else {
            trimmed_name.to_string()
        };
    }

    if trimmed_name.is_empty() {
        path.join(" / ")
    } else {
        format!("{} / {}", path.join(" / "), trimmed_name)
    }
}

fn normalize_method(method: &str) -> String {
    let uppercase = method.trim().to_uppercase();
    match uppercase.as_str() {
        "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "HEAD" | "OPTIONS" => uppercase,
        _ => "GET".to_string(),
    }
}

fn stringify_postman_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => String::new(),
        serde_json::Value::String(value) => value.clone(),
        serde_json::Value::Bool(value) => value.to_string(),
        serde_json::Value::Number(value) => value.to_string(),
        _ => value.to_string(),
    }
}

fn empty_body() -> RequestBody {
    RequestBody {
        mode: "none".to_string(),
        raw: String::new(),
        form: vec![empty_kv()],
        files: vec![],
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
    }
}

fn empty_kv() -> KeyValueRow {
    KeyValueRow {
        id: Uuid::new_v4().to_string(),
        key: String::new(),
        value: String::new(),
        enabled: true,
    }
}
