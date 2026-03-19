use sqlx::{Row, SqlitePool};

use crate::{
    domain::settings::AppSettings,
    error::AppResult,
};

const THEME_KEY: &str = "theme";
const REQUEST_TIMEOUT_MS_KEY: &str = "request_timeout_ms";
const FOLLOW_REDIRECTS_KEY: &str = "follow_redirects";
const VALIDATE_TLS_KEY: &str = "validate_tls";
const HISTORY_LIMIT_KEY: &str = "history_limit";

pub fn default_settings() -> AppSettings {
    AppSettings {
        theme: "system".to_string(),
        request_timeout_ms: 30_000,
        follow_redirects: true,
        validate_tls: true,
        history_limit: 200,
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
    ensure_defaults(pool).await?;

    let rows = sqlx::query("SELECT key, value_json FROM app_settings")
        .fetch_all(pool)
        .await?;

    let mut settings = default_settings();

    for row in rows {
        let key: String = row.get("key");
        let value_json: String = row.get("value_json");

        match key.as_str() {
            THEME_KEY => settings.theme = serde_json::from_str(&value_json)?,
            REQUEST_TIMEOUT_MS_KEY => settings.request_timeout_ms = serde_json::from_str(&value_json)?,
            FOLLOW_REDIRECTS_KEY => settings.follow_redirects = serde_json::from_str(&value_json)?,
            VALIDATE_TLS_KEY => settings.validate_tls = serde_json::from_str(&value_json)?,
            HISTORY_LIMIT_KEY => settings.history_limit = serde_json::from_str(&value_json)?,
            _ => {}
        }
    }

    Ok(settings)
}

pub async fn save_settings(pool: &SqlitePool, settings: &AppSettings) -> AppResult<()> {
    for (key, value_json) in serialize_settings(settings)? {
        upsert_setting(pool, key, &value_json).await?;
    }

    Ok(())
}

pub async fn history_limit(pool: &SqlitePool) -> AppResult<u32> {
    Ok(get_settings(pool).await?.history_limit)
}

fn serialize_settings(settings: &AppSettings) -> AppResult<Vec<(&'static str, String)>> {
    Ok(vec![
        (THEME_KEY, serde_json::to_string(&settings.theme)?),
        (
            REQUEST_TIMEOUT_MS_KEY,
            serde_json::to_string(&settings.request_timeout_ms)?,
        ),
        (
            FOLLOW_REDIRECTS_KEY,
            serde_json::to_string(&settings.follow_redirects)?,
        ),
        (VALIDATE_TLS_KEY, serde_json::to_string(&settings.validate_tls)?),
        (HISTORY_LIMIT_KEY, serde_json::to_string(&settings.history_limit)?),
    ])
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

async fn upsert_setting(pool: &SqlitePool, key: &str, value_json: &str) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO app_settings (key, value_json, updated_at) VALUES (?1, ?2, ?3) ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at",
    )
    .bind(key)
    .bind(value_json)
    .bind(now_iso())
    .execute(pool)
    .await?;

    Ok(())
}

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339()
}
