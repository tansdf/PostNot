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
mod postnot;
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
        "postnot" => postnot::import_postnot_collection(pool, source).await,
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

#[cfg(test)]
mod tests {
    use sqlx::sqlite::SqlitePoolOptions;

    use super::*;

    #[test]
    fn draft_imports_reject_empty_sources_at_the_shared_boundary() {
        let curl_error = import_curl_to_draft(" \n\t ").expect_err("empty cURL source should fail");
        assert_eq!(curl_error.to_string(), "Import source cannot be empty.");

        let openapi_error =
            import_openapi_to_draft("").expect_err("empty OpenAPI source should fail");
        assert_eq!(openapi_error.to_string(), "Import source cannot be empty.");
    }

    #[tokio::test]
    async fn collection_dispatch_rejects_empty_and_unknown_formats() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory database");

        let empty = import_requests(
            &pool,
            &ImportRequestInput {
                format: "curl".to_string(),
                source: "  ".to_string(),
                target_collection_id: None,
            },
        )
        .await
        .expect_err("empty collection source should fail");
        assert_eq!(empty.to_string(), "Import source cannot be empty.");

        let unsupported = import_requests(
            &pool,
            &ImportRequestInput {
                format: "insomnia".to_string(),
                source: "{}".to_string(),
                target_collection_id: None,
            },
        )
        .await
        .expect_err("unknown collection format should fail");
        assert_eq!(unsupported.to_string(), "Unsupported import format.");
    }
}
