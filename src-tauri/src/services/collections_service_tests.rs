use sqlx::{Row, SqlitePool};

use super::{
    create_collection, create_collection_folder, delete_collection,
    delete_saved_realtime_request_with_revision, ensure_starter_collection,
    get_saved_realtime_request, get_saved_request, list_collections, list_saved_realtime_requests,
    list_saved_requests, move_collection_item, save_realtime_request, save_request,
    search_collection_entities, update_saved_realtime_request_with_revision,
    update_saved_request_with_revision, STARTER_COLLECTION_DESCRIPTION, STARTER_COLLECTION_NAME,
    STARTER_COLLECTION_SEEDED_KEY,
};
use crate::domain::{
    collections::{CreateCollectionFolderInput, CreateCollectionInput, MoveCollectionItemInput},
    realtime::{
        RawWebSocketComposer, RealtimeRequestCommon, RealtimeRequestDraft, ReconnectPolicy,
        SocketIoComposer, SocketIoTransport,
    },
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
          request_type TEXT NOT NULL DEFAULT 'http'
            CHECK (request_type IN ('http', 'websocket', 'socketio')),
          realtime_request_json TEXT NULL,
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

#[tokio::test]
async fn optimistic_request_update_rejects_a_stale_revision() {
    let pool = setup_test_db().await;
    let collection = create_collection(&pool, &collection_input("Concurrency"))
        .await
        .expect("create collection");
    let created = save_request(
        &pool,
        &collection.id,
        None,
        &request("Original", "GET", "https://example.test"),
    )
    .await
    .expect("create request");

    let updated = update_saved_request_with_revision(
        &pool,
        &created.id,
        &request("First edit", "GET", "https://example.test"),
        Some(&created.updated_at),
    )
    .await
    .expect("first revision update");
    assert_ne!(updated.updated_at, created.updated_at);

    let stale = update_saved_request_with_revision(
        &pool,
        &created.id,
        &request("Stale edit", "GET", "https://example.test"),
        Some(&created.updated_at),
    )
    .await
    .expect_err("stale revision must conflict");
    assert!(matches!(stale, crate::error::AppError::Conflict { .. }));
}

#[tokio::test]
async fn realtime_migration_defaults_existing_items_to_http() {
    let pool = SqlitePool::connect("sqlite::memory:")
        .await
        .expect("in-memory database");
    sqlx::query(
        r#"
        CREATE TABLE collection_items (
          id TEXT PRIMARY KEY,
          kind TEXT NOT NULL CHECK (kind IN ('folder', 'request'))
        )
        "#,
    )
    .execute(&pool)
    .await
    .expect("create legacy collection items");
    sqlx::query("INSERT INTO collection_items (id, kind) VALUES ('request-1', 'request')")
        .execute(&pool)
        .await
        .expect("insert legacy request");

    sqlx::raw_sql(include_str!("../../migrations/0010_realtime_requests.sql"))
        .execute(&pool)
        .await
        .expect("apply realtime migration");

    let row = sqlx::query(
        "SELECT request_type, realtime_request_json FROM collection_items WHERE id = 'request-1'",
    )
    .fetch_one(&pool)
    .await
    .expect("read migrated request");
    assert_eq!(row.get::<String, _>("request_type"), "http");
    assert_eq!(row.get::<Option<String>, _>("realtime_request_json"), None);
}

#[tokio::test]
async fn realtime_crud_round_trips_and_enforces_optimistic_revision() {
    let pool = setup_test_db().await;
    let collection = create_collection(&pool, &collection_input("Realtime"))
        .await
        .expect("create collection");
    let folder = create_collection_folder(&pool, &collection.id, &folder_input("Streams", None))
        .await
        .expect("create folder");
    let websocket = websocket_request("Events", "wss://example.test/events");

    let created = save_realtime_request(&pool, &collection.id, Some(&folder.id), &websocket)
        .await
        .expect("save websocket request");
    assert_eq!(
        created.request_type,
        crate::domain::realtime::RequestType::Websocket
    );
    assert_eq!(created.url, "wss://example.test/events");

    let detail = get_saved_realtime_request(&pool, &created.id)
        .await
        .expect("get websocket request");
    assert_eq!(detail.parent_id.as_deref(), Some(folder.id.as_str()));
    assert_eq!(
        serde_json::to_value(detail.request).expect("serialize detail"),
        serde_json::to_value(&websocket).expect("serialize fixture")
    );

    let socketio = socketio_request("Presence", "https://example.test");
    let updated = update_saved_realtime_request_with_revision(
        &pool,
        &created.id,
        &socketio,
        Some(&created.updated_at),
    )
    .await
    .expect("update websocket as socket.io");
    assert_eq!(
        updated.request_type,
        crate::domain::realtime::RequestType::Socketio
    );
    assert_ne!(updated.updated_at, created.updated_at);

    let stale = update_saved_realtime_request_with_revision(
        &pool,
        &created.id,
        &websocket,
        Some(&created.updated_at),
    )
    .await
    .expect_err("stale realtime revision must conflict");
    assert!(matches!(stale, crate::error::AppError::Conflict { .. }));
}

#[tokio::test]
async fn protocol_specific_lists_and_getters_do_not_cross_protocols() {
    let pool = setup_test_db().await;
    let collection = create_collection(&pool, &collection_input("Mixed"))
        .await
        .expect("create collection");
    let http = save_request(
        &pool,
        &collection.id,
        None,
        &request("Health", "GET", "https://example.test/health"),
    )
    .await
    .expect("save http request");
    let realtime = save_realtime_request(
        &pool,
        &collection.id,
        None,
        &websocket_request("Events", "wss://example.test/events"),
    )
    .await
    .expect("save realtime request");

    let http_items = list_saved_requests(&pool, &collection.id)
        .await
        .expect("list http requests");
    assert_eq!(http_items.len(), 1);
    assert_eq!(http_items[0].id, http.id);

    let realtime_items = list_saved_realtime_requests(&pool, &collection.id)
        .await
        .expect("list realtime requests");
    assert_eq!(realtime_items.len(), 1);
    assert_eq!(realtime_items[0].id, realtime.id);

    assert!(get_saved_request(&pool, &realtime.id).await.is_err());
    assert!(get_saved_realtime_request(&pool, &http.id).await.is_err());

    let collections = list_collections(&pool).await.expect("list collections");
    assert_eq!(
        collections[0].request_count, 2,
        "collection counts include every request protocol"
    );
    let search = search_collection_entities(&pool, "Events", None)
        .await
        .expect("search realtime request");
    assert_eq!(search[0].request_type.as_deref(), Some("websocket"));
}

#[tokio::test]
async fn realtime_delete_requires_the_current_revision() {
    let pool = setup_test_db().await;
    let collection = create_collection(&pool, &collection_input("Realtime"))
        .await
        .expect("create collection");
    let created = save_realtime_request(
        &pool,
        &collection.id,
        None,
        &websocket_request("Events", "wss://example.test/events"),
    )
    .await
    .expect("save realtime request");

    let stale = delete_saved_realtime_request_with_revision(&pool, &created.id, "stale-revision")
        .await
        .expect_err("stale delete must conflict");
    assert!(matches!(stale, crate::error::AppError::Conflict { .. }));
    get_saved_realtime_request(&pool, &created.id)
        .await
        .expect("stale delete preserves request");

    delete_saved_realtime_request_with_revision(&pool, &created.id, &created.updated_at)
        .await
        .expect("delete current revision");
    assert!(get_saved_realtime_request(&pool, &created.id)
        .await
        .is_err());
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

fn realtime_common(name: &str, url: &str) -> RealtimeRequestCommon {
    RealtimeRequestCommon {
        name: name.to_string(),
        url: url.to_string(),
        query_params: Vec::new(),
        headers: Vec::new(),
        auth: RequestAuth::default(),
        reconnect: ReconnectPolicy::default(),
    }
}

fn websocket_request(name: &str, url: &str) -> RealtimeRequestDraft {
    RealtimeRequestDraft::Websocket {
        common: realtime_common(name, url),
        subprotocols: vec!["graphql-transport-ws".to_string()],
        composer: RawWebSocketComposer::default(),
    }
}

fn socketio_request(name: &str, url: &str) -> RealtimeRequestDraft {
    RealtimeRequestDraft::Socketio {
        common: realtime_common(name, url),
        path: "/socket.io/".to_string(),
        namespace: "/presence".to_string(),
        auth_payload: serde_json::json!({"tenant": "test"}),
        transport: SocketIoTransport::Auto,
        composer: SocketIoComposer {
            event: "join".to_string(),
            arguments: serde_json::json!(["room-1"]),
            ..SocketIoComposer::default()
        },
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
