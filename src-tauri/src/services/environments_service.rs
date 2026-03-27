use std::collections::HashMap;

use chrono::Utc;
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use crate::{
    domain::{
        environments::{EnvironmentDetail, EnvironmentInput, EnvironmentSummary},
        requests::{FileRow, KeyValueRow, RequestAuth, RequestBody, SendRequestPayload},
    },
    error::{AppError, AppResult},
};

pub async fn list_environments(pool: &SqlitePool) -> AppResult<Vec<EnvironmentSummary>> {
    let rows = sqlx::query(
        r#"
        SELECT id, name, is_active, updated_at, variables_json
        FROM environments
        ORDER BY is_active DESC, updated_at DESC, name ASC
        "#,
    )
    .fetch_all(pool)
    .await?;

    rows.into_iter()
        .map(|row| {
            let variables: Vec<KeyValueRow> = serde_json::from_str(&row.get::<String, _>("variables_json"))?;

            Ok(EnvironmentSummary {
                id: row.get("id"),
                name: row.get("name"),
                is_active: row.get::<i64, _>("is_active") != 0,
                variable_count: variables
                    .iter()
                    .filter(|item| item.enabled && !item.key.trim().is_empty())
                    .count() as i64,
                updated_at: row.get("updated_at"),
            })
        })
        .collect()
}

pub async fn create_environment(pool: &SqlitePool) -> AppResult<EnvironmentDetail> {
    let id = Uuid::new_v4().to_string();
    let now = now_iso();

    sqlx::query(
        "INSERT INTO environments (id, name, is_active, variables_json, created_at, updated_at) VALUES (?1, ?2, 0, ?3, ?4, ?5)",
    )
    .bind(&id)
    .bind("Untitled environment")
    .bind("[]")
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;

    get_environment(pool, &id).await
}

pub async fn get_environment(pool: &SqlitePool, environment_id: &str) -> AppResult<EnvironmentDetail> {
    let row = sqlx::query(
        "SELECT id, name, is_active, variables_json, updated_at FROM environments WHERE id = ?1",
    )
    .bind(environment_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::Message("Environment not found.".to_string()))?;

    Ok(EnvironmentDetail {
        id: row.get("id"),
        name: row.get("name"),
        is_active: row.get::<i64, _>("is_active") != 0,
        variables: serde_json::from_str(&row.get::<String, _>("variables_json"))?,
        updated_at: row.get("updated_at"),
    })
}

pub async fn update_environment(
    pool: &SqlitePool,
    environment_id: &str,
    input: &EnvironmentInput,
) -> AppResult<EnvironmentDetail> {
    let name = input.name.trim();
    if name.is_empty() {
        return Err(AppError::Message("Environment name is required.".to_string()));
    }

    let result = sqlx::query(
        "UPDATE environments SET name = ?2, variables_json = ?3, updated_at = ?4 WHERE id = ?1",
    )
    .bind(environment_id)
    .bind(name)
    .bind(serde_json::to_string(&input.variables)?)
    .bind(now_iso())
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::Message("Environment not found.".to_string()));
    }

    get_environment(pool, environment_id).await
}

pub async fn delete_environment(pool: &SqlitePool, environment_id: &str) -> AppResult<()> {
    sqlx::query("DELETE FROM environments WHERE id = ?1")
        .bind(environment_id)
        .execute(pool)
        .await?;

    Ok(())
}

pub async fn set_active_environment(pool: &SqlitePool, environment_id: Option<&str>) -> AppResult<()> {
    sqlx::query("UPDATE environments SET is_active = 0")
        .execute(pool)
        .await?;

    if let Some(environment_id) = environment_id {
        let result = sqlx::query("UPDATE environments SET is_active = 1, updated_at = ?2 WHERE id = ?1")
            .bind(environment_id)
            .bind(now_iso())
            .execute(pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::Message("Environment not found.".to_string()));
        }
    }

    Ok(())
}

pub async fn get_active_environment(pool: &SqlitePool) -> AppResult<Option<EnvironmentDetail>> {
    let row = sqlx::query(
        "SELECT id, name, is_active, variables_json, updated_at FROM environments WHERE is_active = 1 LIMIT 1",
    )
    .fetch_optional(pool)
    .await?;

    match row {
        Some(row) => Ok(Some(EnvironmentDetail {
            id: row.get("id"),
            name: row.get("name"),
            is_active: row.get::<i64, _>("is_active") != 0,
            variables: serde_json::from_str(&row.get::<String, _>("variables_json"))?,
            updated_at: row.get("updated_at"),
        })),
        None => Ok(None),
    }
}

pub fn resolve_request(
    payload: &SendRequestPayload,
    active_environment: Option<&EnvironmentDetail>,
) -> SendRequestPayload {
    let variables = build_variable_map(active_environment);

    SendRequestPayload {
        name: resolve_string(&payload.name, &variables),
        method: payload.method.clone(),
        url: resolve_string(&payload.url, &variables),
        query_params: payload
            .query_params
            .iter()
            .map(|item| KeyValueRow {
                id: item.id.clone(),
                key: resolve_string(&item.key, &variables),
                value: resolve_string(&item.value, &variables),
                enabled: item.enabled,
            })
            .collect(),
        headers: payload
            .headers
            .iter()
            .map(|item| KeyValueRow {
                id: item.id.clone(),
                key: resolve_string(&item.key, &variables),
                value: resolve_string(&item.value, &variables),
                enabled: item.enabled,
            })
            .collect(),
        body: RequestBody {
            mode: payload.body.mode.clone(),
            raw: resolve_string(&payload.body.raw, &variables),
            form: payload
                .body
                .form
                .iter()
                .map(|item| KeyValueRow {
                    id: item.id.clone(),
                    key: resolve_string(&item.key, &variables),
                    value: resolve_string(&item.value, &variables),
                    enabled: item.enabled,
                })
                .collect(),
            files: payload
                .body
                .files
                .iter()
                .map(|file| FileRow {
                    id: file.id.clone(),
                    name: resolve_string(&file.name, &variables),
                    path: resolve_string(&file.path, &variables),
                    enabled: file.enabled,
                })
                .collect(),
        },
        auth: RequestAuth {
            auth_type: payload.auth.auth_type.clone(),
            basic_username: resolve_string(&payload.auth.basic_username, &variables),
            basic_password: resolve_string(&payload.auth.basic_password, &variables),
            bearer_token: resolve_string(&payload.auth.bearer_token, &variables),
            api_key_name: resolve_string(&payload.auth.api_key_name, &variables),
            api_key_value: resolve_string(&payload.auth.api_key_value, &variables),
            api_key_in: payload.auth.api_key_in.clone(),
        },
    }
}

fn build_variable_map(active_environment: Option<&EnvironmentDetail>) -> HashMap<String, String> {
    active_environment
        .map(|environment| {
            environment
                .variables
                .iter()
                .filter(|item| item.enabled && !item.key.trim().is_empty())
                .map(|item| (item.key.trim().to_string(), item.value.clone()))
                .collect()
        })
        .unwrap_or_default()
}

fn resolve_string(input: &str, variables: &HashMap<String, String>) -> String {
    let mut resolved = String::with_capacity(input.len());
    let mut rest = input;

    loop {
        let Some(start) = rest.find("{{") else {
            resolved.push_str(rest);
            break;
        };

        resolved.push_str(&rest[..start]);
        let after_start = &rest[start + 2..];

        let Some(end) = after_start.find("}}") else {
            resolved.push_str(&rest[start..]);
            break;
        };

        let key = after_start[..end].trim();

        match variables.get(key) {
            Some(value) => resolved.push_str(value),
            None => {
                resolved.push_str("{{");
                resolved.push_str(&after_start[..end]);
                resolved.push_str("}}");
            }
        }

        rest = &after_start[end + 2..];
    }

    resolved
}

fn now_iso() -> String {
    Utc::now().to_rfc3339()
}
