use chrono::Utc;
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use crate::{
    domain::{
        collections::{
            CollectionSummary, CreateCollectionInput, SavedRequestDetail, SavedRequestSummary,
        },
        requests::SendRequestPayload,
    },
    error::{AppError, AppResult},
};

pub async fn list_collections(pool: &SqlitePool) -> AppResult<Vec<CollectionSummary>> {
    let rows = sqlx::query(
        r#"
        SELECT
          collections.id,
          collections.name,
          collections.description,
          collections.updated_at,
          COUNT(collection_items.id) AS request_count
        FROM collections
        LEFT JOIN collection_items
          ON collection_items.collection_id = collections.id
          AND collection_items.kind = 'request'
        GROUP BY collections.id
        ORDER BY collections.updated_at DESC, collections.name ASC
        "#,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(map_collection_summary).collect())
}

pub async fn create_collection(
    pool: &SqlitePool,
    input: &CreateCollectionInput,
) -> AppResult<CollectionSummary> {
    let name = input.name.trim();
    if name.is_empty() {
        return Err(AppError::Message(
            "Collection name is required.".to_string(),
        ));
    }

    let id = Uuid::new_v4().to_string();
    let now = now_iso();

    sqlx::query(
        "INSERT INTO collections (id, name, description, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
    )
    .bind(&id)
    .bind(name)
    .bind(input.description.trim())
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;

    get_collection(pool, &id).await
}

pub async fn update_collection(
    pool: &SqlitePool,
    collection_id: &str,
    input: &CreateCollectionInput,
) -> AppResult<CollectionSummary> {
    let name = input.name.trim();
    if name.is_empty() {
        return Err(AppError::Message(
            "Collection name is required.".to_string(),
        ));
    }

    let result = sqlx::query(
        "UPDATE collections SET name = ?2, description = ?3, updated_at = ?4 WHERE id = ?1",
    )
    .bind(collection_id)
    .bind(name)
    .bind(input.description.trim())
    .bind(now_iso())
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::Message("Collection not found.".to_string()));
    }

    get_collection(pool, collection_id).await
}

pub async fn delete_collection(pool: &SqlitePool, collection_id: &str) -> AppResult<()> {
    sqlx::query("DELETE FROM collections WHERE id = ?1")
        .bind(collection_id)
        .execute(pool)
        .await?;

    Ok(())
}

pub async fn list_saved_requests(
    pool: &SqlitePool,
    collection_id: &str,
) -> AppResult<Vec<SavedRequestSummary>> {
    let rows = sqlx::query(
        r#"
        SELECT id, collection_id, name, method, url, updated_at
        FROM collection_items
        WHERE collection_id = ?1 AND kind = 'request' AND parent_id IS NULL
        ORDER BY sort_order ASC, updated_at DESC
        "#,
    )
    .bind(collection_id)
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(map_saved_request_summary).collect())
}

pub async fn list_saved_request_details(
    pool: &SqlitePool,
    collection_id: &str,
) -> AppResult<Vec<SavedRequestDetail>> {
    let rows = sqlx::query(
        r#"
        SELECT id, collection_id, name, method, url, query_params_json, headers_json, body_json, auth_json, updated_at
        FROM collection_items
        WHERE collection_id = ?1 AND kind = 'request' AND parent_id IS NULL
        ORDER BY sort_order ASC, updated_at DESC
        "#,
    )
    .bind(collection_id)
    .fetch_all(pool)
    .await?;

    rows.into_iter().map(map_saved_request_detail).collect()
}

pub async fn save_request(
    pool: &SqlitePool,
    collection_id: &str,
    request: &SendRequestPayload,
) -> AppResult<SavedRequestSummary> {
    let collection_name: Option<String> =
        sqlx::query_scalar("SELECT name FROM collections WHERE id = ?1")
            .bind(collection_id)
            .fetch_optional(pool)
            .await?;

    if collection_name.is_none() {
        return Err(AppError::Message("Collection not found.".to_string()));
    }

    let item_id = Uuid::new_v4().to_string();
    let item_name = saved_request_name(request);
    let now = now_iso();
    let sort_order: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM collection_items WHERE collection_id = ?1 AND parent_id IS NULL",
    )
    .bind(collection_id)
    .fetch_one(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO collection_items (
          id, collection_id, parent_id, kind, name, sort_order, method, url,
          query_params_json, headers_json, body_json, auth_json,
          prerequest_script, test_script, created_at, updated_at
        ) VALUES (?1, ?2, NULL, 'request', ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, '', '', ?11, ?12)
        "#,
    )
    .bind(&item_id)
    .bind(collection_id)
    .bind(&item_name)
    .bind(sort_order)
    .bind(&request.method)
    .bind(&request.url)
    .bind(serde_json::to_string(&request.query_params)?)
    .bind(serde_json::to_string(&request.headers)?)
    .bind(serde_json::to_string(&request.body)?)
    .bind(serde_json::to_string(&request.auth)?)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;

    touch_collection(pool, collection_id).await?;
    get_saved_request_summary(pool, &item_id).await
}

pub async fn update_saved_request(
    pool: &SqlitePool,
    item_id: &str,
    request: &SendRequestPayload,
) -> AppResult<SavedRequestSummary> {
    let collection_id: Option<String> = sqlx::query_scalar(
        "SELECT collection_id FROM collection_items WHERE id = ?1 AND kind = 'request'",
    )
    .bind(item_id)
    .fetch_optional(pool)
    .await?;

    let Some(collection_id) = collection_id else {
        return Err(AppError::Message("Saved request not found.".to_string()));
    };

    let item_name = saved_request_name(request);
    let now = now_iso();

    sqlx::query(
        r#"
        UPDATE collection_items
        SET name = ?2,
            method = ?3,
            url = ?4,
            query_params_json = ?5,
            headers_json = ?6,
            body_json = ?7,
            auth_json = ?8,
            updated_at = ?9
        WHERE id = ?1 AND kind = 'request'
        "#,
    )
    .bind(item_id)
    .bind(&item_name)
    .bind(&request.method)
    .bind(&request.url)
    .bind(serde_json::to_string(&request.query_params)?)
    .bind(serde_json::to_string(&request.headers)?)
    .bind(serde_json::to_string(&request.body)?)
    .bind(serde_json::to_string(&request.auth)?)
    .bind(&now)
    .execute(pool)
    .await?;

    touch_collection(pool, &collection_id).await?;
    get_saved_request_summary(pool, item_id).await
}

pub async fn get_saved_request(pool: &SqlitePool, item_id: &str) -> AppResult<SavedRequestDetail> {
    let row = sqlx::query(
        r#"
        SELECT id, collection_id, name, method, url, query_params_json, headers_json, body_json, auth_json, updated_at
        FROM collection_items
        WHERE id = ?1 AND kind = 'request'
        "#,
    )
    .bind(item_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::Message("Saved request not found.".to_string()))?;

    Ok(SavedRequestDetail {
        id: row.get("id"),
        collection_id: row.get("collection_id"),
        name: row.get("name"),
        updated_at: row.get("updated_at"),
        request: SendRequestPayload {
            name: row.get("name"),
            method: row.get("method"),
            url: row.get("url"),
            query_params: serde_json::from_str(&row.get::<String, _>("query_params_json"))?,
            headers: serde_json::from_str(&row.get::<String, _>("headers_json"))?,
            body: serde_json::from_str(&row.get::<String, _>("body_json"))?,
            auth: serde_json::from_str(&row.get::<String, _>("auth_json"))?,
        },
    })
}

pub async fn delete_saved_request(pool: &SqlitePool, item_id: &str) -> AppResult<()> {
    let collection_id: Option<String> =
        sqlx::query_scalar("SELECT collection_id FROM collection_items WHERE id = ?1")
            .bind(item_id)
            .fetch_optional(pool)
            .await?;

    sqlx::query("DELETE FROM collection_items WHERE id = ?1")
        .bind(item_id)
        .execute(pool)
        .await?;

    if let Some(collection_id) = collection_id {
        touch_collection(pool, &collection_id).await?;
    }

    Ok(())
}

pub async fn get_collection(
    pool: &SqlitePool,
    collection_id: &str,
) -> AppResult<CollectionSummary> {
    let row = sqlx::query(
        r#"
        SELECT
          collections.id,
          collections.name,
          collections.description,
          collections.updated_at,
          COUNT(collection_items.id) AS request_count
        FROM collections
        LEFT JOIN collection_items
          ON collection_items.collection_id = collections.id
          AND collection_items.kind = 'request'
        WHERE collections.id = ?1
        GROUP BY collections.id
        "#,
    )
    .bind(collection_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::Message("Collection not found.".to_string()))?;

    Ok(map_collection_summary(row))
}

async fn get_saved_request_summary(
    pool: &SqlitePool,
    item_id: &str,
) -> AppResult<SavedRequestSummary> {
    let row = sqlx::query(
        "SELECT id, collection_id, name, method, url, updated_at FROM collection_items WHERE id = ?1 AND kind = 'request'",
    )
    .bind(item_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::Message("Saved request not found.".to_string()))?;

    Ok(map_saved_request_summary(row))
}

async fn touch_collection(pool: &SqlitePool, collection_id: &str) -> AppResult<()> {
    sqlx::query("UPDATE collections SET updated_at = ?2 WHERE id = ?1")
        .bind(collection_id)
        .bind(now_iso())
        .execute(pool)
        .await?;

    Ok(())
}

fn map_collection_summary(row: sqlx::sqlite::SqliteRow) -> CollectionSummary {
    CollectionSummary {
        id: row.get("id"),
        name: row.get("name"),
        description: row.get("description"),
        request_count: row.get("request_count"),
        updated_at: row.get("updated_at"),
    }
}

fn map_saved_request_summary(row: sqlx::sqlite::SqliteRow) -> SavedRequestSummary {
    SavedRequestSummary {
        id: row.get("id"),
        collection_id: row.get("collection_id"),
        name: row.get("name"),
        method: row.get("method"),
        url: row.get("url"),
        updated_at: row.get("updated_at"),
    }
}

fn map_saved_request_detail(row: sqlx::sqlite::SqliteRow) -> AppResult<SavedRequestDetail> {
    Ok(SavedRequestDetail {
        id: row.get("id"),
        collection_id: row.get("collection_id"),
        name: row.get("name"),
        updated_at: row.get("updated_at"),
        request: SendRequestPayload {
            name: row.get("name"),
            method: row.get("method"),
            url: row.get("url"),
            query_params: serde_json::from_str(&row.get::<String, _>("query_params_json"))?,
            headers: serde_json::from_str(&row.get::<String, _>("headers_json"))?,
            body: serde_json::from_str(&row.get::<String, _>("body_json"))?,
            auth: serde_json::from_str(&row.get::<String, _>("auth_json"))?,
        },
    })
}

fn saved_request_name(request: &SendRequestPayload) -> String {
    let trimmed_name = request.name.trim();
    if !trimmed_name.is_empty() {
        return trimmed_name.to_string();
    }

    format!("{} {}", request.method, request.url)
}

fn now_iso() -> String {
    Utc::now().to_rfc3339()
}
