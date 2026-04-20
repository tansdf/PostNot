use sqlx::SqlitePool;
use std::sync::Arc;

use crate::{
    domain::{
        environments::{ImportEnvironmentInput, ImportEnvironmentResult},
        imports::{ImportRequestInput, ImportResult, ImportedRequestDraft},
    },
    error::{AppError, AppResult},
    services::secret_store_service::SecretStore,
};

mod curl;
mod openapi;
mod postman;
mod shared;

fn require_non_empty_source(source: &str) -> AppResult<&str> {
    let trimmed = source.trim();
    if trimmed.is_empty() {
        return Err(AppError::Message(
            "Import source cannot be empty.".to_string(),
        ));
    }
    Ok(trimmed)
}

pub async fn import_requests(
    pool: &SqlitePool,
    input: &ImportRequestInput,
) -> AppResult<ImportResult> {
    let source = require_non_empty_source(&input.source)?;

    match input.format.as_str() {
        "postman" => postman::import_postman_collection(pool, source).await,
        "openapi" => openapi::import_openapi_collection(pool, source).await,
        "curl" => {
            curl::import_curl_request(pool, source, input.target_collection_id.as_deref()).await
        }
        _ => Err(AppError::Message("Unsupported import format.".to_string())),
    }
}

pub fn import_curl_to_draft(source: &str) -> AppResult<ImportedRequestDraft> {
    let source = require_non_empty_source(source)?;

    Ok(ImportedRequestDraft {
        request: curl::parse_curl_command(source)?,
    })
}

pub fn import_openapi_to_draft(source: &str) -> AppResult<ImportedRequestDraft> {
    let source = require_non_empty_source(source)?;

    openapi::import_openapi_to_draft(source)
}

pub async fn import_postman_environment(
    pool: &SqlitePool,
    secret_store: Arc<dyn SecretStore>,
    input: &ImportEnvironmentInput,
) -> AppResult<ImportEnvironmentResult> {
    let source = require_non_empty_source(&input.source)?;

    let normalized_input = ImportEnvironmentInput {
        source: source.to_string(),
        set_active: input.set_active,
    };

    postman::import_postman_environment(pool, secret_store, &normalized_input).await
}
