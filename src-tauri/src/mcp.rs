use std::{
    collections::HashSet,
    path::PathBuf,
    sync::{Arc, RwLock},
};

use rmcp::{
    handler::server::wrapper::Parameters, schemars::JsonSchema, tool, tool_handler, tool_router,
    transport::stdio, ErrorData as McpError, ServerHandler, ServiceExt,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use crate::{
    db,
    domain::{
        activity::{AgentActor, NewAgentActivity},
        collections::{
            CreateCollectionFolderInput, CreateCollectionInput, SavedRealtimeRequestDetail,
        },
        realtime::{RawMessageMode, RealtimeRequestDraft, RequestType},
        requests::SendRequestPayload,
    },
    error::{AppError, AppResult},
    services::{
        activity_service, collections_service, environments_service, request_preview_service,
        secret_store_service, settings_service,
    },
    storage::paths,
};

const REDACTED: &str = "***";

#[derive(Debug, Clone, Default)]
pub struct McpOptions {
    pub data_dir: Option<PathBuf>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct CollectionIdParams {
    collection_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct SearchParams {
    query: String,
    limit: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct RequestIdParams {
    request_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct CreateCollectionParams {
    name: String,
    description: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct CreateFolderParams {
    collection_id: String,
    parent_id: Option<String>,
    name: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct CreateRequestParams {
    collection_id: String,
    parent_id: Option<String>,
    request: SendRequestPayload,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct CreateRequestsParams {
    collection_id: String,
    parent_id: Option<String>,
    requests: Vec<SendRequestPayload>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct UpdateRequestParams {
    request_id: String,
    expected_updated_at: String,
    request: SendRequestPayload,
    #[serde(default)]
    preserve_redacted_fields: Vec<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct CreateRealtimeRequestParams {
    collection_id: String,
    parent_id: Option<String>,
    request: RealtimeRequestDraft,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct UpdateRealtimeRequestParams {
    request_id: String,
    expected_updated_at: String,
    request: RealtimeRequestDraft,
    #[serde(default)]
    preserve_redacted_fields: Vec<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct DeleteRealtimeRequestParams {
    request_id: String,
    expected_updated_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SafeSavedRequest {
    id: String,
    collection_id: String,
    parent_id: Option<String>,
    name: String,
    updated_at: String,
    request: SendRequestPayload,
    redacted_fields: Vec<String>,
    warnings: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SafeSavedRealtimeRequest {
    id: String,
    collection_id: String,
    parent_id: Option<String>,
    name: String,
    request_type: RequestType,
    updated_at: String,
    request: RealtimeRequestDraft,
    redacted_fields: Vec<String>,
    warnings: Vec<String>,
}

struct MutationDetails<'a> {
    collection_id: Option<&'a str>,
    fields: &'a [&'a str],
    warnings: Vec<String>,
}

#[derive(Clone)]
struct PostNotMcp {
    pool: SqlitePool,
    actor: Arc<RwLock<AgentActor>>,
    tool_router: rmcp::handler::server::router::tool::ToolRouter<Self>,
}

#[tool_router]
impl PostNotMcp {
    fn new(pool: SqlitePool) -> Self {
        Self {
            pool,
            actor: Arc::new(RwLock::new(AgentActor {
                name: "mcp-client".to_string(),
                version: String::new(),
                session_id: Uuid::new_v4().to_string(),
            })),
            tool_router: Self::tool_router(),
        }
    }

    #[tool(description = "List PostNot collections with request counts and stable IDs.")]
    async fn list_collections(&self) -> Result<String, McpError> {
        json_result(collections_service::list_collections(&self.pool).await)
    }

    #[tool(description = "Get the nested folder and saved-request tree for one collection.")]
    async fn get_collection(
        &self,
        Parameters(params): Parameters<CollectionIdParams>,
    ) -> Result<String, McpError> {
        json_result(
            collections_service::list_collection_items(&self.pool, &params.collection_id).await,
        )
    }

    #[tool(
        description = "Search PostNot collections, folders, request names, methods, URLs, and breadcrumbs."
    )]
    async fn search_collection_items(
        &self,
        Parameters(params): Parameters<SearchParams>,
    ) -> Result<String, McpError> {
        json_result(
            collections_service::search_collection_entities(
                &self.pool,
                &params.query,
                params.limit,
            )
            .await,
        )
    }

    #[tool(
        description = "Read a saved request. Credential-looking literals are returned as *** and listed in redactedFields."
    )]
    async fn get_saved_request(
        &self,
        Parameters(params): Parameters<RequestIdParams>,
    ) -> Result<String, McpError> {
        let detail = collections_service::get_saved_request(&self.pool, &params.request_id)
            .await
            .map_err(to_mcp_error)?;
        let safe = redact_saved_request(detail);
        serde_json::to_string_pretty(&safe).map_err(json_mcp_error)
    }

    #[tool(
        description = "List saved raw WebSocket and Socket.IO definitions in one collection. This authoring tool never connects or sends traffic."
    )]
    async fn list_realtime_requests(
        &self,
        Parameters(params): Parameters<CollectionIdParams>,
    ) -> Result<String, McpError> {
        json_result(
            collections_service::list_saved_realtime_requests(&self.pool, &params.collection_id)
                .await,
        )
    }

    #[tool(
        description = "Read a saved raw WebSocket or Socket.IO definition without connecting. Credential-looking literals are returned as *** and listed in redactedFields."
    )]
    async fn get_realtime_request(
        &self,
        Parameters(params): Parameters<RequestIdParams>,
    ) -> Result<String, McpError> {
        let detail =
            collections_service::get_saved_realtime_request(&self.pool, &params.request_id)
                .await
                .map_err(to_mcp_error)?;
        let safe = redact_saved_realtime_request(detail);
        serde_json::to_string_pretty(&safe).map_err(json_mcp_error)
    }

    #[tool(
        description = "Read active environment variable context. Secret values are always omitted; non-secret values are included."
    )]
    async fn get_environment_context(&self) -> Result<String, McpError> {
        let row = sqlx::query("SELECT id, name, is_active, variables_json, updated_at FROM environments WHERE is_active = 1 LIMIT 1")
            .fetch_optional(&self.pool)
            .await
            .map_err(|error| to_mcp_error(error.into()))?;
        let Some(row) = row else {
            return Ok("null".to_string());
        };
        let variables: Vec<crate::domain::environments::EnvironmentVariable> =
            serde_json::from_str(&row.get::<String, _>("variables_json"))
                .map_err(json_mcp_error)?;
        let variables: Vec<Value> = variables
            .into_iter()
            .map(|variable| {
                let mut value = json!({
                    "id": variable.id, "key": variable.key, "enabled": variable.enabled,
                    "isSecret": variable.is_secret
                });
                if !variable.is_secret {
                    value["value"] = Value::String(variable.value);
                }
                value
            })
            .collect();
        Ok(json!({
            "id": row.get::<String, _>("id"), "name": row.get::<String, _>("name"),
            "isActive": true, "updatedAt": row.get::<String, _>("updated_at"), "variables": variables
        }).to_string())
    }

    #[tool(
        description = "Build PostNot's canonical masked preview for a saved request without executing scripts or network traffic."
    )]
    async fn preview_saved_request(
        &self,
        Parameters(params): Parameters<RequestIdParams>,
    ) -> Result<String, McpError> {
        let detail = collections_service::get_saved_request(&self.pool, &params.request_id)
            .await
            .map_err(to_mcp_error)?;
        let settings = settings_service::get_settings(&self.pool)
            .await
            .map_err(to_mcp_error)?;
        let environment = environments_service::get_active_environment(
            &self.pool,
            secret_store_service::default_secret_store(),
        )
        .await
        .map_err(to_mcp_error)?;
        let resolved = environments_service::resolve_request(&detail.request, environment.as_ref());
        let preview = request_preview_service::build_request_preview(
            &detail.request,
            &resolved.payload,
            &resolved.secret_usage,
            &settings,
            environment.as_ref(),
        )
        .map_err(to_mcp_error)?;
        serde_json::to_string_pretty(&preview).map_err(json_mcp_error)
    }

    #[tool(description = "Create a PostNot collection. Existing collections are never modified.")]
    async fn create_collection(
        &self,
        Parameters(params): Parameters<CreateCollectionParams>,
    ) -> Result<String, McpError> {
        let batch = Uuid::new_v4().to_string();
        let result = collections_service::create_collection(
            &self.pool,
            &CreateCollectionInput {
                name: params.name,
                description: params.description.unwrap_or_default(),
                pre_request_script: String::new(),
                test_script: String::new(),
            },
        )
        .await;
        self.finish_mutation(
            &batch,
            "create_collection",
            "collection",
            result,
            &["name", "description"],
        )
        .await
    }

    #[tool(
        description = "Create a root or nested PostNot folder. Existing hierarchy objects are never modified."
    )]
    async fn create_folder(
        &self,
        Parameters(params): Parameters<CreateFolderParams>,
    ) -> Result<String, McpError> {
        let batch = Uuid::new_v4().to_string();
        let collection_id = params.collection_id.clone();
        let result = collections_service::create_collection_folder(
            &self.pool,
            &params.collection_id,
            &CreateCollectionFolderInput {
                name: params.name,
                parent_id: params.parent_id,
                pre_request_script: String::new(),
                test_script: String::new(),
            },
        )
        .await;
        self.finish_mutation_with_collection(
            &batch,
            "create_folder",
            "folder",
            Some(&collection_id),
            result,
            &["name", "parentId"],
        )
        .await
    }

    #[tool(
        description = "Create one reusable saved request in a PostNot collection or folder. Scripts are stored but never executed by MCP."
    )]
    async fn create_request(
        &self,
        Parameters(mut params): Parameters<CreateRequestParams>,
    ) -> Result<String, McpError> {
        normalize_request_ids(&mut params.request);
        if let Err(error) = validate_request(&params.request) {
            let batch = Uuid::new_v4().to_string();
            self.record_failure(
                &batch,
                "create_request",
                "request",
                Some(&params.collection_id),
                &error,
            )
            .await;
            return Err(to_mcp_error(error));
        }
        let warnings = credential_warnings(&params.request);
        let batch = Uuid::new_v4().to_string();
        let collection_id = params.collection_id.clone();
        let result = collections_service::save_request(
            &self.pool,
            &params.collection_id,
            params.parent_id.as_deref(),
            &params.request,
        )
        .await;
        self.finish_mutation_with_warnings(
            &batch,
            "create_request",
            "request",
            result,
            MutationDetails {
                collection_id: Some(&collection_id),
                fields: request_fields(),
                warnings,
            },
        )
        .await
    }

    #[tool(
        description = "Atomically create multiple saved requests in one collection or folder. Any invalid request rolls back the batch."
    )]
    async fn create_requests(
        &self,
        Parameters(mut params): Parameters<CreateRequestsParams>,
    ) -> Result<String, McpError> {
        for request in &mut params.requests {
            normalize_request_ids(request);
        }
        for request in &params.requests {
            if let Err(error) = validate_request(request) {
                let batch = Uuid::new_v4().to_string();
                self.record_failure(
                    &batch,
                    "create_requests",
                    "request",
                    Some(&params.collection_id),
                    &error,
                )
                .await;
                return Err(to_mcp_error(error));
            }
        }
        let warnings: Vec<String> = params
            .requests
            .iter()
            .flat_map(credential_warnings)
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        let batch = Uuid::new_v4().to_string();
        let collection_id = params.collection_id.clone();
        let result = collections_service::save_requests_atomic(
            &self.pool,
            &params.collection_id,
            params.parent_id.as_deref(),
            &params.requests,
        )
        .await;
        match result {
            Ok(items) => {
                for item in &items {
                    self.record(
                        &batch,
                        "create_requests",
                        "request",
                        Some(&item.id),
                        &item.name,
                        Some(&collection_id),
                        "succeeded",
                        request_fields(),
                        None,
                        None,
                    )
                    .await;
                }
                Ok(
                    json!({ "created": items, "warnings": warnings, "activityBatchId": batch })
                        .to_string(),
                )
            }
            Err(error) => {
                self.record_failure(
                    &batch,
                    "create_requests",
                    "request",
                    Some(&collection_id),
                    &error,
                )
                .await;
                Err(to_mcp_error(error))
            }
        }
    }

    #[tool(
        description = "Create a saved raw WebSocket or Socket.IO definition. This authoring tool stores the definition but never connects or sends traffic."
    )]
    async fn create_realtime_request(
        &self,
        Parameters(mut params): Parameters<CreateRealtimeRequestParams>,
    ) -> Result<String, McpError> {
        normalize_realtime_request_ids(&mut params.request);
        if let Err(error) = validate_realtime_request(&params.request) {
            let batch = Uuid::new_v4().to_string();
            self.record_failure(
                &batch,
                "create_realtime_request",
                "realtime_request",
                Some(&params.collection_id),
                &error,
            )
            .await;
            return Err(to_mcp_error(error));
        }

        let warnings = realtime_credential_warnings(&params.request);
        let batch = Uuid::new_v4().to_string();
        let collection_id = params.collection_id.clone();
        let result = collections_service::save_realtime_request(
            &self.pool,
            &params.collection_id,
            params.parent_id.as_deref(),
            &params.request,
        )
        .await;
        self.finish_mutation_with_warnings(
            &batch,
            "create_realtime_request",
            "realtime_request",
            result,
            MutationDetails {
                collection_id: Some(&collection_id),
                fields: realtime_request_fields(),
                warnings,
            },
        )
        .await
    }

    #[tool(
        description = "Fully replace a saved request using optimistic concurrency. Pass expectedUpdatedAt from get_saved_request."
    )]
    async fn update_request(
        &self,
        Parameters(mut params): Parameters<UpdateRequestParams>,
    ) -> Result<String, McpError> {
        normalize_request_ids(&mut params.request);
        if let Err(error) = validate_request(&params.request) {
            let batch = Uuid::new_v4().to_string();
            self.record_failure(&batch, "update_request", "request", None, &error)
                .await;
            return Err(to_mcp_error(error));
        }
        let current = collections_service::get_saved_request(&self.pool, &params.request_id)
            .await
            .map_err(to_mcp_error)?;
        restore_redacted_fields(
            &mut params.request,
            &current.request,
            &params.preserve_redacted_fields,
        );
        let warnings = credential_warnings(&params.request);
        let batch = Uuid::new_v4().to_string();
        let collection_id = current.collection_id.clone();
        let result = collections_service::update_saved_request_with_revision(
            &self.pool,
            &params.request_id,
            &params.request,
            Some(&params.expected_updated_at),
        )
        .await;
        self.finish_mutation_with_warnings(
            &batch,
            "update_request",
            "request",
            result,
            MutationDetails {
                collection_id: Some(&collection_id),
                fields: request_fields(),
                warnings,
            },
        )
        .await
    }

    #[tool(
        description = "Fully replace a saved raw WebSocket or Socket.IO definition using optimistic concurrency. This authoring tool never connects or sends traffic."
    )]
    async fn update_realtime_request(
        &self,
        Parameters(mut params): Parameters<UpdateRealtimeRequestParams>,
    ) -> Result<String, McpError> {
        normalize_realtime_request_ids(&mut params.request);
        if let Err(error) = validate_realtime_request(&params.request) {
            let batch = Uuid::new_v4().to_string();
            self.record_failure(
                &batch,
                "update_realtime_request",
                "realtime_request",
                None,
                &error,
            )
            .await;
            return Err(to_mcp_error(error));
        }

        let current =
            collections_service::get_saved_realtime_request(&self.pool, &params.request_id)
                .await
                .map_err(to_mcp_error)?;
        restore_realtime_redacted_fields(
            &mut params.request,
            &current.request,
            &params.preserve_redacted_fields,
        );
        let warnings = realtime_credential_warnings(&params.request);
        let batch = Uuid::new_v4().to_string();
        let collection_id = current.collection_id.clone();
        let result = collections_service::update_saved_realtime_request_with_revision(
            &self.pool,
            &params.request_id,
            &params.request,
            Some(&params.expected_updated_at),
        )
        .await;
        self.finish_mutation_with_warnings(
            &batch,
            "update_realtime_request",
            "realtime_request",
            result,
            MutationDetails {
                collection_id: Some(&collection_id),
                fields: realtime_request_fields(),
                warnings,
            },
        )
        .await
    }

    #[tool(
        description = "Delete a saved raw WebSocket or Socket.IO definition using optimistic concurrency. No live connection is affected."
    )]
    async fn delete_realtime_request(
        &self,
        Parameters(params): Parameters<DeleteRealtimeRequestParams>,
    ) -> Result<String, McpError> {
        let current =
            collections_service::get_saved_realtime_request(&self.pool, &params.request_id)
                .await
                .map_err(to_mcp_error)?;
        let batch = Uuid::new_v4().to_string();
        let result = collections_service::delete_saved_realtime_request_with_revision(
            &self.pool,
            &params.request_id,
            &params.expected_updated_at,
        )
        .await;

        match result {
            Ok(()) => {
                self.record(
                    &batch,
                    "delete_realtime_request",
                    "realtime_request",
                    Some(&current.id),
                    &current.name,
                    Some(&current.collection_id),
                    "succeeded",
                    &["deleted"],
                    None,
                    None,
                )
                .await;
                Ok(
                    json!({ "deleted": true, "requestId": current.id, "activityBatchId": batch })
                        .to_string(),
                )
            }
            Err(error) => {
                self.record_failure(
                    &batch,
                    "delete_realtime_request",
                    "realtime_request",
                    Some(&current.collection_id),
                    &error,
                )
                .await;
                Err(to_mcp_error(error))
            }
        }
    }

    async fn finish_mutation<T: Serialize + TargetMetadata>(
        &self,
        batch: &str,
        operation: &str,
        kind: &str,
        result: AppResult<T>,
        fields: &[&str],
    ) -> Result<String, McpError> {
        self.finish_mutation_with_warnings(
            batch,
            operation,
            kind,
            result,
            MutationDetails {
                collection_id: None,
                fields,
                warnings: Vec::new(),
            },
        )
        .await
    }

    async fn finish_mutation_with_collection<T: Serialize + TargetMetadata>(
        &self,
        batch: &str,
        operation: &str,
        kind: &str,
        collection_id: Option<&str>,
        result: AppResult<T>,
        fields: &[&str],
    ) -> Result<String, McpError> {
        self.finish_mutation_with_warnings(
            batch,
            operation,
            kind,
            result,
            MutationDetails {
                collection_id,
                fields,
                warnings: Vec::new(),
            },
        )
        .await
    }

    async fn finish_mutation_with_warnings<T: Serialize + TargetMetadata>(
        &self,
        batch: &str,
        operation: &str,
        kind: &str,
        result: AppResult<T>,
        details: MutationDetails<'_>,
    ) -> Result<String, McpError> {
        match result {
            Ok(value) => {
                self.record(
                    batch,
                    operation,
                    kind,
                    Some(value.target_id()),
                    value.target_name(),
                    details.collection_id.or(value.collection_id()),
                    "succeeded",
                    details.fields,
                    None,
                    None,
                )
                .await;
                Ok(
                    json!({ "item": value, "warnings": details.warnings, "activityBatchId": batch })
                        .to_string(),
                )
            }
            Err(error) => {
                self.record_failure(batch, operation, kind, details.collection_id, &error)
                    .await;
                Err(to_mcp_error(error))
            }
        }
    }

    async fn record_failure(
        &self,
        batch: &str,
        operation: &str,
        kind: &str,
        collection_id: Option<&str>,
        error: &AppError,
    ) {
        self.record(
            batch,
            operation,
            kind,
            None,
            "",
            collection_id,
            "failed",
            &[],
            Some(error.code()),
            Some(&error.to_string()),
        )
        .await;
    }

    #[allow(clippy::too_many_arguments)]
    async fn record(
        &self,
        batch: &str,
        operation: &str,
        kind: &str,
        target_id: Option<&str>,
        target_name: &str,
        collection_id: Option<&str>,
        outcome: &str,
        fields: &[&str],
        error_code: Option<&str>,
        error_message: Option<&str>,
    ) {
        let actor = self
            .actor
            .read()
            .map(|actor| actor.clone())
            .unwrap_or(AgentActor {
                name: "mcp-client".to_string(),
                version: String::new(),
                session_id: Uuid::new_v4().to_string(),
            });
        let _ = activity_service::record(
            &self.pool,
            &NewAgentActivity {
                batch_id: batch,
                actor: &actor,
                operation,
                outcome,
                target_kind: kind,
                target_id,
                target_name,
                collection_id,
                changed_fields: fields,
                error_code,
                error_message,
            },
        )
        .await;
    }
}

#[tool_handler]
impl ServerHandler for PostNotMcp {
    fn initialize(
        &self,
        request: rmcp::model::InitializeRequestParams,
        context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> impl std::future::Future<Output = Result<rmcp::model::InitializeResult, McpError>> + Send + '_
    {
        if let Ok(mut actor) = self.actor.write() {
            actor.name = request.client_info.name.clone();
            actor.version = request.client_info.version.clone();
        }
        if context.peer.peer_info().is_none() {
            context.peer.set_peer_info(request);
        }
        std::future::ready(Ok(self.get_info()))
    }

    fn get_info(&self) -> rmcp::model::ServerInfo {
        rmcp::model::ServerInfo {
            capabilities: rmcp::model::ServerCapabilities::builder()
                .enable_tools()
                .build(),
            instructions: Some("Author and inspect local PostNot collections safely. MCP never executes requests or scripts, opens realtime connections, sends traffic, imports files, or moves collection items. Realtime definition deletion requires an exact revision.".to_string()),
            server_info: rmcp::model::Implementation {
                name: "postnot".to_string(),
                title: Some("PostNot".to_string()),
                version: env!("CARGO_PKG_VERSION").to_string(),
                description: Some("Local-first API request authoring".to_string()),
                icons: None,
                website_url: Some("https://post-not.com".to_string()),
            },
            ..Default::default()
        }
    }
}

trait TargetMetadata {
    fn target_id(&self) -> &str;
    fn target_name(&self) -> &str;
    fn collection_id(&self) -> Option<&str> {
        None
    }
}
impl TargetMetadata for crate::domain::collections::CollectionSummary {
    fn target_id(&self) -> &str {
        &self.id
    }
    fn target_name(&self) -> &str {
        &self.name
    }
    fn collection_id(&self) -> Option<&str> {
        Some(&self.id)
    }
}
impl TargetMetadata for crate::domain::collections::CollectionItemSummary {
    fn target_id(&self) -> &str {
        &self.id
    }
    fn target_name(&self) -> &str {
        &self.name
    }
    fn collection_id(&self) -> Option<&str> {
        Some(&self.collection_id)
    }
}
impl TargetMetadata for crate::domain::collections::SavedRequestSummary {
    fn target_id(&self) -> &str {
        &self.id
    }
    fn target_name(&self) -> &str {
        &self.name
    }
    fn collection_id(&self) -> Option<&str> {
        Some(&self.collection_id)
    }
}
impl TargetMetadata for crate::domain::collections::SavedRealtimeRequestSummary {
    fn target_id(&self) -> &str {
        &self.id
    }
    fn target_name(&self) -> &str {
        &self.name
    }
    fn collection_id(&self) -> Option<&str> {
        Some(&self.collection_id)
    }
}

pub async fn run(options: McpOptions) -> Result<(), String> {
    let database_path =
        paths::headless_database_path(options.data_dir).map_err(|error| error.to_string())?;
    let pool = db::init_path(&database_path)
        .await
        .map_err(|error| error.to_string())?;
    db::ensure_application_defaults(&pool)
        .await
        .map_err(|error| error.to_string())?;
    let service = PostNotMcp::new(pool)
        .serve(stdio())
        .await
        .map_err(|error| error.to_string())?;
    service.waiting().await.map_err(|error| error.to_string())?;
    Ok(())
}

fn json_result<T: Serialize>(result: AppResult<T>) -> Result<String, McpError> {
    serde_json::to_string_pretty(&result.map_err(to_mcp_error)?).map_err(json_mcp_error)
}
fn json_mcp_error(error: serde_json::Error) -> McpError {
    McpError::internal_error(error.to_string(), None)
}
fn to_mcp_error(error: AppError) -> McpError {
    McpError::invalid_params(error.to_string(), Some(json!({ "code": error.code() })))
}

fn validate_request(request: &SendRequestPayload) -> AppResult<()> {
    if request.name.trim().is_empty() {
        return Err(AppError::Message("Request name is required.".to_string()));
    }
    if request.method.trim().is_empty() {
        return Err(AppError::Message("HTTP method is required.".to_string()));
    }
    request
        .method
        .parse::<reqwest::Method>()
        .map_err(|_| AppError::Message("HTTP method is invalid.".to_string()))?;
    if request.url.trim().is_empty() {
        return Err(AppError::Message("Request URL is required.".to_string()));
    }
    if !request.url.contains("{{") {
        url::Url::parse(&request.url)?;
    }
    if !["none", "json", "raw", "form-urlencoded", "multipart"]
        .contains(&request.body.mode.as_str())
    {
        return Err(AppError::Message(
            "Request body mode is invalid.".to_string(),
        ));
    }
    if !["none", "basic", "bearer", "api-key", "oauth2"].contains(&request.auth.auth_type.as_str())
    {
        return Err(AppError::Message(
            "Request auth type is invalid.".to_string(),
        ));
    }
    if !["header", "query"].contains(&request.auth.api_key_in.as_str()) {
        return Err(AppError::Message(
            "API key placement is invalid.".to_string(),
        ));
    }
    Ok(())
}

fn validate_realtime_request(request: &RealtimeRequestDraft) -> AppResult<()> {
    let common = request.common();
    if common.name.trim().is_empty() {
        return Err(AppError::Message(
            "Realtime request name is required.".to_string(),
        ));
    }
    if common.url.trim().is_empty() {
        return Err(AppError::Message(
            "Realtime request URL is required.".to_string(),
        ));
    }
    if !common.url.contains("{{") {
        let url = url::Url::parse(&common.url)?;
        let supported = match request {
            RealtimeRequestDraft::Websocket { .. } => {
                matches!(url.scheme(), "ws" | "wss")
            }
            RealtimeRequestDraft::Socketio { .. } => {
                matches!(url.scheme(), "http" | "https" | "ws" | "wss")
            }
        };
        if !supported {
            return Err(AppError::Message(
                "Realtime request URL scheme is not supported.".to_string(),
            ));
        }
    }
    if !["none", "basic", "bearer", "api-key", "oauth2"].contains(&common.auth.auth_type.as_str()) {
        return Err(AppError::Message(
            "Realtime request auth type is invalid.".to_string(),
        ));
    }
    if !["header", "query"].contains(&common.auth.api_key_in.as_str()) {
        return Err(AppError::Message(
            "API key placement is invalid.".to_string(),
        ));
    }
    if let RealtimeRequestDraft::Socketio {
        auth_payload,
        composer,
        ..
    } = request
    {
        if !auth_payload.is_object() {
            return Err(AppError::Message(
                "Socket.IO auth payload must be a JSON object.".to_string(),
            ));
        }
        if !composer.arguments.is_array() {
            return Err(AppError::Message(
                "Socket.IO event arguments must be a JSON array.".to_string(),
            ));
        }
    }
    Ok(())
}

fn request_fields() -> &'static [&'static str] {
    &[
        "name",
        "method",
        "url",
        "queryParams",
        "headers",
        "body",
        "auth",
        "preRequestScript",
        "testScript",
    ]
}

fn realtime_request_fields() -> &'static [&'static str] {
    &[
        "requestType",
        "name",
        "url",
        "queryParams",
        "headers",
        "auth",
        "reconnect",
        "subprotocols",
        "path",
        "namespace",
        "authPayload",
        "transport",
        "composer",
    ]
}

fn normalize_request_ids(request: &mut SendRequestPayload) {
    for row in request
        .query_params
        .iter_mut()
        .chain(request.headers.iter_mut())
        .chain(request.body.form.iter_mut())
    {
        if row.id.trim().is_empty() {
            row.id = Uuid::new_v4().to_string();
        }
    }
    for file in &mut request.body.files {
        if file.id.trim().is_empty() {
            file.id = Uuid::new_v4().to_string();
        }
    }
}

fn normalize_realtime_request_ids(request: &mut RealtimeRequestDraft) {
    let common = realtime_common_mut(request);
    for row in common
        .query_params
        .iter_mut()
        .chain(common.headers.iter_mut())
    {
        if row.id.trim().is_empty() {
            row.id = Uuid::new_v4().to_string();
        }
    }
}

fn realtime_common_mut(
    request: &mut RealtimeRequestDraft,
) -> &mut crate::domain::realtime::RealtimeRequestCommon {
    match request {
        RealtimeRequestDraft::Websocket { common, .. }
        | RealtimeRequestDraft::Socketio { common, .. } => common,
    }
}

fn sensitive_key(key: &str) -> bool {
    let key = key.trim().to_ascii_lowercase().replace(['_', '-'], "");
    [
        "authorization",
        "cookie",
        "setcookie",
        "apikey",
        "xapikey",
        "token",
        "accesstoken",
        "clientsecret",
        "password",
        "passwd",
    ]
    .iter()
    .any(|candidate| key.contains(candidate))
}

fn credential_warnings(request: &SendRequestPayload) -> Vec<String> {
    let mut warnings = Vec::new();
    let auth = &request.auth;
    if [
        auth.basic_password.as_str(),
        auth.bearer_token.as_str(),
        auth.api_key_value.as_str(),
        auth.oauth2_access_token.as_str(),
        auth.oauth2_client_secret.as_str(),
    ]
    .iter()
    .any(|value| !value.is_empty() && !value.contains("{{"))
    {
        warnings.push(
            "Literal authentication credentials were stored locally. Prefer {{variables}}."
                .to_string(),
        );
    }
    if request
        .headers
        .iter()
        .any(|row| sensitive_key(&row.key) && !row.value.is_empty() && !row.value.contains("{{"))
    {
        warnings.push(
            "A credential-looking header value was stored locally. Prefer {{variables}}."
                .to_string(),
        );
    }
    if request
        .query_params
        .iter()
        .chain(request.body.form.iter())
        .any(|row| sensitive_key(&row.key) && !row.value.is_empty() && !row.value.contains("{{"))
    {
        warnings.push(
            "A credential-looking query or form value was stored locally. Prefer {{variables}}."
                .to_string(),
        );
    }
    if raw_body_has_sensitive_keys(&request.body.raw) {
        warnings.push(
            "A credential-looking body value was stored locally. Prefer {{variables}}.".to_string(),
        );
    }
    warnings
}

fn realtime_credential_warnings(request: &RealtimeRequestDraft) -> Vec<String> {
    let common = request.common();
    let mut warnings = Vec::new();
    let auth = &common.auth;
    if [
        auth.basic_password.as_str(),
        auth.bearer_token.as_str(),
        auth.api_key_value.as_str(),
        auth.oauth2_access_token.as_str(),
        auth.oauth2_client_secret.as_str(),
    ]
    .iter()
    .any(|value| !value.is_empty() && !value.contains("{{"))
    {
        warnings.push(
            "Literal authentication credentials were stored locally. Prefer {{variables}}."
                .to_string(),
        );
    }
    if common
        .headers
        .iter()
        .any(|row| sensitive_key(&row.key) && !row.value.is_empty() && !row.value.contains("{{"))
    {
        warnings.push(
            "A credential-looking header value was stored locally. Prefer {{variables}}."
                .to_string(),
        );
    }
    if common
        .query_params
        .iter()
        .any(|row| sensitive_key(&row.key) && !row.value.is_empty() && !row.value.contains("{{"))
    {
        warnings.push(
            "A credential-looking query value was stored locally. Prefer {{variables}}."
                .to_string(),
        );
    }
    match request {
        RealtimeRequestDraft::Websocket { composer, .. }
            if composer.mode == RawMessageMode::Json
                && raw_body_has_sensitive_keys(&composer.content) =>
        {
            warnings.push(
                "A credential-looking composer value was stored locally. Prefer {{variables}}."
                    .to_string(),
            );
        }
        RealtimeRequestDraft::Socketio {
            auth_payload,
            composer,
            ..
        } if json_has_sensitive_keys(auth_payload)
            || json_has_sensitive_keys(&composer.arguments) =>
        {
            warnings.push(
                "A credential-looking Socket.IO payload value was stored locally. Prefer {{variables}}."
                    .to_string(),
            );
        }
        _ => {}
    }
    warnings
}

fn redact_saved_request(
    detail: crate::domain::collections::SavedRequestDetail,
) -> SafeSavedRequest {
    let mut request = detail.request;
    let mut fields = Vec::new();
    macro_rules! redact {
        ($field:expr, $path:expr) => {
            if !$field.is_empty() {
                $field = REDACTED.to_string();
                fields.push($path.to_string());
            }
        };
    }
    redact!(request.auth.basic_password, "auth.basicPassword");
    redact!(request.auth.bearer_token, "auth.bearerToken");
    redact!(request.auth.api_key_value, "auth.apiKeyValue");
    redact!(request.auth.oauth2_access_token, "auth.oauth2AccessToken");
    redact!(request.auth.oauth2_client_secret, "auth.oauth2ClientSecret");
    for row in &mut request.headers {
        if sensitive_key(&row.key) && !row.value.is_empty() {
            row.value = REDACTED.to_string();
            fields.push(format!("headers.{}.value", row.id));
        }
    }
    for row in &mut request.query_params {
        if sensitive_key(&row.key) && !row.value.is_empty() {
            row.value = REDACTED.to_string();
            fields.push(format!("queryParams.{}.value", row.id));
        }
    }
    for row in &mut request.body.form {
        if sensitive_key(&row.key) && !row.value.is_empty() {
            row.value = REDACTED.to_string();
            fields.push(format!("body.form.{}.value", row.id));
        }
    }
    if raw_body_has_sensitive_keys(&request.body.raw) {
        request.body.raw = REDACTED.to_string();
        fields.push("body.raw".to_string());
    }
    if let Ok(mut url) = url::Url::parse(&request.url) {
        let pairs: Vec<(String, String)> = url
            .query_pairs()
            .map(|(key, value)| (key.into_owned(), value.into_owned()))
            .collect();
        if pairs
            .iter()
            .any(|(key, value)| sensitive_key(key) && !value.is_empty())
        {
            url.query_pairs_mut()
                .clear()
                .extend_pairs(pairs.iter().map(|(key, value)| {
                    (
                        key,
                        if sensitive_key(key) && !value.is_empty() {
                            REDACTED
                        } else {
                            value.as_str()
                        },
                    )
                }));
            request.url = url.to_string();
            fields.push("url".to_string());
        }
    }
    let warnings = if fields.is_empty() {
        Vec::new()
    } else {
        vec!["Credential-looking values were redacted. Pass the matching redactedFields paths to preserve them during update_request.".to_string()]
    };
    SafeSavedRequest {
        id: detail.id,
        collection_id: detail.collection_id,
        parent_id: detail.parent_id,
        name: detail.name,
        updated_at: detail.updated_at,
        request,
        redacted_fields: fields,
        warnings,
    }
}

fn redact_saved_realtime_request(detail: SavedRealtimeRequestDetail) -> SafeSavedRealtimeRequest {
    let mut request = detail.request;
    let mut fields = Vec::new();
    {
        let common = realtime_common_mut(&mut request);
        redact_request_auth(&mut common.auth, &mut fields);
        redact_key_value_rows(&mut common.headers, "headers", &mut fields);
        redact_key_value_rows(&mut common.query_params, "queryParams", &mut fields);
        redact_sensitive_url(&mut common.url, &mut fields);
    }

    match &mut request {
        RealtimeRequestDraft::Websocket { composer, .. } => {
            if composer.mode == RawMessageMode::Json
                && raw_body_has_sensitive_keys(&composer.content)
            {
                composer.content = REDACTED.to_string();
                fields.push("composer.content".to_string());
            }
        }
        RealtimeRequestDraft::Socketio {
            auth_payload,
            composer,
            ..
        } => {
            redact_sensitive_json(auth_payload, "authPayload", &mut fields);
            redact_sensitive_json(&mut composer.arguments, "composer.arguments", &mut fields);
        }
    }

    let warnings = if fields.is_empty() {
        Vec::new()
    } else {
        vec!["Credential-looking values were redacted. Pass the matching redactedFields paths to preserve them during update_realtime_request.".to_string()]
    };
    SafeSavedRealtimeRequest {
        id: detail.id,
        collection_id: detail.collection_id,
        parent_id: detail.parent_id,
        name: detail.name,
        request_type: detail.request_type,
        updated_at: detail.updated_at,
        request,
        redacted_fields: fields,
        warnings,
    }
}

fn redact_request_auth(auth: &mut crate::domain::requests::RequestAuth, fields: &mut Vec<String>) {
    for (value, path) in [
        (&mut auth.basic_password, "auth.basicPassword"),
        (&mut auth.bearer_token, "auth.bearerToken"),
        (&mut auth.api_key_value, "auth.apiKeyValue"),
        (&mut auth.oauth2_access_token, "auth.oauth2AccessToken"),
        (&mut auth.oauth2_client_secret, "auth.oauth2ClientSecret"),
    ] {
        if !value.is_empty() {
            *value = REDACTED.to_string();
            fields.push(path.to_string());
        }
    }
}

fn redact_key_value_rows(
    rows: &mut [crate::domain::requests::KeyValueRow],
    field_name: &str,
    fields: &mut Vec<String>,
) {
    for row in rows {
        if sensitive_key(&row.key) && !row.value.is_empty() {
            row.value = REDACTED.to_string();
            fields.push(format!("{field_name}.{}.value", row.id));
        }
    }
}

fn redact_sensitive_url(url_text: &mut String, fields: &mut Vec<String>) {
    let Ok(mut url) = url::Url::parse(url_text.as_str()) else {
        return;
    };
    let pairs: Vec<(String, String)> = url
        .query_pairs()
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect();
    if !pairs
        .iter()
        .any(|(key, value)| sensitive_key(key) && !value.is_empty())
    {
        return;
    }

    url.query_pairs_mut()
        .clear()
        .extend_pairs(pairs.iter().map(|(key, value)| {
            (
                key,
                if sensitive_key(key) && !value.is_empty() {
                    REDACTED
                } else {
                    value.as_str()
                },
            )
        }));
    *url_text = url.to_string();
    fields.push("url".to_string());
}

fn redact_sensitive_json(value: &mut Value, path: &str, fields: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                let child_path = format!("{path}.{key}");
                if sensitive_key(key) && !child.is_null() {
                    *child = Value::String(REDACTED.to_string());
                    fields.push(child_path);
                } else {
                    redact_sensitive_json(child, &child_path, fields);
                }
            }
        }
        Value::Array(items) => {
            for (index, child) in items.iter_mut().enumerate() {
                redact_sensitive_json(child, &format!("{path}.{index}"), fields);
            }
        }
        _ => {}
    }
}

fn restore_redacted_fields(
    next: &mut SendRequestPayload,
    current: &SendRequestPayload,
    fields: &[String],
) {
    let fields: HashSet<&str> = fields.iter().map(String::as_str).collect();
    macro_rules! restore {
        ($next:expr, $current:expr, $path:expr) => {
            if fields.contains($path) && $next == REDACTED {
                $next = $current.clone();
            }
        };
    }
    restore!(
        next.auth.basic_password,
        current.auth.basic_password,
        "auth.basicPassword"
    );
    restore!(next.url, current.url, "url");
    restore!(next.body.raw, current.body.raw, "body.raw");
    restore!(
        next.auth.bearer_token,
        current.auth.bearer_token,
        "auth.bearerToken"
    );
    restore!(
        next.auth.api_key_value,
        current.auth.api_key_value,
        "auth.apiKeyValue"
    );
    restore!(
        next.auth.oauth2_access_token,
        current.auth.oauth2_access_token,
        "auth.oauth2AccessToken"
    );
    restore!(
        next.auth.oauth2_client_secret,
        current.auth.oauth2_client_secret,
        "auth.oauth2ClientSecret"
    );
    for row in &mut next.headers {
        let path = format!("headers.{}.value", row.id);
        if fields.contains(path.as_str()) && row.value == REDACTED {
            if let Some(original) = current
                .headers
                .iter()
                .find(|item| item.id == row.id && sensitive_key(&item.key))
            {
                row.value = original.value.clone();
            }
        }
    }
    for row in &mut next.query_params {
        let path = format!("queryParams.{}.value", row.id);
        if fields.contains(path.as_str()) && row.value == REDACTED {
            if let Some(original) = current
                .query_params
                .iter()
                .find(|item| item.id == row.id && sensitive_key(&item.key))
            {
                row.value = original.value.clone();
            }
        }
    }
    for row in &mut next.body.form {
        let path = format!("body.form.{}.value", row.id);
        if fields.contains(path.as_str()) && row.value == REDACTED {
            if let Some(original) = current
                .body
                .form
                .iter()
                .find(|item| item.id == row.id && sensitive_key(&item.key))
            {
                row.value = original.value.clone();
            }
        }
    }
}

fn restore_realtime_redacted_fields(
    next: &mut RealtimeRequestDraft,
    current: &RealtimeRequestDraft,
    fields: &[String],
) {
    let fields: HashSet<&str> = fields.iter().map(String::as_str).collect();
    {
        let next_common = realtime_common_mut(next);
        let current_common = current.common();
        restore_auth_fields(&mut next_common.auth, &current_common.auth, &fields);
        restore_key_value_rows(
            &mut next_common.headers,
            &current_common.headers,
            "headers",
            &fields,
        );
        restore_key_value_rows(
            &mut next_common.query_params,
            &current_common.query_params,
            "queryParams",
            &fields,
        );
        if fields.contains("url") {
            next_common.url = current_common.url.clone();
        }
    }

    match (next, current) {
        (
            RealtimeRequestDraft::Websocket {
                composer: next_composer,
                ..
            },
            RealtimeRequestDraft::Websocket {
                composer: current_composer,
                ..
            },
        ) if fields.contains("composer.content") && next_composer.content == REDACTED => {
            next_composer.content = current_composer.content.clone();
        }
        (
            RealtimeRequestDraft::Socketio {
                auth_payload: next_auth,
                composer: next_composer,
                ..
            },
            RealtimeRequestDraft::Socketio {
                auth_payload: current_auth,
                composer: current_composer,
                ..
            },
        ) => {
            restore_sensitive_json(next_auth, current_auth, "authPayload", &fields);
            restore_sensitive_json(
                &mut next_composer.arguments,
                &current_composer.arguments,
                "composer.arguments",
                &fields,
            );
        }
        _ => {}
    }
}

fn restore_auth_fields(
    next: &mut crate::domain::requests::RequestAuth,
    current: &crate::domain::requests::RequestAuth,
    fields: &HashSet<&str>,
) {
    for (next_value, current_value, path) in [
        (
            &mut next.basic_password,
            &current.basic_password,
            "auth.basicPassword",
        ),
        (
            &mut next.bearer_token,
            &current.bearer_token,
            "auth.bearerToken",
        ),
        (
            &mut next.api_key_value,
            &current.api_key_value,
            "auth.apiKeyValue",
        ),
        (
            &mut next.oauth2_access_token,
            &current.oauth2_access_token,
            "auth.oauth2AccessToken",
        ),
        (
            &mut next.oauth2_client_secret,
            &current.oauth2_client_secret,
            "auth.oauth2ClientSecret",
        ),
    ] {
        if fields.contains(path) && next_value == REDACTED {
            *next_value = current_value.clone();
        }
    }
}

fn restore_key_value_rows(
    next: &mut [crate::domain::requests::KeyValueRow],
    current: &[crate::domain::requests::KeyValueRow],
    field_name: &str,
    fields: &HashSet<&str>,
) {
    for row in next {
        let path = format!("{field_name}.{}.value", row.id);
        if fields.contains(path.as_str()) && row.value == REDACTED {
            if let Some(original) = current
                .iter()
                .find(|item| item.id == row.id && sensitive_key(&item.key))
            {
                row.value = original.value.clone();
            }
        }
    }
}

fn restore_sensitive_json(next: &mut Value, current: &Value, path: &str, fields: &HashSet<&str>) {
    if fields.contains(path) && next.as_str() == Some(REDACTED) {
        *next = current.clone();
        return;
    }
    match (next, current) {
        (Value::Object(next_map), Value::Object(current_map)) => {
            for (key, next_child) in next_map {
                if let Some(current_child) = current_map.get(key) {
                    restore_sensitive_json(
                        next_child,
                        current_child,
                        &format!("{path}.{key}"),
                        fields,
                    );
                }
            }
        }
        (Value::Array(next_items), Value::Array(current_items)) => {
            for (index, (next_child, current_child)) in
                next_items.iter_mut().zip(current_items).enumerate()
            {
                restore_sensitive_json(
                    next_child,
                    current_child,
                    &format!("{path}.{index}"),
                    fields,
                );
            }
        }
        _ => {}
    }
}

fn raw_body_has_sensitive_keys(raw: &str) -> bool {
    serde_json::from_str::<Value>(raw)
        .map(|value| json_has_sensitive_keys(&value))
        .unwrap_or(false)
}

fn json_has_sensitive_keys(value: &Value) -> bool {
    match value {
        Value::Object(map) => map.iter().any(|(key, value)| {
            (sensitive_key(key) && !value.is_null()) || json_has_sensitive_keys(value)
        }),
        Value::Array(items) => items.iter().any(json_has_sensitive_keys),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        collections::{SavedRealtimeRequestDetail, SavedRequestDetail},
        realtime::{RealtimeRequestCommon, ReconnectPolicy, SocketIoComposer, SocketIoTransport},
        requests::{RequestAuth, RequestBody},
    };

    fn request() -> SendRequestPayload {
        SendRequestPayload {
            name: "Secret request".into(),
            method: "GET".into(),
            url: "https://example.test".into(),
            query_params: vec![],
            headers: vec![crate::domain::requests::KeyValueRow {
                id: "header".into(),
                key: "Authorization".into(),
                value: "Bearer literal".into(),
                enabled: true,
            }],
            body: RequestBody {
                mode: "none".into(),
                raw: String::new(),
                form: vec![],
                files: vec![],
            },
            auth: RequestAuth {
                auth_type: "bearer".into(),
                basic_username: String::new(),
                basic_password: String::new(),
                bearer_token: "literal-token".into(),
                api_key_name: String::new(),
                api_key_value: String::new(),
                api_key_in: "header".into(),
                oauth2_access_token: String::new(),
                oauth2_token_url: String::new(),
                oauth2_client_id: String::new(),
                oauth2_client_secret: String::new(),
                oauth2_scope: String::new(),
            },
            pre_request_script: String::new(),
            test_script: String::new(),
        }
    }

    #[test]
    fn saved_request_reads_redact_and_updates_can_preserve_literals() {
        let original = request();
        let safe = redact_saved_request(SavedRequestDetail {
            id: "request".into(),
            collection_id: "collection".into(),
            parent_id: None,
            name: original.name.clone(),
            updated_at: "revision".into(),
            request: original.clone(),
        });
        assert_eq!(safe.request.auth.bearer_token, REDACTED);
        assert_eq!(safe.request.headers[0].value, REDACTED);
        let mut replacement = safe.request;
        restore_redacted_fields(&mut replacement, &original, &safe.redacted_fields);
        assert_eq!(replacement.auth.bearer_token, "literal-token");
        assert_eq!(replacement.headers[0].value, "Bearer literal");
    }

    #[test]
    fn request_validation_rejects_unknown_modes() {
        let mut invalid = request();
        invalid.body.mode = "mystery".into();
        assert!(validate_request(&invalid).is_err());
    }

    #[test]
    fn realtime_reads_redact_and_updates_can_preserve_literals() {
        let original = RealtimeRequestDraft::Socketio {
            common: RealtimeRequestCommon {
                name: "Presence".into(),
                url: "https://example.test?token=url-secret".into(),
                query_params: vec![crate::domain::requests::KeyValueRow {
                    id: "query".into(),
                    key: "api_key".into(),
                    value: "query-secret".into(),
                    enabled: true,
                }],
                headers: vec![crate::domain::requests::KeyValueRow {
                    id: "header".into(),
                    key: "Authorization".into(),
                    value: "Bearer header-secret".into(),
                    enabled: true,
                }],
                auth: RequestAuth {
                    auth_type: "bearer".into(),
                    bearer_token: "auth-secret".into(),
                    ..RequestAuth::default()
                },
                reconnect: ReconnectPolicy::default(),
            },
            path: "/socket.io/".into(),
            namespace: "/".into(),
            auth_payload: json!({"clientSecret": "payload-secret"}),
            transport: SocketIoTransport::Auto,
            composer: SocketIoComposer {
                event: "join".into(),
                arguments: json!([{"password": "argument-secret"}]),
                ..SocketIoComposer::default()
            },
        };
        let safe = redact_saved_realtime_request(SavedRealtimeRequestDetail {
            id: "request".into(),
            collection_id: "collection".into(),
            parent_id: None,
            name: "Presence".into(),
            request_type: RequestType::Socketio,
            updated_at: "revision".into(),
            request: original.clone(),
        });

        let safe_json = serde_json::to_string(&safe).expect("serialize safe projection");
        for secret in [
            "url-secret",
            "query-secret",
            "header-secret",
            "auth-secret",
            "payload-secret",
            "argument-secret",
        ] {
            assert!(
                !safe_json.contains(secret),
                "safe projection leaked {secret}"
            );
        }
        assert!(safe.redacted_fields.contains(&"url".to_string()));
        assert!(safe
            .redacted_fields
            .contains(&"authPayload.clientSecret".to_string()));
        assert!(safe
            .redacted_fields
            .contains(&"composer.arguments.0.password".to_string()));

        let mut replacement = safe.request;
        restore_realtime_redacted_fields(&mut replacement, &original, &safe.redacted_fields);
        assert_eq!(
            serde_json::to_value(replacement).expect("serialize replacement"),
            serde_json::to_value(original).expect("serialize original")
        );
    }
}
