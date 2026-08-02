use sqlx::SqlitePool;
use uuid::Uuid;

use crate::{
    domain::{
        collections::CreateCollectionInput,
        imports::{ImportDetails, ImportResult},
        portability::{
            PostNotCollectionDocument, PostNotCollectionItem, POSTNOT_COLLECTION_SCHEMA,
            POSTNOT_COLLECTION_VERSION,
        },
        realtime::{
            RealtimeConnectionDraft, REALTIME_MESSAGE_SCHEMA_VERSION,
            LEGACY_REALTIME_REQUEST_SCHEMA_VERSION,
        },
    },
    error::{AppError, AppResult},
    services::{
        collections_service::{
            self, ImportCollectionFolder, ImportCollectionRealtimeMessage,
            ImportCollectionRequest,
        },
        realtime_connections_service,
    },
};

pub(super) async fn import_postnot_collection(
    pool: &SqlitePool,
    source: &str,
) -> AppResult<ImportResult> {
    let document: PostNotCollectionDocument = serde_json::from_str(source)
        .map_err(|error| AppError::Message(format!("Invalid PostNot collection JSON: {error}")))?;
    validate_document(&document)?;

    let mut folders = Vec::new();
    let mut requests = Vec::new();
    let mut realtime_messages = Vec::new();
    let mut connections = Vec::new();
    let mut imported_items = Vec::new();
    flatten_items(
        None,
        &document.items,
        &mut folders,
        &mut requests,
        &mut realtime_messages,
        &mut connections,
        &mut imported_items,
    )?;
    let imported_request_count = requests.len() + realtime_messages.len();
    let collection_name = document.collection.name.trim().to_string();

    let created = collections_service::import_mixed_collection_atomic(
        pool,
        &CreateCollectionInput {
            name: collection_name,
            description: document.collection.description,
            pre_request_script: document.collection.pre_request_script,
            test_script: document.collection.test_script,
        },
        &folders,
        &requests,
        &realtime_messages,
    )
    .await?;
    let profile_count_before = realtime_connections_service::list_profiles(pool).await?.len();
    for connection in &connections {
        realtime_connections_service::get_or_create_exact_profile(pool, connection).await?;
    }
    let created_profile_count = realtime_connections_service::list_profiles(pool).await?.len().saturating_sub(profile_count_before);

    Ok(ImportResult {
        collection_id: created.id,
        collection_name: created.name,
        imported_request_count,
        created_collection: true,
        created_realtime_connection_profile_count: created_profile_count,
        details: Some(ImportDetails {
            format: "postnot".to_string(),
            summary: format!(
                "{} request{} imported from PostNot.",
                imported_request_count,
                if imported_request_count == 1 { "" } else { "s" }
            ),
            imported_items,
            warnings: if connections.is_empty() {
                Vec::new()
            } else {
                vec![format!(
                    "{} standalone connection profile{} created; matching existing profiles were reused.",
                    created_profile_count,
                    if created_profile_count == 1 { " was" } else { "s were" }
                )]
            },
            errors: Vec::new(),
        }),
    })
}

fn validate_document(document: &PostNotCollectionDocument) -> AppResult<()> {
    if document.schema != POSTNOT_COLLECTION_SCHEMA {
        return Err(AppError::Message(
            "Unsupported PostNot collection schema.".to_string(),
        ));
    }
    if !matches!(document.version, 1 | POSTNOT_COLLECTION_VERSION) {
        return Err(AppError::Message(format!(
            "Unsupported PostNot collection version: {}.",
            document.version
        )));
    }
    if document.collection.name.trim().is_empty() {
        return Err(AppError::Message(
            "PostNot collection name is required.".to_string(),
        ));
    }
    Ok(())
}

fn flatten_items(
    parent_id: Option<&str>,
    items: &[PostNotCollectionItem],
    folders: &mut Vec<ImportCollectionFolder>,
    requests: &mut Vec<ImportCollectionRequest>,
    realtime_messages: &mut Vec<ImportCollectionRealtimeMessage>,
    connections: &mut Vec<RealtimeConnectionDraft>,
    imported_items: &mut Vec<String>,
) -> AppResult<()> {
    for (sort_order, item) in items.iter().enumerate() {
        match item {
            PostNotCollectionItem::Folder {
                name,
                pre_request_script,
                test_script,
                items,
            } => {
                if name.trim().is_empty() {
                    return Err(AppError::Message(
                        "PostNot folder name is required.".to_string(),
                    ));
                }
                let folder_id = Uuid::new_v4().to_string();
                folders.push(ImportCollectionFolder {
                    id: folder_id.clone(),
                    parent_id: parent_id.map(str::to_string),
                    sort_order: sort_order as i64,
                    name: name.trim().to_string(),
                    pre_request_script: pre_request_script.clone(),
                    test_script: test_script.clone(),
                });
                flatten_items(
                    Some(&folder_id),
                    items,
                    folders,
                    requests,
                    realtime_messages,
                    connections,
                    imported_items,
                )?;
            }
            PostNotCollectionItem::Http { request } => {
                requests.push(ImportCollectionRequest {
                    parent_id: parent_id.map(str::to_string),
                    sort_order: sort_order as i64,
                    request: request.clone(),
                });
                imported_items.push(request.name.clone());
            }
            PostNotCollectionItem::Message { message } => {
                if message.version != REALTIME_MESSAGE_SCHEMA_VERSION {
                    return Err(AppError::Message(format!(
                        "Unsupported saved realtime message version: {}.",
                        message.version
                    )));
                }
                realtime_messages.push(ImportCollectionRealtimeMessage {
                    parent_id: parent_id.map(str::to_string),
                    sort_order: sort_order as i64,
                    message: message.message.clone(),
                });
                imported_items.push(message.message.name().to_string());
            }
            PostNotCollectionItem::Realtime { request } => {
                if request.version != LEGACY_REALTIME_REQUEST_SCHEMA_VERSION {
                    return Err(AppError::Message(format!(
                        "Unsupported legacy realtime request version: {}.",
                        request.version
                    )));
                }
                let (connection, message) = request.request.clone().split();
                connections.push(connection);
                imported_items.push(message.name().to_string());
                realtime_messages.push(ImportCollectionRealtimeMessage {
                    parent_id: parent_id.map(str::to_string),
                    sort_order: sort_order as i64,
                    message,
                });
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};

    use super::*;
    use crate::{
        domain::{
            collections::{CreateCollectionFolderInput, CreateCollectionInput},
            realtime::{
                LegacyRealtimeRequestDraft, RawWebSocketComposer, RealtimeConnectionCommon,
                RealtimeMessageDraft, ReconnectPolicy, VersionedLegacyRealtimeRequest,
            },
            requests::{RequestAuth, RequestBody, SendRequestPayload},
        },
        services::{collections_service, exports_service},
    };

    async fn setup_test_db() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory database");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("run migrations");
        pool
    }

    fn http_request() -> SendRequestPayload {
        SendRequestPayload {
            name: "Health".to_string(),
            method: "GET".to_string(),
            url: "https://example.test/health".to_string(),
            query_params: Vec::new(),
            headers: Vec::new(),
            body: RequestBody::default(),
            auth: RequestAuth::default(),
            pre_request_script: "pn.variables.set('kind', 'http');".to_string(),
            test_script: "pn.test('ok', true);".to_string(),
        }
    }

    fn realtime_message() -> RealtimeMessageDraft {
        RealtimeMessageDraft::Websocket {
            name: "Events".to_string(),
            composer: RawWebSocketComposer::default(),
        }
    }

    async fn seed_mixed_collection(pool: &SqlitePool) -> String {
        let collection = collections_service::create_collection(
            pool,
            &CreateCollectionInput {
                name: "Mixed APIs".to_string(),
                description: "Portable collection".to_string(),
                pre_request_script: "collection-pre".to_string(),
                test_script: "collection-test".to_string(),
            },
        )
        .await
        .expect("create collection");
        let folder = collections_service::create_collection_folder(
            pool,
            &collection.id,
            &CreateCollectionFolderInput {
                name: "Realtime".to_string(),
                parent_id: None,
                pre_request_script: "folder-pre".to_string(),
                test_script: "folder-test".to_string(),
            },
        )
        .await
        .expect("create folder");
        collections_service::save_request(pool, &collection.id, Some(&folder.id), &http_request())
            .await
            .expect("save http request");
        collections_service::save_realtime_message(
            pool,
            &collection.id,
            Some(&folder.id),
            &realtime_message(),
        )
        .await
        .expect("save realtime request");
        collection.id
    }

    #[tokio::test]
    async fn postnot_collection_round_trip_preserves_mixed_definitions_and_hierarchy() {
        let pool = setup_test_db().await;
        let source_id = seed_mixed_collection(&pool).await;
        let (source, _) = exports_service::serialize_postnot_collection(&pool, &source_id)
            .await
            .expect("serialize PostNot collection");

        let result = import_postnot_collection(&pool, &source)
            .await
            .expect("import PostNot collection");
        assert_eq!(result.imported_request_count, 2);
        assert_eq!(
            result
                .details
                .as_ref()
                .map(|details| details.format.as_str()),
            Some("postnot")
        );

        let imported = collections_service::get_collection(&pool, &result.collection_id)
            .await
            .expect("read imported collection");
        assert_eq!(imported.name, "Mixed APIs");
        assert_eq!(imported.description, "Portable collection");
        assert_eq!(imported.pre_request_script, "collection-pre");
        assert_eq!(imported.test_script, "collection-test");
        assert_eq!(imported.request_count, 2);

        let items = collections_service::list_collection_items(&pool, &result.collection_id)
            .await
            .expect("list imported items");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, "Realtime");
        assert_eq!(items[0].pre_request_script, "folder-pre");
        assert_eq!(items[0].test_script, "folder-test");
        assert_eq!(items[0].children.len(), 2);
        assert!(items[0]
            .children
            .iter()
            .any(|item| item.request_type.as_deref() == Some("http")));
        let realtime_item = items[0]
            .children
            .iter()
            .find(|item| item.request_type.as_deref() == Some("websocket"))
            .expect("imported websocket");
        let detail = collections_service::get_saved_realtime_message(&pool, &realtime_item.id)
            .await
            .expect("read imported websocket");
        assert_eq!(
            serde_json::to_value(detail.message).expect("serialize imported message"),
            serde_json::to_value(realtime_message()).expect("serialize source message")
        );
    }

    #[tokio::test]
    async fn postman_export_reports_realtime_omissions_without_failing() {
        let pool = setup_test_db().await;
        let source_id = seed_mixed_collection(&pool).await;
        let (json, _, omitted_count) =
            exports_service::serialize_postman_collection(&pool, &source_id)
                .await
                .expect("serialize Postman collection");

        assert_eq!(omitted_count, 1);
        assert!(json.contains("\"Health\""));
        assert!(!json.contains("\"Events\""));
    }

    #[tokio::test]
    async fn version_one_combined_entries_split_into_profiles_and_messages() {
        let pool = setup_test_db().await;
        let document = PostNotCollectionDocument {
            schema: POSTNOT_COLLECTION_SCHEMA.to_string(),
            version: 1,
            collection: crate::domain::portability::PostNotCollectionMetadata {
                name: "Legacy".to_string(), description: String::new(),
                pre_request_script: String::new(), test_script: String::new(),
            },
            items: vec![PostNotCollectionItem::Realtime {
                request: VersionedLegacyRealtimeRequest {
                    version: LEGACY_REALTIME_REQUEST_SCHEMA_VERSION,
                    request: LegacyRealtimeRequestDraft::Websocket {
                        common: RealtimeConnectionCommon {
                            name: "Legacy socket".to_string(), url: "wss://example.test".to_string(),
                            query_params: Vec::new(), headers: Vec::new(), auth: RequestAuth::default(), reconnect: ReconnectPolicy::default(),
                        },
                        subprotocols: Vec::new(), composer: RawWebSocketComposer::default(),
                    },
                },
            }],
        };
        let result = import_postnot_collection(&pool, &serde_json::to_string(&document).unwrap()).await.unwrap();
        assert_eq!(result.created_realtime_connection_profile_count, 1);
        assert_eq!(realtime_connections_service::list_profiles(&pool).await.unwrap().len(), 1);
        assert_eq!(collections_service::list_saved_realtime_messages(&pool, &result.collection_id).await.unwrap().len(), 1);
    }
}
