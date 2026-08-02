use chrono::{SecondsFormat, Utc};
use sqlx::{Row, Sqlite, SqlitePool, Transaction};
use uuid::Uuid;
use serde::{Deserialize, Serialize};

use crate::{
    domain::realtime::{
        LegacyRealtimeRequestDraft, RealtimeConnectionDraft, RealtimeConnectionProfileDetail,
        RealtimeConnectionProfileSummary, RequestType, VersionedLegacyRealtimeRequest,
        VersionedRealtimeConnection, VersionedRealtimeMessage,
        LEGACY_REALTIME_REQUEST_SCHEMA_VERSION, REALTIME_CONNECTION_SCHEMA_VERSION,
        REALTIME_MESSAGE_SCHEMA_VERSION,
    },
    error::{AppError, AppResult},
};

const REALTIME_WORKSPACE_STATE_KEY: &str = "realtime_workspace_state";
pub const REALTIME_CONNECTIONS_DOCUMENT_SCHEMA: &str = "https://postnot.dev/schemas/realtime-connections.json";
pub const REALTIME_CONNECTIONS_DOCUMENT_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RealtimeConnectionsDocument {
    pub schema: String,
    pub version: u32,
    pub connections: Vec<VersionedRealtimeConnection>,
}

pub async fn list_profiles(pool: &SqlitePool) -> AppResult<Vec<RealtimeConnectionProfileSummary>> {
    let rows = sqlx::query(
        "SELECT id, name, protocol, config_json, updated_at FROM realtime_connections ORDER BY updated_at DESC, name ASC",
    )
    .fetch_all(pool)
    .await?;
    rows.into_iter().map(map_summary).collect()
}

pub async fn get_profile(
    pool: &SqlitePool,
    profile_id: &str,
) -> AppResult<RealtimeConnectionProfileDetail> {
    let row = sqlx::query(
        "SELECT id, name, protocol, config_json, updated_at FROM realtime_connections WHERE id = ?1",
    )
    .bind(profile_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::Message("Realtime connection profile not found.".to_string()))?;
    map_detail(row)
}

pub async fn create_profile(
    pool: &SqlitePool,
    connection: &RealtimeConnectionDraft,
) -> AppResult<RealtimeConnectionProfileDetail> {
    validate_connection(connection)?;
    let id = Uuid::new_v4().to_string();
    let now = now_iso();
    let config_json = serde_json::to_string(&VersionedRealtimeConnection::new(connection.clone()))?;
    sqlx::query(
        "INSERT INTO realtime_connections (id, name, protocol, config_json, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )
    .bind(&id)
    .bind(connection.common().name.trim())
    .bind(connection.protocol().as_str())
    .bind(config_json)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;
    get_profile(pool, &id).await
}

pub async fn get_or_create_exact_profile(
    pool: &SqlitePool,
    connection: &RealtimeConnectionDraft,
) -> AppResult<RealtimeConnectionProfileDetail> {
    validate_connection(connection)?;
    let config_json = serde_json::to_string(&VersionedRealtimeConnection::new(connection.clone()))?;
    if let Some(id) = sqlx::query_scalar::<_, String>(
        "SELECT id FROM realtime_connections WHERE name = ?1 AND protocol = ?2 AND config_json = ?3 LIMIT 1",
    )
    .bind(connection.common().name.trim())
    .bind(connection.protocol().as_str())
    .bind(config_json)
    .fetch_optional(pool)
    .await?
    {
        return get_profile(pool, &id).await;
    }
    create_profile(pool, connection).await
}

pub async fn update_profile(
    pool: &SqlitePool,
    profile_id: &str,
    connection: &RealtimeConnectionDraft,
    expected_updated_at: Option<&str>,
) -> AppResult<RealtimeConnectionProfileDetail> {
    validate_connection(connection)?;
    let now = now_iso();
    let config_json = serde_json::to_string(&VersionedRealtimeConnection::new(connection.clone()))?;
    let result = sqlx::query(
        r#"
        UPDATE realtime_connections
        SET name = ?2, protocol = ?3, config_json = ?4, updated_at = ?5
        WHERE id = ?1 AND (?6 IS NULL OR updated_at = ?6)
        "#,
    )
    .bind(profile_id)
    .bind(connection.common().name.trim())
    .bind(connection.protocol().as_str())
    .bind(config_json)
    .bind(&now)
    .bind(expected_updated_at)
    .execute(pool)
    .await?;
    if result.rows_affected() == 0 {
        return revision_or_missing(pool, profile_id).await;
    }
    get_profile(pool, profile_id).await
}

pub async fn delete_profile(
    pool: &SqlitePool,
    profile_id: &str,
    expected_updated_at: Option<&str>,
) -> AppResult<()> {
    let result = sqlx::query(
        "DELETE FROM realtime_connections WHERE id = ?1 AND (?2 IS NULL OR updated_at = ?2)",
    )
    .bind(profile_id)
    .bind(expected_updated_at)
    .execute(pool)
    .await?;
    if result.rows_affected() == 0 {
        return revision_or_missing(pool, profile_id).await;
    }
    Ok(())
}

pub async fn export_profiles(
    pool: &SqlitePool,
    profile_ids: &[String],
    include_sensitive: bool,
) -> AppResult<String> {
    if profile_ids.is_empty() {
        return Err(AppError::Message("Select at least one connection profile to export.".to_string()));
    }
    let mut connections = Vec::with_capacity(profile_ids.len());
    for id in profile_ids {
        let mut connection = get_profile(pool, id).await?.connection;
        if !include_sensitive {
            redact_connection(&mut connection)?;
        }
        connections.push(VersionedRealtimeConnection::new(connection));
    }
    Ok(serde_json::to_string_pretty(&RealtimeConnectionsDocument {
        schema: REALTIME_CONNECTIONS_DOCUMENT_SCHEMA.to_string(),
        version: REALTIME_CONNECTIONS_DOCUMENT_VERSION,
        connections,
    })?)
}

pub async fn import_profiles(
    pool: &SqlitePool,
    source: &str,
) -> AppResult<Vec<RealtimeConnectionProfileDetail>> {
    let document: RealtimeConnectionsDocument = serde_json::from_str(source)?;
    if document.schema != REALTIME_CONNECTIONS_DOCUMENT_SCHEMA
        || document.version != REALTIME_CONNECTIONS_DOCUMENT_VERSION
    {
        return Err(AppError::Message("Unsupported realtime connection profile document.".to_string()));
    }
    let mut transaction = pool.begin().await?;
    let mut ids = Vec::with_capacity(document.connections.len());
    for versioned in document.connections {
        if versioned.version != REALTIME_CONNECTION_SCHEMA_VERSION {
            return Err(AppError::Message(format!(
                "Unsupported realtime connection version: {}.", versioned.version
            )));
        }
        validate_connection(&versioned.connection)?;
        ids.push(find_or_create_matching_profile(&mut transaction, &versioned.connection).await?);
    }
    transaction.commit().await?;
    let mut profiles = Vec::with_capacity(ids.len());
    for id in ids {
        profiles.push(get_profile(pool, &id).await?);
    }
    Ok(profiles)
}

fn redact_connection(connection: &mut RealtimeConnectionDraft) -> AppResult<()> {
    let mut value = serde_json::to_value(&*connection)?;
    redact_json(&mut value);
    *connection = serde_json::from_value(value)?;
    Ok(())
}

fn redact_json(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            let row_key = map.get("key").and_then(|v| v.as_str()).map(str::to_string);
            if let (Some(key), Some(value)) = (row_key.as_deref(), map.get_mut("value")) {
                if is_sensitive_key(key) { *value = serde_json::Value::String("***".to_string()); }
            }
            for (key, child) in map.iter_mut() {
                if is_sensitive_key(key) && !matches!(child, serde_json::Value::Object(_) | serde_json::Value::Array(_)) {
                    *child = serde_json::Value::String("***".to_string());
                } else if key == "url" {
                    if let Some(text) = child.as_str() {
                        if let Ok(mut url) = url::Url::parse(text) {
                            let pairs = url.query_pairs().map(|(k, v)| (k.into_owned(), v.into_owned())).collect::<Vec<_>>();
                            if !pairs.is_empty() {
                                url.query_pairs_mut().clear().extend_pairs(pairs.iter().map(|(k, v)| (k, if is_sensitive_key(k) { "***" } else { v.as_str() })));
                                *child = serde_json::Value::String(url.to_string());
                            }
                        }
                    }
                } else { redact_json(child); }
            }
        }
        serde_json::Value::Array(items) => items.iter_mut().for_each(redact_json),
        _ => {}
    }
}

fn is_sensitive_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase().replace(['_', '-'], "");
    key.contains("password") || key.contains("secret") || key.contains("token")
        || key.contains("apikey") || key == "authorization" || key == "cookie"
}

pub async fn migrate_legacy_realtime_data(pool: &SqlitePool) -> AppResult<()> {
    let mut transaction = pool.begin().await?;
    let rows = sqlx::query(
        r#"
        SELECT id, name, realtime_message_json
        FROM collection_items
        WHERE kind = 'request'
          AND request_type IN ('websocket', 'socketio')
          AND realtime_message_json IS NOT NULL
        ORDER BY created_at ASC, id ASC
        "#,
    )
    .fetch_all(&mut *transaction)
    .await?;

    let mut item_profiles = std::collections::HashMap::new();
    for row in rows {
        let item_id: String = row.get("id");
        let json: String = row.get("realtime_message_json");
        if let Ok(versioned) = serde_json::from_str::<VersionedRealtimeMessage>(&json) {
            if versioned.version != REALTIME_MESSAGE_SCHEMA_VERSION {
                return Err(AppError::Message(format!(
                    "Unsupported realtime message version {} for item {}.",
                    versioned.version, item_id
                )));
            }
            continue;
        }

        let legacy: VersionedLegacyRealtimeRequest = serde_json::from_str(&json).map_err(|error| {
            AppError::Message(format!(
                "Could not migrate realtime collection item {item_id}: {error}"
            ))
        })?;
        if legacy.version != LEGACY_REALTIME_REQUEST_SCHEMA_VERSION {
            return Err(AppError::Message(format!(
                "Unsupported legacy realtime request version {} for item {}.",
                legacy.version, item_id
            )));
        }
        let (connection, message) = legacy.request.split();
        let profile_id = find_or_create_matching_profile(&mut transaction, &connection).await?;
        let message_json = serde_json::to_string(&VersionedRealtimeMessage::new(message.clone()))?;
        sqlx::query(
            "UPDATE collection_items SET name = ?2, url = NULL, realtime_message_json = ?3 WHERE id = ?1",
        )
        .bind(&item_id)
        .bind(message.name())
        .bind(message_json)
        .execute(&mut *transaction)
        .await?;
        item_profiles.insert(item_id, profile_id);
    }

    migrate_workspace_state(&mut transaction, &item_profiles).await?;
    transaction.commit().await?;
    Ok(())
}

async fn find_or_create_matching_profile(
    transaction: &mut Transaction<'_, Sqlite>,
    connection: &RealtimeConnectionDraft,
) -> AppResult<String> {
    let config_json = serde_json::to_string(&VersionedRealtimeConnection::new(connection.clone()))?;
    if let Some(id) = sqlx::query_scalar::<_, String>(
        "SELECT id FROM realtime_connections WHERE name = ?1 AND protocol = ?2 AND config_json = ?3 LIMIT 1",
    )
    .bind(connection.common().name.trim())
    .bind(connection.protocol().as_str())
    .bind(&config_json)
    .fetch_optional(&mut **transaction)
    .await?
    {
        return Ok(id);
    }
    let id = Uuid::new_v4().to_string();
    let now = now_iso();
    sqlx::query(
        "INSERT INTO realtime_connections (id, name, protocol, config_json, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )
    .bind(&id)
    .bind(connection.common().name.trim())
    .bind(connection.protocol().as_str())
    .bind(config_json)
    .bind(&now)
    .bind(&now)
    .execute(&mut **transaction)
    .await?;
    Ok(id)
}

async fn migrate_workspace_state(
    transaction: &mut Transaction<'_, Sqlite>,
    item_profiles: &std::collections::HashMap<String, String>,
) -> AppResult<()> {
    let Some(raw) = sqlx::query_scalar::<_, String>(
        "SELECT value_json FROM app_settings WHERE key = ?1",
    )
    .bind(REALTIME_WORKSPACE_STATE_KEY)
    .fetch_optional(&mut **transaction)
    .await?
    else {
        return Ok(());
    };
    let mut state: serde_json::Value = serde_json::from_str(&raw)?;
    let Some(tabs) = state.get_mut("tabs").and_then(serde_json::Value::as_array_mut) else {
        return Ok(());
    };
    let mut changed = false;
    for tab_value in tabs {
        let Some(tab) = tab_value.as_object_mut() else {
            continue;
        };
        if tab.contains_key("connectionDraft") {
            continue;
        }
        let Some(draft_value) = tab.remove("draft") else {
            continue;
        };
        let legacy: LegacyRealtimeRequestDraft = serde_json::from_value(draft_value)?;
        let (connection, message) = legacy.split();
        let saved_message_id = tab
            .remove("savedRequestId")
            .and_then(|value| value.as_str().map(str::to_string));
        let selected_profile_id = saved_message_id
            .as_ref()
            .and_then(|id| item_profiles.get(id))
            .cloned();
        let baseline = tab
            .remove("baselineDraft")
            .filter(|value| !value.is_null())
            .map(serde_json::from_value::<LegacyRealtimeRequestDraft>)
            .transpose()?
            .map(LegacyRealtimeRequestDraft::split);
        tab.insert("connectionDraft".to_string(), serde_json::to_value(&connection)?);
        tab.insert("messageDraft".to_string(), serde_json::to_value(&message)?);
        tab.insert(
            "baselineConnectionDraft".to_string(),
            baseline
                .as_ref()
                .map(|(connection, _)| serde_json::to_value(connection))
                .transpose()?
                .unwrap_or(serde_json::Value::Null),
        );
        tab.insert(
            "baselineMessageDraft".to_string(),
            baseline
                .as_ref()
                .map(|(_, message)| serde_json::to_value(message))
                .transpose()?
                .unwrap_or(serde_json::Value::Null),
        );
        tab.insert("selectedProfileId".to_string(), serde_json::to_value(selected_profile_id)?);
        tab.insert("selectedMessageId".to_string(), serde_json::to_value(saved_message_id)?);
        tab.insert("connectionExternallyChanged".to_string(), serde_json::json!(false));
        let message_changed = tab
            .remove("externallyChanged")
            .unwrap_or(serde_json::json!(false));
        tab.insert("messageExternallyChanged".to_string(), message_changed);
        tab.insert("status".to_string(), serde_json::json!("disconnected"));
        tab.insert("generation".to_string(), serde_json::json!(0));
        tab.insert("lastSequence".to_string(), serde_json::json!(0));
        tab.insert("statusMessage".to_string(), serde_json::json!("Disconnected"));
        tab.insert("reconnectRequired".to_string(), serde_json::json!(false));
        tab.insert("transcript".to_string(), serde_json::json!([]));
        tab.insert("transcriptSizeBytes".to_string(), serde_json::json!(0));
        tab.insert("errorText".to_string(), serde_json::json!(""));
        changed = true;
    }
    if changed {
        sqlx::query(
            "UPDATE app_settings SET value_json = ?2, updated_at = ?3 WHERE key = ?1",
        )
        .bind(REALTIME_WORKSPACE_STATE_KEY)
        .bind(serde_json::to_string(&state)?)
        .bind(now_iso())
        .execute(&mut **transaction)
        .await?;
    }
    Ok(())
}

pub fn validate_connection(connection: &RealtimeConnectionDraft) -> AppResult<()> {
    let common = connection.common();
    if common.name.trim().is_empty() {
        return Err(AppError::Message("Connection profile name is required.".to_string()));
    }
    if common.url.trim().is_empty() {
        return Err(AppError::Message("Connection URL is required.".to_string()));
    }
    let url = url::Url::parse(common.url.trim())?;
    let valid_scheme = match connection {
        RealtimeConnectionDraft::Websocket { .. } => matches!(url.scheme(), "ws" | "wss"),
        RealtimeConnectionDraft::Socketio { .. } => {
            matches!(url.scheme(), "http" | "https" | "ws" | "wss")
        }
    };
    if !valid_scheme {
        return Err(AppError::Message("Connection URL uses an unsupported scheme.".to_string()));
    }
    Ok(())
}

fn map_summary(row: sqlx::sqlite::SqliteRow) -> AppResult<RealtimeConnectionProfileSummary> {
    let detail = map_detail(row)?;
    Ok(RealtimeConnectionProfileSummary {
        id: detail.id,
        name: detail.name,
        protocol: detail.protocol,
        url: detail.url,
        updated_at: detail.updated_at,
    })
}

fn map_detail(row: sqlx::sqlite::SqliteRow) -> AppResult<RealtimeConnectionProfileDetail> {
    let protocol = parse_protocol(&row.get::<String, _>("protocol"))?;
    let versioned: VersionedRealtimeConnection =
        serde_json::from_str(&row.get::<String, _>("config_json"))?;
    if versioned.version != REALTIME_CONNECTION_SCHEMA_VERSION {
        return Err(AppError::Message(format!(
            "Unsupported realtime connection profile version: {}.",
            versioned.version
        )));
    }
    if versioned.connection.protocol() != protocol {
        return Err(AppError::Message(
            "Realtime connection profile protocol does not match its configuration.".to_string(),
        ));
    }
    Ok(RealtimeConnectionProfileDetail {
        id: row.get("id"),
        name: row.get("name"),
        protocol,
        url: versioned.connection.common().url.clone(),
        updated_at: row.get("updated_at"),
        connection: versioned.connection,
    })
}

fn parse_protocol(value: &str) -> AppResult<RequestType> {
    match value {
        "websocket" => Ok(RequestType::Websocket),
        "socketio" => Ok(RequestType::Socketio),
        _ => Err(AppError::Message(format!("Unsupported realtime protocol: {value}."))),
    }
}

async fn revision_or_missing<T>(pool: &SqlitePool, profile_id: &str) -> AppResult<T> {
    let current: Option<String> =
        sqlx::query_scalar("SELECT updated_at FROM realtime_connections WHERE id = ?1")
            .bind(profile_id)
            .fetch_optional(pool)
            .await?;
    match current {
        Some(current_updated_at) => Err(AppError::Conflict { current_updated_at }),
        None => Err(AppError::Message(
            "Realtime connection profile not found.".to_string(),
        )),
    }
}

fn now_iso() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new().max_connections(1).connect("sqlite::memory:").await.expect("pool");
        sqlx::migrate!("./migrations").run(&pool).await.expect("migrations");
        sqlx::query("INSERT INTO collections (id, name, description, created_at, updated_at) VALUES ('c', 'C', '', 'now', 'now')").execute(&pool).await.expect("collection");
        pool
    }

    fn legacy(name: &str, url: &str, content: &str) -> String {
        serde_json::json!({
            "version": 1,
            "requestType": "websocket",
            "name": name,
            "url": url,
            "composer": { "mode": "text", "content": content, "binary": null }
        }).to_string()
    }

    async fn insert_legacy(pool: &SqlitePool, id: &str, json: &str) {
        sqlx::query("INSERT INTO collection_items (id, collection_id, kind, name, sort_order, request_type, realtime_message_json, created_at, updated_at) VALUES (?1, 'c', 'request', 'Old', 0, 'websocket', ?2, 'now', 'now')")
            .bind(id).bind(json).execute(pool).await.expect("legacy item");
    }

    #[tokio::test]
    async fn upgrader_splits_messages_reuses_exact_profiles_and_is_idempotent() {
        let pool = pool().await;
        insert_legacy(&pool, "one", &legacy("Echo", "wss://example.test", "first")).await;
        insert_legacy(&pool, "two", &legacy("Echo", "wss://example.test", "second")).await;
        insert_legacy(&pool, "three", &legacy("Other", "wss://example.test", "third")).await;
        migrate_legacy_realtime_data(&pool).await.expect("upgrade");
        assert_eq!(sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM realtime_connections").fetch_one(&pool).await.unwrap(), 2);
        let messages = sqlx::query_scalar::<_, String>("SELECT realtime_message_json FROM collection_items ORDER BY id").fetch_all(&pool).await.unwrap();
        assert!(messages.iter().all(|json| serde_json::from_str::<VersionedRealtimeMessage>(json).is_ok()));
        migrate_legacy_realtime_data(&pool).await.expect("rerun");
        assert_eq!(sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM realtime_connections").fetch_one(&pool).await.unwrap(), 2);
    }

    #[tokio::test]
    async fn upgrader_rolls_back_every_record_when_one_is_invalid() {
        let pool = pool().await;
        let original = legacy("Echo", "wss://example.test", "first");
        insert_legacy(&pool, "one", &original).await;
        insert_legacy(&pool, "two", "{not json").await;
        assert!(migrate_legacy_realtime_data(&pool).await.is_err());
        assert_eq!(sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM realtime_connections").fetch_one(&pool).await.unwrap(), 0);
        let retained = sqlx::query_scalar::<_, String>("SELECT realtime_message_json FROM collection_items WHERE id = 'one'").fetch_one(&pool).await.unwrap();
        assert_eq!(retained, original);
        sqlx::query("UPDATE collection_items SET realtime_message_json = ?1 WHERE id = 'two'").bind(legacy("Fixed", "wss://fixed.example.test", "ok")).execute(&pool).await.unwrap();
        migrate_legacy_realtime_data(&pool).await.expect("retry after correction");
        assert_eq!(sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM realtime_connections").fetch_one(&pool).await.unwrap(), 2);
    }

    #[tokio::test]
    async fn upgrader_splits_workspace_and_removes_runtime_state() {
        let pool = pool().await;
        let draft: serde_json::Value = serde_json::from_str(&legacy("Echo", "wss://example.test", "hello")).unwrap();
        let draft = draft.as_object().unwrap().iter().filter(|(key, _)| key.as_str() != "version").map(|(key, value)| (key.clone(), value.clone())).collect::<serde_json::Map<_, _>>();
        let state = serde_json::json!({"activeTabId":"tab", "tabs":[{"id":"tab", "draft": draft, "baselineDraft": draft, "status":"connected", "generation":9, "transcript":[{"secret":"runtime"}]}]});
        sqlx::query("INSERT INTO app_settings (key, value_json, updated_at) VALUES (?1, ?2, 'now')").bind(REALTIME_WORKSPACE_STATE_KEY).bind(state.to_string()).execute(&pool).await.unwrap();
        migrate_legacy_realtime_data(&pool).await.expect("upgrade workspace");
        let raw = sqlx::query_scalar::<_, String>("SELECT value_json FROM app_settings WHERE key = ?1").bind(REALTIME_WORKSPACE_STATE_KEY).fetch_one(&pool).await.unwrap();
        let state: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let tab = &state["tabs"][0];
        assert!(tab.get("connectionDraft").is_some());
        assert!(tab.get("messageDraft").is_some());
        assert_eq!(tab["status"], "disconnected");
        assert_eq!(tab["generation"], 0);
        assert_eq!(tab["transcript"], serde_json::json!([]));
    }

    #[tokio::test]
    async fn profile_document_redacts_credentials_by_default_and_round_trips() {
        let pool = pool().await;
        let connection: RealtimeConnectionDraft = serde_json::from_value(serde_json::json!({
            "protocol":"websocket", "name":"Secure", "url":"wss://example.test?token=literal",
            "headers":[{"id":"h","key":"Authorization","value":"Bearer literal","enabled":true}]
        })).unwrap();
        let profile = create_profile(&pool, &connection).await.unwrap();
        let json = export_profiles(&pool, &[profile.id], false).await.unwrap();
        assert!(!json.contains("Bearer literal"));
        assert!(!json.contains("token=literal"));
        let imported = import_profiles(&pool, &json).await.unwrap();
        assert_eq!(imported.len(), 1);
    }

    #[tokio::test]
    async fn profile_crud_enforces_revisions_and_deletion_is_independent() {
        let pool = pool().await;
        let connection: RealtimeConnectionDraft = serde_json::from_value(serde_json::json!({
            "protocol":"websocket", "name":"Echo", "url":"wss://example.test"
        })).unwrap();
        let created = create_profile(&pool, &connection).await.unwrap();
        let mut changed = connection.clone();
        if let RealtimeConnectionDraft::Websocket { common, .. } = &mut changed { common.name = "Echo changed".to_string(); }
        let updated = update_profile(&pool, &created.id, &changed, Some(&created.updated_at)).await.unwrap();
        assert!(matches!(update_profile(&pool, &created.id, &connection, Some(&created.updated_at)).await, Err(AppError::Conflict { .. })));
        sqlx::query("INSERT INTO collection_items (id, collection_id, kind, name, sort_order, request_type, realtime_message_json, created_at, updated_at) VALUES ('message', 'c', 'request', 'Message', 0, 'websocket', '{\"version\":1,\"message\":{\"protocol\":\"websocket\",\"name\":\"Message\",\"composer\":{\"mode\":\"text\",\"content\":\"\",\"binary\":null}}}', 'now', 'now')").execute(&pool).await.unwrap();
        delete_profile(&pool, &created.id, Some(&updated.updated_at)).await.unwrap();
        assert_eq!(sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM collection_items WHERE id = 'message'").fetch_one(&pool).await.unwrap(), 1);
    }
}
