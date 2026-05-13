use sqlx::SqlitePool;

use crate::{
    domain::{
        playbooks::{
            AddPlaybookStepInput, CreatePlaybookRunInput, FinishPlaybookRunInput,
            PlaybookInput, RecordPlaybookRunStepInput, ReorderPlaybookStepsInput,
            UpdatePlaybookStepInput,
        },
        requests::{RequestAuth, RequestBody, SendRequestPayload},
    },
    services::{collections_service, playbooks_service},
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
        CREATE TABLE playbooks (
          id TEXT PRIMARY KEY,
          name TEXT NOT NULL,
          description TEXT NOT NULL DEFAULT '',
          default_delay_ms INTEGER NOT NULL DEFAULT 0,
          stop_on_failure INTEGER NOT NULL DEFAULT 1,
          fail_on_http_error INTEGER NOT NULL DEFAULT 1,
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL
        )
        "#,
        r#"
        CREATE TABLE playbook_steps (
          id TEXT PRIMARY KEY,
          playbook_id TEXT NOT NULL,
          saved_request_id TEXT NULL,
          saved_request_name TEXT NOT NULL DEFAULT '',
          name_override TEXT NOT NULL DEFAULT '',
          notes TEXT NOT NULL DEFAULT '',
          enabled INTEGER NOT NULL DEFAULT 1,
          sort_order INTEGER NOT NULL DEFAULT 0,
          delay_after_ms INTEGER NULL,
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL,
          FOREIGN KEY (playbook_id) REFERENCES playbooks(id) ON DELETE CASCADE,
          FOREIGN KEY (saved_request_id) REFERENCES collection_items(id) ON DELETE SET NULL
        )
        "#,
        r#"
        CREATE TABLE playbook_runs (
          id TEXT PRIMARY KEY,
          playbook_id TEXT NOT NULL,
          status TEXT NOT NULL,
          started_at TEXT NOT NULL,
          finished_at TEXT NULL,
          total_steps INTEGER NOT NULL DEFAULT 0,
          passed_steps INTEGER NOT NULL DEFAULT 0,
          failed_steps INTEGER NOT NULL DEFAULT 0,
          skipped_steps INTEGER NOT NULL DEFAULT 0,
          total_duration_ms INTEGER NOT NULL DEFAULT 0,
          stopped_reason TEXT NOT NULL DEFAULT '',
          FOREIGN KEY (playbook_id) REFERENCES playbooks(id) ON DELETE CASCADE
        )
        "#,
        r#"
        CREATE TABLE playbook_run_steps (
          id TEXT PRIMARY KEY,
          run_id TEXT NOT NULL,
          step_id TEXT NULL,
          saved_request_id TEXT NULL,
          saved_request_name TEXT NOT NULL DEFAULT '',
          method TEXT NOT NULL DEFAULT '',
          url TEXT NOT NULL DEFAULT '',
          status TEXT NOT NULL,
          status_code INTEGER NULL,
          duration_ms INTEGER NOT NULL DEFAULT 0,
          response_size_bytes INTEGER NOT NULL DEFAULT 0,
          test_passed_count INTEGER NOT NULL DEFAULT 0,
          test_failed_count INTEGER NOT NULL DEFAULT 0,
          test_error_text TEXT NOT NULL DEFAULT '',
          error_text TEXT NOT NULL DEFAULT '',
          executed_at TEXT NOT NULL,
          FOREIGN KEY (run_id) REFERENCES playbook_runs(id) ON DELETE CASCADE,
          FOREIGN KEY (step_id) REFERENCES playbook_steps(id) ON DELETE SET NULL,
          FOREIGN KEY (saved_request_id) REFERENCES collection_items(id) ON DELETE SET NULL
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

fn playbook_input(name: &str) -> PlaybookInput {
    PlaybookInput {
        name: name.to_string(),
        description: "Regression flow".to_string(),
        default_delay_ms: 25,
        stop_on_failure: true,
        fail_on_http_error: true,
    }
}

fn request(name: &str, path: &str) -> SendRequestPayload {
    SendRequestPayload {
        name: name.to_string(),
        method: "GET".to_string(),
        url: format!("https://api.example.com{path}"),
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

async fn create_collection_with_requests(pool: &SqlitePool) -> (String, String, String) {
    let collection = collections_service::create_collection(
        pool,
        &crate::domain::collections::CreateCollectionInput {
            name: "API".to_string(),
            description: String::new(),
            pre_request_script: String::new(),
            test_script: String::new(),
        },
    )
    .await
    .expect("create collection");
    let first = collections_service::save_request(pool, &collection.id, None, &request("First", "/one"))
        .await
        .expect("save first");
    let second =
        collections_service::save_request(pool, &collection.id, None, &request("Second", "/two"))
            .await
            .expect("save second");

    (collection.id, first.id, second.id)
}

#[tokio::test]
async fn playbook_crud_and_step_reorder_work() {
    let pool = setup_test_db().await;
    let (_, first_request_id, second_request_id) = create_collection_with_requests(&pool).await;
    let playbook = playbooks_service::create_playbook(&pool, &playbook_input("Deploy"))
        .await
        .expect("create playbook");

    let first = playbooks_service::add_playbook_step(
        &pool,
        &playbook.id,
        &AddPlaybookStepInput {
            saved_request_id: first_request_id,
            name_override: String::new(),
            notes: String::new(),
            enabled: true,
            delay_after_ms: None,
        },
    )
    .await
    .expect("add first step");
    let second = playbooks_service::add_playbook_step(
        &pool,
        &playbook.id,
        &AddPlaybookStepInput {
            saved_request_id: second_request_id,
            name_override: String::new(),
            notes: "wait for cache".to_string(),
            enabled: false,
            delay_after_ms: Some(100),
        },
    )
    .await
    .expect("add second step");

    let updated = playbooks_service::update_playbook_step(
        &pool,
        &second.id,
        &UpdatePlaybookStepInput {
            name_override: "Second call".to_string(),
            notes: "updated".to_string(),
            enabled: true,
            delay_after_ms: Some(250),
        },
    )
    .await
    .expect("update second step");
    assert_eq!(updated.name_override, "Second call");
    assert_eq!(updated.delay_after_ms, Some(250));

    let reordered = playbooks_service::reorder_playbook_steps(
        &pool,
        &playbook.id,
        &ReorderPlaybookStepsInput {
            step_ids: vec![second.id.clone(), first.id.clone()],
        },
    )
    .await
    .expect("reorder");
    assert_eq!(reordered[0].id, second.id);
    assert_eq!(reordered[1].id, first.id);
}

#[tokio::test]
async fn deleted_saved_request_becomes_missing_step() {
    let pool = setup_test_db().await;
    let (_, request_id, _) = create_collection_with_requests(&pool).await;
    let playbook = playbooks_service::create_playbook(&pool, &playbook_input("Smoke"))
        .await
        .expect("create playbook");
    let step = playbooks_service::add_playbook_step(
        &pool,
        &playbook.id,
        &AddPlaybookStepInput {
            saved_request_id: request_id.clone(),
            name_override: String::new(),
            notes: String::new(),
            enabled: true,
            delay_after_ms: None,
        },
    )
    .await
    .expect("add step");

    collections_service::delete_saved_request(&pool, &request_id)
        .await
        .expect("delete saved request");

    let detail = playbooks_service::get_playbook(&pool, &playbook.id)
        .await
        .expect("load playbook");
    assert_eq!(detail.steps[0].id, step.id);
    assert!(detail.steps[0].missing_saved_request);
}

#[tokio::test]
async fn run_step_persistence_updates_run_counts() {
    let pool = setup_test_db().await;
    let (_, request_id, _) = create_collection_with_requests(&pool).await;
    let playbook = playbooks_service::create_playbook(&pool, &playbook_input("Smoke"))
        .await
        .expect("create playbook");
    let step = playbooks_service::add_playbook_step(
        &pool,
        &playbook.id,
        &AddPlaybookStepInput {
            saved_request_id: request_id.clone(),
            name_override: String::new(),
            notes: String::new(),
            enabled: true,
            delay_after_ms: None,
        },
    )
    .await
    .expect("add step");
    let run = playbooks_service::create_playbook_run(
        &pool,
        &CreatePlaybookRunInput {
            playbook_id: playbook.id.clone(),
            total_steps: 2,
        },
    )
    .await
    .expect("create run");

    playbooks_service::record_playbook_run_step(
        &pool,
        &run.id,
        &RecordPlaybookRunStepInput {
            step_id: Some(step.id),
            saved_request_id: Some(request_id),
            saved_request_name: "First".to_string(),
            method: "GET".to_string(),
            url: "https://api.example.com/one".to_string(),
            status: "failed".to_string(),
            status_code: Some(500),
            duration_ms: 42,
            response_size_bytes: 128,
            test_passed_count: 1,
            test_failed_count: 1,
            test_error_text: "boom".to_string(),
            error_text: "HTTP 500".to_string(),
        },
    )
    .await
    .expect("record step");
    let finished = playbooks_service::finish_playbook_run(
        &pool,
        &run.id,
        &FinishPlaybookRunInput {
            status: "failed".to_string(),
            stopped_reason: "HTTP 500".to_string(),
            total_duration_ms: 42,
        },
    )
    .await
    .expect("finish run");

    assert_eq!(finished.failed_steps, 1);
    assert_eq!(finished.passed_steps, 0);
    assert_eq!(finished.total_duration_ms, 42);
}

#[tokio::test]
async fn deleting_playbook_cascades_steps_and_run_logs() {
    let pool = setup_test_db().await;
    let (_, request_id, _) = create_collection_with_requests(&pool).await;
    let playbook = playbooks_service::create_playbook(&pool, &playbook_input("Cleanup"))
        .await
        .expect("create playbook");
    playbooks_service::add_playbook_step(
        &pool,
        &playbook.id,
        &AddPlaybookStepInput {
            saved_request_id: request_id,
            name_override: String::new(),
            notes: String::new(),
            enabled: true,
            delay_after_ms: None,
        },
    )
    .await
    .expect("add step");
    let run = playbooks_service::create_playbook_run(
        &pool,
        &CreatePlaybookRunInput {
            playbook_id: playbook.id.clone(),
            total_steps: 1,
        },
    )
    .await
    .expect("create run");

    playbooks_service::delete_playbook(&pool, &playbook.id)
        .await
        .expect("delete playbook");

    let step_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM playbook_steps")
        .fetch_one(&pool)
        .await
        .expect("count steps");
    let run_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM playbook_runs WHERE id = ?1")
        .bind(&run.id)
        .fetch_one(&pool)
        .await
        .expect("count runs");
    assert_eq!(step_count, 0);
    assert_eq!(run_count, 0);
}
