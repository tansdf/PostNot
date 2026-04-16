use sqlx::SqlitePool;
use url::Url;
use uuid::Uuid;

use crate::{
    domain::{
        collections::CreateCollectionInput,
        imports::ImportResult,
        requests::{KeyValueRow, RequestAuth, RequestBody, SendRequestPayload},
    },
    error::{AppError, AppResult},
    services::collections_service,
};

use super::shared::{empty_auth, empty_kv, normalize_method};

pub(super) async fn import_curl_request(
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
                    pre_request_script: String::new(),
                    test_script: String::new(),
                },
            )
            .await?;

            (created.id, created.name, true)
        };

    collections_service::save_request(pool, &collection_id, None, &request).await?;

    Ok(ImportResult {
        collection_id,
        collection_name,
        imported_request_count: 1,
        created_collection,
    })
}

pub(super) fn parse_curl_command(source: &str) -> AppResult<SendRequestPayload> {
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
        pre_request_script: String::new(),
        test_script: String::new(),
    })
}

fn looks_like_json(value: &str) -> bool {
    let trimmed = value.trim();
    (trimmed.starts_with('{') && trimmed.ends_with('}'))
        || (trimmed.starts_with('[') && trimmed.ends_with(']'))
}
