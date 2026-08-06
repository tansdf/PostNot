use serde::Deserialize;
use sqlx::SqlitePool;
use std::{collections::HashMap, sync::Arc};
use uuid::Uuid;

use crate::{
    domain::{
        collections::CreateCollectionInput,
        environments::{
            EnvironmentInput, EnvironmentVariable, ImportEnvironmentInput, ImportEnvironmentResult,
        },
        imports::{ImportDetails, ImportResult},
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

    let mut folders = Vec::new();
    let mut requests = Vec::new();
    let mut imported_items = Vec::new();
    build_postman_import_items(
        None,
        &collection.item,
        &mut folders,
        &mut requests,
        &mut imported_items,
    )?;
    let imported_request_count = requests.len();

    if imported_request_count == 0 {
        return Err(AppError::Message(
            "No requests were found in this Postman collection.".to_string(),
        ));
    }

    let created_collection = collections_service::import_collection_atomic(
        pool,
        &CreateCollectionInput {
            name: collection_name.clone(),
            description,
            pre_request_script,
            test_script,
        },
        &folders,
        &requests,
    )
    .await?;

    Ok(ImportResult {
        collection_id: created_collection.id,
        collection_name: created_collection.name,
        imported_request_count,
        created_collection: true,
        created_realtime_connection_profile_count: 0,
        details: Some(ImportDetails {
            format: "postman".to_string(),
            summary: format!(
                "{} request{} imported from Postman.",
                imported_request_count,
                if imported_request_count == 1 { "" } else { "s" }
            ),
            imported_items,
            warnings: Vec::new(),
            errors: Vec::new(),
        }),
    })
}

fn build_postman_import_items(
    parent_id: Option<String>,
    items: &[PostmanItem],
    folders: &mut Vec<collections_service::ImportCollectionFolder>,
    requests: &mut Vec<collections_service::ImportCollectionRequest>,
    imported_items: &mut Vec<String>,
) -> AppResult<()> {
    let mut stack: Vec<(Option<String>, &PostmanItem)> = items
        .iter()
        .rev()
        .map(|item| (parent_id.clone(), item))
        .collect();
    let mut next_sort_order = HashMap::<Option<String>, i64>::new();

    while let Some((current_parent_id, item)) = stack.pop() {
        if !item.item.is_empty() {
            let folder_id = Uuid::new_v4().to_string();
            let sort_order = next_sort_order
                .entry(current_parent_id.clone())
                .or_insert(0);
            let folder_sort_order = *sort_order;
            *sort_order += 1;
            folders.push(collections_service::ImportCollectionFolder {
                id: folder_id.clone(),
                parent_id: current_parent_id.clone(),
                sort_order: folder_sort_order,
                name: normalized_folder_name(&item.name),
                pre_request_script: join_postman_script_events(&item.event, "prerequest"),
                test_script: join_postman_script_events(&item.event, "test"),
            });

            for child in item.item.iter().rev() {
                stack.push((Some(folder_id.clone()), child));
            }
            continue;
        }

        if let Some(request) = &item.request {
            let request = map_postman_request(item, request)?;
            let sort_order = next_sort_order
                .entry(current_parent_id.clone())
                .or_insert(0);
            let request_sort_order = *sort_order;
            *sort_order += 1;
            imported_items.push(request.name.clone());
            requests.push(collections_service::ImportCollectionRequest {
                parent_id: current_parent_id,
                sort_order: request_sort_order,
                request,
            });
        }
    }

    Ok(())
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

#[cfg(test)]
mod tests {
    use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};

    use super::*;
    use crate::services::secret_store_service::{InMemorySecretStore, SecretStore};

    async fn setup_test_db() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory database");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("run migrations");
        pool
    }

    #[tokio::test]
    async fn collection_import_preserves_hierarchy_scripts_and_request_fields() {
        let pool = setup_test_db().await;
        let source = r#"{
          "info": {
            "name": "  Billing API  ",
            "description": { "content": "  Imported billing requests  " }
          },
          "event": [
            { "listen": "prerequest", "script": { "exec": ["collection-pre-1", "collection-pre-2"] } },
            { "listen": "test", "script": { "exec": "collection-test" } }
          ],
          "item": [{
            "name": "Invoices",
            "event": [{ "listen": "prerequest", "script": { "exec": ["folder-pre"] } }],
            "item": [{
              "name": "Create invoice",
              "event": [
                { "listen": "prerequest", "script": { "exec": ["request-pre"] } },
                { "listen": "test", "script": { "exec": ["request-test-1", "request-test-2"] } }
              ],
              "request": {
                "method": "post",
                "url": {
                  "raw": "https://api.example.test/invoices?limit=10&archived=true#section",
                  "query": [
                    { "key": "limit", "value": "10" },
                    { "key": "archived", "value": "true", "disabled": true }
                  ]
                },
                "header": [
                  { "key": "X-Trace", "value": "trace-1" },
                  { "key": "X-Skip", "value": "ignored", "disabled": true }
                ],
                "auth": { "type": "bearer", "bearer": [{ "key": "token", "value": "{{token}}" }] },
                "body": {
                  "mode": "raw",
                  "raw": "{\"amount\":42}",
                  "options": { "raw": { "language": "json" } }
                }
              }
            }]
          }]
        }"#;

        let result = import_postman_collection(&pool, source)
            .await
            .expect("import Postman collection");

        assert_eq!(result.collection_name, "Billing API");
        assert_eq!(result.imported_request_count, 1);
        assert_eq!(result.details.as_ref().unwrap().format, "postman");

        let collection = collections_service::get_collection(&pool, &result.collection_id)
            .await
            .expect("read imported collection");
        assert_eq!(collection.description, "Imported billing requests");
        assert_eq!(
            collection.pre_request_script,
            "collection-pre-1\ncollection-pre-2"
        );
        assert_eq!(collection.test_script, "collection-test");

        let items = collections_service::list_collection_items(&pool, &result.collection_id)
            .await
            .expect("list imported items");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, "Invoices");
        assert_eq!(items[0].pre_request_script, "folder-pre");
        assert_eq!(items[0].children.len(), 1);

        let detail = collections_service::get_saved_request(&pool, &items[0].children[0].id)
            .await
            .expect("read imported request");
        let request = detail.request;
        assert_eq!(request.name, "Create invoice");
        assert_eq!(request.method, "POST");
        assert_eq!(request.url, "https://api.example.test/invoices#section");
        assert_eq!(request.query_params.len(), 2);
        assert!(request.query_params[0].enabled);
        assert!(!request.query_params[1].enabled);
        assert_eq!(request.headers[0].key, "X-Trace");
        assert!(!request.headers[1].enabled);
        assert_eq!(request.auth.auth_type, "bearer");
        assert_eq!(request.auth.bearer_token, "{{token}}");
        assert_eq!(request.body.mode, "json");
        assert_eq!(request.body.raw, "{\"amount\":42}");
        assert_eq!(request.pre_request_script, "request-pre");
        assert_eq!(request.test_script, "request-test-1\nrequest-test-2");
    }

    #[test]
    fn maps_urlencoded_multipart_and_supported_auth_types() {
        let urlencoded: PostmanItem = serde_json::from_str(
            r#"{
              "name": "Login",
              "request": {
                "method": "POST",
                "url": "https://api.example.test/login",
                "auth": { "type": "basic", "basic": [
                  { "key": "username", "value": "alice" },
                  { "key": "password", "value": "secret" }
                ] },
                "body": { "mode": "urlencoded", "urlencoded": [
                  { "key": "grant_type", "value": "password" },
                  { "key": "unused", "value": "x", "disabled": true }
                ] }
              }
            }"#,
        )
        .expect("parse Postman item");
        let request = map_postman_request(&urlencoded, urlencoded.request.as_ref().unwrap())
            .expect("map urlencoded request");
        assert_eq!(request.body.mode, "form-urlencoded");
        assert_eq!(request.body.form[0].key, "grant_type");
        assert!(!request.body.form[1].enabled);
        assert_eq!(request.auth.auth_type, "basic");
        assert_eq!(request.auth.basic_username, "alice");
        assert_eq!(request.auth.basic_password, "secret");

        let multipart: PostmanItem = serde_json::from_str(
            r#"{
              "name": "Upload",
              "request": {
                "method": "POST",
                "url": "https://api.example.test/upload",
                "auth": { "type": "apikey", "apikey": [
                  { "key": "key", "value": "X-API-Key" },
                  { "key": "value", "value": "{{api_key}}" },
                  { "key": "in", "value": "query" }
                ] },
                "body": { "mode": "formdata", "formdata": [
                  { "key": "caption", "value": "Quarterly report", "type": "text" },
                  { "key": "report", "src": ["/tmp/report.pdf", "/tmp/ignored.pdf"], "type": "file" }
                ] }
              }
            }"#,
        )
        .expect("parse Postman item");
        let request = map_postman_request(&multipart, multipart.request.as_ref().unwrap())
            .expect("map multipart request");
        assert_eq!(request.body.mode, "multipart");
        assert_eq!(request.body.form[0].key, "caption");
        assert_eq!(request.body.files[0].name, "report");
        assert_eq!(request.body.files[0].path, "/tmp/report.pdf");
        assert_eq!(request.auth.auth_type, "api-key");
        assert_eq!(request.auth.api_key_name, "X-API-Key");
        assert_eq!(request.auth.api_key_value, "{{api_key}}");
        assert_eq!(request.auth.api_key_in, "query");

        let oauth: PostmanItem = serde_json::from_str(
            r#"{
              "name": "OAuth",
              "request": {
                "method": "GET",
                "url": "https://api.example.test/me",
                "auth": { "type": "oauth2", "oauth2": [
                  { "key": "accessToken", "value": "{{access_token}}" },
                  { "key": "tokenUrl", "value": "https://auth.example.test/token" },
                  { "key": "clientId", "value": "client" },
                  { "key": "clientSecret", "value": "{{client_secret}}" },
                  { "key": "scope", "value": "read write" }
                ] }
              }
            }"#,
        )
        .expect("parse Postman item");
        let request = map_postman_request(&oauth, oauth.request.as_ref().unwrap())
            .expect("map OAuth request");
        assert_eq!(request.auth.auth_type, "oauth2");
        assert_eq!(request.auth.oauth2_access_token, "{{access_token}}");
        assert_eq!(
            request.auth.oauth2_token_url,
            "https://auth.example.test/token"
        );
        assert_eq!(request.auth.oauth2_client_id, "client");
        assert_eq!(request.auth.oauth2_client_secret, "{{client_secret}}");
        assert_eq!(request.auth.oauth2_scope, "read write");
    }

    #[tokio::test]
    async fn environment_import_preserves_values_activation_and_secret_storage() {
        let pool = setup_test_db().await;
        let store = Arc::new(InMemorySecretStore::default());
        let secret_store: Arc<dyn SecretStore> = store.clone();
        let input = ImportEnvironmentInput {
            source: r#"{
              "name": "  Production  ",
              "values": [
                { "key": "host", "value": "api.example.test", "enabled": true },
                { "key": "retries", "value": 3 },
                { "key": "feature", "value": true, "disabled": true },
                { "key": "token", "value": "top-secret", "type": "secret" },
                { "key": "", "value": "not-counted" }
              ]
            }"#
            .to_string(),
            set_active: true,
        };

        let result = import_postman_environment(&pool, secret_store.clone(), &input)
            .await
            .expect("import Postman environment");

        assert_eq!(result.environment_name, "Production");
        assert_eq!(result.imported_variable_count, 4);
        assert!(result.activated);

        let environment =
            environments_service::get_environment(&pool, secret_store, &result.environment_id)
                .await
                .expect("read imported environment");
        assert!(environment.is_active);
        assert!(environment
            .variables
            .iter()
            .any(|variable| variable.key == "retries" && variable.value == "3"));
        assert!(environment.variables.iter().any(|variable| {
            variable.key == "feature" && variable.value == "true" && !variable.enabled
        }));
        let secret = environment
            .variables
            .iter()
            .find(|variable| variable.key == "token")
            .expect("secret variable");
        assert!(secret.is_secret);
        assert_eq!(secret.value, "top-secret");

        let stored: String =
            sqlx::query_scalar("SELECT variables_json FROM environments WHERE id = ?1")
                .bind(&result.environment_id)
                .fetch_one(&pool)
                .await
                .expect("read stored variables");
        assert!(!stored.contains("top-secret"));
        assert_eq!(
            store
                .get_environment_variable_secret(&result.environment_id, &secret.id)
                .expect("read stored secret"),
            Some("top-secret".to_string())
        );
    }

    #[tokio::test]
    async fn collection_import_rejects_documents_without_requests() {
        let pool = setup_test_db().await;
        let error = import_postman_collection(
            &pool,
            r#"{"info":{"name":"Empty"},"item":[{"name":"Folder","item":[]}]}"#,
        )
        .await
        .expect_err("empty Postman collection should fail");

        assert_eq!(
            error.to_string(),
            "No requests were found in this Postman collection."
        );
    }
}
