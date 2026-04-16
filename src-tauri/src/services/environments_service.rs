use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use chrono::{SecondsFormat, Utc};
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use crate::{
    domain::{
        environments::{
            EnvironmentDetail, EnvironmentInput, EnvironmentSummary, EnvironmentVariable,
        },
        requests::{FileRow, KeyValueRow, RequestAuth, RequestBody, SendRequestPayload},
    },
    error::{AppError, AppResult},
    services::secret_store_service::SecretStore,
};

#[derive(Debug, Clone)]
struct EnvironmentRecord {
    id: String,
    name: String,
    is_active: bool,
    variables: Vec<EnvironmentVariable>,
    updated_at: String,
}

#[derive(Debug, Clone)]
struct VariableValue {
    value: String,
    is_secret: bool,
}

#[derive(Debug, Clone, Copy)]
struct DynamicVariableCall<'a> {
    name: &'a str,
    argument: Option<usize>,
}

#[derive(Debug, Clone, Default)]
pub struct RequestSecretUsage {
    pub name: bool,
    pub url: bool,
    pub query_param_ids: HashSet<String>,
    pub header_ids: HashSet<String>,
    pub body_raw: bool,
    pub body_form_ids: HashSet<String>,
    pub body_file_ids: HashSet<String>,
    pub auth_basic_username: bool,
    pub auth_basic_password: bool,
    pub auth_bearer_token: bool,
    pub auth_api_key_name: bool,
    pub auth_api_key_value: bool,
}

#[derive(Debug, Clone)]
pub struct ResolvedRequestPayload {
    pub payload: SendRequestPayload,
    pub secret_usage: RequestSecretUsage,
}

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
            let variables = decode_environment_variables(&row.get::<String, _>("variables_json"))?;

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

    Ok(EnvironmentDetail {
        id,
        name: "Untitled environment".to_string(),
        is_active: false,
        variables: Vec::new(),
        updated_at: now,
    })
}

pub async fn get_environment(
    pool: &SqlitePool,
    secret_store: Arc<dyn SecretStore>,
    environment_id: &str,
) -> AppResult<EnvironmentDetail> {
    let record = fetch_environment_record(pool, environment_id).await?;
    hydrate_environment_detail(secret_store, record).await
}

pub async fn update_environment(
    pool: &SqlitePool,
    secret_store: Arc<dyn SecretStore>,
    environment_id: &str,
    input: &EnvironmentInput,
) -> AppResult<EnvironmentDetail> {
    let name = input.name.trim();
    if name.is_empty() {
        return Err(AppError::Message(
            "Environment name is required.".to_string(),
        ));
    }

    let existing = fetch_environment_record(pool, environment_id).await?;
    let previous_secret_snapshot = load_secret_snapshot(
        secret_store.clone(),
        environment_id.to_string(),
        &existing.variables,
    )
    .await?;
    let next_secret_snapshot = collect_secret_snapshot(&input.variables);
    let affected_secret_ids = secret_ids_union(&existing.variables, &input.variables);

    if let Err(error) = sync_secret_snapshot(
        secret_store.clone(),
        environment_id.to_string(),
        next_secret_snapshot.clone(),
        affected_secret_ids.clone(),
    )
    .await
    {
        if let Err(rollback_err) = sync_secret_snapshot(
            secret_store,
            environment_id.to_string(),
            previous_secret_snapshot,
            affected_secret_ids,
        )
        .await
        {
            log::warn!(
                "Secret store rollback failed after sync error for environment {environment_id}: {rollback_err}"
            );
        }
        return Err(error);
    }

    let update_result = sqlx::query(
        "UPDATE environments SET name = ?2, variables_json = ?3, updated_at = ?4 WHERE id = ?1",
    )
    .bind(environment_id)
    .bind(name)
    .bind(serde_json::to_string(&strip_secret_values(
        &input.variables,
    ))?)
    .bind(now_iso())
    .execute(pool)
    .await;

    match update_result {
        Ok(result) => {
            if result.rows_affected() == 0 {
                if let Err(rollback_err) = sync_secret_snapshot(
                    secret_store.clone(),
                    environment_id.to_string(),
                    previous_secret_snapshot,
                    affected_secret_ids,
                )
                .await
                {
                    log::warn!(
                        "Secret store rollback failed after missing environment row for {environment_id}: {rollback_err}"
                    );
                }
                return Err(AppError::Message("Environment not found.".to_string()));
            }
        }
        Err(error) => {
            if let Err(rollback_err) = sync_secret_snapshot(
                secret_store.clone(),
                environment_id.to_string(),
                previous_secret_snapshot,
                affected_secret_ids,
            )
            .await
            {
                log::warn!(
                    "Secret store rollback failed after SQLite error updating environment {environment_id}: {rollback_err}"
                );
            }
            return Err(error.into());
        }
    }

    get_environment(pool, secret_store, environment_id).await
}

pub async fn create_environment_from_input(
    pool: &SqlitePool,
    secret_store: Arc<dyn SecretStore>,
    input: &EnvironmentInput,
    set_active: bool,
) -> AppResult<EnvironmentDetail> {
    let created = create_environment(pool).await?;

    if let Err(error) = update_environment(pool, secret_store.clone(), &created.id, input).await {
        if let Err(cleanup_err) = delete_environment(pool, secret_store, &created.id).await {
            log::warn!(
                "Failed to delete partially created environment {} after import/update error: {cleanup_err}",
                created.id
            );
        }
        return Err(error);
    }

    if set_active {
        set_active_environment(pool, Some(&created.id)).await?;
    }

    get_environment(pool, secret_store, &created.id).await
}

pub async fn delete_environment(
    pool: &SqlitePool,
    secret_store: Arc<dyn SecretStore>,
    environment_id: &str,
) -> AppResult<()> {
    let Some(existing) = fetch_environment_record_optional(pool, environment_id).await? else {
        return Ok(());
    };

    let previous_secret_snapshot = load_secret_snapshot(
        secret_store.clone(),
        environment_id.to_string(),
        &existing.variables,
    )
    .await?;
    let affected_secret_ids: HashSet<String> = existing
        .variables
        .iter()
        .filter(|item| item.is_secret)
        .map(|item| item.id.clone())
        .collect();

    if let Err(error) = sync_secret_snapshot(
        secret_store.clone(),
        environment_id.to_string(),
        HashMap::new(),
        affected_secret_ids.clone(),
    )
    .await
    {
        if let Err(rollback_err) = sync_secret_snapshot(
            secret_store,
            environment_id.to_string(),
            previous_secret_snapshot,
            affected_secret_ids,
        )
        .await
        {
            log::warn!(
                "Secret store rollback failed after delete sync error for environment {environment_id}: {rollback_err}"
            );
        }
        return Err(error);
    }

    if let Err(error) = sqlx::query("DELETE FROM environments WHERE id = ?1")
        .bind(environment_id)
        .execute(pool)
        .await
    {
        if let Err(rollback_err) = sync_secret_snapshot(
            secret_store,
            environment_id.to_string(),
            previous_secret_snapshot,
            affected_secret_ids,
        )
        .await
        {
            log::warn!(
                "Secret store rollback failed after SQLite delete error for environment {environment_id}: {rollback_err}"
            );
        }
        return Err(error.into());
    }

    Ok(())
}

pub async fn set_active_environment(
    pool: &SqlitePool,
    environment_id: Option<&str>,
) -> AppResult<()> {
    sqlx::query("UPDATE environments SET is_active = 0")
        .execute(pool)
        .await?;

    if let Some(environment_id) = environment_id {
        let result = sqlx::query("UPDATE environments SET is_active = 1 WHERE id = ?1")
            .bind(environment_id)
            .execute(pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::Message("Environment not found.".to_string()));
        }
    }

    Ok(())
}

pub async fn get_active_environment(
    pool: &SqlitePool,
    secret_store: Arc<dyn SecretStore>,
) -> AppResult<Option<EnvironmentDetail>> {
    let row = sqlx::query(
        "SELECT id, name, is_active, variables_json, updated_at FROM environments WHERE is_active = 1 LIMIT 1",
    )
    .fetch_optional(pool)
    .await?;

    match row {
        Some(row) => {
            let record = record_from_row(row)?;
            Ok(Some(
                hydrate_environment_detail(secret_store, record).await?,
            ))
        }
        None => Ok(None),
    }
}

pub fn resolve_request(
    payload: &SendRequestPayload,
    active_environment: Option<&EnvironmentDetail>,
) -> ResolvedRequestPayload {
    let variables = build_variable_map(active_environment);

    let (name, name_secret) = resolve_string(&payload.name, &variables);
    let (url, url_secret) = resolve_string(&payload.url, &variables);

    let mut secret_usage = RequestSecretUsage {
        name: name_secret,
        url: url_secret,
        ..RequestSecretUsage::default()
    };

    let query_params = payload
        .query_params
        .iter()
        .map(|item| {
            let (key, key_secret) = resolve_string(&item.key, &variables);
            let (value, value_secret) = resolve_string(&item.value, &variables);

            if key_secret || value_secret {
                secret_usage.query_param_ids.insert(item.id.clone());
            }

            KeyValueRow {
                id: item.id.clone(),
                key,
                value,
                enabled: item.enabled,
            }
        })
        .collect();

    let headers = payload
        .headers
        .iter()
        .map(|item| {
            let (key, key_secret) = resolve_string(&item.key, &variables);
            let (value, value_secret) = resolve_string(&item.value, &variables);

            if key_secret || value_secret {
                secret_usage.header_ids.insert(item.id.clone());
            }

            KeyValueRow {
                id: item.id.clone(),
                key,
                value,
                enabled: item.enabled,
            }
        })
        .collect();

    let (body_raw, body_raw_secret) = resolve_string(&payload.body.raw, &variables);
    secret_usage.body_raw = body_raw_secret;

    let body_form = payload
        .body
        .form
        .iter()
        .map(|item| {
            let (key, key_secret) = resolve_string(&item.key, &variables);
            let (value, value_secret) = resolve_string(&item.value, &variables);

            if key_secret || value_secret {
                secret_usage.body_form_ids.insert(item.id.clone());
            }

            KeyValueRow {
                id: item.id.clone(),
                key,
                value,
                enabled: item.enabled,
            }
        })
        .collect();

    let body_files = payload
        .body
        .files
        .iter()
        .map(|file| {
            let (name, name_secret) = resolve_string(&file.name, &variables);
            let (path, path_secret) = resolve_string(&file.path, &variables);

            if name_secret || path_secret {
                secret_usage.body_file_ids.insert(file.id.clone());
            }

            FileRow {
                id: file.id.clone(),
                name,
                path,
                enabled: file.enabled,
            }
        })
        .collect();

    let (basic_username, auth_basic_username) =
        resolve_string(&payload.auth.basic_username, &variables);
    let (basic_password, auth_basic_password) =
        resolve_string(&payload.auth.basic_password, &variables);
    let (bearer_token, auth_bearer_token) = resolve_string(&payload.auth.bearer_token, &variables);
    let (api_key_name, auth_api_key_name) = resolve_string(&payload.auth.api_key_name, &variables);
    let (api_key_value, auth_api_key_value) =
        resolve_string(&payload.auth.api_key_value, &variables);

    secret_usage.auth_basic_username = auth_basic_username;
    secret_usage.auth_basic_password = auth_basic_password;
    secret_usage.auth_bearer_token = auth_bearer_token;
    secret_usage.auth_api_key_name = auth_api_key_name;
    secret_usage.auth_api_key_value = auth_api_key_value;

    ResolvedRequestPayload {
        payload: SendRequestPayload {
            name,
            method: payload.method.clone(),
            url,
            query_params,
            headers,
            body: RequestBody {
                mode: payload.body.mode.clone(),
                raw: body_raw,
                form: body_form,
                files: body_files,
            },
            auth: RequestAuth {
                auth_type: payload.auth.auth_type.clone(),
                basic_username,
                basic_password,
                bearer_token,
                api_key_name,
                api_key_value,
                api_key_in: payload.auth.api_key_in.clone(),
            },
            pre_request_script: payload.pre_request_script.clone(),
            test_script: payload.test_script.clone(),
        },
        secret_usage,
    }
}

pub fn redact_secret_history_payload(
    original: &SendRequestPayload,
    resolved: &SendRequestPayload,
    usage: &RequestSecretUsage,
) -> SendRequestPayload {
    SendRequestPayload {
        name: if usage.name {
            original.name.clone()
        } else {
            resolved.name.clone()
        },
        method: resolved.method.clone(),
        url: if usage.url {
            original.url.clone()
        } else {
            resolved.url.clone()
        },
        query_params: resolved
            .query_params
            .iter()
            .map(|row| {
                if usage.query_param_ids.contains(&row.id) {
                    original
                        .query_params
                        .iter()
                        .find(|item| item.id == row.id)
                        .cloned()
                        .unwrap_or_else(|| row.clone())
                } else {
                    row.clone()
                }
            })
            .collect(),
        headers: resolved
            .headers
            .iter()
            .map(|row| {
                if usage.header_ids.contains(&row.id) {
                    original
                        .headers
                        .iter()
                        .find(|item| item.id == row.id)
                        .cloned()
                        .unwrap_or_else(|| row.clone())
                } else {
                    row.clone()
                }
            })
            .collect(),
        body: RequestBody {
            mode: resolved.body.mode.clone(),
            raw: if usage.body_raw {
                original.body.raw.clone()
            } else {
                resolved.body.raw.clone()
            },
            form: resolved
                .body
                .form
                .iter()
                .map(|row| {
                    if usage.body_form_ids.contains(&row.id) {
                        original
                            .body
                            .form
                            .iter()
                            .find(|item| item.id == row.id)
                            .cloned()
                            .unwrap_or_else(|| row.clone())
                    } else {
                        row.clone()
                    }
                })
                .collect(),
            files: resolved
                .body
                .files
                .iter()
                .map(|row| {
                    if usage.body_file_ids.contains(&row.id) {
                        original
                            .body
                            .files
                            .iter()
                            .find(|item| item.id == row.id)
                            .cloned()
                            .unwrap_or_else(|| row.clone())
                    } else {
                        row.clone()
                    }
                })
                .collect(),
        },
        auth: RequestAuth {
            auth_type: resolved.auth.auth_type.clone(),
            basic_username: if usage.auth_basic_username {
                original.auth.basic_username.clone()
            } else {
                resolved.auth.basic_username.clone()
            },
            basic_password: if usage.auth_basic_password {
                original.auth.basic_password.clone()
            } else {
                resolved.auth.basic_password.clone()
            },
            bearer_token: if usage.auth_bearer_token {
                original.auth.bearer_token.clone()
            } else {
                resolved.auth.bearer_token.clone()
            },
            api_key_name: if usage.auth_api_key_name {
                original.auth.api_key_name.clone()
            } else {
                resolved.auth.api_key_name.clone()
            },
            api_key_value: if usage.auth_api_key_value {
                original.auth.api_key_value.clone()
            } else {
                resolved.auth.api_key_value.clone()
            },
            api_key_in: resolved.auth.api_key_in.clone(),
        },
        pre_request_script: original.pre_request_script.clone(),
        test_script: original.test_script.clone(),
    }
}

async fn fetch_environment_record(
    pool: &SqlitePool,
    environment_id: &str,
) -> AppResult<EnvironmentRecord> {
    fetch_environment_record_optional(pool, environment_id)
        .await?
        .ok_or_else(|| AppError::Message("Environment not found.".to_string()))
}

async fn fetch_environment_record_optional(
    pool: &SqlitePool,
    environment_id: &str,
) -> AppResult<Option<EnvironmentRecord>> {
    let row = sqlx::query(
        "SELECT id, name, is_active, variables_json, updated_at FROM environments WHERE id = ?1",
    )
    .bind(environment_id)
    .fetch_optional(pool)
    .await?;

    row.map(record_from_row).transpose()
}

fn record_from_row(row: sqlx::sqlite::SqliteRow) -> AppResult<EnvironmentRecord> {
    Ok(EnvironmentRecord {
        id: row.get("id"),
        name: row.get("name"),
        is_active: row.get::<i64, _>("is_active") != 0,
        variables: decode_environment_variables(&row.get::<String, _>("variables_json"))?,
        updated_at: row.get("updated_at"),
    })
}

async fn hydrate_environment_detail(
    secret_store: Arc<dyn SecretStore>,
    record: EnvironmentRecord,
) -> AppResult<EnvironmentDetail> {
    let environment_id = record.id.clone();
    let variables =
        hydrate_environment_variables(secret_store, environment_id, record.variables).await?;

    Ok(EnvironmentDetail {
        id: record.id,
        name: record.name,
        is_active: record.is_active,
        variables,
        updated_at: record.updated_at,
    })
}

async fn hydrate_environment_variables(
    secret_store: Arc<dyn SecretStore>,
    environment_id: String,
    variables: Vec<EnvironmentVariable>,
) -> AppResult<Vec<EnvironmentVariable>> {
    tokio::task::spawn_blocking(move || {
        let mut hydrated = Vec::with_capacity(variables.len());

        for mut variable in variables {
            if variable.is_secret {
                variable.value = secret_store
                    .get_environment_variable_secret(&environment_id, &variable.id)?
                    .ok_or_else(|| {
                        AppError::Message(format!(
                            "A secure secret value for '{}' is missing from the system credential store.",
                            variable.key
                        ))
                    })?;
            }

            hydrated.push(variable);
        }

        Ok(hydrated)
    })
    .await
    .map_err(|error| AppError::Message(error.to_string()))?
}

async fn load_secret_snapshot(
    secret_store: Arc<dyn SecretStore>,
    environment_id: String,
    variables: &[EnvironmentVariable],
) -> AppResult<HashMap<String, String>> {
    let secret_rows: Vec<(String, String)> = variables
        .iter()
        .filter(|item| item.is_secret)
        .map(|item| (item.id.clone(), item.key.clone()))
        .collect();

    tokio::task::spawn_blocking(move || {
        let mut snapshot = HashMap::new();

        for (variable_id, key) in secret_rows {
            let value = secret_store
                .get_environment_variable_secret(&environment_id, &variable_id)?
                .ok_or_else(|| {
                    AppError::Message(format!(
                        "A secure secret value for '{}' is missing from the system credential store.",
                        key
                    ))
                })?;
            snapshot.insert(variable_id, value);
        }

        Ok(snapshot)
    })
    .await
    .map_err(|error| AppError::Message(error.to_string()))?
}

async fn sync_secret_snapshot(
    secret_store: Arc<dyn SecretStore>,
    environment_id: String,
    snapshot: HashMap<String, String>,
    variable_ids: HashSet<String>,
) -> AppResult<()> {
    tokio::task::spawn_blocking(move || {
        for variable_id in variable_ids {
            match snapshot.get(&variable_id) {
                Some(value) => secret_store.set_environment_variable_secret(
                    &environment_id,
                    &variable_id,
                    value,
                )?,
                None => secret_store
                    .delete_environment_variable_secret(&environment_id, &variable_id)?,
            }
        }

        Ok(())
    })
    .await
    .map_err(|error| AppError::Message(error.to_string()))?
}

fn collect_secret_snapshot(variables: &[EnvironmentVariable]) -> HashMap<String, String> {
    variables
        .iter()
        .filter(|item| item.is_secret)
        .map(|item| (item.id.clone(), item.value.clone()))
        .collect()
}

fn secret_ids_union(
    existing: &[EnvironmentVariable],
    next: &[EnvironmentVariable],
) -> HashSet<String> {
    existing
        .iter()
        .chain(next.iter())
        .filter(|item| item.is_secret)
        .map(|item| item.id.clone())
        .collect()
}

fn strip_secret_values(variables: &[EnvironmentVariable]) -> Vec<EnvironmentVariable> {
    variables
        .iter()
        .cloned()
        .map(|mut item| {
            if item.is_secret {
                item.value.clear();
            }

            item
        })
        .collect()
}

fn decode_environment_variables(source: &str) -> AppResult<Vec<EnvironmentVariable>> {
    Ok(serde_json::from_str(source)?)
}

fn build_variable_map(
    active_environment: Option<&EnvironmentDetail>,
) -> HashMap<String, VariableValue> {
    active_environment
        .map(|environment| {
            environment
                .variables
                .iter()
                .filter(|item| item.enabled && !item.key.trim().is_empty())
                .map(|item| {
                    (
                        item.key.trim().to_string(),
                        VariableValue {
                            value: item.value.clone(),
                            is_secret: item.is_secret,
                        },
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

fn parse_dynamic_variable(key: &str) -> Option<DynamicVariableCall<'_>> {
    let trimmed = key.trim();

    if !trimmed.starts_with('$') {
        return None;
    }

    let Some(bracket_start) = trimmed.find('[') else {
        return Some(DynamicVariableCall {
            name: trimmed,
            argument: None,
        });
    };

    if !trimmed.ends_with(']') {
        return None;
    }

    let name = &trimmed[..bracket_start];
    let argument = trimmed[bracket_start + 1..trimmed.len() - 1]
        .parse::<usize>()
        .ok()?;

    Some(DynamicVariableCall {
        name,
        argument: Some(argument),
    })
}

fn random_u32() -> u32 {
    let bytes = Uuid::new_v4().into_bytes();
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

fn random_u16() -> u16 {
    let bytes = Uuid::new_v4().into_bytes();
    u16::from_le_bytes([bytes[0], bytes[1]])
}

fn random_byte() -> u8 {
    Uuid::new_v4().into_bytes()[0]
}

fn random_choice<'a>(items: &'a [&'a str]) -> &'a str {
    let index = (random_u32() as usize) % items.len();
    items[index]
}

fn random_alphanumeric(length: usize) -> String {
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";

    let mut output = String::with_capacity(length);

    while output.len() < length {
        for byte in Uuid::new_v4().into_bytes() {
            if output.len() == length {
                break;
            }

            output.push(CHARSET[(byte as usize) % CHARSET.len()] as char);
        }
    }

    output
}

fn random_hex_color() -> String {
    format!("#{:06x}", random_u32() % 0x0100_0000)
}

fn random_abbreviation() -> String {
    let length = 3 + (random_byte() as usize % 3);
    let mut output = String::with_capacity(length);

    while output.len() < length {
        for byte in Uuid::new_v4().into_bytes() {
            if output.len() == length {
                break;
            }

            output.push((b'A' + (byte % 26)) as char);
        }
    }

    output
}

fn random_ipv4() -> String {
    let bytes = Uuid::new_v4().into_bytes();
    format!("{}.{}.{}.{}", bytes[0], bytes[1], bytes[2], bytes[3])
}

fn random_ipv6() -> String {
    let bytes = Uuid::new_v4().into_bytes();
    bytes
        .chunks_exact(2)
        .map(|chunk| format!("{:02x}{:02x}", chunk[0], chunk[1]))
        .collect::<Vec<_>>()
        .join(":")
}

fn random_mac_address() -> String {
    let bytes = Uuid::new_v4().into_bytes();
    bytes[..6]
        .iter()
        .map(|byte| format!("{:02x}", byte))
        .collect::<Vec<_>>()
        .join(":")
}

fn random_password() -> String {
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!@#$%^&*";

    let mut output = String::with_capacity(15);

    while output.len() < 15 {
        for byte in Uuid::new_v4().into_bytes() {
            if output.len() == 15 {
                break;
            }

            output.push(CHARSET[(byte as usize) % CHARSET.len()] as char);
        }
    }

    output
}

fn resolve_dynamic_variable(key: &str) -> Option<String> {
    const COLORS: &[&str] = &[
        "red", "orange", "amber", "yellow", "lime", "green", "emerald", "teal", "cyan", "blue",
        "indigo", "violet", "pink",
    ];
    const LOCALES: &[&str] = &[
        "en", "en_US", "en_GB", "de", "es", "fr", "it", "ja", "ko", "nl", "pl", "pt_BR",
        "ru", "sv", "tr", "zh_CN",
    ];
    const USER_AGENTS: &[&str] = &[
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/135.0.0.0 Safari/537.36",
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 14_4) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.4 Safari/605.1.15",
        "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/134.0.0.0 Safari/537.36",
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:136.0) Gecko/20100101 Firefox/136.0",
    ];
    const PROTOCOLS: &[&str] = &["http", "https"];

    let parsed = parse_dynamic_variable(key)?;

    match parsed.name {
        "$guid" | "$randomUUID" if parsed.argument.is_none() => {
            Some(Uuid::new_v4().to_string())
        }
        "$timestamp" if parsed.argument.is_none() => Some(Utc::now().timestamp().to_string()),
        "$isoTimestamp" if parsed.argument.is_none() => Some(
            Utc::now()
                .to_rfc3339_opts(SecondsFormat::Millis, true),
        ),
        "$randomAlphaNumeric" => Some(random_alphanumeric(parsed.argument.unwrap_or(1))),
        "$randomBoolean" if parsed.argument.is_none() => Some((random_byte() % 2 == 0).to_string()),
        "$randomInt" if parsed.argument.is_none() => Some((random_u32() % 1001).to_string()),
        "$randomColor" if parsed.argument.is_none() => Some(random_choice(COLORS).to_string()),
        "$randomHexColor" if parsed.argument.is_none() => Some(random_hex_color()),
        "$randomAbbreviation" if parsed.argument.is_none() => Some(random_abbreviation()),
        "$randomIP" if parsed.argument.is_none() => Some(random_ipv4()),
        "$randomIPV6" if parsed.argument.is_none() => Some(random_ipv6()),
        "$randomMACAddress" if parsed.argument.is_none() => Some(random_mac_address()),
        "$randomPassword" if parsed.argument.is_none() => Some(random_password()),
        "$randomLocale" if parsed.argument.is_none() => Some(random_choice(LOCALES).to_string()),
        "$randomUserAgent" if parsed.argument.is_none() => {
            Some(random_choice(USER_AGENTS).to_string())
        }
        "$randomProtocol" if parsed.argument.is_none() => {
            Some(random_choice(PROTOCOLS).to_string())
        }
        "$randomSemver" if parsed.argument.is_none() => Some(format!(
            "{}.{}.{}",
            random_u16() % 10,
            random_u16() % 10,
            random_u16() % 10
        )),
        _ => None,
    }
}

fn resolve_string(input: &str, variables: &HashMap<String, VariableValue>) -> (String, bool) {
    let mut resolved = String::with_capacity(input.len());
    let mut rest = input;
    let mut used_secret = false;

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

        if let Some(value) = resolve_dynamic_variable(key) {
            resolved.push_str(&value);
        } else if let Some(value) = variables.get(key) {
            resolved.push_str(&value.value);
            used_secret |= value.is_secret;
        } else {
            resolved.push_str("{{");
            resolved.push_str(&after_start[..end]);
            resolved.push_str("}}");
        }

        rest = &after_start[end + 2..];
    }

    (resolved, used_secret)
}

fn now_iso() -> String {
    Utc::now().to_rfc3339()
}

#[cfg(test)]
mod tests {
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

        let (random_uuid, _) =
            environments_service::resolve_string("{{$randomUUID}}", &variables);
        assert!(Uuid::parse_str(&random_uuid).is_ok());

        let (timestamp, _) = environments_service::resolve_string("{{$timestamp}}", &variables);
        assert!(timestamp.parse::<i64>().is_ok());

        let (iso_timestamp, _) =
            environments_service::resolve_string("{{$isoTimestamp}}", &variables);
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
        assert!(random_hex_color[1..].chars().all(|ch| ch.is_ascii_hexdigit()));

        let (random_ip, _) = environments_service::resolve_string("{{$randomIP}}", &variables);
        let ipv4_parts: Vec<&str> = random_ip.split('.').collect();
        assert_eq!(ipv4_parts.len(), 4);
        assert!(ipv4_parts
            .iter()
            .all(|part| part.parse::<u8>().is_ok()));

        let (random_ipv6, _) = environments_service::resolve_string("{{$randomIPV6}}", &variables);
        let ipv6_parts: Vec<&str> = random_ipv6.split(':').collect();
        assert_eq!(ipv6_parts.len(), 8);
        assert!(ipv6_parts
            .iter()
            .all(|part| part.len() == 4 && part.chars().all(|ch| ch.is_ascii_hexdigit())));

        let (random_mac, _) =
            environments_service::resolve_string("{{$randomMACAddress}}", &variables);
        let mac_parts: Vec<&str> = random_mac.split(':').collect();
        assert_eq!(mac_parts.len(), 6);
        assert!(mac_parts
            .iter()
            .all(|part| part.len() == 2 && part.chars().all(|ch| ch.is_ascii_hexdigit())));

        let (random_protocol, _) =
            environments_service::resolve_string("{{$randomProtocol}}", &variables);
        assert!(matches!(random_protocol.as_str(), "http" | "https"));

        let (random_semver, _) =
            environments_service::resolve_string("{{$randomSemver}}", &variables);
        let semver_parts: Vec<&str> = random_semver.split('.').collect();
        assert_eq!(semver_parts.len(), 3);
        assert!(semver_parts
            .iter()
            .all(|part| part.parse::<u16>().is_ok()));
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
        assert_eq!(history_snapshot.pre_request_script, payload.pre_request_script);
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
        assert_ne!(resolved.payload.query_params[0].value, payload.query_params[0].value);
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
}
