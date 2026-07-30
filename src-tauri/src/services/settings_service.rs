use serde::de::DeserializeOwned;
use sqlx::{Executor, Row, Sqlite, SqlitePool};

use crate::{
    domain::{
        collections::CollectionSidebarState, settings::AppSettings,
        workspace::RequestWorkspaceState,
    },
    error::{AppError, AppResult},
};

const THEME_KEY: &str = "theme";
const UI_SCALE_KEY: &str = "ui_scale";
const REQUEST_TIMEOUT_MS_KEY: &str = "request_timeout_ms";
const FOLLOW_REDIRECTS_KEY: &str = "follow_redirects";
const VALIDATE_TLS_KEY: &str = "validate_tls";
const HISTORY_LIMIT_KEY: &str = "history_limit";
const IS_HISTORY_COLLAPSED_KEY: &str = "is_history_collapsed";
const ENVIRONMENT_AUTOSAVE_KEY: &str = "environment_autosave";
const NOTIFICATION_TIMEOUT_MS_KEY: &str = "notification_timeout_ms";
const REALTIME_CONNECT_TIMEOUT_MS_KEY: &str = "realtime_connect_timeout_ms";
const REALTIME_MAX_CONCURRENT_SESSIONS_KEY: &str = "realtime_max_concurrent_sessions";
const REALTIME_MAX_MESSAGE_BYTES_KEY: &str = "realtime_max_message_bytes";
const REALTIME_TRANSCRIPT_MAX_ENTRIES_KEY: &str = "realtime_transcript_max_entries";
const REALTIME_TRANSCRIPT_MAX_BYTES_KEY: &str = "realtime_transcript_max_bytes";
const LAST_UPDATE_CHECKED_AT_KEY: &str = "last_update_checked_at";
const COLLECTION_SIDEBAR_STATE_KEY: &str = "collection_sidebar_state";
const REQUEST_WORKSPACE_STATE_KEY: &str = "request_workspace_state";
const REALTIME_WORKSPACE_STATE_KEY: &str = "realtime_workspace_state";
const DEFAULT_UI_SCALE: f64 = 1.0;
const MIN_UI_SCALE: f64 = 0.6;
const MAX_UI_SCALE: f64 = 1.5;
const MIN_NOTIFICATION_TIMEOUT_MS: u64 = 1_000;
const MAX_NOTIFICATION_TIMEOUT_MS: u64 = 60_000;
const MIN_REALTIME_CONNECT_TIMEOUT_MS: u64 = 1_000;
const MAX_REALTIME_CONNECT_TIMEOUT_MS: u64 = 120_000;
const MIN_REALTIME_MAX_CONCURRENT_SESSIONS: u32 = 1;
const MAX_REALTIME_MAX_CONCURRENT_SESSIONS: u32 = 100;
const MIN_REALTIME_MAX_MESSAGE_BYTES: u64 = 64 * 1024;
const MAX_REALTIME_MAX_MESSAGE_BYTES: u64 = 256 * 1024 * 1024;
const MIN_REALTIME_TRANSCRIPT_MAX_ENTRIES: u32 = 1;
const MAX_REALTIME_TRANSCRIPT_MAX_ENTRIES: u32 = 10_000;
const MIN_REALTIME_TRANSCRIPT_MAX_BYTES: u64 = 64 * 1024;
const MAX_REALTIME_TRANSCRIPT_MAX_BYTES: u64 = 512 * 1024 * 1024;

pub fn default_settings() -> AppSettings {
    AppSettings {
        theme: "system".to_string(),
        ui_scale: DEFAULT_UI_SCALE,
        request_timeout_ms: 30_000,
        follow_redirects: true,
        validate_tls: true,
        history_limit: 200,
        is_history_collapsed: false,
        environment_autosave: true,
        notification_timeout_ms: 5_000,
        realtime_connect_timeout_ms: 30_000,
        realtime_max_concurrent_sessions: 20,
        realtime_max_message_bytes: 64 * 1024 * 1024,
        realtime_transcript_max_entries: 2_000,
        realtime_transcript_max_bytes: 64 * 1024 * 1024,
        last_update_checked_at: None,
    }
}

pub async fn ensure_defaults(pool: &SqlitePool) -> AppResult<()> {
    let settings = default_settings();

    for (key, value_json) in serialize_settings(&settings)? {
        insert_default(pool, key, &value_json).await?;
    }

    Ok(())
}

pub async fn get_settings(pool: &SqlitePool) -> AppResult<AppSettings> {
    let rows = sqlx::query("SELECT key, value_json FROM app_settings")
        .fetch_all(pool)
        .await?;

    let mut settings = default_settings();

    for row in rows {
        let key: String = row.get("key");
        let value_json: String = row.get("value_json");
        macro_rules! parse_setting {
            () => {
                deserialize_setting(&key, &value_json)?
            };
        }

        match key.as_str() {
            THEME_KEY => settings.theme = parse_setting!(),
            UI_SCALE_KEY => settings.ui_scale = normalize_ui_scale(parse_setting!()),
            REQUEST_TIMEOUT_MS_KEY => settings.request_timeout_ms = parse_setting!(),
            FOLLOW_REDIRECTS_KEY => settings.follow_redirects = parse_setting!(),
            VALIDATE_TLS_KEY => settings.validate_tls = parse_setting!(),
            HISTORY_LIMIT_KEY => settings.history_limit = parse_setting!(),
            IS_HISTORY_COLLAPSED_KEY => settings.is_history_collapsed = parse_setting!(),
            ENVIRONMENT_AUTOSAVE_KEY => settings.environment_autosave = parse_setting!(),
            NOTIFICATION_TIMEOUT_MS_KEY => {
                settings.notification_timeout_ms =
                    normalize_notification_timeout_ms(parse_setting!())
            }
            REALTIME_CONNECT_TIMEOUT_MS_KEY => {
                settings.realtime_connect_timeout_ms =
                    normalize_realtime_connect_timeout_ms(parse_setting!())
            }
            REALTIME_MAX_CONCURRENT_SESSIONS_KEY => {
                settings.realtime_max_concurrent_sessions =
                    normalize_realtime_max_concurrent_sessions(parse_setting!())
            }
            REALTIME_MAX_MESSAGE_BYTES_KEY => {
                settings.realtime_max_message_bytes =
                    normalize_realtime_max_message_bytes(parse_setting!())
            }
            REALTIME_TRANSCRIPT_MAX_ENTRIES_KEY => {
                settings.realtime_transcript_max_entries =
                    normalize_realtime_transcript_max_entries(parse_setting!())
            }
            REALTIME_TRANSCRIPT_MAX_BYTES_KEY => {
                settings.realtime_transcript_max_bytes =
                    normalize_realtime_transcript_max_bytes(parse_setting!())
            }
            LAST_UPDATE_CHECKED_AT_KEY => settings.last_update_checked_at = parse_setting!(),
            _ => {}
        }
    }

    Ok(settings)
}

pub async fn save_settings(pool: &SqlitePool, settings: &AppSettings) -> AppResult<()> {
    let mut settings = normalize_settings(settings);
    settings.last_update_checked_at = get_setting(pool, LAST_UPDATE_CHECKED_AT_KEY)
        .await?
        .unwrap_or_default();
    let serialized_settings = serialize_settings(&settings)?;
    let updated_at = now_iso();
    let mut transaction = pool.begin().await?;

    for (key, value_json) in serialized_settings {
        upsert_setting(&mut *transaction, key, &value_json, &updated_at).await?;
    }

    transaction.commit().await?;

    Ok(())
}

pub async fn save_last_update_checked_at(pool: &SqlitePool, checked_at: &str) -> AppResult<()> {
    upsert_setting(
        pool,
        LAST_UPDATE_CHECKED_AT_KEY,
        &serde_json::to_string(&Some(checked_at.to_string()))?,
        &now_iso(),
    )
    .await
}

pub async fn get_collection_sidebar_state(pool: &SqlitePool) -> AppResult<CollectionSidebarState> {
    Ok(get_setting(pool, COLLECTION_SIDEBAR_STATE_KEY)
        .await?
        .unwrap_or(CollectionSidebarState {
            expanded_collection_ids: Vec::new(),
            expanded_folder_ids: Vec::new(),
        }))
}

pub async fn save_collection_sidebar_state(
    pool: &SqlitePool,
    state: &CollectionSidebarState,
) -> AppResult<()> {
    upsert_setting(
        pool,
        COLLECTION_SIDEBAR_STATE_KEY,
        &serde_json::to_string(state)?,
        &now_iso(),
    )
    .await
}

pub async fn get_request_workspace_state(
    pool: &SqlitePool,
) -> AppResult<Option<RequestWorkspaceState>> {
    get_setting(pool, REQUEST_WORKSPACE_STATE_KEY).await
}

pub async fn save_request_workspace_state(
    pool: &SqlitePool,
    state: &RequestWorkspaceState,
) -> AppResult<()> {
    upsert_setting(
        pool,
        REQUEST_WORKSPACE_STATE_KEY,
        &serde_json::to_string(state)?,
        &now_iso(),
    )
    .await
}

pub async fn get_realtime_workspace_state(
    pool: &SqlitePool,
) -> AppResult<Option<serde_json::Value>> {
    get_setting(pool, REALTIME_WORKSPACE_STATE_KEY).await
}

pub async fn save_realtime_workspace_state(
    pool: &SqlitePool,
    state: &serde_json::Value,
) -> AppResult<()> {
    let normalized = normalize_realtime_workspace_state(state.clone())?;
    upsert_setting(
        pool,
        REALTIME_WORKSPACE_STATE_KEY,
        &serde_json::to_string(&normalized)?,
        &now_iso(),
    )
    .await
}

fn normalize_realtime_workspace_state(
    mut state: serde_json::Value,
) -> AppResult<serde_json::Value> {
    let object = state.as_object_mut().ok_or_else(|| {
        AppError::Message("Realtime workspace state must be an object.".to_string())
    })?;
    let tabs = object
        .get_mut("tabs")
        .and_then(serde_json::Value::as_array_mut)
        .ok_or_else(|| {
            AppError::Message("Realtime workspace state must contain a tabs array.".to_string())
        })?;
    for tab in tabs {
        let tab = tab.as_object_mut().ok_or_else(|| {
            AppError::Message("Realtime workspace tabs must be objects.".to_string())
        })?;
        tab.insert(
            "status".to_string(),
            serde_json::Value::String("disconnected".to_string()),
        );
        tab.insert("generation".to_string(), serde_json::json!(0));
        tab.insert("lastSequence".to_string(), serde_json::json!(0));
        tab.insert(
            "statusMessage".to_string(),
            serde_json::Value::String("Disconnected".to_string()),
        );
        tab.insert(
            "reconnectRequired".to_string(),
            serde_json::Value::Bool(false),
        );
        tab.insert(
            "transcript".to_string(),
            serde_json::Value::Array(Vec::new()),
        );
        tab.insert("transcriptSizeBytes".to_string(), serde_json::json!(0));
        tab.insert(
            "errorText".to_string(),
            serde_json::Value::String(String::new()),
        );
    }
    Ok(state)
}

pub async fn history_limit(pool: &SqlitePool) -> AppResult<u32> {
    Ok(get_setting(pool, HISTORY_LIMIT_KEY)
        .await?
        .unwrap_or(default_settings().history_limit))
}

fn serialize_settings(settings: &AppSettings) -> AppResult<Vec<(&'static str, String)>> {
    macro_rules! setting {
        ($key:expr, $value:expr) => {
            ($key, serde_json::to_string(&$value)?)
        };
    }

    Ok(vec![
        setting!(THEME_KEY, settings.theme),
        setting!(UI_SCALE_KEY, settings.ui_scale),
        setting!(REQUEST_TIMEOUT_MS_KEY, settings.request_timeout_ms),
        setting!(FOLLOW_REDIRECTS_KEY, settings.follow_redirects),
        setting!(VALIDATE_TLS_KEY, settings.validate_tls),
        setting!(HISTORY_LIMIT_KEY, settings.history_limit),
        setting!(IS_HISTORY_COLLAPSED_KEY, settings.is_history_collapsed),
        setting!(ENVIRONMENT_AUTOSAVE_KEY, settings.environment_autosave),
        setting!(
            NOTIFICATION_TIMEOUT_MS_KEY,
            settings.notification_timeout_ms
        ),
        setting!(
            REALTIME_CONNECT_TIMEOUT_MS_KEY,
            settings.realtime_connect_timeout_ms
        ),
        setting!(
            REALTIME_MAX_CONCURRENT_SESSIONS_KEY,
            settings.realtime_max_concurrent_sessions
        ),
        setting!(
            REALTIME_MAX_MESSAGE_BYTES_KEY,
            settings.realtime_max_message_bytes
        ),
        setting!(
            REALTIME_TRANSCRIPT_MAX_ENTRIES_KEY,
            settings.realtime_transcript_max_entries
        ),
        setting!(
            REALTIME_TRANSCRIPT_MAX_BYTES_KEY,
            settings.realtime_transcript_max_bytes
        ),
        setting!(LAST_UPDATE_CHECKED_AT_KEY, settings.last_update_checked_at),
    ])
}

fn normalize_settings(settings: &AppSettings) -> AppSettings {
    let mut normalized = settings.clone();
    normalized.ui_scale = normalize_ui_scale(normalized.ui_scale);
    normalized.notification_timeout_ms =
        normalize_notification_timeout_ms(normalized.notification_timeout_ms);
    normalized.realtime_connect_timeout_ms =
        normalize_realtime_connect_timeout_ms(normalized.realtime_connect_timeout_ms);
    normalized.realtime_max_concurrent_sessions =
        normalize_realtime_max_concurrent_sessions(normalized.realtime_max_concurrent_sessions);
    normalized.realtime_max_message_bytes =
        normalize_realtime_max_message_bytes(normalized.realtime_max_message_bytes);
    normalized.realtime_transcript_max_entries =
        normalize_realtime_transcript_max_entries(normalized.realtime_transcript_max_entries);
    normalized.realtime_transcript_max_bytes =
        normalize_realtime_transcript_max_bytes(normalized.realtime_transcript_max_bytes);
    normalized
}

fn normalize_ui_scale(value: f64) -> f64 {
    value.clamp(MIN_UI_SCALE, MAX_UI_SCALE)
}

fn normalize_notification_timeout_ms(value: u64) -> u64 {
    value.clamp(MIN_NOTIFICATION_TIMEOUT_MS, MAX_NOTIFICATION_TIMEOUT_MS)
}

fn normalize_realtime_connect_timeout_ms(value: u64) -> u64 {
    value.clamp(
        MIN_REALTIME_CONNECT_TIMEOUT_MS,
        MAX_REALTIME_CONNECT_TIMEOUT_MS,
    )
}

fn normalize_realtime_max_concurrent_sessions(value: u32) -> u32 {
    value.clamp(
        MIN_REALTIME_MAX_CONCURRENT_SESSIONS,
        MAX_REALTIME_MAX_CONCURRENT_SESSIONS,
    )
}

fn normalize_realtime_max_message_bytes(value: u64) -> u64 {
    value.clamp(
        MIN_REALTIME_MAX_MESSAGE_BYTES,
        MAX_REALTIME_MAX_MESSAGE_BYTES,
    )
}

fn normalize_realtime_transcript_max_entries(value: u32) -> u32 {
    value.clamp(
        MIN_REALTIME_TRANSCRIPT_MAX_ENTRIES,
        MAX_REALTIME_TRANSCRIPT_MAX_ENTRIES,
    )
}

fn normalize_realtime_transcript_max_bytes(value: u64) -> u64 {
    value.clamp(
        MIN_REALTIME_TRANSCRIPT_MAX_BYTES,
        MAX_REALTIME_TRANSCRIPT_MAX_BYTES,
    )
}

async fn insert_default(pool: &SqlitePool, key: &str, value_json: &str) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO app_settings (key, value_json, updated_at) VALUES (?1, ?2, ?3) ON CONFLICT(key) DO NOTHING",
    )
    .bind(key)
    .bind(value_json)
    .bind(now_iso())
    .execute(pool)
    .await?;

    Ok(())
}

fn deserialize_setting<T: DeserializeOwned>(key: &str, value_json: &str) -> AppResult<T> {
    serde_json::from_str(value_json)
        .map_err(|error| AppError::Message(format!("Invalid value for setting '{key}': {error}")))
}

async fn get_setting<T: DeserializeOwned>(pool: &SqlitePool, key: &str) -> AppResult<Option<T>> {
    sqlx::query_scalar::<_, String>("SELECT value_json FROM app_settings WHERE key = ?1")
        .bind(key)
        .fetch_optional(pool)
        .await?
        .map(|value_json| deserialize_setting(key, &value_json))
        .transpose()
}

async fn upsert_setting<'e, E>(
    executor: E,
    key: &str,
    value_json: &str,
    updated_at: &str,
) -> AppResult<()>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query(
        "INSERT INTO app_settings (key, value_json, updated_at) VALUES (?1, ?2, ?3) ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at",
    )
    .bind(key)
    .bind(value_json)
    .bind(updated_at)
    .execute(executor)
    .await?;

    Ok(())
}

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339()
}

#[cfg(test)]
mod tests {
    use sqlx::SqlitePool;

    use super::{
        default_settings, ensure_defaults, get_settings, history_limit,
        normalize_realtime_connect_timeout_ms, normalize_realtime_max_concurrent_sessions,
        normalize_realtime_max_message_bytes, normalize_realtime_transcript_max_bytes,
        normalize_realtime_transcript_max_entries, normalize_realtime_workspace_state,
        normalize_ui_scale, save_settings, HISTORY_LIMIT_KEY, THEME_KEY,
    };

    async fn setup_test_db() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:")
            .await
            .expect("in-memory database");

        sqlx::query(
            r#"
            CREATE TABLE app_settings (
              key TEXT PRIMARY KEY,
              value_json TEXT NOT NULL,
              updated_at TEXT NOT NULL
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("create app settings table");

        pool
    }

    #[test]
    fn normalize_ui_scale_matches_settings_ui_range() {
        assert_eq!(normalize_ui_scale(0.4), 0.6);
        assert_eq!(normalize_ui_scale(0.6), 0.6);
        assert_eq!(normalize_ui_scale(1.0), 1.0);
        assert_eq!(normalize_ui_scale(1.5), 1.5);
        assert_eq!(normalize_ui_scale(1.8), 1.5);
    }

    #[test]
    fn normalize_realtime_limits_matches_supported_ranges() {
        assert_eq!(normalize_realtime_connect_timeout_ms(0), 1_000);
        assert_eq!(normalize_realtime_connect_timeout_ms(u64::MAX), 120_000);
        assert_eq!(normalize_realtime_max_concurrent_sessions(0), 1);
        assert_eq!(normalize_realtime_max_concurrent_sessions(u32::MAX), 100);
        assert_eq!(normalize_realtime_max_message_bytes(1), 64 * 1024);
        assert_eq!(
            normalize_realtime_max_message_bytes(u64::MAX),
            256 * 1024 * 1024
        );
        assert_eq!(normalize_realtime_transcript_max_entries(0), 1);
        assert_eq!(normalize_realtime_transcript_max_entries(u32::MAX), 10_000);
        assert_eq!(normalize_realtime_transcript_max_bytes(1), 64 * 1024);
        assert_eq!(
            normalize_realtime_transcript_max_bytes(u64::MAX),
            512 * 1024 * 1024
        );
    }

    #[test]
    fn realtime_workspace_restore_is_always_disconnected_and_empty() {
        let normalized = normalize_realtime_workspace_state(serde_json::json!({
            "activeTabId": "tab-1",
            "tabs": [{
                "id": "tab-1",
                "draft": {"requestType": "websocket"},
                "status": "connected",
                "generation": 9,
                "lastSequence": 31,
                "statusMessage": "Connected",
                "reconnectRequired": true,
                "transcript": [{"id": "entry"}],
                "transcriptSizeBytes": 100,
                "errorText": "old error"
            }]
        }))
        .expect("normalize");
        let tab = &normalized["tabs"][0];
        assert_eq!(tab["status"], "disconnected");
        assert_eq!(tab["generation"], 0);
        assert_eq!(tab["lastSequence"], 0);
        assert_eq!(tab["transcript"], serde_json::json!([]));
        assert_eq!(tab["transcriptSizeBytes"], 0);
        assert_eq!(tab["errorText"], "");
        assert_eq!(tab["draft"]["requestType"], "websocket");
    }

    #[tokio::test]
    async fn save_settings_is_atomic() {
        let pool = setup_test_db().await;
        sqlx::query("INSERT INTO app_settings (key, value_json, updated_at) VALUES (?1, ?2, ?3)")
            .bind(THEME_KEY)
            .bind("\"before\"")
            .bind("before")
            .execute(&pool)
            .await
            .expect("seed theme");
        ensure_defaults(&pool).await.expect("seed setting defaults");
        sqlx::query(
            r#"
            CREATE TRIGGER abort_ui_scale
            BEFORE UPDATE ON app_settings
            WHEN NEW.key = 'ui_scale'
            BEGIN
              SELECT RAISE(ABORT, 'ui scale blocked');
            END
            "#,
        )
        .execute(&pool)
        .await
        .expect("create abort trigger");

        let mut settings = default_settings();
        settings.theme = "after".to_string();

        assert!(save_settings(&pool, &settings).await.is_err());

        let theme: String =
            sqlx::query_scalar("SELECT value_json FROM app_settings WHERE key = ?1")
                .bind(THEME_KEY)
                .fetch_one(&pool)
                .await
                .expect("read theme");
        assert_eq!(theme, "\"before\"");
    }

    #[tokio::test]
    async fn history_limit_ignores_unrelated_corrupt_setting() {
        let pool = setup_test_db().await;
        sqlx::query("INSERT INTO app_settings (key, value_json, updated_at) VALUES (?1, ?2, ?3)")
            .bind(THEME_KEY)
            .bind("not JSON")
            .bind("now")
            .execute(&pool)
            .await
            .expect("seed corrupt theme");
        sqlx::query("INSERT INTO app_settings (key, value_json, updated_at) VALUES (?1, ?2, ?3)")
            .bind(HISTORY_LIMIT_KEY)
            .bind("123")
            .bind("now")
            .execute(&pool)
            .await
            .expect("seed history limit");

        assert_eq!(history_limit(&pool).await.expect("load history limit"), 123);
    }

    #[tokio::test]
    async fn get_settings_names_corrupt_setting_key() {
        let pool = setup_test_db().await;
        sqlx::query("INSERT INTO app_settings (key, value_json, updated_at) VALUES (?1, ?2, ?3)")
            .bind(THEME_KEY)
            .bind("not JSON")
            .bind("now")
            .execute(&pool)
            .await
            .expect("seed corrupt theme");

        let error = get_settings(&pool)
            .await
            .expect_err("corrupt setting should fail");
        assert!(error.to_string().contains(THEME_KEY));
        assert!(!error.to_string().contains("SELECT"));
    }
}
