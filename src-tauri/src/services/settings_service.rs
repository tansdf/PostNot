use sqlx::{Row, SqlitePool};

use crate::{domain::settings::AppSettings, error::AppResult};

const THEME_KEY: &str = "theme";
const UI_SCALE_KEY: &str = "ui_scale";
const REQUEST_TIMEOUT_MS_KEY: &str = "request_timeout_ms";
const FOLLOW_REDIRECTS_KEY: &str = "follow_redirects";
const VALIDATE_TLS_KEY: &str = "validate_tls";
const HISTORY_LIMIT_KEY: &str = "history_limit";
const NOTIFICATION_TIMEOUT_MS_KEY: &str = "notification_timeout_ms";
const LAST_UPDATE_CHECKED_AT_KEY: &str = "last_update_checked_at";
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
            UI_SCALE_KEY => {
                settings.ui_scale = normalize_ui_scale(serde_json::from_str(&value_json)?)
            }
            REQUEST_TIMEOUT_MS_KEY => {
                settings.request_timeout_ms = serde_json::from_str(&value_json)?
            }
            FOLLOW_REDIRECTS_KEY => settings.follow_redirects = serde_json::from_str(&value_json)?,
            VALIDATE_TLS_KEY => settings.validate_tls = serde_json::from_str(&value_json)?,
            HISTORY_LIMIT_KEY => settings.history_limit = serde_json::from_str(&value_json)?,
            NOTIFICATION_TIMEOUT_MS_KEY => {
                settings.notification_timeout_ms =
                    normalize_notification_timeout_ms(serde_json::from_str(&value_json)?)
            }
            LAST_UPDATE_CHECKED_AT_KEY => {
                settings.last_update_checked_at = serde_json::from_str(&value_json)?
            }
            _ => {}
        }
    }

    Ok(settings)
}

pub async fn save_settings(pool: &SqlitePool, settings: &AppSettings) -> AppResult<()> {
    let mut settings = normalize_settings(settings);
    settings.last_update_checked_at = get_settings(pool).await?.last_update_checked_at;

    for (key, value_json) in serialize_settings(&settings)? {
        upsert_setting(pool, key, &value_json).await?;
    }

    Ok(())
}

pub async fn save_last_update_checked_at(pool: &SqlitePool, checked_at: &str) -> AppResult<()> {
    upsert_setting(
        pool,
        LAST_UPDATE_CHECKED_AT_KEY,
        &serde_json::to_string(&Some(checked_at.to_string()))?,
    )
    .await
}

pub async fn history_limit(pool: &SqlitePool) -> AppResult<u32> {
    Ok(get_settings(pool).await?.history_limit)
}

fn serialize_settings(settings: &AppSettings) -> AppResult<Vec<(&'static str, String)>> {
    Ok(vec![
        (THEME_KEY, serde_json::to_string(&settings.theme)?),
        (UI_SCALE_KEY, serde_json::to_string(&settings.ui_scale)?),
        (
            REQUEST_TIMEOUT_MS_KEY,
            serde_json::to_string(&settings.request_timeout_ms)?,
        ),
        (
            FOLLOW_REDIRECTS_KEY,
            serde_json::to_string(&settings.follow_redirects)?,
        ),
        (
            VALIDATE_TLS_KEY,
            serde_json::to_string(&settings.validate_tls)?,
        ),
        (
            HISTORY_LIMIT_KEY,
            serde_json::to_string(&settings.history_limit)?,
        ),
        (
            NOTIFICATION_TIMEOUT_MS_KEY,
            serde_json::to_string(&settings.notification_timeout_ms)?,
        ),
        (
            LAST_UPDATE_CHECKED_AT_KEY,
            serde_json::to_string(&settings.last_update_checked_at)?,
        ),
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

#[cfg(test)]
mod tests {
    use super::normalize_ui_scale;

    #[test]
    fn normalize_ui_scale_matches_settings_ui_range() {
        assert_eq!(normalize_ui_scale(0.4), 0.6);
        assert_eq!(normalize_ui_scale(0.6), 0.6);
        assert_eq!(normalize_ui_scale(1.0), 1.0);
        assert_eq!(normalize_ui_scale(1.5), 1.5);
        assert_eq!(normalize_ui_scale(1.8), 1.5);
    }
}
