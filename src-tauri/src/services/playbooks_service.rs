use chrono::Utc;
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use crate::{
    domain::{
        collections::SavedRequestDetail,
        playbooks::{
            AddPlaybookStepInput, CreatePlaybookRunInput, FinishPlaybookRunInput, PlaybookDetail,
            PlaybookExecutionContext, PlaybookFolderScripts, PlaybookInheritedScripts,
            PlaybookInput, PlaybookRunDetail, PlaybookRunStep, PlaybookRunSummary, PlaybookStep,
            PlaybookSummary, RecordPlaybookRunStepInput, ReorderPlaybookStepsInput,
            UpdatePlaybookStepInput,
        },
        requests::SendRequestPayload,
    },
    error::{AppError, AppResult},
    services::collections_service,
};

pub async fn list_playbooks(pool: &SqlitePool) -> AppResult<Vec<PlaybookSummary>> {
    let rows = sqlx::query(
        r#"
        SELECT
          playbooks.id,
          playbooks.name,
          playbooks.description,
          playbooks.default_delay_ms,
          playbooks.stop_on_failure,
          playbooks.fail_on_http_error,
          playbooks.updated_at,
          COUNT(playbook_steps.id) AS step_count
        FROM playbooks
        LEFT JOIN playbook_steps ON playbook_steps.playbook_id = playbooks.id
        GROUP BY playbooks.id
        ORDER BY playbooks.updated_at DESC, playbooks.name ASC
        "#,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(map_playbook_summary).collect())
}

pub async fn create_playbook(
    pool: &SqlitePool,
    input: &PlaybookInput,
) -> AppResult<PlaybookDetail> {
    let name = normalize_name(&input.name)?;
    validate_delay(input.default_delay_ms)?;

    let id = Uuid::new_v4().to_string();
    let now = now_iso();

    sqlx::query(
        r#"
        INSERT INTO playbooks (
          id, name, description, default_delay_ms, stop_on_failure,
          fail_on_http_error, created_at, updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
        "#,
    )
    .bind(&id)
    .bind(name)
    .bind(input.description.trim())
    .bind(input.default_delay_ms)
    .bind(bool_to_i64(input.stop_on_failure))
    .bind(bool_to_i64(input.fail_on_http_error))
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;

    get_playbook(pool, &id).await
}

pub async fn get_playbook(pool: &SqlitePool, playbook_id: &str) -> AppResult<PlaybookDetail> {
    let row = sqlx::query(
        r#"
        SELECT id, name, description, default_delay_ms, stop_on_failure,
               fail_on_http_error, updated_at
        FROM playbooks
        WHERE id = ?1
        "#,
    )
    .bind(playbook_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::Message("Playbook not found.".to_string()))?;

    Ok(PlaybookDetail {
        id: row.get("id"),
        name: row.get("name"),
        description: row.get("description"),
        default_delay_ms: row.get("default_delay_ms"),
        stop_on_failure: int_to_bool(row.get("stop_on_failure")),
        fail_on_http_error: int_to_bool(row.get("fail_on_http_error")),
        steps: list_playbook_steps(pool, playbook_id).await?,
        updated_at: row.get("updated_at"),
    })
}

pub async fn update_playbook(
    pool: &SqlitePool,
    playbook_id: &str,
    input: &PlaybookInput,
) -> AppResult<PlaybookDetail> {
    ensure_playbook_exists(pool, playbook_id).await?;
    let name = normalize_name(&input.name)?;
    validate_delay(input.default_delay_ms)?;
    let now = now_iso();

    sqlx::query(
        r#"
        UPDATE playbooks
        SET name = ?2,
            description = ?3,
            default_delay_ms = ?4,
            stop_on_failure = ?5,
            fail_on_http_error = ?6,
            updated_at = ?7
        WHERE id = ?1
        "#,
    )
    .bind(playbook_id)
    .bind(name)
    .bind(input.description.trim())
    .bind(input.default_delay_ms)
    .bind(bool_to_i64(input.stop_on_failure))
    .bind(bool_to_i64(input.fail_on_http_error))
    .bind(&now)
    .execute(pool)
    .await?;

    get_playbook(pool, playbook_id).await
}

pub async fn duplicate_playbook(pool: &SqlitePool, playbook_id: &str) -> AppResult<PlaybookDetail> {
    let source = get_playbook(pool, playbook_id).await?;
    let duplicate = create_playbook(
        pool,
        &PlaybookInput {
            name: format!("{} copy", source.name),
            description: source.description,
            default_delay_ms: source.default_delay_ms,
            stop_on_failure: source.stop_on_failure,
            fail_on_http_error: source.fail_on_http_error,
        },
    )
    .await?;

    for step in source.steps {
        if let Some(saved_request_id) = step.saved_request_id {
            add_playbook_step(
                pool,
                &duplicate.id,
                &AddPlaybookStepInput {
                    saved_request_id,
                    name_override: step.name_override,
                    notes: step.notes,
                    enabled: step.enabled,
                    delay_after_ms: step.delay_after_ms,
                },
            )
            .await?;
        }
    }

    get_playbook(pool, &duplicate.id).await
}

pub async fn delete_playbook(pool: &SqlitePool, playbook_id: &str) -> AppResult<()> {
    let result = sqlx::query("DELETE FROM playbooks WHERE id = ?1")
        .bind(playbook_id)
        .execute(pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::Message("Playbook not found.".to_string()));
    }

    Ok(())
}

pub async fn list_playbook_steps(
    pool: &SqlitePool,
    playbook_id: &str,
) -> AppResult<Vec<PlaybookStep>> {
    ensure_playbook_exists(pool, playbook_id).await?;

    let rows = sqlx::query(
        r#"
        SELECT
          playbook_steps.id,
          playbook_steps.playbook_id,
          playbook_steps.saved_request_id,
          playbook_steps.saved_request_name,
          playbook_steps.name_override,
          playbook_steps.notes,
          playbook_steps.enabled,
          playbook_steps.sort_order,
          playbook_steps.delay_after_ms,
          playbook_steps.updated_at,
          collections.name AS collection_name,
          collection_items.name AS live_request_name,
          collection_items.method,
          collection_items.url
        FROM playbook_steps
        LEFT JOIN collection_items
          ON collection_items.id = playbook_steps.saved_request_id
          AND collection_items.kind = 'request'
        LEFT JOIN collections
          ON collections.id = collection_items.collection_id
        WHERE playbook_steps.playbook_id = ?1
        ORDER BY playbook_steps.sort_order ASC, playbook_steps.created_at ASC
        "#,
    )
    .bind(playbook_id)
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(map_playbook_step).collect())
}

pub async fn add_playbook_step(
    pool: &SqlitePool,
    playbook_id: &str,
    input: &AddPlaybookStepInput,
) -> AppResult<PlaybookStep> {
    ensure_playbook_exists(pool, playbook_id).await?;
    validate_delay_option(input.delay_after_ms)?;

    let saved_request =
        collections_service::get_saved_request(pool, &input.saved_request_id).await?;
    let id = Uuid::new_v4().to_string();
    let now = now_iso();
    let sort_order = next_step_sort_order(pool, playbook_id).await?;

    sqlx::query(
        r#"
        INSERT INTO playbook_steps (
          id, playbook_id, saved_request_id, saved_request_name, name_override,
          notes, enabled, sort_order, delay_after_ms, created_at, updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
        "#,
    )
    .bind(&id)
    .bind(playbook_id)
    .bind(&saved_request.id)
    .bind(&saved_request.name)
    .bind(input.name_override.trim())
    .bind(input.notes.trim())
    .bind(bool_to_i64(input.enabled))
    .bind(sort_order)
    .bind(input.delay_after_ms)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;

    touch_playbook(pool, playbook_id).await?;
    get_playbook_step(pool, &id).await
}

pub async fn update_playbook_step(
    pool: &SqlitePool,
    step_id: &str,
    input: &UpdatePlaybookStepInput,
) -> AppResult<PlaybookStep> {
    validate_delay_option(input.delay_after_ms)?;
    let playbook_id = playbook_id_for_step(pool, step_id).await?;
    let now = now_iso();

    sqlx::query(
        r#"
        UPDATE playbook_steps
        SET name_override = ?2,
            notes = ?3,
            enabled = ?4,
            delay_after_ms = ?5,
            updated_at = ?6
        WHERE id = ?1
        "#,
    )
    .bind(step_id)
    .bind(input.name_override.trim())
    .bind(input.notes.trim())
    .bind(bool_to_i64(input.enabled))
    .bind(input.delay_after_ms)
    .bind(&now)
    .execute(pool)
    .await?;

    touch_playbook(pool, &playbook_id).await?;
    get_playbook_step(pool, step_id).await
}

pub async fn reorder_playbook_steps(
    pool: &SqlitePool,
    playbook_id: &str,
    input: &ReorderPlaybookStepsInput,
) -> AppResult<Vec<PlaybookStep>> {
    ensure_playbook_exists(pool, playbook_id).await?;
    let existing = list_playbook_steps(pool, playbook_id).await?;
    if existing.len() != input.step_ids.len() {
        return Err(AppError::Message(
            "Reorder payload must include every playbook step.".to_string(),
        ));
    }

    let existing_ids: std::collections::HashSet<String> =
        existing.into_iter().map(|step| step.id).collect();
    let requested_ids: std::collections::HashSet<String> = input.step_ids.iter().cloned().collect();
    if existing_ids != requested_ids {
        return Err(AppError::Message(
            "Reorder payload contains steps from another playbook.".to_string(),
        ));
    }

    let now = now_iso();
    for (index, step_id) in input.step_ids.iter().enumerate() {
        sqlx::query("UPDATE playbook_steps SET sort_order = ?2, updated_at = ?3 WHERE id = ?1")
            .bind(step_id)
            .bind(index as i64)
            .bind(&now)
            .execute(pool)
            .await?;
    }

    touch_playbook(pool, playbook_id).await?;
    list_playbook_steps(pool, playbook_id).await
}

pub async fn delete_playbook_step(pool: &SqlitePool, step_id: &str) -> AppResult<()> {
    let playbook_id = playbook_id_for_step(pool, step_id).await?;
    sqlx::query("DELETE FROM playbook_steps WHERE id = ?1")
        .bind(step_id)
        .execute(pool)
        .await?;
    touch_playbook(pool, &playbook_id).await?;
    renumber_steps(pool, &playbook_id).await
}

pub async fn get_playbook_execution_context(
    pool: &SqlitePool,
    step_id: &str,
) -> AppResult<PlaybookExecutionContext> {
    let row = sqlx::query(
        r#"
        SELECT saved_request_id
        FROM playbook_steps
        WHERE id = ?1
        "#,
    )
    .bind(step_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::Message("Playbook step not found.".to_string()))?;

    let saved_request_id: Option<String> = row.get("saved_request_id");
    let Some(saved_request_id) = saved_request_id else {
        return Err(AppError::Message(
            "This playbook step points to a deleted saved request.".to_string(),
        ));
    };

    let saved_request = collections_service::get_saved_request(pool, &saved_request_id).await?;
    let inherited_scripts = get_inherited_scripts(
        pool,
        &saved_request.collection_id,
        saved_request.parent_id.as_deref(),
    )
    .await?;

    Ok(PlaybookExecutionContext {
        step_id: step_id.to_string(),
        saved_request,
        inherited_scripts,
    })
}

pub async fn create_playbook_run(
    pool: &SqlitePool,
    input: &CreatePlaybookRunInput,
) -> AppResult<PlaybookRunSummary> {
    ensure_playbook_exists(pool, &input.playbook_id).await?;
    if input.total_steps < 0 {
        return Err(AppError::Message(
            "Total steps cannot be negative.".to_string(),
        ));
    }

    let id = Uuid::new_v4().to_string();
    let now = now_iso();

    sqlx::query(
        r#"
        INSERT INTO playbook_runs (
          id, playbook_id, status, started_at, total_steps
        ) VALUES (?1, ?2, 'running', ?3, ?4)
        "#,
    )
    .bind(&id)
    .bind(&input.playbook_id)
    .bind(&now)
    .bind(input.total_steps)
    .execute(pool)
    .await?;

    get_playbook_run_summary(pool, &id).await
}

pub async fn finish_playbook_run(
    pool: &SqlitePool,
    run_id: &str,
    input: &FinishPlaybookRunInput,
) -> AppResult<PlaybookRunSummary> {
    let status = input.status.trim();
    if !matches!(status, "passed" | "failed" | "canceled" | "running") {
        return Err(AppError::Message(
            "Unsupported playbook run status.".to_string(),
        ));
    }
    if input.total_duration_ms < 0 {
        return Err(AppError::Message(
            "Total duration cannot be negative.".to_string(),
        ));
    }

    let finished_at = if status == "running" {
        None
    } else {
        Some(now_iso())
    };

    sqlx::query(
        r#"
        UPDATE playbook_runs
        SET status = ?2,
            finished_at = ?3,
            total_duration_ms = ?4,
            stopped_reason = ?5
        WHERE id = ?1
        "#,
    )
    .bind(run_id)
    .bind(status)
    .bind(finished_at)
    .bind(input.total_duration_ms)
    .bind(input.stopped_reason.trim())
    .execute(pool)
    .await?;

    get_playbook_run_summary(pool, run_id).await
}

pub async fn record_playbook_run_step(
    pool: &SqlitePool,
    run_id: &str,
    input: &RecordPlaybookRunStepInput,
) -> AppResult<PlaybookRunStep> {
    ensure_run_exists(pool, run_id).await?;
    if !matches!(
        input.status.as_str(),
        "passed" | "failed" | "skipped" | "canceled"
    ) {
        return Err(AppError::Message(
            "Unsupported playbook step status.".to_string(),
        ));
    }

    let id = Uuid::new_v4().to_string();
    let now = now_iso();

    sqlx::query(
        r#"
        INSERT INTO playbook_run_steps (
          id, run_id, step_id, saved_request_id, saved_request_name, method, url,
          status, status_code, duration_ms, response_size_bytes, test_passed_count,
          test_failed_count, test_error_text, error_text, executed_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)
        "#,
    )
    .bind(&id)
    .bind(run_id)
    .bind(input.step_id.as_deref())
    .bind(input.saved_request_id.as_deref())
    .bind(input.saved_request_name.trim())
    .bind(input.method.trim())
    .bind(input.url.trim())
    .bind(input.status.trim())
    .bind(input.status_code)
    .bind(input.duration_ms)
    .bind(input.response_size_bytes)
    .bind(input.test_passed_count)
    .bind(input.test_failed_count)
    .bind(input.test_error_text.trim())
    .bind(input.error_text.trim())
    .bind(&now)
    .execute(pool)
    .await?;

    refresh_run_counts(pool, run_id).await?;
    get_playbook_run_step(pool, &id).await
}

pub async fn list_playbook_runs(
    pool: &SqlitePool,
    playbook_id: &str,
    limit: Option<i64>,
) -> AppResult<Vec<PlaybookRunSummary>> {
    ensure_playbook_exists(pool, playbook_id).await?;
    let limit = limit.unwrap_or(20).clamp(1, 100);
    let rows = sqlx::query(
        r#"
        SELECT id, playbook_id, status, started_at, finished_at, total_steps,
               passed_steps, failed_steps, skipped_steps, total_duration_ms, stopped_reason
        FROM playbook_runs
        WHERE playbook_id = ?1
        ORDER BY started_at DESC
        LIMIT ?2
        "#,
    )
    .bind(playbook_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(map_run_summary).collect())
}

pub async fn get_playbook_run(pool: &SqlitePool, run_id: &str) -> AppResult<PlaybookRunDetail> {
    let summary = get_playbook_run_summary(pool, run_id).await?;
    let rows = sqlx::query(
        r#"
        SELECT id, run_id, step_id, saved_request_id, saved_request_name, method,
               url, status, status_code, duration_ms, response_size_bytes,
               test_passed_count, test_failed_count, test_error_text, error_text, executed_at
        FROM playbook_run_steps
        WHERE run_id = ?1
        ORDER BY executed_at ASC
        "#,
    )
    .bind(run_id)
    .fetch_all(pool)
    .await?;

    Ok(PlaybookRunDetail {
        id: summary.id,
        playbook_id: summary.playbook_id,
        status: summary.status,
        started_at: summary.started_at,
        finished_at: summary.finished_at,
        total_steps: summary.total_steps,
        passed_steps: summary.passed_steps,
        failed_steps: summary.failed_steps,
        skipped_steps: summary.skipped_steps,
        total_duration_ms: summary.total_duration_ms,
        stopped_reason: summary.stopped_reason,
        steps: rows.into_iter().map(map_run_step).collect(),
    })
}

async fn get_playbook_step(pool: &SqlitePool, step_id: &str) -> AppResult<PlaybookStep> {
    let row = sqlx::query(
        r#"
        SELECT
          playbook_steps.id,
          playbook_steps.playbook_id,
          playbook_steps.saved_request_id,
          playbook_steps.saved_request_name,
          playbook_steps.name_override,
          playbook_steps.notes,
          playbook_steps.enabled,
          playbook_steps.sort_order,
          playbook_steps.delay_after_ms,
          playbook_steps.updated_at,
          collections.name AS collection_name,
          collection_items.name AS live_request_name,
          collection_items.method,
          collection_items.url
        FROM playbook_steps
        LEFT JOIN collection_items
          ON collection_items.id = playbook_steps.saved_request_id
          AND collection_items.kind = 'request'
        LEFT JOIN collections
          ON collections.id = collection_items.collection_id
        WHERE playbook_steps.id = ?1
        "#,
    )
    .bind(step_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::Message("Playbook step not found.".to_string()))?;

    Ok(map_playbook_step(row))
}

async fn get_playbook_run_summary(
    pool: &SqlitePool,
    run_id: &str,
) -> AppResult<PlaybookRunSummary> {
    let row = sqlx::query(
        r#"
        SELECT id, playbook_id, status, started_at, finished_at, total_steps,
               passed_steps, failed_steps, skipped_steps, total_duration_ms, stopped_reason
        FROM playbook_runs
        WHERE id = ?1
        "#,
    )
    .bind(run_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::Message("Playbook run not found.".to_string()))?;

    Ok(map_run_summary(row))
}

async fn get_playbook_run_step(pool: &SqlitePool, id: &str) -> AppResult<PlaybookRunStep> {
    let row = sqlx::query(
        r#"
        SELECT id, run_id, step_id, saved_request_id, saved_request_name, method,
               url, status, status_code, duration_ms, response_size_bytes,
               test_passed_count, test_failed_count, test_error_text, error_text, executed_at
        FROM playbook_run_steps
        WHERE id = ?1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::Message("Playbook run step not found.".to_string()))?;

    Ok(map_run_step(row))
}

async fn get_inherited_scripts(
    pool: &SqlitePool,
    collection_id: &str,
    parent_id: Option<&str>,
) -> AppResult<PlaybookInheritedScripts> {
    let collection_row =
        sqlx::query("SELECT prerequest_script, test_script FROM collections WHERE id = ?1")
            .bind(collection_id)
            .fetch_optional(pool)
            .await?
            .ok_or_else(|| AppError::Message("Collection not found.".to_string()))?;

    let mut folder_scripts = Vec::new();
    if let Some(parent_id) = parent_id {
        folder_scripts = folder_script_path(pool, collection_id, parent_id).await?;
    }

    Ok(PlaybookInheritedScripts {
        pre_request_script: collection_row.get("prerequest_script"),
        test_script: collection_row.get("test_script"),
        folder_scripts,
    })
}

async fn folder_script_path(
    pool: &SqlitePool,
    collection_id: &str,
    folder_id: &str,
) -> AppResult<Vec<PlaybookFolderScripts>> {
    let mut current_id = Some(folder_id.to_string());
    let mut reversed = Vec::new();

    while let Some(id) = current_id {
        let row = sqlx::query(
            r#"
            SELECT id, parent_id, name, prerequest_script, test_script
            FROM collection_items
            WHERE id = ?1 AND collection_id = ?2 AND kind = 'folder'
            "#,
        )
        .bind(&id)
        .bind(collection_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::Message("Collection folder not found.".to_string()))?;

        reversed.push(PlaybookFolderScripts {
            name: row.get("name"),
            pre_request_script: row.get("prerequest_script"),
            test_script: row.get("test_script"),
        });
        current_id = row.get("parent_id");
    }

    reversed.reverse();
    Ok(reversed)
}

async fn refresh_run_counts(pool: &SqlitePool, run_id: &str) -> AppResult<()> {
    let counts = sqlx::query(
        r#"
        SELECT
          SUM(CASE WHEN status = 'passed' THEN 1 ELSE 0 END) AS passed_steps,
          SUM(CASE WHEN status = 'failed' THEN 1 ELSE 0 END) AS failed_steps,
          SUM(CASE WHEN status = 'skipped' THEN 1 ELSE 0 END) AS skipped_steps
        FROM playbook_run_steps
        WHERE run_id = ?1
        "#,
    )
    .bind(run_id)
    .fetch_one(pool)
    .await?;

    let passed_steps: i64 = counts.get::<Option<i64>, _>("passed_steps").unwrap_or(0);
    let failed_steps: i64 = counts.get::<Option<i64>, _>("failed_steps").unwrap_or(0);
    let skipped_steps: i64 = counts.get::<Option<i64>, _>("skipped_steps").unwrap_or(0);

    sqlx::query(
        r#"
        UPDATE playbook_runs
        SET passed_steps = ?2,
            failed_steps = ?3,
            skipped_steps = ?4
        WHERE id = ?1
        "#,
    )
    .bind(run_id)
    .bind(passed_steps)
    .bind(failed_steps)
    .bind(skipped_steps)
    .execute(pool)
    .await?;

    Ok(())
}

async fn renumber_steps(pool: &SqlitePool, playbook_id: &str) -> AppResult<()> {
    let steps = list_playbook_steps(pool, playbook_id).await?;
    let now = now_iso();
    for (index, step) in steps.iter().enumerate() {
        sqlx::query("UPDATE playbook_steps SET sort_order = ?2, updated_at = ?3 WHERE id = ?1")
            .bind(&step.id)
            .bind(index as i64)
            .bind(&now)
            .execute(pool)
            .await?;
    }
    Ok(())
}

async fn ensure_playbook_exists(pool: &SqlitePool, playbook_id: &str) -> AppResult<()> {
    let exists: Option<String> = sqlx::query_scalar("SELECT id FROM playbooks WHERE id = ?1")
        .bind(playbook_id)
        .fetch_optional(pool)
        .await?;

    if exists.is_none() {
        return Err(AppError::Message("Playbook not found.".to_string()));
    }

    Ok(())
}

async fn ensure_run_exists(pool: &SqlitePool, run_id: &str) -> AppResult<()> {
    let exists: Option<String> = sqlx::query_scalar("SELECT id FROM playbook_runs WHERE id = ?1")
        .bind(run_id)
        .fetch_optional(pool)
        .await?;

    if exists.is_none() {
        return Err(AppError::Message("Playbook run not found.".to_string()));
    }

    Ok(())
}

async fn playbook_id_for_step(pool: &SqlitePool, step_id: &str) -> AppResult<String> {
    sqlx::query_scalar("SELECT playbook_id FROM playbook_steps WHERE id = ?1")
        .bind(step_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::Message("Playbook step not found.".to_string()))
}

async fn next_step_sort_order(pool: &SqlitePool, playbook_id: &str) -> AppResult<i64> {
    let next: Option<i64> =
        sqlx::query_scalar("SELECT MAX(sort_order) + 1 FROM playbook_steps WHERE playbook_id = ?1")
            .bind(playbook_id)
            .fetch_one(pool)
            .await?;

    Ok(next.unwrap_or(0))
}

async fn touch_playbook(pool: &SqlitePool, playbook_id: &str) -> AppResult<()> {
    sqlx::query("UPDATE playbooks SET updated_at = ?2 WHERE id = ?1")
        .bind(playbook_id)
        .bind(now_iso())
        .execute(pool)
        .await?;
    Ok(())
}

fn map_playbook_summary(row: sqlx::sqlite::SqliteRow) -> PlaybookSummary {
    PlaybookSummary {
        id: row.get("id"),
        name: row.get("name"),
        description: row.get("description"),
        default_delay_ms: row.get("default_delay_ms"),
        stop_on_failure: int_to_bool(row.get("stop_on_failure")),
        fail_on_http_error: int_to_bool(row.get("fail_on_http_error")),
        step_count: row.get("step_count"),
        updated_at: row.get("updated_at"),
    }
}

fn map_playbook_step(row: sqlx::sqlite::SqliteRow) -> PlaybookStep {
    let saved_request_id: Option<String> = row.get("saved_request_id");
    let live_request_name: Option<String> = row.get("live_request_name");
    let fallback_name: String = row.get("saved_request_name");
    let missing_saved_request = saved_request_id.is_none() || live_request_name.is_none();

    PlaybookStep {
        id: row.get("id"),
        playbook_id: row.get("playbook_id"),
        saved_request_id,
        saved_request_name: live_request_name.unwrap_or(fallback_name),
        collection_name: row.get("collection_name"),
        method: row.get("method"),
        url: row.get("url"),
        name_override: row.get("name_override"),
        notes: row.get("notes"),
        enabled: int_to_bool(row.get("enabled")),
        sort_order: row.get("sort_order"),
        delay_after_ms: row.get("delay_after_ms"),
        missing_saved_request,
        updated_at: row.get("updated_at"),
    }
}

fn map_run_summary(row: sqlx::sqlite::SqliteRow) -> PlaybookRunSummary {
    PlaybookRunSummary {
        id: row.get("id"),
        playbook_id: row.get("playbook_id"),
        status: row.get("status"),
        started_at: row.get("started_at"),
        finished_at: row.get("finished_at"),
        total_steps: row.get("total_steps"),
        passed_steps: row.get("passed_steps"),
        failed_steps: row.get("failed_steps"),
        skipped_steps: row.get("skipped_steps"),
        total_duration_ms: row.get("total_duration_ms"),
        stopped_reason: row.get("stopped_reason"),
    }
}

fn map_run_step(row: sqlx::sqlite::SqliteRow) -> PlaybookRunStep {
    PlaybookRunStep {
        id: row.get("id"),
        run_id: row.get("run_id"),
        step_id: row.get("step_id"),
        saved_request_id: row.get("saved_request_id"),
        saved_request_name: row.get("saved_request_name"),
        method: row.get("method"),
        url: row.get("url"),
        status: row.get("status"),
        status_code: row.get("status_code"),
        duration_ms: row.get("duration_ms"),
        response_size_bytes: row.get("response_size_bytes"),
        test_passed_count: row.get("test_passed_count"),
        test_failed_count: row.get("test_failed_count"),
        test_error_text: row.get("test_error_text"),
        error_text: row.get("error_text"),
        executed_at: row.get("executed_at"),
    }
}

fn normalize_name(name: &str) -> AppResult<&str> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::Message("Playbook name is required.".to_string()));
    }
    Ok(name)
}

fn validate_delay(value: i64) -> AppResult<()> {
    if !(0..=3_600_000).contains(&value) {
        return Err(AppError::Message(
            "Delay must be between 0 ms and 3,600,000 ms.".to_string(),
        ));
    }
    Ok(())
}

fn validate_delay_option(value: Option<i64>) -> AppResult<()> {
    if let Some(value) = value {
        validate_delay(value)?;
    }
    Ok(())
}

fn bool_to_i64(value: bool) -> i64 {
    if value {
        1
    } else {
        0
    }
}

fn int_to_bool(value: i64) -> bool {
    value != 0
}

fn now_iso() -> String {
    Utc::now().to_rfc3339()
}

#[allow(dead_code)]
fn _request_name(request: &SendRequestPayload) -> String {
    request.name.trim().to_string()
}

#[allow(dead_code)]
fn _saved_request_name(request: &SavedRequestDetail) -> String {
    request.name.trim().to_string()
}

#[cfg(test)]
#[path = "playbooks_service_tests.rs"]
mod tests;
