use chrono::{SecondsFormat, Utc};
use sqlx::{Row, SqlitePool};

use crate::{
    domain::activity::{AgentActivityEntry, AgentActivityPage, NewAgentActivity},
    error::AppResult,
};

const MAX_AGENT_ACTIVITY: i64 = 1_000;

pub async fn record(pool: &SqlitePool, activity: &NewAgentActivity<'_>) -> AppResult<i64> {
    let changed_fields_json = serde_json::to_string(activity.changed_fields)?;
    let result = sqlx::query(
        r#"
        INSERT INTO agent_activity (
          batch_id, occurred_at, actor_name, actor_version, session_id,
          operation, outcome, target_kind, target_id, target_name,
          collection_id, changed_fields_json, error_code, error_message
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
        "#,
    )
    .bind(activity.batch_id)
    .bind(Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true))
    .bind(&activity.actor.name)
    .bind(&activity.actor.version)
    .bind(&activity.actor.session_id)
    .bind(activity.operation)
    .bind(activity.outcome)
    .bind(activity.target_kind)
    .bind(activity.target_id)
    .bind(activity.target_name)
    .bind(activity.collection_id)
    .bind(changed_fields_json)
    .bind(activity.error_code)
    .bind(activity.error_message)
    .execute(pool)
    .await?;

    prune(pool).await?;
    Ok(result.last_insert_rowid())
}

async fn prune(pool: &SqlitePool) -> AppResult<()> {
    sqlx::query(
        "DELETE FROM agent_activity WHERE id NOT IN (SELECT id FROM agent_activity ORDER BY id DESC LIMIT ?1)",
    )
    .bind(MAX_AGENT_ACTIVITY)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list(
    pool: &SqlitePool,
    after_id: Option<i64>,
    limit: Option<usize>,
) -> AppResult<AgentActivityPage> {
    let limit = limit.unwrap_or(100).clamp(1, 250) as i64;
    let latest_id = sqlx::query_scalar::<_, i64>("SELECT COALESCE(MAX(id), 0) FROM agent_activity")
        .fetch_one(pool)
        .await?;

    let rows = if let Some(after_id) = after_id {
        sqlx::query(r#"SELECT * FROM agent_activity WHERE id > ?1 ORDER BY id ASC LIMIT ?2"#)
            .bind(after_id)
            .bind(limit)
            .fetch_all(pool)
            .await?
    } else {
        sqlx::query(r#"SELECT * FROM agent_activity ORDER BY id DESC LIMIT ?1"#)
            .bind(limit)
            .fetch_all(pool)
            .await?
    };

    let entries = rows
        .into_iter()
        .map(|row| -> AppResult<AgentActivityEntry> {
            Ok(AgentActivityEntry {
                id: row.get("id"),
                batch_id: row.get("batch_id"),
                occurred_at: row.get("occurred_at"),
                actor_name: row.get("actor_name"),
                actor_version: row.get("actor_version"),
                session_id: row.get("session_id"),
                operation: row.get("operation"),
                outcome: row.get("outcome"),
                target_kind: row.get("target_kind"),
                target_id: row.get("target_id"),
                target_name: row.get("target_name"),
                collection_id: row.get("collection_id"),
                changed_fields: serde_json::from_str(&row.get::<String, _>("changed_fields_json"))?,
                error_code: row.get("error_code"),
                error_message: row.get("error_message"),
            })
        })
        .collect::<AppResult<Vec<_>>>()?;

    Ok(AgentActivityPage { entries, latest_id })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::activity::{AgentActor, NewAgentActivity};
    use sqlx::SqlitePool;
    use uuid::Uuid;

    async fn pool() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:").await.expect("pool");
        sqlx::query(r#"CREATE TABLE agent_activity (
          id INTEGER PRIMARY KEY AUTOINCREMENT, batch_id TEXT NOT NULL, occurred_at TEXT NOT NULL,
          actor_name TEXT NOT NULL, actor_version TEXT NOT NULL DEFAULT '', session_id TEXT NOT NULL,
          operation TEXT NOT NULL, outcome TEXT NOT NULL, target_kind TEXT NOT NULL, target_id TEXT NULL,
          target_name TEXT NOT NULL DEFAULT '', collection_id TEXT NULL, changed_fields_json TEXT NOT NULL DEFAULT '[]',
          error_code TEXT NULL, error_message TEXT NULL)"#).execute(&pool).await.expect("schema");
        pool
    }

    #[tokio::test]
    async fn activity_cursor_returns_only_new_entries() {
        let pool = pool().await;
        let actor = AgentActor {
            name: "test-client".into(),
            version: "1".into(),
            session_id: Uuid::new_v4().to_string(),
        };
        for target in ["one", "two"] {
            record(
                &pool,
                &NewAgentActivity {
                    batch_id: "batch",
                    actor: &actor,
                    operation: "create_request",
                    outcome: "succeeded",
                    target_kind: "request",
                    target_id: Some(target),
                    target_name: target,
                    collection_id: Some("collection"),
                    changed_fields: &["name"],
                    error_code: None,
                    error_message: None,
                },
            )
            .await
            .expect("record");
        }
        let first = list(&pool, None, Some(1)).await.expect("latest");
        assert_eq!(first.entries[0].target_id.as_deref(), Some("two"));
        let after = list(&pool, Some(1), Some(10)).await.expect("after cursor");
        assert_eq!(after.entries.len(), 1);
        assert_eq!(after.entries[0].target_id.as_deref(), Some("two"));
    }
}
