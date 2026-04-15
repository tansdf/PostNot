use std::sync::Arc;

use serde::Serialize;
use sqlx::SqlitePool;

use crate::{
    domain::{
        collections::{CollectionItemSummary, SavedRequestDetail},
        environments::EnvironmentVariable,
        exports::ExportResult,
        requests::{FileRow, KeyValueRow, RequestAuth, RequestBody},
    },
    error::{AppError, AppResult},
    services::{collections_service, environments_service, secret_store_service::SecretStore},
};

pub async fn export_collection(
    pool: &SqlitePool,
    collection_id: &str,
) -> AppResult<Option<ExportResult>> {
    let collection = collections_service::get_collection(pool, collection_id).await?;
    let items = collections_service::list_collection_items(pool, collection_id).await?;
    let requests = collections_service::list_saved_request_details(pool, collection_id).await?;
    let requests_by_id = requests
        .into_iter()
        .map(|request| (request.id.clone(), request))
        .collect();

    let payload = PostmanCollectionExport {
        info: PostmanCollectionInfoExport {
            name: collection.name.clone(),
            description: optional_string(&collection.description),
            schema: "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"
                .to_string(),
        },
        event: build_events(&collection.pre_request_script, &collection.test_script),
        item: map_collection_items(&items, &requests_by_id)?,
    };

    let json = serde_json::to_string_pretty(&payload)?;
    let suggested_name = format!(
        "{}.postman_collection.json",
        sanitize_file_stem(&collection.name, "collection")
    );

    save_json_file(
        "Export collection as Postman Collection JSON",
        &suggested_name,
        json,
    )
    .await
}

pub async fn export_environment(
    pool: &SqlitePool,
    secret_store: Arc<dyn SecretStore>,
    environment_id: &str,
) -> AppResult<Option<ExportResult>> {
    let environment =
        environments_service::get_environment(pool, secret_store, environment_id).await?;
    let payload = PostmanEnvironmentExport {
        id: environment.id.clone(),
        name: environment.name.clone(),
        values: environment
            .variables
            .iter()
            .filter(|item| !item.key.trim().is_empty())
            .map(map_environment_value)
            .collect(),
        postman_variable_scope: "environment".to_string(),
        postman_exported_at: chrono::Utc::now().to_rfc3339(),
        postman_exported_using: "PostNot".to_string(),
    };

    let json = serde_json::to_string_pretty(&payload)?;
    let suggested_name = format!(
        "{}.postman_environment.json",
        sanitize_file_stem(&environment.name, "environment")
    );

    save_json_file(
        "Export environment as Postman Environment JSON",
        &suggested_name,
        json,
    )
    .await
}

async fn save_json_file(
    title: &str,
    suggested_name: &str,
    json: String,
) -> AppResult<Option<ExportResult>> {
    let title = title.to_string();
    let suggested_name = suggested_name.to_string();

    tauri::async_runtime::spawn_blocking(move || -> AppResult<Option<ExportResult>> {
        let Some(path) = rfd::FileDialog::new()
            .set_title(&title)
            .set_file_name(&suggested_name)
            .add_filter("JSON", &["json"])
            .save_file()
        else {
            return Ok(None);
        };

        std::fs::write(&path, json)?;

        Ok(Some(ExportResult {
            file_path: path.to_string_lossy().to_string(),
        }))
    })
    .await
    .map_err(|error| AppError::Message(error.to_string()))?
}

fn map_collection_items(
    items: &[CollectionItemSummary],
    requests_by_id: &std::collections::HashMap<String, SavedRequestDetail>,
) -> AppResult<Vec<PostmanCollectionItemExport>> {
    items.iter()
        .map(|item| match item.kind.as_str() {
            "folder" => Ok(PostmanCollectionItemExport {
                name: item.name.clone(),
                request: None,
                event: build_events(&item.pre_request_script, &item.test_script),
                item: map_collection_items(&item.children, requests_by_id)?,
            }),
            "request" => {
                let request = requests_by_id.get(&item.id).ok_or_else(|| {
                    AppError::Message(format!(
                        "Saved request details were missing for collection item {}.",
                        item.id
                    ))
                })?;
                Ok(map_saved_request_item(request))
            }
            _ => Err(AppError::Message(format!(
                "Unsupported collection item kind: {}",
                item.kind
            ))),
        })
        .collect()
}

fn map_saved_request_item(request: &SavedRequestDetail) -> PostmanCollectionItemExport {
    PostmanCollectionItemExport {
        name: request.name.clone(),
        request: Some(PostmanRequestExport {
            method: request.request.method.clone(),
            header: request
                .request
                .headers
                .iter()
                .filter(|item| has_any_text(item))
                .map(map_header)
                .collect(),
            auth: map_auth(&request.request.auth),
            body: map_body(&request.request.body),
            url: map_url(&request.request.url, &request.request.query_params),
        }),
        event: build_events(
            &request.request.pre_request_script,
            &request.request.test_script,
        ),
        item: Vec::new(),
    }
}

fn build_events(pre_request_script: &str, test_script: &str) -> Vec<PostmanEventExport> {
    let mut events = Vec::new();

    if !pre_request_script.trim().is_empty() {
        events.push(PostmanEventExport {
            listen: "prerequest".to_string(),
            script: PostmanScriptExport {
                script_type: "text/javascript".to_string(),
                exec: pre_request_script
                    .lines()
                    .map(|line| line.to_string())
                    .collect(),
            },
        });
    }

    if !test_script.trim().is_empty() {
        events.push(PostmanEventExport {
            listen: "test".to_string(),
            script: PostmanScriptExport {
                script_type: "text/javascript".to_string(),
                exec: test_script.lines().map(|line| line.to_string()).collect(),
            },
        });
    }

    events
}

fn map_url(base_url: &str, query_params: &[KeyValueRow]) -> PostmanUrlExport {
    let exported_query: Vec<PostmanQueryExport> = query_params
        .iter()
        .filter(|item| has_any_text(item))
        .map(|item| PostmanQueryExport {
            key: item.key.clone(),
            value: item.value.clone(),
            disabled: !item.enabled,
        })
        .collect();

    PostmanUrlExport {
        raw: build_export_url(base_url, query_params),
        query: exported_query,
    }
}

fn map_header(header: &KeyValueRow) -> PostmanHeaderExport {
    PostmanHeaderExport {
        key: header.key.clone(),
        value: header.value.clone(),
        disabled: !header.enabled,
    }
}

fn map_auth(auth: &RequestAuth) -> Option<PostmanAuthExport> {
    match auth.auth_type.as_str() {
        "basic" => Some(PostmanAuthExport {
            auth_type: "basic".to_string(),
            basic: vec![
                PostmanAuthValueExport {
                    key: "username".to_string(),
                    value: auth.basic_username.clone(),
                },
                PostmanAuthValueExport {
                    key: "password".to_string(),
                    value: auth.basic_password.clone(),
                },
            ],
            bearer: vec![],
            api_key: vec![],
        }),
        "bearer" => Some(PostmanAuthExport {
            auth_type: "bearer".to_string(),
            basic: vec![],
            bearer: vec![PostmanAuthValueExport {
                key: "token".to_string(),
                value: auth.bearer_token.clone(),
            }],
            api_key: vec![],
        }),
        "api-key" => Some(PostmanAuthExport {
            auth_type: "apikey".to_string(),
            basic: vec![],
            bearer: vec![],
            api_key: vec![
                PostmanAuthValueExport {
                    key: "key".to_string(),
                    value: auth.api_key_name.clone(),
                },
                PostmanAuthValueExport {
                    key: "value".to_string(),
                    value: auth.api_key_value.clone(),
                },
                PostmanAuthValueExport {
                    key: "in".to_string(),
                    value: auth.api_key_in.clone(),
                },
            ],
        }),
        _ => None,
    }
}

fn map_body(body: &RequestBody) -> Option<PostmanBodyExport> {
    match body.mode.as_str() {
        "json" => Some(PostmanBodyExport {
            body_mode: "raw".to_string(),
            raw: body.raw.clone(),
            urlencoded: vec![],
            formdata: vec![],
            options: Some(PostmanBodyOptionsExport {
                raw: Some(PostmanRawOptionsExport {
                    language: "json".to_string(),
                }),
            }),
        }),
        "raw" => Some(PostmanBodyExport {
            body_mode: "raw".to_string(),
            raw: body.raw.clone(),
            urlencoded: vec![],
            formdata: vec![],
            options: None,
        }),
        "form-urlencoded" => Some(PostmanBodyExport {
            body_mode: "urlencoded".to_string(),
            raw: String::new(),
            urlencoded: body
                .form
                .iter()
                .filter(|item| has_any_text(item))
                .map(|item| PostmanFormValueExport {
                    key: item.key.clone(),
                    value: item.value.clone(),
                    disabled: !item.enabled,
                })
                .collect(),
            formdata: vec![],
            options: None,
        }),
        "multipart" => Some(PostmanBodyExport {
            body_mode: "formdata".to_string(),
            raw: String::new(),
            urlencoded: vec![],
            formdata: build_formdata(body),
            options: None,
        }),
        _ => None,
    }
}

fn build_formdata(body: &RequestBody) -> Vec<PostmanFormDataValueExport> {
    let form_fields = body
        .form
        .iter()
        .filter(|item| has_any_text(item))
        .map(|item| PostmanFormDataValueExport {
            key: item.key.clone(),
            value: Some(item.value.clone()),
            src: None,
            item_type: "text".to_string(),
            disabled: !item.enabled,
        });

    let files = body
        .files
        .iter()
        .filter(|item| has_file_text(item))
        .map(|item| PostmanFormDataValueExport {
            key: item.name.clone(),
            value: None,
            src: Some(item.path.clone()),
            item_type: "file".to_string(),
            disabled: !item.enabled,
        });

    form_fields.chain(files).collect()
}

fn map_environment_value(item: &EnvironmentVariable) -> PostmanEnvironmentValueExport {
    PostmanEnvironmentValueExport {
        key: item.key.clone(),
        value: if item.is_secret {
            String::new()
        } else {
            item.value.clone()
        },
        enabled: item.enabled,
        value_type: if item.is_secret {
            "secret".to_string()
        } else {
            "any".to_string()
        },
    }
}

fn build_export_url(base_url: &str, query_params: &[KeyValueRow]) -> String {
    let active_query: Vec<String> = query_params
        .iter()
        .filter(|item| item.enabled && !item.key.trim().is_empty())
        .map(|item| {
            if item.value.is_empty() {
                item.key.clone()
            } else {
                format!("{}={}", item.key, item.value)
            }
        })
        .collect();

    if active_query.is_empty() {
        return base_url.to_string();
    }

    let separator = if base_url.contains('?') { "&" } else { "?" };
    format!("{}{}{}", base_url, separator, active_query.join("&"))
}

fn sanitize_file_stem(value: &str, fallback: &str) -> String {
    let sanitized = value
        .trim()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();

    if sanitized.is_empty() {
        fallback.to_string()
    } else {
        sanitized
    }
}

fn optional_string(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn has_any_text(item: &KeyValueRow) -> bool {
    !item.key.trim().is_empty() || !item.value.trim().is_empty()
}

fn has_file_text(item: &FileRow) -> bool {
    !item.name.trim().is_empty() || !item.path.trim().is_empty()
}

fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Debug, Serialize)]
struct PostmanCollectionExport {
    info: PostmanCollectionInfoExport,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    event: Vec<PostmanEventExport>,
    item: Vec<PostmanCollectionItemExport>,
}

#[derive(Debug, Serialize)]
struct PostmanCollectionInfoExport {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    schema: String,
}

#[derive(Debug, Serialize)]
struct PostmanCollectionItemExport {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    request: Option<PostmanRequestExport>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    event: Vec<PostmanEventExport>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    item: Vec<PostmanCollectionItemExport>,
}

#[derive(Debug, Serialize)]
struct PostmanRequestExport {
    method: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    header: Vec<PostmanHeaderExport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    auth: Option<PostmanAuthExport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    body: Option<PostmanBodyExport>,
    url: PostmanUrlExport,
}

#[derive(Debug, Serialize)]
struct PostmanEventExport {
    listen: String,
    script: PostmanScriptExport,
}

#[derive(Debug, Serialize)]
struct PostmanScriptExport {
    #[serde(rename = "type")]
    script_type: String,
    exec: Vec<String>,
}

#[derive(Debug, Serialize)]
struct PostmanHeaderExport {
    key: String,
    value: String,
    #[serde(skip_serializing_if = "is_false")]
    disabled: bool,
}

#[derive(Debug, Serialize)]
struct PostmanAuthExport {
    #[serde(rename = "type")]
    auth_type: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    basic: Vec<PostmanAuthValueExport>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    bearer: Vec<PostmanAuthValueExport>,
    #[serde(rename = "apikey", skip_serializing_if = "Vec::is_empty")]
    api_key: Vec<PostmanAuthValueExport>,
}

#[derive(Debug, Serialize)]
struct PostmanAuthValueExport {
    key: String,
    value: String,
}

#[derive(Debug, Serialize)]
struct PostmanBodyExport {
    #[serde(rename = "mode")]
    body_mode: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    raw: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    urlencoded: Vec<PostmanFormValueExport>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    formdata: Vec<PostmanFormDataValueExport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<PostmanBodyOptionsExport>,
}

#[derive(Debug, Serialize)]
struct PostmanBodyOptionsExport {
    #[serde(skip_serializing_if = "Option::is_none")]
    raw: Option<PostmanRawOptionsExport>,
}

#[derive(Debug, Serialize)]
struct PostmanRawOptionsExport {
    language: String,
}

#[derive(Debug, Serialize)]
struct PostmanFormValueExport {
    key: String,
    value: String,
    #[serde(skip_serializing_if = "is_false")]
    disabled: bool,
}

#[derive(Debug, Serialize)]
struct PostmanFormDataValueExport {
    key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    src: Option<String>,
    #[serde(rename = "type")]
    item_type: String,
    #[serde(skip_serializing_if = "is_false")]
    disabled: bool,
}

#[derive(Debug, Serialize)]
struct PostmanUrlExport {
    raw: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    query: Vec<PostmanQueryExport>,
}

#[derive(Debug, Serialize)]
struct PostmanQueryExport {
    key: String,
    value: String,
    #[serde(skip_serializing_if = "is_false")]
    disabled: bool,
}

#[derive(Debug, Serialize)]
struct PostmanEnvironmentExport {
    id: String,
    name: String,
    values: Vec<PostmanEnvironmentValueExport>,
    #[serde(rename = "_postman_variable_scope")]
    postman_variable_scope: String,
    #[serde(rename = "_postman_exported_at")]
    postman_exported_at: String,
    #[serde(rename = "_postman_exported_using")]
    postman_exported_using: String,
}

#[derive(Debug, Serialize)]
struct PostmanEnvironmentValueExport {
    key: String,
    value: String,
    enabled: bool,
    #[serde(rename = "type")]
    value_type: String,
}
