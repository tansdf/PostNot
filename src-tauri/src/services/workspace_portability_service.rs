use std::{
    collections::{BTreeSet, HashMap, HashSet},
    sync::Arc,
};

use chrono::{SecondsFormat, Utc};
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use crate::{
    domain::{
        environments::EnvironmentVariable,
        realtime::{
            BinaryPayloadSource, RealtimeMessageDraft, VersionedRealtimeConnection,
            VersionedRealtimeMessage, REALTIME_CONNECTION_SCHEMA_VERSION,
            REALTIME_MESSAGE_SCHEMA_VERSION,
        },
        requests::SendRequestPayload,
        workspace_portability::{
            ExportPortableWorkspaceInput, ImportedPortableRealtimeDraft,
            ImportedPortableRequestDraft, PortableCollection, PortableCollectionItem,
            PortableEnvironment, PortableEnvironmentVariable, PortablePlaybook,
            PortablePlaybookStep, PortableRealtimeConnection, PortableWorkspaceCounts,
            PortableWorkspaceDocument, PortableWorkspaceDrafts, PortableWorkspaceImportPreview,
            PortableWorkspaceImportResult, PortableWorkspaceProducer, WorkspaceRedaction,
            POSTNOT_WORKSPACE_SCHEMA, POSTNOT_WORKSPACE_VERSION,
        },
    },
    error::{AppError, AppResult},
    services::{
        collections_service, credential_redaction_service, realtime_connections_service,
        secret_store_service::SecretStore,
    },
};

const MAX_WORKSPACE_SOURCE_BYTES: usize = 64 * 1024 * 1024;
const SCRIPT_WARNING: &str = "Scripts are portable JavaScript and may make network requests or change active-environment variables when run. Review imported scripts before execution.";
const LOCAL_FILE_WARNING: &str = "Local file references are preserved as paths. Files are not embedded, so those references may need to be selected again on another device.";

#[derive(Debug, Clone)]
struct ImportedItemTarget {
    id: String,
    collection_id: String,
    parent_id: Option<String>,
    is_http: bool,
    is_message: bool,
}

pub async fn build_document(
    pool: &SqlitePool,
    input: &ExportPortableWorkspaceInput,
) -> AppResult<PortableWorkspaceDocument> {
    let mut transaction = pool.begin().await?;
    let collection_rows = sqlx::query(
        r#"
        SELECT id, name, description, prerequest_script, test_script
        FROM collections
        ORDER BY created_at ASC, id ASC
        "#,
    )
    .fetch_all(&mut *transaction)
    .await?;

    let mut collections = Vec::with_capacity(collection_rows.len());
    let mut collection_indexes = HashMap::new();
    for row in collection_rows {
        let export_id: String = row.get("id");
        collection_indexes.insert(export_id.clone(), collections.len());
        collections.push(PortableCollection {
            export_id,
            name: row.get("name"),
            description: row.get("description"),
            pre_request_script: row.get("prerequest_script"),
            test_script: row.get("test_script"),
            items: Vec::new(),
        });
    }

    let item_rows = sqlx::query(
        r#"
        SELECT id, collection_id, parent_id, kind, name, sort_order, method, url,
               query_params_json, headers_json, body_json, auth_json,
               prerequest_script, test_script, request_type, realtime_message_json
        FROM collection_items
        ORDER BY collection_id ASC, sort_order ASC, created_at ASC, id ASC
        "#,
    )
    .fetch_all(&mut *transaction)
    .await?;

    let mut redactions = Vec::new();
    let mut warnings = BTreeSet::new();
    let mut http_export_ids = HashSet::new();
    let mut message_export_ids = HashSet::new();
    for row in item_rows {
        let collection_id: String = row.get("collection_id");
        let Some(collection_index) = collection_indexes.get(&collection_id).copied() else {
            return Err(AppError::Message(
                "A collection item refers to a missing collection.".to_string(),
            ));
        };
        let export_id: String = row.get("id");
        let kind: String = row.get("kind");
        let parent_export_id: Option<String> = row.get("parent_id");
        let sort_order: i64 = row.get("sort_order");
        let item = if kind == "folder" {
            let pre_request_script: String = row.get("prerequest_script");
            let test_script: String = row.get("test_script");
            if !pre_request_script.trim().is_empty() || !test_script.trim().is_empty() {
                warnings.insert(SCRIPT_WARNING.to_string());
            }
            PortableCollectionItem::Folder {
                export_id,
                parent_export_id,
                sort_order,
                name: row.get("name"),
                pre_request_script,
                test_script,
            }
        } else {
            match row.get::<String, _>("request_type").as_str() {
                "http" => {
                    let mut request = SendRequestPayload {
                        name: row.get("name"),
                        method: row.get::<Option<String>, _>("method").unwrap_or_default(),
                        url: row.get::<Option<String>, _>("url").unwrap_or_default(),
                        query_params: serde_json::from_str(
                            &row.get::<String, _>("query_params_json"),
                        )?,
                        headers: serde_json::from_str(&row.get::<String, _>("headers_json"))?,
                        body: serde_json::from_str(&row.get::<String, _>("body_json"))?,
                        auth: serde_json::from_str(&row.get::<String, _>("auth_json"))?,
                        pre_request_script: row.get("prerequest_script"),
                        test_script: row.get("test_script"),
                    };
                    if credential_redaction_service::contains_local_files(&request) {
                        warnings.insert(LOCAL_FILE_WARNING.to_string());
                    }
                    if credential_redaction_service::has_scripts(&request) {
                        warnings.insert(SCRIPT_WARNING.to_string());
                    }
                    credential_redaction_service::redact_request(
                        &mut request,
                        "httpRequest",
                        &export_id,
                        &mut redactions,
                    );
                    http_export_ids.insert(export_id.clone());
                    PortableCollectionItem::Http {
                        export_id,
                        parent_export_id,
                        sort_order,
                        request: Box::new(request),
                    }
                }
                "websocket" | "socketio" => {
                    let source = row
                        .get::<Option<String>, _>("realtime_message_json")
                        .ok_or_else(|| {
                            AppError::Message(
                                "A saved realtime message is missing its payload.".to_string(),
                            )
                        })?;
                    let mut message: VersionedRealtimeMessage = serde_json::from_str(&source)?;
                    if realtime_message_contains_local_file(&message.message) {
                        warnings.insert(LOCAL_FILE_WARNING.to_string());
                    }
                    credential_redaction_service::redact_realtime_message(
                        &mut message.message,
                        "realtimeMessage",
                        &export_id,
                        &mut redactions,
                    )?;
                    message_export_ids.insert(export_id.clone());
                    PortableCollectionItem::Message {
                        export_id,
                        parent_export_id,
                        sort_order,
                        message,
                    }
                }
                value => {
                    return Err(AppError::Message(format!(
                        "Unsupported stored request type: {value}."
                    )))
                }
            }
        };
        collections[collection_index].items.push(item);
    }

    if collections.iter().any(|collection| {
        !collection.pre_request_script.trim().is_empty()
            || !collection.test_script.trim().is_empty()
    }) {
        warnings.insert(SCRIPT_WARNING.to_string());
    }

    let profile_rows = sqlx::query(
        "SELECT id, config_json FROM realtime_connections ORDER BY created_at ASC, id ASC",
    )
    .fetch_all(&mut *transaction)
    .await?;
    let mut realtime_connections = Vec::with_capacity(profile_rows.len());
    let mut profile_export_ids = HashSet::new();
    for row in profile_rows {
        let export_id: String = row.get("id");
        let mut connection: VersionedRealtimeConnection =
            serde_json::from_str(&row.get::<String, _>("config_json"))?;
        credential_redaction_service::redact_realtime_connection(
            &mut connection.connection,
            "realtimeConnection",
            &export_id,
            &mut redactions,
        )?;
        profile_export_ids.insert(export_id.clone());
        realtime_connections.push(PortableRealtimeConnection {
            export_id,
            connection,
        });
    }

    let environment_rows = sqlx::query(
        "SELECT id, name, variables_json FROM environments ORDER BY created_at ASC, id ASC",
    )
    .fetch_all(&mut *transaction)
    .await?;
    let mut environments = Vec::with_capacity(environment_rows.len());
    for row in environment_rows {
        let export_id: String = row.get("id");
        let variables: Vec<EnvironmentVariable> =
            serde_json::from_str(&row.get::<String, _>("variables_json"))?;
        let variables = variables
            .into_iter()
            .map(|variable| {
                if variable.is_secret {
                    redactions.push(WorkspaceRedaction {
                        resource_kind: "environmentVariable".to_string(),
                        resource_export_id: variable.id.clone(),
                        path: "value".to_string(),
                        reason: "Secret environment values stay in the system credential store and are never exported.".to_string(),
                    });
                }
                PortableEnvironmentVariable {
                    export_id: variable.id,
                    key: variable.key,
                    value: if variable.is_secret { String::new() } else { variable.value },
                    enabled: variable.enabled,
                    is_secret: variable.is_secret,
                }
            })
            .collect();
        environments.push(PortableEnvironment {
            export_id,
            name: row.get("name"),
            variables,
        });
    }

    let playbook_rows = sqlx::query(
        r#"
        SELECT id, name, description, default_delay_ms, stop_on_failure, fail_on_http_error
        FROM playbooks
        ORDER BY created_at ASC, id ASC
        "#,
    )
    .fetch_all(&mut *transaction)
    .await?;
    let mut playbooks = Vec::with_capacity(playbook_rows.len());
    for row in playbook_rows {
        let export_id: String = row.get("id");
        let step_rows = sqlx::query(
            r#"
            SELECT id, saved_request_id, saved_request_name, name_override, notes, enabled,
                   sort_order, delay_after_ms
            FROM playbook_steps
            WHERE playbook_id = ?1
            ORDER BY sort_order ASC, created_at ASC, id ASC
            "#,
        )
        .bind(&export_id)
        .fetch_all(&mut *transaction)
        .await?;
        let steps = step_rows
            .into_iter()
            .map(|step| {
                let saved_request_export_id: Option<String> = step.get("saved_request_id");
                PortablePlaybookStep {
                    export_id: step.get("id"),
                    saved_request_export_id: saved_request_export_id
                        .filter(|id| http_export_ids.contains(id)),
                    saved_request_name: step.get("saved_request_name"),
                    name_override: step.get("name_override"),
                    notes: step.get("notes"),
                    enabled: step.get::<i64, _>("enabled") != 0,
                    sort_order: step.get("sort_order"),
                    delay_after_ms: step.get("delay_after_ms"),
                }
            })
            .collect();
        playbooks.push(PortablePlaybook {
            export_id,
            name: row.get("name"),
            description: row.get("description"),
            default_delay_ms: row.get("default_delay_ms"),
            stop_on_failure: row.get::<i64, _>("stop_on_failure") != 0,
            fail_on_http_error: row.get::<i64, _>("fail_on_http_error") != 0,
            steps,
        });
    }
    transaction.commit().await?;

    let drafts = if input.include_open_drafts {
        redact_and_normalize_drafts(
            input.drafts.clone(),
            &http_export_ids,
            &message_export_ids,
            &profile_export_ids,
            &mut redactions,
            &mut warnings,
        )?
    } else {
        PortableWorkspaceDrafts::default()
    };

    Ok(PortableWorkspaceDocument {
        schema: POSTNOT_WORKSPACE_SCHEMA.to_string(),
        version: POSTNOT_WORKSPACE_VERSION,
        exported_at: now_iso(),
        exported_by: PortableWorkspaceProducer {
            application: "PostNot".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        },
        collections,
        realtime_connections,
        environments,
        playbooks,
        drafts,
        redactions,
        warnings: warnings.into_iter().collect(),
    })
}

pub fn serialize_document(document: &PortableWorkspaceDocument) -> AppResult<String> {
    Ok(serde_json::to_string_pretty(document)?)
}

pub fn inspect_source(source: &str) -> AppResult<PortableWorkspaceImportPreview> {
    let document = parse_and_validate(source)?;
    Ok(PortableWorkspaceImportPreview {
        version: document.version,
        exported_at: document.exported_at.clone(),
        exported_by_version: document.exported_by.version.clone(),
        counts: counts_for_document(&document),
        redaction_count: document.redactions.len(),
        credential_fields_requiring_input: credential_redactions(&document).len(),
        warnings: document.warnings.clone(),
    })
}

pub async fn import_source(
    pool: &SqlitePool,
    secret_store: Arc<dyn SecretStore>,
    source: &str,
    include_open_drafts: bool,
) -> AppResult<PortableWorkspaceImportResult> {
    let document = parse_and_validate(source)?;
    let counts = counts_for_document(&document);
    let credential_fields_requiring_input = credential_redactions(&document);
    let now = now_iso();
    let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await?;

    let mut profile_targets = HashMap::new();
    let mut reused_realtime_connection_count = 0;
    for profile in &document.realtime_connections {
        let name = profile.connection.connection.common().name.trim();
        let protocol = profile.connection.connection.protocol().as_str();
        let config_json = serde_json::to_string(&profile.connection)?;
        let existing = sqlx::query_scalar::<_, String>(
            r#"
            SELECT id FROM realtime_connections
            WHERE name = ?1 AND protocol = ?2 AND config_json = ?3
            LIMIT 1
            "#,
        )
        .bind(name)
        .bind(protocol)
        .bind(&config_json)
        .fetch_optional(&mut *transaction)
        .await?;
        let target_id = match existing {
            Some(id) => {
                reused_realtime_connection_count += 1;
                id
            }
            None => {
                let id = Uuid::new_v4().to_string();
                sqlx::query(
                    r#"
                    INSERT INTO realtime_connections
                        (id, name, protocol, config_json, created_at, updated_at)
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                    "#,
                )
                .bind(&id)
                .bind(name)
                .bind(protocol)
                .bind(&config_json)
                .bind(&now)
                .bind(&now)
                .execute(&mut *transaction)
                .await?;
                id
            }
        };
        profile_targets.insert(profile.export_id.clone(), target_id);
    }

    let mut collection_targets = HashMap::new();
    for collection in &document.collections {
        let id = Uuid::new_v4().to_string();
        sqlx::query(
            r#"
            INSERT INTO collections
                (id, name, description, prerequest_script, test_script, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
        )
        .bind(&id)
        .bind(collection.name.trim())
        .bind(&collection.description)
        .bind(&collection.pre_request_script)
        .bind(&collection.test_script)
        .bind(&now)
        .bind(&now)
        .execute(&mut *transaction)
        .await?;
        collection_targets.insert(collection.export_id.clone(), id);
    }

    let mut item_targets = HashMap::new();
    for collection in &document.collections {
        let collection_id = collection_targets
            .get(&collection.export_id)
            .expect("validated collection target")
            .clone();
        for item in &collection.items {
            item_targets.insert(
                item.export_id().to_string(),
                ImportedItemTarget {
                    id: Uuid::new_v4().to_string(),
                    collection_id: collection_id.clone(),
                    parent_id: item.parent_export_id().map(ToOwned::to_owned),
                    is_http: matches!(item, PortableCollectionItem::Http { .. }),
                    is_message: matches!(item, PortableCollectionItem::Message { .. }),
                },
            );
        }
    }

    for collection in &document.collections {
        let mut items = collection.items.iter().collect::<Vec<_>>();
        items.sort_by_key(|item| item_depth(item, &collection.items));
        for item in items {
            insert_collection_item(&mut transaction, item, &item_targets, &now).await?;
        }
    }

    let mut secret_targets = Vec::new();
    for environment in &document.environments {
        let environment_id = Uuid::new_v4().to_string();
        let mut variables = Vec::with_capacity(environment.variables.len());
        for variable in &environment.variables {
            let variable_id = Uuid::new_v4().to_string();
            variables.push(EnvironmentVariable {
                id: variable_id.clone(),
                key: variable.key.clone(),
                value: if variable.is_secret {
                    String::new()
                } else {
                    variable.value.clone()
                },
                enabled: variable.enabled,
                is_secret: variable.is_secret,
            });
            if variable.is_secret {
                secret_targets.push((environment_id.clone(), variable_id));
            }
        }
        sqlx::query(
            r#"
            INSERT INTO environments
                (id, name, is_active, variables_json, created_at, updated_at)
            VALUES (?1, ?2, 0, ?3, ?4, ?5)
            "#,
        )
        .bind(&environment_id)
        .bind(environment.name.trim())
        .bind(serde_json::to_string(&variables)?)
        .bind(&now)
        .bind(&now)
        .execute(&mut *transaction)
        .await?;
    }

    for playbook in &document.playbooks {
        let playbook_id = Uuid::new_v4().to_string();
        sqlx::query(
            r#"
            INSERT INTO playbooks
                (id, name, description, default_delay_ms, stop_on_failure,
                 fail_on_http_error, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            "#,
        )
        .bind(&playbook_id)
        .bind(playbook.name.trim())
        .bind(&playbook.description)
        .bind(playbook.default_delay_ms)
        .bind(i64::from(playbook.stop_on_failure))
        .bind(i64::from(playbook.fail_on_http_error))
        .bind(&now)
        .bind(&now)
        .execute(&mut *transaction)
        .await?;
        for step in &playbook.steps {
            let saved_request_id = step
                .saved_request_export_id
                .as_ref()
                .and_then(|id| item_targets.get(id))
                .filter(|target| target.is_http)
                .map(|target| target.id.clone());
            sqlx::query(
                r#"
                INSERT INTO playbook_steps
                    (id, playbook_id, saved_request_id, saved_request_name, name_override,
                     notes, enabled, sort_order, delay_after_ms, created_at, updated_at)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
                "#,
            )
            .bind(Uuid::new_v4().to_string())
            .bind(&playbook_id)
            .bind(saved_request_id)
            .bind(&step.saved_request_name)
            .bind(&step.name_override)
            .bind(&step.notes)
            .bind(i64::from(step.enabled))
            .bind(step.sort_order)
            .bind(step.delay_after_ms)
            .bind(&now)
            .bind(&now)
            .execute(&mut *transaction)
            .await?;
        }
    }

    let request_drafts = if include_open_drafts {
        document
            .drafts
            .requests
            .iter()
            .map(|draft| {
                let target = draft
                    .saved_request_export_id
                    .as_ref()
                    .and_then(|id| item_targets.get(id))
                    .filter(|target| target.is_http);
                ImportedPortableRequestDraft {
                    saved_request_id: target.map(|item| item.id.clone()),
                    collection_id: target.map(|item| item.collection_id.clone()),
                    parent_id: target.and_then(|item| remap_parent(item, &item_targets)),
                    request: draft.request.clone(),
                }
            })
            .collect()
    } else {
        Vec::new()
    };
    let realtime_drafts = if include_open_drafts {
        document
            .drafts
            .realtime
            .iter()
            .map(|draft| {
                let target = draft
                    .selected_message_export_id
                    .as_ref()
                    .and_then(|id| item_targets.get(id))
                    .filter(|target| target.is_message);
                ImportedPortableRealtimeDraft {
                    selected_profile_id: draft
                        .selected_profile_export_id
                        .as_ref()
                        .and_then(|id| profile_targets.get(id))
                        .cloned(),
                    selected_message_id: target.map(|item| item.id.clone()),
                    collection_id: target.map(|item| item.collection_id.clone()),
                    parent_id: target.and_then(|item| remap_parent(item, &item_targets)),
                    connection: draft.connection.clone(),
                    message: draft.message.clone(),
                }
            })
            .collect()
    } else {
        Vec::new()
    };

    if let Err(error) = write_blank_secrets(secret_store.clone(), &secret_targets).await {
        transaction.rollback().await?;
        return Err(error);
    }
    if let Err(error) = transaction.commit().await {
        remove_secrets(secret_store, &secret_targets).await;
        return Err(error.into());
    }

    Ok(PortableWorkspaceImportResult {
        counts,
        reused_realtime_connection_count,
        credential_fields_requiring_input,
        request_drafts,
        realtime_drafts,
        warnings: document.warnings,
    })
}

pub fn counts_for_document(document: &PortableWorkspaceDocument) -> PortableWorkspaceCounts {
    let mut counts = PortableWorkspaceCounts {
        collections: document.collections.len(),
        realtime_connections: document.realtime_connections.len(),
        environments: document.environments.len(),
        environment_variables: document
            .environments
            .iter()
            .map(|environment| environment.variables.len())
            .sum(),
        playbooks: document.playbooks.len(),
        playbook_steps: document
            .playbooks
            .iter()
            .map(|playbook| playbook.steps.len())
            .sum(),
        request_drafts: document.drafts.requests.len(),
        realtime_drafts: document.drafts.realtime.len(),
        ..PortableWorkspaceCounts::default()
    };
    for item in document
        .collections
        .iter()
        .flat_map(|collection| &collection.items)
    {
        match item {
            PortableCollectionItem::Folder { .. } => counts.folders += 1,
            PortableCollectionItem::Http { .. } => counts.http_requests += 1,
            PortableCollectionItem::Message { .. } => counts.realtime_messages += 1,
        }
    }
    counts
}

fn parse_and_validate(source: &str) -> AppResult<PortableWorkspaceDocument> {
    if source.len() > MAX_WORKSPACE_SOURCE_BYTES {
        return Err(AppError::Message(
            "Portable workspace files are limited to 64 MiB.".to_string(),
        ));
    }
    let document: PortableWorkspaceDocument = serde_json::from_str(source).map_err(|error| {
        AppError::Message(format!(
            "This is not a valid portable workspace file. {error}"
        ))
    })?;
    validate_document(&document)?;
    Ok(document)
}

fn validate_document(document: &PortableWorkspaceDocument) -> AppResult<()> {
    if document.schema != POSTNOT_WORKSPACE_SCHEMA {
        return Err(AppError::Message(
            "This JSON file is not a PostNot portable workspace.".to_string(),
        ));
    }
    if document.version != POSTNOT_WORKSPACE_VERSION {
        return Err(AppError::Message(format!(
            "Unsupported portable workspace version: {}.",
            document.version
        )));
    }
    if document.exported_by.application != "PostNot" {
        return Err(AppError::Message(
            "This portable workspace was not produced by PostNot.".to_string(),
        ));
    }
    chrono::DateTime::parse_from_rfc3339(&document.exported_at).map_err(|_| {
        AppError::Message(
            "Portable workspace exportedAt must be an RFC 3339 timestamp.".to_string(),
        )
    })?;
    if document.exported_by.version.trim().is_empty() {
        return Err(AppError::Message(
            "Portable workspace producer version cannot be empty.".to_string(),
        ));
    }

    let collection_ids = unique_ids(
        document
            .collections
            .iter()
            .map(|collection| collection.export_id.as_str()),
        "collection",
    )?;
    let mut http_ids = HashSet::new();
    let mut message_ids = HashSet::new();
    let mut all_item_ids = HashSet::new();
    for collection in &document.collections {
        require_name(&collection.name, "Collection")?;
        let item_ids = collection
            .items
            .iter()
            .map(PortableCollectionItem::export_id)
            .collect::<Vec<_>>();
        for id in &item_ids {
            if !all_item_ids.insert((*id).to_string()) {
                return Err(AppError::Message(format!(
                    "Duplicate collection item export ID: {id}."
                )));
            }
        }
        let folder_ids = collection
            .items
            .iter()
            .filter_map(|item| match item {
                PortableCollectionItem::Folder { export_id, .. } => Some(export_id.as_str()),
                _ => None,
            })
            .collect::<HashSet<_>>();
        let parent_map = collection
            .items
            .iter()
            .map(|item| (item.export_id(), item.parent_export_id()))
            .collect::<HashMap<_, _>>();
        for item in &collection.items {
            if let Some(parent_id) = item.parent_export_id() {
                if !folder_ids.contains(parent_id) {
                    return Err(AppError::Message(format!(
                        "Collection item '{}' has a missing or non-folder parent.",
                        item.export_id()
                    )));
                }
                let mut cursor = Some(parent_id);
                let mut visited = HashSet::new();
                while let Some(id) = cursor {
                    if id == item.export_id() || !visited.insert(id) {
                        return Err(AppError::Message(
                            "A portable collection contains a folder cycle.".to_string(),
                        ));
                    }
                    cursor = parent_map.get(id).copied().flatten();
                }
            }
            match item {
                PortableCollectionItem::Folder { name, .. } => require_name(name, "Folder")?,
                PortableCollectionItem::Http {
                    export_id, request, ..
                } => {
                    if request.method.trim().is_empty() {
                        return Err(AppError::Message(format!(
                            "HTTP request '{export_id}' has no method."
                        )));
                    }
                    http_ids.insert(export_id.clone());
                }
                PortableCollectionItem::Message {
                    export_id, message, ..
                } => {
                    validate_versioned_message(message)?;
                    message_ids.insert(export_id.clone());
                }
            }
        }
    }
    debug_assert_eq!(collection_ids.len(), document.collections.len());

    let profile_ids = unique_ids(
        document
            .realtime_connections
            .iter()
            .map(|profile| profile.export_id.as_str()),
        "realtime connection",
    )?;
    for profile in &document.realtime_connections {
        if profile.connection.version != REALTIME_CONNECTION_SCHEMA_VERSION {
            return Err(AppError::Message(format!(
                "Unsupported realtime connection version: {}.",
                profile.connection.version
            )));
        }
        realtime_connections_service::validate_connection(&profile.connection.connection)?;
    }

    unique_ids(
        document
            .environments
            .iter()
            .map(|environment| environment.export_id.as_str()),
        "environment",
    )?;
    let mut variable_ids = HashSet::new();
    for environment in &document.environments {
        require_name(&environment.name, "Environment")?;
        for variable in &environment.variables {
            if variable.export_id.trim().is_empty()
                || !variable_ids.insert(variable.export_id.clone())
            {
                return Err(AppError::Message(format!(
                    "Duplicate or empty environment-variable export ID: {}.",
                    variable.export_id
                )));
            }
            if variable.is_secret && !variable.value.is_empty() {
                return Err(AppError::Message(format!(
                    "Secret environment variable '{}' must not contain a value in a portable workspace.",
                    variable.key
                )));
            }
        }
    }

    unique_ids(
        document
            .playbooks
            .iter()
            .map(|playbook| playbook.export_id.as_str()),
        "playbook",
    )?;
    let mut step_ids = HashSet::new();
    for playbook in &document.playbooks {
        require_name(&playbook.name, "Playbook")?;
        if playbook.default_delay_ms < 0 {
            return Err(AppError::Message(
                "Playbook default delays cannot be negative.".to_string(),
            ));
        }
        for step in &playbook.steps {
            if step.export_id.trim().is_empty() || !step_ids.insert(step.export_id.clone()) {
                return Err(AppError::Message(format!(
                    "Duplicate or empty playbook-step export ID: {}.",
                    step.export_id
                )));
            }
            if step.delay_after_ms.is_some_and(|delay| delay < 0) {
                return Err(AppError::Message(
                    "Playbook step delays cannot be negative.".to_string(),
                ));
            }
            if step
                .saved_request_export_id
                .as_ref()
                .is_some_and(|id| !http_ids.contains(id))
            {
                return Err(AppError::Message(
                    "A playbook step refers to a missing HTTP request.".to_string(),
                ));
            }
        }
    }

    for draft in &document.drafts.requests {
        if draft
            .saved_request_export_id
            .as_ref()
            .is_some_and(|id| !http_ids.contains(id))
        {
            return Err(AppError::Message(
                "An open request draft refers to a missing saved request.".to_string(),
            ));
        }
    }
    for draft in &document.drafts.realtime {
        if draft
            .selected_profile_export_id
            .as_ref()
            .is_some_and(|id| !profile_ids.contains(id))
        {
            return Err(AppError::Message(
                "An open realtime draft refers to a missing connection profile.".to_string(),
            ));
        }
        if draft
            .selected_message_export_id
            .as_ref()
            .is_some_and(|id| !message_ids.contains(id))
        {
            return Err(AppError::Message(
                "An open realtime draft refers to a missing saved message.".to_string(),
            ));
        }
        realtime_connections_service::validate_connection(&draft.connection)?;
        collections_service::validate_realtime_message(&draft.message)?;
        if draft.connection.protocol() != draft.message.protocol() {
            return Err(AppError::Message(
                "An open realtime draft has mismatched connection and message protocols."
                    .to_string(),
            ));
        }
    }
    Ok(())
}

fn validate_versioned_message(message: &VersionedRealtimeMessage) -> AppResult<()> {
    if message.version != REALTIME_MESSAGE_SCHEMA_VERSION {
        return Err(AppError::Message(format!(
            "Unsupported realtime message version: {}.",
            message.version
        )));
    }
    collections_service::validate_realtime_message(&message.message)
}

fn unique_ids<'a>(
    ids: impl Iterator<Item = &'a str>,
    resource_name: &str,
) -> AppResult<HashSet<String>> {
    let mut unique = HashSet::new();
    for id in ids {
        if id.trim().is_empty() || !unique.insert(id.to_string()) {
            return Err(AppError::Message(format!(
                "Duplicate or empty {resource_name} export ID: {id}."
            )));
        }
    }
    Ok(unique)
}

fn require_name(name: &str, resource_name: &str) -> AppResult<()> {
    if name.trim().is_empty() {
        return Err(AppError::Message(format!(
            "{resource_name} names cannot be empty."
        )));
    }
    Ok(())
}

fn credential_redactions(document: &PortableWorkspaceDocument) -> Vec<WorkspaceRedaction> {
    let mut entries = document.redactions.clone();
    for environment in &document.environments {
        for variable in &environment.variables {
            if variable.is_secret
                && !entries.iter().any(|entry| {
                    entry.resource_kind == "environmentVariable"
                        && entry.resource_export_id == variable.export_id
                        && entry.path == "value"
                })
            {
                entries.push(WorkspaceRedaction {
                    resource_kind: "environmentVariable".to_string(),
                    resource_export_id: variable.export_id.clone(),
                    path: "value".to_string(),
                    reason: "Secret environment values must be entered on this device.".to_string(),
                });
            }
        }
    }
    entries
}

fn redact_and_normalize_drafts(
    mut drafts: PortableWorkspaceDrafts,
    http_ids: &HashSet<String>,
    message_ids: &HashSet<String>,
    profile_ids: &HashSet<String>,
    redactions: &mut Vec<WorkspaceRedaction>,
    warnings: &mut BTreeSet<String>,
) -> AppResult<PortableWorkspaceDrafts> {
    for (index, draft) in drafts.requests.iter_mut().enumerate() {
        if draft
            .saved_request_export_id
            .as_ref()
            .is_some_and(|id| !http_ids.contains(id))
        {
            draft.saved_request_export_id = None;
        }
        if credential_redaction_service::contains_local_files(&draft.request) {
            warnings.insert(LOCAL_FILE_WARNING.to_string());
        }
        if credential_redaction_service::has_scripts(&draft.request) {
            warnings.insert(SCRIPT_WARNING.to_string());
        }
        credential_redaction_service::redact_request(
            &mut draft.request,
            "requestDraft",
            &format!("request-draft-{index}"),
            redactions,
        );
    }
    for (index, draft) in drafts.realtime.iter_mut().enumerate() {
        if draft
            .selected_profile_export_id
            .as_ref()
            .is_some_and(|id| !profile_ids.contains(id))
        {
            draft.selected_profile_export_id = None;
        }
        if draft
            .selected_message_export_id
            .as_ref()
            .is_some_and(|id| !message_ids.contains(id))
        {
            draft.selected_message_export_id = None;
        }
        credential_redaction_service::redact_realtime_connection(
            &mut draft.connection,
            "realtimeDraft",
            &format!("realtime-draft-{index}"),
            redactions,
        )?;
        credential_redaction_service::redact_realtime_message(
            &mut draft.message,
            "realtimeDraft",
            &format!("realtime-draft-{index}"),
            redactions,
        )?;
        if realtime_message_contains_local_file(&draft.message) {
            warnings.insert(LOCAL_FILE_WARNING.to_string());
        }
    }
    Ok(drafts)
}

async fn insert_collection_item(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    item: &PortableCollectionItem,
    targets: &HashMap<String, ImportedItemTarget>,
    now: &str,
) -> AppResult<()> {
    let target = targets
        .get(item.export_id())
        .expect("validated collection item target");
    let parent_id = item
        .parent_export_id()
        .and_then(|id| targets.get(id))
        .map(|parent| parent.id.clone());
    match item {
        PortableCollectionItem::Folder {
            sort_order,
            name,
            pre_request_script,
            test_script,
            ..
        } => {
            sqlx::query(
                r#"
                INSERT INTO collection_items
                    (id, collection_id, parent_id, kind, name, sort_order, method, url,
                     query_params_json, headers_json, body_json, auth_json,
                     prerequest_script, test_script, request_type, realtime_message_json,
                     created_at, updated_at)
                VALUES (?1, ?2, ?3, 'folder', ?4, ?5, NULL, NULL,
                        '[]', '[]', '{}', '{}', ?6, ?7, 'http', NULL, ?8, ?9)
                "#,
            )
            .bind(&target.id)
            .bind(&target.collection_id)
            .bind(parent_id)
            .bind(name.trim())
            .bind(sort_order)
            .bind(pre_request_script)
            .bind(test_script)
            .bind(now)
            .bind(now)
            .execute(&mut **transaction)
            .await?;
        }
        PortableCollectionItem::Http {
            sort_order,
            request,
            ..
        } => {
            sqlx::query(
                r#"
                INSERT INTO collection_items
                    (id, collection_id, parent_id, kind, name, sort_order, method, url,
                     query_params_json, headers_json, body_json, auth_json,
                     prerequest_script, test_script, request_type, realtime_message_json,
                     created_at, updated_at)
                VALUES (?1, ?2, ?3, 'request', ?4, ?5, ?6, ?7,
                        ?8, ?9, ?10, ?11, ?12, ?13, 'http', NULL, ?14, ?15)
                "#,
            )
            .bind(&target.id)
            .bind(&target.collection_id)
            .bind(parent_id)
            .bind(portable_request_name(request))
            .bind(sort_order)
            .bind(&request.method)
            .bind(&request.url)
            .bind(serde_json::to_string(&request.query_params)?)
            .bind(serde_json::to_string(&request.headers)?)
            .bind(serde_json::to_string(&request.body)?)
            .bind(serde_json::to_string(&request.auth)?)
            .bind(&request.pre_request_script)
            .bind(&request.test_script)
            .bind(now)
            .bind(now)
            .execute(&mut **transaction)
            .await?;
        }
        PortableCollectionItem::Message {
            sort_order,
            message,
            ..
        } => {
            sqlx::query(
                r#"
                INSERT INTO collection_items
                    (id, collection_id, parent_id, kind, name, sort_order, method, url,
                     query_params_json, headers_json, body_json, auth_json,
                     prerequest_script, test_script, request_type, realtime_message_json,
                     created_at, updated_at)
                VALUES (?1, ?2, ?3, 'request', ?4, ?5, NULL, NULL,
                        '[]', '[]', '{}', '{}', '', '', ?6, ?7, ?8, ?9)
                "#,
            )
            .bind(&target.id)
            .bind(&target.collection_id)
            .bind(parent_id)
            .bind(message.message.name().trim())
            .bind(sort_order)
            .bind(message.message.protocol().as_str())
            .bind(serde_json::to_string(message)?)
            .bind(now)
            .bind(now)
            .execute(&mut **transaction)
            .await?;
        }
    }
    Ok(())
}

fn item_depth(item: &PortableCollectionItem, items: &[PortableCollectionItem]) -> usize {
    let parents = items
        .iter()
        .map(|candidate| (candidate.export_id(), candidate.parent_export_id()))
        .collect::<HashMap<_, _>>();
    let mut depth = 0;
    let mut cursor = item.parent_export_id();
    while let Some(id) = cursor {
        depth += 1;
        cursor = parents.get(id).copied().flatten();
    }
    depth
}

fn remap_parent(
    target: &ImportedItemTarget,
    targets: &HashMap<String, ImportedItemTarget>,
) -> Option<String> {
    target
        .parent_id
        .as_ref()
        .and_then(|id| targets.get(id))
        .map(|parent| parent.id.clone())
}

fn portable_request_name(request: &SendRequestPayload) -> String {
    if request.name.trim().is_empty() {
        format!("{} {}", request.method, request.url)
            .trim()
            .to_string()
    } else {
        request.name.trim().to_string()
    }
}

fn realtime_message_contains_local_file(message: &RealtimeMessageDraft) -> bool {
    let binary = match message {
        RealtimeMessageDraft::Websocket { composer, .. } => composer.binary.as_ref(),
        RealtimeMessageDraft::Socketio { composer, .. } => composer.binary.as_ref(),
    };
    matches!(binary, Some(BinaryPayloadSource::File { path }) if !path.trim().is_empty())
}

async fn write_blank_secrets(
    secret_store: Arc<dyn SecretStore>,
    targets: &[(String, String)],
) -> AppResult<()> {
    let targets = targets.to_vec();
    tokio::task::spawn_blocking(move || {
        let mut written: Vec<(String, String)> = Vec::new();
        for (environment_id, variable_id) in &targets {
            if let Err(error) =
                secret_store.set_environment_variable_secret(environment_id, variable_id, "")
            {
                for (written_environment_id, written_variable_id) in written {
                    let _ = secret_store.delete_environment_variable_secret(
                        &written_environment_id,
                        &written_variable_id,
                    );
                }
                return Err(error);
            }
            written.push((environment_id.clone(), variable_id.clone()));
        }
        Ok(())
    })
    .await?
}

async fn remove_secrets(secret_store: Arc<dyn SecretStore>, targets: &[(String, String)]) {
    let targets = targets.to_vec();
    if let Err(error) = tokio::task::spawn_blocking(move || {
        for (environment_id, variable_id) in targets {
            let _ = secret_store.delete_environment_variable_secret(&environment_id, &variable_id);
        }
    })
    .await
    {
        log::warn!("Failed to remove imported secret placeholders after rollback: {error}");
    }
}

fn now_iso() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

#[cfg(test)]
mod tests {
    use std::{path::PathBuf, sync::Arc};

    use super::*;
    use crate::{
        db,
        domain::{
            realtime::{
                RealtimeConnectionCommon, RealtimeConnectionDraft, VersionedRealtimeConnection,
            },
            requests::{KeyValueRow, RequestAuth, RequestBody},
            workspace_portability::{PortableRequestDraft, PortableWorkspaceDrafts},
        },
        services::{environments_service, secret_store_service::InMemorySecretStore},
    };

    #[test]
    fn rejects_wrong_schema_before_import() {
        let source = r#"{"$schema":"wrong","version":1,"exportedAt":"now","exportedBy":{"application":"PostNot","version":"1"},"collections":[],"realtimeConnections":[],"environments":[],"playbooks":[]}"#;
        assert!(inspect_source(source)
            .unwrap_err()
            .to_string()
            .contains("not a PostNot portable workspace"));
    }

    #[tokio::test]
    async fn round_trip_is_additive_redacted_and_remaps_drafts() {
        let database_path = temporary_database_path();
        let pool = db::init_path(&database_path)
            .await
            .expect("initialize test database");
        let now = now_iso();
        let request = SendRequestPayload {
            name: "Portable request".to_string(),
            method: "GET".to_string(),
            url: "https://example.test/items?token=literal".to_string(),
            query_params: Vec::new(),
            headers: vec![KeyValueRow {
                id: "header-1".to_string(),
                key: "Authorization".to_string(),
                value: "Bearer literal".to_string(),
                enabled: true,
            }],
            body: RequestBody::default(),
            auth: RequestAuth::default(),
            pre_request_script: "pn.variables.set('portable', 'yes');".to_string(),
            test_script: String::new(),
        };
        sqlx::query(
            "INSERT INTO collections (id, name, description, prerequest_script, test_script, created_at, updated_at) VALUES ('collection-1', 'Portable', '', '', '', ?1, ?2)",
        )
        .bind(&now)
        .bind(&now)
        .execute(&pool)
        .await
        .expect("insert collection");
        sqlx::query(
            r#"
            INSERT INTO collection_items
                (id, collection_id, parent_id, kind, name, sort_order, method, url,
                 query_params_json, headers_json, body_json, auth_json, prerequest_script,
                 test_script, request_type, realtime_message_json, created_at, updated_at)
            VALUES ('request-1', 'collection-1', NULL, 'request', ?1, 0, ?2, ?3,
                    ?4, ?5, ?6, ?7, ?8, ?9, 'http', NULL, ?10, ?11)
            "#,
        )
        .bind(&request.name)
        .bind(&request.method)
        .bind(&request.url)
        .bind(serde_json::to_string(&request.query_params).unwrap())
        .bind(serde_json::to_string(&request.headers).unwrap())
        .bind(serde_json::to_string(&request.body).unwrap())
        .bind(serde_json::to_string(&request.auth).unwrap())
        .bind(&request.pre_request_script)
        .bind(&request.test_script)
        .bind(&now)
        .bind(&now)
        .execute(&pool)
        .await
        .expect("insert request");

        let connection = RealtimeConnectionDraft::Websocket {
            common: RealtimeConnectionCommon {
                name: "Portable socket".to_string(),
                url: "wss://example.test/socket".to_string(),
                query_params: Vec::new(),
                headers: Vec::new(),
                auth: RequestAuth::default(),
                reconnect: Default::default(),
            },
            subprotocols: Vec::new(),
        };
        let versioned_connection = VersionedRealtimeConnection::new(connection.clone());
        sqlx::query(
            "INSERT INTO realtime_connections (id, name, protocol, config_json, created_at, updated_at) VALUES ('profile-1', 'Portable socket', 'websocket', ?1, ?2, ?3)",
        )
        .bind(serde_json::to_string(&versioned_connection).unwrap())
        .bind(&now)
        .bind(&now)
        .execute(&pool)
        .await
        .expect("insert profile");

        let variables = vec![EnvironmentVariable {
            id: "variable-1".to_string(),
            key: "api_secret".to_string(),
            value: String::new(),
            enabled: true,
            is_secret: true,
        }];
        sqlx::query(
            "INSERT INTO environments (id, name, is_active, variables_json, created_at, updated_at) VALUES ('environment-1', 'Portable environment', 0, ?1, ?2, ?3)",
        )
        .bind(serde_json::to_string(&variables).unwrap())
        .bind(&now)
        .bind(&now)
        .execute(&pool)
        .await
        .expect("insert environment");

        sqlx::query(
            "INSERT INTO playbooks (id, name, description, default_delay_ms, stop_on_failure, fail_on_http_error, created_at, updated_at) VALUES ('playbook-1', 'Portable playbook', '', 0, 1, 1, ?1, ?2)",
        )
        .bind(&now)
        .bind(&now)
        .execute(&pool)
        .await
        .expect("insert playbook");
        sqlx::query(
            "INSERT INTO playbook_steps (id, playbook_id, saved_request_id, saved_request_name, name_override, notes, enabled, sort_order, delay_after_ms, created_at, updated_at) VALUES ('step-1', 'playbook-1', 'request-1', 'Portable request', '', '', 1, 0, NULL, ?1, ?2)",
        )
        .bind(&now)
        .bind(&now)
        .execute(&pool)
        .await
        .expect("insert playbook step");

        let document = build_document(
            &pool,
            &ExportPortableWorkspaceInput {
                include_open_drafts: true,
                drafts: PortableWorkspaceDrafts {
                    requests: vec![PortableRequestDraft {
                        saved_request_export_id: Some("request-1".to_string()),
                        request: request.clone(),
                    }],
                    realtime: Vec::new(),
                },
            },
        )
        .await
        .expect("build portable workspace");
        assert!(document.redactions.len() >= 3);
        assert!(document.environments[0].variables[0].value.is_empty());
        let exported_request = match &document.collections[0].items[0] {
            PortableCollectionItem::Http { request, .. } => request,
            _ => panic!("expected HTTP request"),
        };
        assert!(exported_request.headers[0].value.is_empty());
        assert!(exported_request.url.contains("token="));
        let source = serialize_document(&document).expect("serialize portable workspace");

        let secret_store = Arc::new(InMemorySecretStore::default());
        let result = import_source(&pool, secret_store.clone(), &source, true)
            .await
            .expect("import portable workspace");
        assert_eq!(result.reused_realtime_connection_count, 1);
        assert_eq!(result.request_drafts.len(), 1);
        assert_ne!(
            result.request_drafts[0].saved_request_id.as_deref(),
            Some("request-1")
        );
        assert_eq!(table_count(&pool, "collections").await, 2);
        assert_eq!(table_count(&pool, "collection_items").await, 2);
        assert_eq!(table_count(&pool, "realtime_connections").await, 1);
        assert_eq!(table_count(&pool, "environments").await, 2);
        assert_eq!(table_count(&pool, "playbooks").await, 2);

        let imported_environment_id: String =
            sqlx::query_scalar("SELECT id FROM environments WHERE id <> 'environment-1' LIMIT 1")
                .fetch_one(&pool)
                .await
                .expect("find imported environment");
        let imported_environment =
            environments_service::get_environment(&pool, secret_store, &imported_environment_id)
                .await
                .expect("hydrate imported environment");
        assert!(imported_environment.variables[0].value.is_empty());

        pool.close().await;
        cleanup_database(&database_path);
    }

    async fn table_count(pool: &SqlitePool, table: &str) -> i64 {
        sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table}"))
            .fetch_one(pool)
            .await
            .expect("count table")
    }

    fn temporary_database_path() -> PathBuf {
        std::env::temp_dir().join(format!("postnot-workspace-test-{}.sqlite", Uuid::new_v4()))
    }

    fn cleanup_database(path: &std::path::Path) {
        for candidate in [
            path.to_path_buf(),
            PathBuf::from(format!("{}-wal", path.display())),
            PathBuf::from(format!("{}-shm", path.display())),
        ] {
            let _ = std::fs::remove_file(candidate);
        }
    }
}
