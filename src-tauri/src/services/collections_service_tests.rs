use sqlx::{Row, SqlitePool};

use super::{
    create_collection, create_collection_folder, delete_collection, ensure_starter_collection,
    list_collections, move_collection_item, save_request, search_collection_entities,
    STARTER_COLLECTION_DESCRIPTION, STARTER_COLLECTION_NAME, STARTER_COLLECTION_SEEDED_KEY,
};
use crate::domain::{
    collections::{CreateCollectionFolderInput, CreateCollectionInput, MoveCollectionItemInput},
    requests::{RequestAuth, RequestBody, SendRequestPayload},
};

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

fn collection_input(name: &str) -> CreateCollectionInput {
    CreateCollectionInput {
        name: name.to_string(),
        description: String::new(),
        pre_request_script: String::new(),
        test_script: String::new(),
    }
}

fn folder_input(name: &str, parent_id: Option<String>) -> CreateCollectionFolderInput {
    CreateCollectionFolderInput {
        name: name.to_string(),
        parent_id,
        pre_request_script: String::new(),
        test_script: String::new(),
    }
}

fn request(name: &str, method: &str, url: &str) -> SendRequestPayload {
    SendRequestPayload {
        name: name.to_string(),
        method: method.to_string(),
        url: url.to_string(),
        query_params: Vec::new(),
        headers: Vec::new(),
        body: RequestBody {
            mode: "none".to_string(),
            raw: String::new(),
            form: Vec::new(),
            files: Vec::new(),
        },
        auth: RequestAuth {
            auth_type: "none".to_string(),
            basic_username: String::new(),
            basic_password: String::new(),
            bearer_token: String::new(),
            api_key_name: String::new(),
            api_key_value: String::new(),
            api_key_in: "header".to_string(),
            oauth2_access_token: String::new(),
            oauth2_token_url: String::new(),
            oauth2_client_id: String::new(),
            oauth2_client_secret: String::new(),
            oauth2_scope: String::new(),
        },
        pre_request_script: String::new(),
        test_script: String::new(),
    }
}

#[tokio::test]
async fn direct_search_preserves_fields_matching_ranking_and_limits() {
    let pool = setup_test_db().await;
    let collection = create_collection(&pool, &collection_input("Billing APIs"))
        .await
        .expect("create collection");
    let admin = create_collection_folder(&pool, &collection.id, &folder_input("Admin Tools", None))
        .await
        .expect("create admin folder");
    let reports = create_collection_folder(
        &pool,
        &collection.id,
        &folder_input("Reports Archive", Some(admin.id.clone())),
    )
    .await
    .expect("create reports folder");
    let exact = save_request(
        &pool,
        &collection.id,
        Some(&reports.id),
        &request(
            "User Report",
            "POST",
            "https://api.example.test/v2/users/report",
        ),
    )
    .await
    .expect("save exact request");
    let prefix = save_request(
        &pool,
        &collection.id,
        Some(&reports.id),
        &request(
            "User Report Detail",
            "GET",
            "https://api.example.test/detail",
        ),
    )
    .await
    .expect("save prefix request");
    for index in 0..4 {
        save_request(
            &pool,
            &collection.id,
            Some(&reports.id),
            &request(
                &format!("Report Variant {index}"),
                "PATCH",
                &format!("https://api.example.test/reports/{index}"),
            ),
        )
        .await
        .expect("save limit fixture");
    }

    let collection_results = search_collection_entities(&pool, "bIlLiNg ApIs", None)
        .await
        .expect("search collection name");
    assert_eq!(collection_results[0].id, collection.id);
    assert_eq!(collection_results[0].request_count, Some(6));

    let ancestor_results = search_collection_entities(&pool, "adm rep", None)
        .await
        .expect("search ancestor prefixes");
    let exact_result = ancestor_results
        .iter()
        .find(|result| result.id == exact.id)
        .expect("nested request from ancestor prefixes");
    assert_eq!(exact_result.collection_name, "Billing APIs");
    assert_eq!(
        exact_result.ancestor_ids,
        vec![admin.id.clone(), reports.id.clone()]
    );
    assert_eq!(
        exact_result.ancestor_names,
        vec!["Admin Tools", "Reports Archive"]
    );

    let method_results = search_collection_entities(&pool, "post", None)
        .await
        .expect("search method");
    assert_eq!(
        method_results
            .iter()
            .map(|result| &result.id)
            .collect::<Vec<_>>(),
        vec![&exact.id]
    );

    let url_results = search_collection_entities(&pool, "v2 USERS", None)
        .await
        .expect("search URL tokens");
    assert_eq!(
        url_results
            .iter()
            .map(|result| &result.id)
            .collect::<Vec<_>>(),
        vec![&exact.id]
    );

    let ranked = search_collection_entities(&pool, "User Report", None)
        .await
        .expect("search ranked names");
    assert_eq!(
        ranked[0].id, exact.id,
        "exact name ranks before prefix name"
    );
    assert_eq!(ranked[1].id, prefix.id);

    let limited = search_collection_entities(&pool, "report", Some(2))
        .await
        .expect("search with limit");
    assert_eq!(limited.len(), 2);
}

#[tokio::test]
async fn search_reflects_committed_moves_without_rebuild() {
    let pool = setup_test_db().await;
    let collection = create_collection(&pool, &collection_input("Workspace"))
        .await
        .expect("create collection");
    let legacy = create_collection_folder(&pool, &collection.id, &folder_input("Legacy", None))
        .await
        .expect("create old folder");
    let current = create_collection_folder(&pool, &collection.id, &folder_input("Current", None))
        .await
        .expect("create new folder");
    let saved = save_request(
        &pool,
        &collection.id,
        Some(&legacy.id),
        &request("Health probe", "GET", "https://api.example.test/health"),
    )
    .await
    .expect("save request");

    sqlx::query("UPDATE collection_items SET name = 'Archived', parent_id = ?2 WHERE id = ?1")
        .bind(&legacy.id)
        .bind(&current.id)
        .execute(&pool)
        .await
        .expect("commit folder rename and move without rebuild");
    sqlx::query("UPDATE collection_items SET name = 'Status check', parent_id = ?2 WHERE id = ?1")
        .bind(&saved.id)
        .bind(&current.id)
        .execute(&pool)
        .await
        .expect("commit request rename and move without rebuild");

    assert!(search_collection_entities(&pool, "Legacy", None)
        .await
        .expect("search old folder")
        .is_empty());
    assert!(search_collection_entities(&pool, "Health probe", None)
        .await
        .expect("search old request")
        .is_empty());
    let current_results = search_collection_entities(&pool, "Current", None)
        .await
        .expect("search new folder");
    let moved = current_results
        .iter()
        .find(|result| result.id == saved.id)
        .expect("moved request appears under new path");
    assert_eq!(moved.ancestor_names, vec!["Current"]);
    let moved_folder = current_results
        .iter()
        .find(|result| result.id == legacy.id)
        .expect("renamed folder appears under new path");
    assert_eq!(moved_folder.name, "Archived");
    assert_eq!(moved_folder.ancestor_names, vec!["Current"]);
}

#[tokio::test]
async fn moving_among_500_siblings_resequences_contiguously() {
    let pool = setup_test_db().await;
    let collection = create_collection(&pool, &collection_input("Large"))
        .await
        .expect("create collection");
    for index in 0..500 {
        sqlx::query(
            "INSERT INTO collection_items (id, collection_id, parent_id, kind, name, sort_order, created_at, updated_at) VALUES (?1, ?2, NULL, 'folder', ?3, ?4, '2026-01-01', '2026-01-01')",
        )
        .bind(format!("folder-{index:03}"))
        .bind(&collection.id)
        .bind(format!("Folder {index:03}"))
        .bind(index)
        .execute(&pool)
        .await
        .expect("insert sibling");
    }

    move_collection_item(
        &pool,
        "folder-499",
        &MoveCollectionItemInput {
            target_collection_id: collection.id.clone(),
            target_parent_id: None,
            target_index: Some(0),
        },
    )
    .await
    .expect("move last sibling first");

    let rows = sqlx::query(
        "SELECT id, sort_order FROM collection_items WHERE collection_id = ?1 AND parent_id IS NULL ORDER BY sort_order",
    )
    .bind(&collection.id)
    .fetch_all(&pool)
    .await
    .expect("list resequenced siblings");
    assert_eq!(rows.len(), 500);
    assert_eq!(rows[0].get::<String, _>("id"), "folder-499");
    for (expected, row) in rows.iter().enumerate() {
        assert_eq!(row.get::<i64, _>("sort_order"), expected as i64);
    }
}

#[tokio::test]
async fn failed_move_preserves_source_collection_parent_and_order() {
    let pool = setup_test_db().await;
    let collection = create_collection(&pool, &collection_input("Atomic"))
        .await
        .expect("create collection");
    let parent = create_collection_folder(&pool, &collection.id, &folder_input("Parent", None))
        .await
        .expect("create parent");
    let child = create_collection_folder(
        &pool,
        &collection.id,
        &folder_input("Child", Some(parent.id.clone())),
    )
    .await
    .expect("create child");
    let before: (String, Option<String>, i64) = sqlx::query_as(
        "SELECT collection_id, parent_id, sort_order FROM collection_items WHERE id = ?1",
    )
    .bind(&parent.id)
    .fetch_one(&pool)
    .await
    .expect("read source state");

    let error = move_collection_item(
        &pool,
        &parent.id,
        &MoveCollectionItemInput {
            target_collection_id: collection.id.clone(),
            target_parent_id: Some(child.id),
            target_index: Some(0),
        },
    )
    .await
    .expect_err("moving a folder into its descendant must fail");
    assert!(error.to_string().contains("subfolders"));

    let after: (String, Option<String>, i64) = sqlx::query_as(
        "SELECT collection_id, parent_id, sort_order FROM collection_items WHERE id = ?1",
    )
    .bind(&parent.id)
    .fetch_one(&pool)
    .await
    .expect("read state after failed move");
    assert_eq!(after, before);
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
