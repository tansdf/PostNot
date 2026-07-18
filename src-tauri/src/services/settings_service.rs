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
const LAST_UPDATE_CHECKED_AT_KEY: &str = "last_update_checked_at";
const COLLECTION_SIDEBAR_STATE_KEY: &str = "collection_sidebar_state";
const REQUEST_WORKSPACE_STATE_KEY: &str = "request_workspace_state";
const DEFAULT_UI_SCALE: f64 = 1.0;
const MIN_UI_SCALE: f64 = 0.6;
const MAX_UI_SCALE: f64 = 1.5;
const MIN_NOTIFICATION_TIMEOUT_MS: u64 = 1_000;
const MAX_NOTIFICATION_TIMEOUT_MS: u64 = 60_000;

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
        setting!(LAST_UPDATE_CHECKED_AT_KEY, settings.last_update_checked_at),
    ])
}

fn normalize_settings(settings: &AppSettings) -> AppSettings {
    let mut normalized = settings.clone();
    normalized.ui_scale = normalize_ui_scale(normalized.ui_scale);
    normalized.notification_timeout_ms =
        normalize_notification_timeout_ms(normalized.notification_timeout_ms);
    normalized
}

fn normalize_ui_scale(value: f64) -> f64 {
    value.clamp(MIN_UI_SCALE, MAX_UI_SCALE)
}

fn normalize_notification_timeout_ms(value: u64) -> u64 {
    value.clamp(MIN_NOTIFICATION_TIMEOUT_MS, MAX_NOTIFICATION_TIMEOUT_MS)
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
        default_settings, ensure_defaults, get_settings, history_limit, normalize_ui_scale,
        save_settings, HISTORY_LIMIT_KEY, THEME_KEY,
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
