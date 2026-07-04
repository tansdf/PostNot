use sqlx::{Row, SqlitePool};

use super::{
    create_collection, delete_collection, ensure_starter_collection, list_collections,
    STARTER_COLLECTION_DESCRIPTION, STARTER_COLLECTION_NAME, STARTER_COLLECTION_SEEDED_KEY,
};
use crate::domain::collections::CreateCollectionInput;

async fn setup_test_db() -> SqlitePool {
    let pool = SqlitePool::connect("sqlite::memory:")
        .await
        .expect("in-memory database");
    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&pool)
        .await
        .expect("enable foreign keys");

    for statement in [
        r#"
        CREATE TABLE collections (
          id TEXT PRIMARY KEY,
          name TEXT NOT NULL,
          description TEXT NOT NULL DEFAULT '',
          prerequest_script TEXT NOT NULL DEFAULT '',
          test_script TEXT NOT NULL DEFAULT '',
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL
        )
        "#,
        r#"
        CREATE TABLE collection_items (
          id TEXT PRIMARY KEY,
          collection_id TEXT NOT NULL,
          parent_id TEXT NULL,
          kind TEXT NOT NULL CHECK (kind IN ('folder', 'request')),
          name TEXT NOT NULL,
          sort_order INTEGER NOT NULL DEFAULT 0,
          method TEXT NULL,
          url TEXT NULL,
          query_params_json TEXT NOT NULL DEFAULT '[]',
          headers_json TEXT NOT NULL DEFAULT '[]',
          body_json TEXT NOT NULL DEFAULT '{}',
          auth_json TEXT NOT NULL DEFAULT '{}',
          prerequest_script TEXT NOT NULL DEFAULT '',
          test_script TEXT NOT NULL DEFAULT '',
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL,
          FOREIGN KEY (collection_id) REFERENCES collections(id) ON DELETE CASCADE,
          FOREIGN KEY (parent_id) REFERENCES collection_items(id) ON DELETE CASCADE
        )
        "#,
        r#"
        CREATE TABLE app_settings (
          key TEXT PRIMARY KEY,
          value_json TEXT NOT NULL,
          updated_at TEXT NOT NULL
        )
        "#,
    ] {
        sqlx::query(statement)
            .execute(&pool)
            .await
            .expect("create table");
    }

    pool
}

async fn starter_seeded_value(pool: &SqlitePool) -> Option<String> {
    sqlx::query("SELECT value_json FROM app_settings WHERE key = ?1")
        .bind(STARTER_COLLECTION_SEEDED_KEY)
        .fetch_optional(pool)
        .await
        .expect("read starter setting")
        .map(|row| row.get("value_json"))
}

#[tokio::test]
async fn starter_collection_is_created_once_for_empty_database() {
    let pool = setup_test_db().await;

    ensure_starter_collection(&pool)
        .await
        .expect("seed starter collection");
    ensure_starter_collection(&pool)
        .await
        .expect("seed remains idempotent");

    let collections = list_collections(&pool).await.expect("list collections");
    assert_eq!(collections.len(), 1);
    assert_eq!(collections[0].name, STARTER_COLLECTION_NAME);
    assert_eq!(collections[0].description, STARTER_COLLECTION_DESCRIPTION);
    assert_eq!(collections[0].request_count, 0);
    assert_eq!(starter_seeded_value(&pool).await.as_deref(), Some("true"));
}

#[tokio::test]
async fn starter_collection_does_not_replace_existing_collections() {
    let pool = setup_test_db().await;
    create_collection(
        &pool,
        &CreateCollectionInput {
            name: "Existing API".to_string(),
            description: "Already present".to_string(),
            pre_request_script: String::new(),
            test_script: String::new(),
        },
    )
    .await
    .expect("create existing collection");

    ensure_starter_collection(&pool)
        .await
        .expect("mark starter seed as handled");

    let collections = list_collections(&pool).await.expect("list collections");
    assert_eq!(collections.len(), 1);
    assert_eq!(collections[0].name, "Existing API");
    assert_eq!(starter_seeded_value(&pool).await.as_deref(), Some("true"));
}

#[tokio::test]
async fn starter_collection_does_not_reappear_after_user_deletes_all_collections() {
    let pool = setup_test_db().await;

    ensure_starter_collection(&pool)
        .await
        .expect("seed starter collection");
    let starter_id = list_collections(&pool)
        .await
        .expect("list starter")
        .remove(0)
        .id;

    delete_collection(&pool, &starter_id)
        .await
        .expect("delete starter");
    ensure_starter_collection(&pool)
        .await
        .expect("starter seed remains handled");

    let collections = list_collections(&pool).await.expect("list collections");
    assert!(collections.is_empty());
    assert_eq!(starter_seeded_value(&pool).await.as_deref(), Some("true"));
}
