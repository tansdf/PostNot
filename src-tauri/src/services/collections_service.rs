use std::collections::HashMap;

use chrono::Utc;
use sqlx::{Row, Sqlite, SqlitePool, Transaction};
use uuid::Uuid;

use crate::{
    domain::{
        collections::{
            CollectionItemSummary, CollectionSummary, CreateCollectionFolderInput,
            CreateCollectionInput, MoveCollectionItemInput, SavedRequestDetail,
            SavedRequestSummary,
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

pub async fn list_collection_items(
    pool: &SqlitePool,
    collection_id: &str,
) -> AppResult<Vec<CollectionItemSummary>> {
    ensure_collection_exists(pool, collection_id).await?;

    let rows = list_collection_item_rows(pool, collection_id).await?;
    Ok(build_collection_item_tree(rows))
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

pub async fn create_collection_folder(
    pool: &SqlitePool,
    collection_id: &str,
    input: &CreateCollectionFolderInput,
) -> AppResult<CollectionItemSummary> {
    let name = input.name.trim();
    if name.is_empty() {
        return Err(AppError::Message("Folder name is required.".to_string()));
    }

    ensure_collection_exists(pool, collection_id).await?;
    validate_parent_folder(pool, collection_id, input.parent_id.as_deref()).await?;

    let item_id = Uuid::new_v4().to_string();
    let now = now_iso();
    let sort_order = next_sort_order(pool, collection_id, input.parent_id.as_deref()).await?;

    sqlx::query(
        r#"
        INSERT INTO collection_items (
          id, collection_id, parent_id, kind, name, sort_order, method, url,
          query_params_json, headers_json, body_json, auth_json,
          prerequest_script, test_script, created_at, updated_at
        ) VALUES (?1, ?2, ?3, 'folder', ?4, ?5, NULL, NULL, '[]', '[]', '{}', '{}', '', '', ?6, ?7)
        "#,
    )
    .bind(&item_id)
    .bind(collection_id)
    .bind(input.parent_id.as_deref())
    .bind(name)
    .bind(sort_order)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;

    touch_collection(pool, collection_id).await?;
    get_collection_item_summary(pool, &item_id).await
}

pub async fn move_collection_item(
    pool: &SqlitePool,
    item_id: &str,
    input: &MoveCollectionItemInput,
) -> AppResult<SavedRequestSummary> {
    let target_collection_id = input.target_collection_id.trim();
    if target_collection_id.is_empty() {
        return Err(AppError::Message(
            "Target collection is required.".to_string(),
        ));
    }

    ensure_collection_exists(pool, target_collection_id).await?;
    validate_parent_folder(pool, target_collection_id, input.target_parent_id.as_deref()).await?;

    let mut transaction = pool.begin().await?;
    let item_row = sqlx::query(
        "SELECT collection_id, parent_id, kind FROM collection_items WHERE id = ?1",
    )
    .bind(item_id)
    .fetch_optional(&mut *transaction)
    .await?
    .ok_or_else(|| AppError::Message("Collection item not found.".to_string()))?;

    let source_collection_id: String = item_row.get("collection_id");
    let source_parent_id: Option<String> = item_row.get("parent_id");
    let kind: String = item_row.get("kind");

    if kind != "request" {
        return Err(AppError::Message(
            "Only saved requests can be moved right now.".to_string(),
        ));
    }

    let source_sibling_ids = list_sibling_ids(
        &mut transaction,
        &source_collection_id,
        source_parent_id.as_deref(),
        Some(item_id),
    )
    .await?;

    let same_parent = source_collection_id == target_collection_id
        && source_parent_id.as_deref() == input.target_parent_id.as_deref();

    let mut destination_sibling_ids = if same_parent {
        source_sibling_ids.clone()
    } else {
        list_sibling_ids(
            &mut transaction,
            target_collection_id,
            input.target_parent_id.as_deref(),
            Some(item_id),
        )
        .await?
    };

    let insert_index = normalize_target_index(input.target_index, destination_sibling_ids.len());
    destination_sibling_ids.insert(insert_index, item_id.to_string());

    let now = now_iso();

    sqlx::query(
        r#"
        UPDATE collection_items
        SET collection_id = ?2,
            parent_id = ?3,
            updated_at = ?4
        WHERE id = ?1 AND kind = 'request'
        "#,
    )
    .bind(item_id)
    .bind(target_collection_id)
    .bind(input.target_parent_id.as_deref())
    .bind(&now)
    .execute(&mut *transaction)
    .await?;

    if same_parent {
        resequence_siblings(
            &mut transaction,
            target_collection_id,
            input.target_parent_id.as_deref(),
            &destination_sibling_ids,
        )
        .await?;
    } else {
        resequence_siblings(
            &mut transaction,
            &source_collection_id,
            source_parent_id.as_deref(),
            &source_sibling_ids,
        )
        .await?;
        resequence_siblings(
            &mut transaction,
            target_collection_id,
            input.target_parent_id.as_deref(),
            &destination_sibling_ids,
        )
        .await?;
    }

    touch_collection_in_transaction(&mut transaction, &source_collection_id, &now).await?;
    if source_collection_id != target_collection_id {
        touch_collection_in_transaction(&mut transaction, target_collection_id, &now).await?;
    }

    transaction.commit().await?;
    get_saved_request_summary(pool, item_id).await
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
        SELECT id, collection_id, parent_id, name, method, url, updated_at
        FROM collection_items
        WHERE collection_id = ?1 AND kind = 'request'
        ORDER BY updated_at DESC, name ASC
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
        SELECT id, collection_id, parent_id, name, method, url, query_params_json, headers_json, body_json, auth_json, prerequest_script, test_script, updated_at
        FROM collection_items
        WHERE collection_id = ?1 AND kind = 'request'
        ORDER BY updated_at DESC, name ASC
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
    parent_id: Option<&str>,
    request: &SendRequestPayload,
) -> AppResult<SavedRequestSummary> {
    ensure_collection_exists(pool, collection_id).await?;
    validate_parent_folder(pool, collection_id, parent_id).await?;

    let item_id = Uuid::new_v4().to_string();
    let item_name = saved_request_name(request);
    let now = now_iso();
    let sort_order = next_sort_order(pool, collection_id, parent_id).await?;

    sqlx::query(
        r#"
        INSERT INTO collection_items (
          id, collection_id, parent_id, kind, name, sort_order, method, url,
          query_params_json, headers_json, body_json, auth_json,
          prerequest_script, test_script, created_at, updated_at
        ) VALUES (?1, ?2, ?3, 'request', ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
        "#,
    )
    .bind(&item_id)
    .bind(collection_id)
    .bind(parent_id)
    .bind(&item_name)
    .bind(sort_order)
    .bind(&request.method)
    .bind(&request.url)
    .bind(serde_json::to_string(&request.query_params)?)
    .bind(serde_json::to_string(&request.headers)?)
    .bind(serde_json::to_string(&request.body)?)
    .bind(serde_json::to_string(&request.auth)?)
    .bind(&request.pre_request_script)
    .bind(&request.test_script)
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
            prerequest_script = ?9,
            test_script = ?10,
            updated_at = ?11
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
    .bind(&request.pre_request_script)
    .bind(&request.test_script)
    .bind(&now)
    .execute(pool)
    .await?;

    touch_collection(pool, &collection_id).await?;
    get_saved_request_summary(pool, item_id).await
}

pub async fn get_saved_request(pool: &SqlitePool, item_id: &str) -> AppResult<SavedRequestDetail> {
    let row = sqlx::query(
        r#"
        SELECT id, collection_id, parent_id, name, method, url, query_params_json, headers_json, body_json, auth_json, prerequest_script, test_script, updated_at
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
        parent_id: row.get("parent_id"),
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
            pre_request_script: row.get("prerequest_script"),
            test_script: row.get("test_script"),
        },
    })
}

pub async fn delete_collection_item(pool: &SqlitePool, item_id: &str) -> AppResult<()> {
    let collection_id: Option<String> =
        sqlx::query_scalar("SELECT collection_id FROM collection_items WHERE id = ?1")
            .bind(item_id)
            .fetch_optional(pool)
            .await?;

    let Some(collection_id) = collection_id else {
        return Err(AppError::Message("Collection item not found.".to_string()));
    };

    sqlx::query("DELETE FROM collection_items WHERE id = ?1")
        .bind(item_id)
        .execute(pool)
        .await?;

    touch_collection(pool, &collection_id).await?;
    Ok(())
}

pub async fn delete_saved_request(pool: &SqlitePool, item_id: &str) -> AppResult<()> {
    let exists: Option<String> =
        sqlx::query_scalar("SELECT id FROM collection_items WHERE id = ?1 AND kind = 'request'")
            .bind(item_id)
            .fetch_optional(pool)
            .await?;

    if exists.is_none() {
        return Err(AppError::Message("Saved request not found.".to_string()));
    }

    delete_collection_item(pool, item_id).await
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

async fn ensure_collection_exists(pool: &SqlitePool, collection_id: &str) -> AppResult<()> {
    let exists: Option<String> = sqlx::query_scalar("SELECT id FROM collections WHERE id = ?1")
        .bind(collection_id)
        .fetch_optional(pool)
        .await?;

    if exists.is_none() {
        return Err(AppError::Message("Collection not found.".to_string()));
    }

    Ok(())
}

async fn validate_parent_folder(
    pool: &SqlitePool,
    collection_id: &str,
    parent_id: Option<&str>,
) -> AppResult<()> {
    let Some(parent_id) = parent_id else {
        return Ok(());
    };

    let row = sqlx::query(
        "SELECT kind FROM collection_items WHERE id = ?1 AND collection_id = ?2",
    )
    .bind(parent_id)
    .bind(collection_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::Message("Target folder not found.".to_string()))?;

    let kind: String = row.get("kind");
    if kind != "folder" {
        return Err(AppError::Message(
            "Target parent must be a folder.".to_string(),
        ));
    }

    Ok(())
}

async fn next_sort_order(
    pool: &SqlitePool,
    collection_id: &str,
    parent_id: Option<&str>,
) -> AppResult<i64> {
    let sort_order: i64 = sqlx::query_scalar(
        r#"
        SELECT COALESCE(MAX(sort_order), -1) + 1
        FROM collection_items
        WHERE collection_id = ?1
          AND (
            (?2 IS NULL AND parent_id IS NULL)
            OR parent_id = ?2
          )
        "#,
    )
    .bind(collection_id)
    .bind(parent_id)
    .fetch_one(pool)
    .await?;

    Ok(sort_order)
}

async fn list_sibling_ids(
    transaction: &mut Transaction<'_, Sqlite>,
    collection_id: &str,
    parent_id: Option<&str>,
    exclude_item_id: Option<&str>,
) -> AppResult<Vec<String>> {
    let rows = sqlx::query(
        r#"
        SELECT id
        FROM collection_items
        WHERE collection_id = ?1
          AND (
            (?2 IS NULL AND parent_id IS NULL)
            OR parent_id = ?2
          )
          AND (?3 IS NULL OR id != ?3)
        ORDER BY sort_order ASC, updated_at DESC, name ASC
        "#,
    )
    .bind(collection_id)
    .bind(parent_id)
    .bind(exclude_item_id)
    .fetch_all(&mut **transaction)
    .await?;

    Ok(rows.into_iter().map(|row| row.get("id")).collect())
}

async fn resequence_siblings(
    transaction: &mut Transaction<'_, Sqlite>,
    collection_id: &str,
    parent_id: Option<&str>,
    item_ids: &[String],
) -> AppResult<()> {
    for (sort_order, sibling_id) in item_ids.iter().enumerate() {
        sqlx::query(
            r#"
            UPDATE collection_items
            SET sort_order = ?2
            WHERE id = ?1
              AND collection_id = ?3
              AND (
                (?4 IS NULL AND parent_id IS NULL)
                OR parent_id = ?4
              )
            "#,
        )
        .bind(sibling_id)
        .bind(sort_order as i64)
        .bind(collection_id)
        .bind(parent_id)
        .execute(&mut **transaction)
        .await?;
    }

    Ok(())
}

async fn get_saved_request_summary(
    pool: &SqlitePool,
    item_id: &str,
) -> AppResult<SavedRequestSummary> {
    let row = sqlx::query(
        "SELECT id, collection_id, parent_id, name, method, url, updated_at FROM collection_items WHERE id = ?1 AND kind = 'request'",
    )
    .bind(item_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::Message("Saved request not found.".to_string()))?;

    Ok(map_saved_request_summary(row))
}

async fn get_collection_item_summary(
    pool: &SqlitePool,
    item_id: &str,
) -> AppResult<CollectionItemSummary> {
    let row = sqlx::query(
        r#"
        SELECT id, collection_id, parent_id, kind, name, method, url, updated_at
        FROM collection_items
        WHERE id = ?1
        "#,
    )
    .bind(item_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::Message("Collection item not found.".to_string()))?;

    Ok(CollectionItemSummary {
        id: row.get("id"),
        collection_id: row.get("collection_id"),
        parent_id: row.get("parent_id"),
        kind: row.get("kind"),
        name: row.get("name"),
        method: row.get("method"),
        url: row.get("url"),
        updated_at: row.get("updated_at"),
        children: Vec::new(),
    })
}

async fn list_collection_item_rows(
    pool: &SqlitePool,
    collection_id: &str,
) -> AppResult<Vec<CollectionItemRow>> {
    let rows = sqlx::query(
        r#"
        SELECT id, collection_id, parent_id, kind, name, method, url, updated_at, sort_order
        FROM collection_items
        WHERE collection_id = ?1
        ORDER BY sort_order ASC, updated_at DESC, name ASC
        "#,
    )
    .bind(collection_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| CollectionItemRow {
            id: row.get("id"),
            collection_id: row.get("collection_id"),
            parent_id: row.get("parent_id"),
            kind: row.get("kind"),
            name: row.get("name"),
            method: row.get("method"),
            url: row.get("url"),
            updated_at: row.get("updated_at"),
            sort_order: row.get("sort_order"),
        })
        .collect())
}

fn build_collection_item_tree(rows: Vec<CollectionItemRow>) -> Vec<CollectionItemSummary> {
    let mut children_by_parent: HashMap<Option<String>, Vec<CollectionItemRow>> = HashMap::new();

    for row in rows {
        children_by_parent
            .entry(row.parent_id.clone())
            .or_default()
            .push(row);
    }

    for children in children_by_parent.values_mut() {
        children.sort_by(|left, right| {
            left.sort_order
                .cmp(&right.sort_order)
                .then_with(|| right.updated_at.cmp(&left.updated_at))
                .then_with(|| left.name.cmp(&right.name))
        });
    }

    build_collection_item_branch(&mut children_by_parent, None)
}

fn build_collection_item_branch(
    children_by_parent: &mut HashMap<Option<String>, Vec<CollectionItemRow>>,
    parent_id: Option<&str>,
) -> Vec<CollectionItemSummary> {
    let key = parent_id.map(|value| value.to_string());
    let Some(rows) = children_by_parent.remove(&key) else {
        return Vec::new();
    };

    rows.into_iter()
        .map(|row| {
            let children = if row.kind == "folder" {
                build_collection_item_branch(children_by_parent, Some(&row.id))
            } else {
                Vec::new()
            };

            CollectionItemSummary {
                id: row.id,
                collection_id: row.collection_id,
                parent_id: row.parent_id,
                kind: row.kind,
                name: row.name,
                method: row.method,
                url: row.url,
                updated_at: row.updated_at,
                children,
            }
        })
        .collect()
}

async fn touch_collection(pool: &SqlitePool, collection_id: &str) -> AppResult<()> {
    let now = now_iso();
    sqlx::query("UPDATE collections SET updated_at = ?2 WHERE id = ?1")
        .bind(collection_id)
        .bind(&now)
        .execute(pool)
        .await?;

    Ok(())
}

async fn touch_collection_in_transaction(
    transaction: &mut Transaction<'_, Sqlite>,
    collection_id: &str,
    now: &str,
) -> AppResult<()> {
    sqlx::query("UPDATE collections SET updated_at = ?2 WHERE id = ?1")
        .bind(collection_id)
        .bind(now)
        .execute(&mut **transaction)
        .await?;

    Ok(())
}

fn normalize_target_index(target_index: Option<i64>, sibling_count: usize) -> usize {
    match target_index {
        Some(index) if index > 0 => (index as usize).min(sibling_count),
        Some(_) => 0,
        None => sibling_count,
    }
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
        parent_id: row.get("parent_id"),
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
        parent_id: row.get("parent_id"),
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
            pre_request_script: row.get("prerequest_script"),
            test_script: row.get("test_script"),
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

#[derive(Debug)]
struct CollectionItemRow {
    id: String,
    collection_id: String,
    parent_id: Option<String>,
    kind: String,
    name: String,
    method: Option<String>,
    url: Option<String>,
    updated_at: String,
    sort_order: i64,
}
