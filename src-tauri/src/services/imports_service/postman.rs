use serde::Deserialize;
use sqlx::SqlitePool;
use std::sync::Arc;
use uuid::Uuid;

use crate::{
    domain::{
        collections::{CreateCollectionFolderInput, CreateCollectionInput},
        environments::{
            EnvironmentInput, EnvironmentVariable, ImportEnvironmentInput, ImportEnvironmentResult,
        },
        imports::ImportResult,
        requests::{FileRow, KeyValueRow, RequestAuth, RequestBody, SendRequestPayload},
    },
    error::{AppError, AppResult},
    services::{collections_service, environments_service, secret_store_service::SecretStore},
};

use super::shared::{
    empty_auth, empty_body, empty_kv, imported_request_name, normalize_method,
    normalized_folder_name, stringify_postman_value,
};

#[derive(Debug, Deserialize)]
struct PostmanCollection {
    info: PostmanInfo,
    #[serde(default)]
    event: Vec<PostmanEvent>,
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
    #[serde(default)]
    event: Vec<PostmanEvent>,
    request: Option<PostmanRequest>,
}

#[derive(Debug, Deserialize)]
struct PostmanEvent {
    #[serde(default)]
    listen: String,
    #[serde(default)]
    script: Option<PostmanScript>,
}

#[derive(Debug, Deserialize)]
struct PostmanScript {
    #[serde(default)]
    exec: PostmanScriptExec,
}

#[derive(Debug, Deserialize, Default)]
#[serde(untagged)]
enum PostmanScriptExec {
    Text(String),
    Lines(Vec<String>),
    #[default]
    Empty,
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
    #[serde(default)]
    oauth2: Vec<PostmanAuthValue>,
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
    #[serde(rename = "type", default)]
    value_type: String,
    enabled: Option<bool>,
    disabled: Option<bool>,
}

pub(super) async fn import_postman_environment(
    pool: &SqlitePool,
    secret_store: Arc<dyn SecretStore>,
    input: &ImportEnvironmentInput,
) -> AppResult<ImportEnvironmentResult> {
    let environment: PostmanEnvironment = serde_json::from_str(&input.source).map_err(|error| {
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
        .map(|item| EnvironmentVariable {
            id: Uuid::new_v4().to_string(),
            key: item.key,
            value: stringify_postman_value(&item.value),
            enabled: !item.disabled.unwrap_or(false) && item.enabled.unwrap_or(true),
            is_secret: item.value_type.eq_ignore_ascii_case("secret"),
        })
        .collect();

    let environment = environments_service::create_environment_from_input(
        pool,
        secret_store,
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

pub(super) async fn import_postman_collection(
    pool: &SqlitePool,
    source: &str,
) -> AppResult<ImportResult> {
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
    let pre_request_script = join_postman_script_events(&collection.event, "prerequest");
    let test_script = join_postman_script_events(&collection.event, "test");

    let created_collection = collections_service::create_collection(
        pool,
        &CreateCollectionInput {
            name: collection_name.clone(),
            description,
            pre_request_script,
            test_script,
        },
    )
    .await?;

    let imported_request_count =
        import_postman_items(pool, &created_collection.id, None, &collection.item).await?;

    if imported_request_count == 0 {
        return Err(AppError::Message(
            "No requests were found in this Postman collection.".to_string(),
        ));
    }

    Ok(ImportResult {
        collection_id: created_collection.id,
        collection_name: created_collection.name,
        imported_request_count,
        created_collection: true,
    })
}

async fn import_postman_items(
    pool: &SqlitePool,
    collection_id: &str,
    parent_id: Option<&str>,
    items: &[PostmanItem],
) -> AppResult<usize> {
    let mut imported_request_count = 0usize;
    let mut stack: Vec<(Option<String>, &PostmanItem)> = items
        .iter()
        .rev()
        .map(|item| (parent_id.map(|value| value.to_string()), item))
        .collect();

    while let Some((current_parent_id, item)) = stack.pop() {
        if !item.item.is_empty() {
            let folder = collections_service::create_collection_folder(
                pool,
                collection_id,
                &CreateCollectionFolderInput {
                    name: normalized_folder_name(&item.name),
                    parent_id: current_parent_id.clone(),
                    pre_request_script: join_postman_script_events(&item.event, "prerequest"),
                    test_script: join_postman_script_events(&item.event, "test"),
                },
            )
            .await?;

            for child in item.item.iter().rev() {
                stack.push((Some(folder.id.clone()), child));
            }
            continue;
        }

        if let Some(request) = &item.request {
            let request = map_postman_request(item, request)?;
            collections_service::save_request(
                pool,
                collection_id,
                current_parent_id.as_deref(),
                &request,
            )
            .await?;
            imported_request_count += 1;
        }
    }

    Ok(imported_request_count)
}

fn map_postman_request(
    item: &PostmanItem,
    request: &PostmanRequest,
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
    let name = imported_request_name(&item.name);
    let pre_request_script = join_postman_script_events(&item.event, "prerequest");
    let test_script = join_postman_script_events(&item.event, "test");

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
        pre_request_script,
        test_script,
    })
}

fn join_postman_script_events(events: &[PostmanEvent], listen: &str) -> String {
    events
        .iter()
        .filter(|event| event.listen.eq_ignore_ascii_case(listen))
        .filter_map(|event| event.script.as_ref())
        .map(postman_script_to_string)
        .filter(|script| !script.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn postman_script_to_string(script: &PostmanScript) -> String {
    match &script.exec {
        PostmanScriptExec::Text(value) => value.clone(),
        PostmanScriptExec::Lines(lines) => lines.join("\n"),
        PostmanScriptExec::Empty => String::new(),
    }
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
        "oauth2" => RequestAuth {
            auth_type: "oauth2".to_string(),
            oauth2_access_token: auth_value(&auth.oauth2, "accessToken"),
            oauth2_token_url: auth_value(&auth.oauth2, "tokenUrl"),
            oauth2_client_id: auth_value(&auth.oauth2, "clientId"),
            oauth2_client_secret: auth_value(&auth.oauth2, "clientSecret"),
            oauth2_scope: auth_value(&auth.oauth2, "scope"),
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
