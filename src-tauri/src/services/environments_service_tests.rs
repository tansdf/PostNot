use std::{collections::HashMap, sync::Arc};

use sqlx::SqlitePool;
use uuid::Uuid;

use crate::{
    domain::{
        environments::{EnvironmentInput, EnvironmentVariable},
        requests::{FileRow, KeyValueRow, RequestAuth, RequestBody, SendRequestPayload},
    },
    services::{
        environments_service,
        secret_store_service::{InMemorySecretStore, SecretStore},
    },
};

async fn setup_test_db() -> SqlitePool {
    let pool = SqlitePool::connect("sqlite::memory:")
        .await
        .expect("in-memory database");

    sqlx::query(
        r#"
        CREATE TABLE environments (
          id TEXT PRIMARY KEY,
          name TEXT NOT NULL,
          is_active INTEGER NOT NULL DEFAULT 0,
          variables_json TEXT NOT NULL DEFAULT '[]',
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL
        );
        CREATE INDEX idx_environments_is_active ON environments(is_active);
        "#,
    )
    .execute(&pool)
    .await
    .expect("create environments table");

    pool
}

fn environment_input() -> EnvironmentInput {
    EnvironmentInput {
        name: "Local".to_string(),
        variables: vec![
            EnvironmentVariable {
                id: "plain".to_string(),
                key: "base_url".to_string(),
                value: "https://api.example.com".to_string(),
                enabled: true,
                is_secret: false,
            },
            EnvironmentVariable {
                id: "secret".to_string(),
                key: "token".to_string(),
                value: "top-secret".to_string(),
                enabled: true,
                is_secret: true,
            },
        ],
    }
}

#[tokio::test]
async fn update_environment_keeps_secret_out_of_sqlite_and_hydrates_on_load() {
    let pool = setup_test_db().await;
    let secret_store: Arc<dyn SecretStore> = Arc::new(InMemorySecretStore::default());
    let created = environments_service::create_environment(&pool)
        .await
        .expect("create environment");

    let saved = environments_service::update_environment(
        &pool,
        secret_store.clone(),
        &created.id,
        &environment_input(),
    )
    .await
    .expect("update environment");

    let raw_json: String =
        sqlx::query_scalar("SELECT variables_json FROM environments WHERE id = ?1")
            .bind(&created.id)
            .fetch_one(&pool)
            .await
            .expect("load stored json");

    assert!(!raw_json.contains("top-secret"));
    assert!(saved
        .variables
        .iter()
        .any(|item| item.is_secret && item.value == "top-secret"));

    let loaded = environments_service::get_environment(&pool, secret_store, &created.id)
        .await
        .expect("get environment");

    assert!(loaded
        .variables
        .iter()
        .any(|item| item.is_secret && item.value == "top-secret"));
}

#[tokio::test]
async fn delete_environment_removes_secret_from_store() {
    let pool = setup_test_db().await;
    let store = Arc::new(InMemorySecretStore::default());
    let secret_store: Arc<dyn SecretStore> = store.clone();
    let created = environments_service::create_environment(&pool)
        .await
        .expect("create environment");

    environments_service::update_environment(
        &pool,
        secret_store.clone(),
        &created.id,
        &environment_input(),
    )
    .await
    .expect("update environment");

    environments_service::delete_environment(&pool, secret_store, &created.id)
        .await
        .expect("delete environment");

    assert_eq!(
        store
            .get_environment_variable_secret(&created.id, "secret")
            .expect("read secret"),
        None
    );
}

#[tokio::test]
async fn set_active_environment_keeps_existing_active_when_target_is_missing() {
    let pool = setup_test_db().await;
    let first = environments_service::create_environment(&pool)
        .await
        .expect("create first environment");
    let _second = environments_service::create_environment(&pool)
        .await
        .expect("create second environment");

    environments_service::set_active_environment(&pool, Some(&first.id))
        .await
        .expect("activate first environment");

    let error = environments_service::set_active_environment(&pool, Some("missing-environment"))
        .await
        .expect_err("missing environment should fail");
    assert_eq!(error.to_string(), "Environment not found.");

    let active_id: Option<String> =
        sqlx::query_scalar("SELECT id FROM environments WHERE is_active = 1")
            .fetch_optional(&pool)
            .await
            .expect("load active environment");

    assert_eq!(active_id, Some(first.id));
}

#[test]
fn resolve_string_supports_dynamic_variables() {
    let variables = HashMap::new();

    let (single_char, used_secret) =
        environments_service::resolve_string("{{$randomAlphaNumeric}}", &variables);
    assert!(!used_secret);
    assert_eq!(single_char.len(), 1);
    assert!(single_char.chars().all(|ch| ch.is_ascii_alphanumeric()));

    let (four_chars, _) =
        environments_service::resolve_string("{{$randomAlphaNumeric[4]}}", &variables);
    assert_eq!(four_chars.len(), 4);
    assert!(four_chars.chars().all(|ch| ch.is_ascii_alphanumeric()));

    let (guid, _) = environments_service::resolve_string("{{$guid}}", &variables);
    assert!(Uuid::parse_str(&guid).is_ok());

    let (random_uuid, _) = environments_service::resolve_string("{{$randomUUID}}", &variables);
    assert!(Uuid::parse_str(&random_uuid).is_ok());

    let (timestamp, _) = environments_service::resolve_string("{{$timestamp}}", &variables);
    assert!(timestamp.parse::<i64>().is_ok());

    let (iso_timestamp, _) = environments_service::resolve_string("{{$isoTimestamp}}", &variables);
    assert!(chrono::DateTime::parse_from_rfc3339(&iso_timestamp).is_ok());

    let (random_boolean, _) =
        environments_service::resolve_string("{{$randomBoolean}}", &variables);
    assert!(matches!(random_boolean.as_str(), "true" | "false"));

    let (random_int, _) = environments_service::resolve_string("{{$randomInt}}", &variables);
    let parsed_random_int = random_int.parse::<u16>().expect("random integer");
    assert!(parsed_random_int <= 1000);

    let (random_hex_color, _) =
        environments_service::resolve_string("{{$randomHexColor}}", &variables);
    assert_eq!(random_hex_color.len(), 7);
    assert!(random_hex_color.starts_with('#'));
    assert!(random_hex_color[1..]
        .chars()
        .all(|ch| ch.is_ascii_hexdigit()));

    let (random_ip, _) = environments_service::resolve_string("{{$randomIP}}", &variables);
    let ipv4_parts: Vec<&str> = random_ip.split('.').collect();
    assert_eq!(ipv4_parts.len(), 4);
    assert!(ipv4_parts.iter().all(|part| part.parse::<u8>().is_ok()));

    let (random_ipv6, _) = environments_service::resolve_string("{{$randomIPV6}}", &variables);
    let ipv6_parts: Vec<&str> = random_ipv6.split(':').collect();
    assert_eq!(ipv6_parts.len(), 8);
    assert!(ipv6_parts
        .iter()
        .all(|part| part.len() == 4 && part.chars().all(|ch| ch.is_ascii_hexdigit())));

    let (random_mac, _) = environments_service::resolve_string("{{$randomMACAddress}}", &variables);
    let mac_parts: Vec<&str> = random_mac.split(':').collect();
    assert_eq!(mac_parts.len(), 6);
    assert!(mac_parts
        .iter()
        .all(|part| part.len() == 2 && part.chars().all(|ch| ch.is_ascii_hexdigit())));

    let (random_protocol, _) =
        environments_service::resolve_string("{{$randomProtocol}}", &variables);
    assert!(matches!(random_protocol.as_str(), "http" | "https"));

    let (random_semver, _) = environments_service::resolve_string("{{$randomSemver}}", &variables);
    let semver_parts: Vec<&str> = random_semver.split('.').collect();
    assert_eq!(semver_parts.len(), 3);
    assert!(semver_parts.iter().all(|part| part.parse::<u16>().is_ok()));
}

#[test]
fn resolve_request_tracks_secret_usage_and_redacts_history_snapshot() {
    let payload = SendRequestPayload {
        name: "Call {{token}}".to_string(),
        method: "GET".to_string(),
        url: "{{base_url}}/items?auth={{token}}".to_string(),
        query_params: vec![KeyValueRow {
            id: "query-1".to_string(),
            key: "page".to_string(),
            value: "{{token}}".to_string(),
            enabled: true,
        }],
        headers: vec![KeyValueRow {
            id: "header-1".to_string(),
            key: "Authorization".to_string(),
            value: "Bearer {{token}}".to_string(),
            enabled: true,
        }],
        body: RequestBody {
            mode: "json".to_string(),
            raw: r#"{"token":"{{token}}","base":"{{base_url}}"}"#.to_string(),
            form: vec![KeyValueRow {
                id: "form-1".to_string(),
                key: "token".to_string(),
                value: "{{token}}".to_string(),
                enabled: true,
            }],
            files: vec![FileRow {
                id: "file-1".to_string(),
                name: "{{token}}".to_string(),
                path: "/tmp/demo.txt".to_string(),
                enabled: true,
            }],
        },
        auth: RequestAuth {
            auth_type: "bearer".to_string(),
            basic_username: String::new(),
            basic_password: String::new(),
            bearer_token: "{{token}}".to_string(),
            api_key_name: String::new(),
            api_key_value: String::new(),
            api_key_in: "header".to_string(),
            oauth2_access_token: String::new(),
            oauth2_token_url: String::new(),
            oauth2_client_id: String::new(),
            oauth2_client_secret: String::new(),
            oauth2_scope: String::new(),
        },
        pre_request_script: "pn.request.addHeader('X-Test', '1');".to_string(),
        test_script: "pn.test('status is ok', () => {});".to_string(),
    };

    let environment = crate::domain::environments::EnvironmentDetail {
        id: "env-1".to_string(),
        name: "Local".to_string(),
        is_active: true,
        updated_at: "2026-01-01T00:00:00Z".to_string(),
        variables: vec![
            EnvironmentVariable {
                id: "plain".to_string(),
                key: "base_url".to_string(),
                value: "https://api.example.com".to_string(),
                enabled: true,
                is_secret: false,
            },
            EnvironmentVariable {
                id: "secret".to_string(),
                key: "token".to_string(),
                value: "top-secret".to_string(),
                enabled: true,
                is_secret: true,
            },
        ],
    };

    let resolved = environments_service::resolve_request(&payload, Some(&environment));
    assert_eq!(
        resolved.payload.url,
        "https://api.example.com/items?auth=top-secret"
    );
    assert!(resolved.secret_usage.url);
    assert!(resolved.secret_usage.query_param_ids.contains("query-1"));
    assert!(resolved.secret_usage.header_ids.contains("header-1"));
    assert!(resolved.secret_usage.body_raw);
    assert!(resolved.secret_usage.body_form_ids.contains("form-1"));
    assert!(resolved.secret_usage.body_file_ids.contains("file-1"));
    assert!(resolved.secret_usage.auth_bearer_token);

    let history_snapshot = environments_service::redact_secret_history_payload(
        &payload,
        &resolved.payload,
        &resolved.secret_usage,
    );

    assert_eq!(history_snapshot.url, payload.url);
    assert_eq!(
        history_snapshot.query_params[0].value,
        payload.query_params[0].value
    );
    assert_eq!(history_snapshot.headers[0].value, payload.headers[0].value);
    assert_eq!(history_snapshot.body.raw, payload.body.raw);
    assert_eq!(
        history_snapshot.auth.bearer_token,
        payload.auth.bearer_token
    );
    assert_eq!(
        history_snapshot.pre_request_script,
        payload.pre_request_script
    );
    assert_eq!(history_snapshot.test_script, payload.test_script);
}

#[test]
fn resolve_request_keeps_dynamic_variables_non_secret_in_history_snapshot() {
    let payload = SendRequestPayload {
        name: "Dynamic request".to_string(),
        method: "GET".to_string(),
        url: "https://api.example.com/items/{{$randomAlphaNumeric[4]}}".to_string(),
        query_params: vec![KeyValueRow {
            id: "query-1".to_string(),
            key: "nonce".to_string(),
            value: "{{$randomInt}}".to_string(),
            enabled: true,
        }],
        headers: vec![KeyValueRow {
            id: "header-1".to_string(),
            key: "X-Request-Id".to_string(),
            value: "{{$guid}}".to_string(),
            enabled: true,
        }],
        body: RequestBody {
            mode: "json".to_string(),
            raw: r#"{"nonce":"{{$randomAlphaNumeric[8]}}"}"#.to_string(),
            form: vec![],
            files: vec![],
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
    };

    let resolved = environments_service::resolve_request(&payload, None);

    assert!(!resolved.secret_usage.url);
    assert!(!resolved.secret_usage.query_param_ids.contains("query-1"));
    assert!(!resolved.secret_usage.header_ids.contains("header-1"));
    assert!(!resolved.secret_usage.body_raw);
    assert_ne!(resolved.payload.url, payload.url);
    assert_ne!(
        resolved.payload.query_params[0].value,
        payload.query_params[0].value
    );
    assert_ne!(resolved.payload.headers[0].value, payload.headers[0].value);
    assert_ne!(resolved.payload.body.raw, payload.body.raw);

    let history_snapshot = environments_service::redact_secret_history_payload(
        &payload,
        &resolved.payload,
        &resolved.secret_usage,
    );

    assert_eq!(history_snapshot.url, resolved.payload.url);
    assert_eq!(
        history_snapshot.query_params[0].value,
        resolved.payload.query_params[0].value
    );
    assert_eq!(
        history_snapshot.headers[0].value,
        resolved.payload.headers[0].value
    );
    assert_eq!(history_snapshot.body.raw, resolved.payload.body.raw);
}
