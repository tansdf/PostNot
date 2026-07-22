use serde::Deserialize;
use sqlx::SqlitePool;
use std::collections::HashMap;
use uuid::Uuid;

use crate::{
    domain::{
        collections::CreateCollectionInput,
        imports::{ImportDetails, ImportResult, ImportedRequestDraft},
        requests::{FileRow, KeyValueRow, RequestAuth, RequestBody, SendRequestPayload},
    },
    error::{AppError, AppResult},
    services::collections_service,
};

use super::shared::{
    create_empty_request_payload, empty_auth, empty_body, empty_kv, json_value_to_input_string,
};

#[derive(Debug)]
struct OpenApiImportedRequest {
    folder_name: Option<String>,
    request: SendRequestPayload,
}

#[derive(Debug, Clone, Deserialize)]
struct OpenApiDocument {
    #[serde(default)]
    openapi: String,
    info: OpenApiInfo,
    #[serde(default)]
    servers: Vec<OpenApiServer>,
    security: Option<Vec<OpenApiSecurityRequirement>>,
    #[serde(default)]
    paths: HashMap<String, OpenApiPathItem>,
    #[serde(default)]
    components: OpenApiComponents,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct OpenApiInfo {
    #[serde(default)]
    title: String,
    #[serde(default)]
    description: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct OpenApiComponents {
    #[serde(default)]
    parameters: HashMap<String, OpenApiParameter>,
    #[serde(default, rename = "requestBodies")]
    request_bodies: HashMap<String, OpenApiRequestBody>,
    #[serde(default)]
    schemas: HashMap<String, OpenApiSchema>,
    #[serde(default)]
    examples: HashMap<String, OpenApiExample>,
    #[serde(default, rename = "securitySchemes")]
    security_schemes: HashMap<String, OpenApiSecurityScheme>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct OpenApiServer {
    #[serde(default)]
    url: String,
    #[serde(default)]
    variables: HashMap<String, OpenApiServerVariable>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct OpenApiServerVariable {
    #[serde(default)]
    default: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct OpenApiPathItem {
    #[serde(default)]
    servers: Vec<OpenApiServer>,
    #[serde(default)]
    parameters: Vec<OpenApiReferenceOr<OpenApiParameter>>,
    get: Option<OpenApiOperation>,
    post: Option<OpenApiOperation>,
    put: Option<OpenApiOperation>,
    patch: Option<OpenApiOperation>,
    delete: Option<OpenApiOperation>,
    head: Option<OpenApiOperation>,
    options: Option<OpenApiOperation>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct OpenApiOperation {
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    description: String,
    #[serde(rename = "operationId", default)]
    operation_id: String,
    #[serde(default)]
    servers: Vec<OpenApiServer>,
    #[serde(default)]
    parameters: Vec<OpenApiReferenceOr<OpenApiParameter>>,
    #[serde(rename = "requestBody")]
    request_body: Option<OpenApiReferenceOr<OpenApiRequestBody>>,
    security: Option<Vec<OpenApiSecurityRequirement>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum OpenApiReferenceOr<T> {
    Ref {
        #[serde(rename = "$ref")]
        ref_path: String,
    },
    Item(T),
}

#[derive(Debug, Clone, Deserialize, Default)]
struct OpenApiParameter {
    #[serde(default)]
    name: String,
    #[serde(rename = "in", default)]
    location: String,
    #[serde(default)]
    required: bool,
    example: Option<serde_json::Value>,
    #[serde(default)]
    examples: HashMap<String, OpenApiReferenceOr<OpenApiExample>>,
    schema: Option<OpenApiReferenceOr<OpenApiSchema>>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct OpenApiRequestBody {
    #[serde(default)]
    content: HashMap<String, OpenApiMediaType>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct OpenApiMediaType {
    example: Option<serde_json::Value>,
    #[serde(default)]
    examples: HashMap<String, OpenApiReferenceOr<OpenApiExample>>,
    schema: Option<OpenApiReferenceOr<OpenApiSchema>>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct OpenApiExample {
    value: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct OpenApiSchema {
    #[serde(rename = "type", default)]
    schema_type: String,
    #[serde(default)]
    format: String,
    #[serde(default)]
    properties: HashMap<String, OpenApiReferenceOr<OpenApiSchema>>,
    #[serde(default)]
    required: Vec<String>,
    items: Option<Box<OpenApiReferenceOr<OpenApiSchema>>>,
    #[serde(rename = "enum", default)]
    enum_values: Vec<serde_json::Value>,
    default: Option<serde_json::Value>,
    example: Option<serde_json::Value>,
    #[serde(rename = "allOf", default)]
    all_of: Vec<OpenApiReferenceOr<OpenApiSchema>>,
    #[serde(rename = "oneOf", default)]
    one_of: Vec<OpenApiReferenceOr<OpenApiSchema>>,
    #[serde(rename = "anyOf", default)]
    any_of: Vec<OpenApiReferenceOr<OpenApiSchema>>,
}

type OpenApiSecurityRequirement = HashMap<String, Vec<String>>;

#[derive(Debug, Clone, Deserialize, Default)]
struct OpenApiSecurityScheme {
    #[serde(rename = "type", default)]
    scheme_type: String,
    #[serde(default)]
    scheme: String,
    #[serde(default)]
    name: String,
    #[serde(rename = "in", default)]
    location: String,
}

pub(super) fn import_openapi_to_draft(source: &str) -> AppResult<ImportedRequestDraft> {
    let requests = build_openapi_requests(&parse_openapi_document(source)?)?;
    match requests.len() {
        0 => Err(AppError::Message(
            "No operations were found in this OpenAPI 3 document.".to_string(),
        )),
        1 => Ok(ImportedRequestDraft {
            request: requests
                .into_iter()
                .next()
                .map(|item| item.request)
                .unwrap_or_else(create_empty_request_payload),
        }),
        count => Err(AppError::Message(format!(
            "This OpenAPI 3 document contains {count} operations. Import it from Collections to create a request collection, or trim the file down to one operation to load a single request."
        ))),
    }
}

pub(super) async fn import_openapi_collection(
    pool: &SqlitePool,
    source: &str,
) -> AppResult<ImportResult> {
    let document = parse_openapi_document(source)?;
    let requests = build_openapi_requests(&document)?;
    if requests.is_empty() {
        return Err(AppError::Message(
            "No operations were found in this OpenAPI 3 document.".to_string(),
        ));
    }

    let collection_name = if document.info.title.trim().is_empty() {
        "Imported OpenAPI collection".to_string()
    } else {
        document.info.title.trim().to_string()
    };

    let mut folders = Vec::new();
    let mut folder_ids = HashMap::<String, String>::new();
    let mut next_sort_order = HashMap::<Option<String>, i64>::new();
    let mut imported_requests = Vec::new();
    let mut imported_items = Vec::new();

    for imported in &requests {
        let parent_id = if let Some(folder_name) = imported.folder_name.as_deref() {
            Some(
                folder_ids
                    .entry(folder_name.to_string())
                    .or_insert_with(|| {
                        let id = Uuid::new_v4().to_string();
                        let sort_order = next_sort_order.entry(None).or_insert(0);
                        let folder_sort_order = *sort_order;
                        *sort_order += 1;
                        folders.push(collections_service::ImportCollectionFolder {
                            id: id.clone(),
                            parent_id: None,
                            sort_order: folder_sort_order,
                            name: folder_name.to_string(),
                            pre_request_script: String::new(),
                            test_script: String::new(),
                        });
                        id
                    })
                    .clone(),
            )
        } else {
            None
        };

        let sort_order = next_sort_order.entry(parent_id.clone()).or_insert(0);
        let request_sort_order = *sort_order;
        *sort_order += 1;
        imported_items.push(imported.request.name.clone());
        imported_requests.push(collections_service::ImportCollectionRequest {
            parent_id,
            sort_order: request_sort_order,
            request: imported.request.clone(),
        });
    }

    let created_collection = collections_service::import_collection_atomic(
        pool,
        &CreateCollectionInput {
            name: collection_name,
            description: document.info.description.trim().to_string(),
            pre_request_script: String::new(),
            test_script: String::new(),
        },
        &folders,
        &imported_requests,
    )
    .await?;

    Ok(ImportResult {
        collection_id: created_collection.id,
        collection_name: created_collection.name,
        imported_request_count: imported_requests.len(),
        created_collection: true,
        details: Some(ImportDetails {
            format: "openapi".to_string(),
            summary: format!(
                "{} request{} imported from OpenAPI.",
                imported_requests.len(),
                if imported_requests.len() == 1 {
                    ""
                } else {
                    "s"
                }
            ),
            imported_items,
            warnings: Vec::new(),
            errors: Vec::new(),
        }),
    })
}

fn parse_openapi_document(source: &str) -> AppResult<OpenApiDocument> {
    let parsed_json = serde_json::from_str::<OpenApiDocument>(source);
    let document = match parsed_json {
        Ok(document) => document,
        Err(json_error) => {
            serde_yaml::from_str::<OpenApiDocument>(source).map_err(|yaml_error| {
                AppError::Message(format!(
                "Invalid OpenAPI 3 JSON or YAML: {json_error}; YAML parse also failed: {yaml_error}"
            ))
            })?
        }
    };

    if !document.openapi.trim().starts_with("3.") {
        return Err(AppError::Message(
            "Only OpenAPI 3.x documents are supported right now.".to_string(),
        ));
    }

    Ok(document)
}

fn build_openapi_requests(document: &OpenApiDocument) -> AppResult<Vec<OpenApiImportedRequest>> {
    let mut requests = Vec::new();
    let mut path_entries: Vec<_> = document.paths.iter().collect();
    path_entries.sort_by_key(|(path, _)| *path);

    for (path, path_item) in path_entries {
        push_openapi_operation(
            &mut requests,
            document,
            path,
            path_item,
            "GET",
            path_item.get.as_ref(),
        )?;
        push_openapi_operation(
            &mut requests,
            document,
            path,
            path_item,
            "POST",
            path_item.post.as_ref(),
        )?;
        push_openapi_operation(
            &mut requests,
            document,
            path,
            path_item,
            "PUT",
            path_item.put.as_ref(),
        )?;
        push_openapi_operation(
            &mut requests,
            document,
            path,
            path_item,
            "PATCH",
            path_item.patch.as_ref(),
        )?;
        push_openapi_operation(
            &mut requests,
            document,
            path,
            path_item,
            "DELETE",
            path_item.delete.as_ref(),
        )?;
        push_openapi_operation(
            &mut requests,
            document,
            path,
            path_item,
            "HEAD",
            path_item.head.as_ref(),
        )?;
        push_openapi_operation(
            &mut requests,
            document,
            path,
            path_item,
            "OPTIONS",
            path_item.options.as_ref(),
        )?;
    }

    Ok(requests)
}

fn push_openapi_operation(
    requests: &mut Vec<OpenApiImportedRequest>,
    document: &OpenApiDocument,
    path: &str,
    path_item: &OpenApiPathItem,
    method: &str,
    operation: Option<&OpenApiOperation>,
) -> AppResult<()> {
    let Some(operation) = operation else {
        return Ok(());
    };

    requests.push(map_openapi_operation(
        document, path, path_item, method, operation,
    )?);
    Ok(())
}

fn map_openapi_operation(
    document: &OpenApiDocument,
    path: &str,
    path_item: &OpenApiPathItem,
    method: &str,
    operation: &OpenApiOperation,
) -> AppResult<OpenApiImportedRequest> {
    let folder_name = operation
        .tags
        .iter()
        .map(|tag| tag.trim())
        .find(|tag| !tag.is_empty())
        .map(|tag| tag.to_string());

    let url = build_openapi_request_url(document, path_item, operation, path);
    let parameters = collect_openapi_parameters(document, path_item, operation)?;
    let mut query_params = Vec::new();
    let mut headers = Vec::new();

    for parameter in parameters {
        let value = openapi_parameter_value(document, &parameter);
        match parameter.location.as_str() {
            "query" => query_params.push(KeyValueRow {
                id: Uuid::new_v4().to_string(),
                key: parameter.name,
                value,
                enabled: true,
            }),
            "header" => headers.push(KeyValueRow {
                id: Uuid::new_v4().to_string(),
                key: parameter.name,
                value,
                enabled: true,
            }),
            _ => {}
        }
    }

    let (body, content_type_header) =
        map_openapi_request_body(document, operation.request_body.as_ref())?;
    if let Some(content_type) = content_type_header {
        upsert_header(&mut headers, "Content-Type", &content_type);
    }

    let auth = map_openapi_auth(document, operation);
    let name = openapi_request_name(method, path, operation);

    Ok(OpenApiImportedRequest {
        folder_name,
        request: SendRequestPayload {
            name,
            method: method.to_string(),
            url,
            query_params: if query_params.is_empty() {
                vec![empty_kv()]
            } else {
                query_params
            },
            headers: if headers.is_empty() {
                vec![empty_kv()]
            } else {
                headers
            },
            body,
            auth,
            pre_request_script: String::new(),
            test_script: String::new(),
        },
    })
}

fn build_openapi_request_url(
    document: &OpenApiDocument,
    path_item: &OpenApiPathItem,
    operation: &OpenApiOperation,
    path: &str,
) -> String {
    let server_url = operation
        .servers
        .first()
        .or_else(|| path_item.servers.first())
        .or_else(|| document.servers.first())
        .map(server_url_to_string)
        .unwrap_or_else(|| "{{baseUrl}}".to_string());

    join_openapi_server_and_path(&server_url, path)
}

fn collect_openapi_parameters(
    document: &OpenApiDocument,
    path_item: &OpenApiPathItem,
    operation: &OpenApiOperation,
) -> AppResult<Vec<OpenApiParameter>> {
    let mut merged = Vec::<OpenApiParameter>::new();

    for parameter in &path_item.parameters {
        merge_openapi_parameter(&mut merged, resolve_openapi_parameter(document, parameter)?);
    }
    for parameter in &operation.parameters {
        merge_openapi_parameter(&mut merged, resolve_openapi_parameter(document, parameter)?);
    }

    Ok(merged)
}

fn merge_openapi_parameter(parameters: &mut Vec<OpenApiParameter>, parameter: OpenApiParameter) {
    if let Some(existing_index) = parameters.iter().position(|existing| {
        existing.location.eq_ignore_ascii_case(&parameter.location)
            && existing.name.eq_ignore_ascii_case(&parameter.name)
    }) {
        parameters[existing_index] = parameter;
    } else {
        parameters.push(parameter);
    }
}

fn resolve_openapi_parameter(
    document: &OpenApiDocument,
    parameter: &OpenApiReferenceOr<OpenApiParameter>,
) -> AppResult<OpenApiParameter> {
    match parameter {
        OpenApiReferenceOr::Item(parameter) => Ok(parameter.clone()),
        OpenApiReferenceOr::Ref { ref_path } => {
            resolve_openapi_component_ref(ref_path, "#/components/parameters/").and_then(|name| {
                document
                    .components
                    .parameters
                    .get(name)
                    .cloned()
                    .ok_or_else(|| {
                        AppError::Message(format!(
                            "OpenAPI parameter reference `{ref_path}` could not be resolved."
                        ))
                    })
            })
        }
    }
}

fn resolve_openapi_request_body(
    document: &OpenApiDocument,
    request_body: &OpenApiReferenceOr<OpenApiRequestBody>,
) -> AppResult<OpenApiRequestBody> {
    match request_body {
        OpenApiReferenceOr::Item(request_body) => Ok(request_body.clone()),
        OpenApiReferenceOr::Ref { ref_path } => {
            resolve_openapi_component_ref(ref_path, "#/components/requestBodies/").and_then(
                |name| {
                    document
                        .components
                        .request_bodies
                        .get(name)
                        .cloned()
                        .ok_or_else(|| {
                            AppError::Message(format!(
                                "OpenAPI request body reference `{ref_path}` could not be resolved."
                            ))
                        })
                },
            )
        }
    }
}

fn resolve_openapi_schema(
    document: &OpenApiDocument,
    schema: &OpenApiReferenceOr<OpenApiSchema>,
) -> AppResult<OpenApiSchema> {
    match schema {
        OpenApiReferenceOr::Item(schema) => Ok(schema.clone()),
        OpenApiReferenceOr::Ref { ref_path } => {
            resolve_openapi_component_ref(ref_path, "#/components/schemas/").and_then(|name| {
                document
                    .components
                    .schemas
                    .get(name)
                    .cloned()
                    .ok_or_else(|| {
                        AppError::Message(format!(
                            "OpenAPI schema reference `{ref_path}` could not be resolved."
                        ))
                    })
            })
        }
    }
}

fn resolve_openapi_example(
    document: &OpenApiDocument,
    example: &OpenApiReferenceOr<OpenApiExample>,
) -> AppResult<OpenApiExample> {
    match example {
        OpenApiReferenceOr::Item(example) => Ok(example.clone()),
        OpenApiReferenceOr::Ref { ref_path } => {
            resolve_openapi_component_ref(ref_path, "#/components/examples/").and_then(|name| {
                document
                    .components
                    .examples
                    .get(name)
                    .cloned()
                    .ok_or_else(|| {
                        AppError::Message(format!(
                            "OpenAPI example reference `{ref_path}` could not be resolved."
                        ))
                    })
            })
        }
    }
}

fn resolve_openapi_security_scheme(
    document: &OpenApiDocument,
    name: &str,
) -> Option<OpenApiSecurityScheme> {
    document.components.security_schemes.get(name).cloned()
}

fn resolve_openapi_component_ref<'a>(ref_path: &'a str, prefix: &str) -> AppResult<&'a str> {
    ref_path.strip_prefix(prefix).ok_or_else(|| {
        AppError::Message(format!(
            "Unsupported OpenAPI reference `{ref_path}`. Only local component references are supported."
        ))
    })
}

fn openapi_parameter_value(document: &OpenApiDocument, parameter: &OpenApiParameter) -> String {
    if let Some(example) = parameter.example.as_ref() {
        return json_value_to_input_string(example);
    }

    if let Some(example) = parameter
        .examples
        .values()
        .next()
        .and_then(|example| resolve_openapi_example(document, example).ok())
        .and_then(|example| example.value)
    {
        return json_value_to_input_string(&example);
    }

    parameter
        .schema
        .as_ref()
        .and_then(|schema| resolve_openapi_schema(document, schema).ok())
        .and_then(|schema| openapi_schema_example_value(document, &schema).ok())
        .map(|value| json_value_to_input_string(&value))
        .unwrap_or_else(|| {
            if parameter.location.eq_ignore_ascii_case("path") || parameter.required {
                format!("{{{{{}}}}}", parameter.name)
            } else {
                String::new()
            }
        })
}

fn map_openapi_request_body(
    document: &OpenApiDocument,
    request_body: Option<&OpenApiReferenceOr<OpenApiRequestBody>>,
) -> AppResult<(RequestBody, Option<String>)> {
    let Some(request_body) = request_body else {
        return Ok((empty_body(), None));
    };

    let request_body = resolve_openapi_request_body(document, request_body)?;
    let Some((content_type, media_type)) = select_openapi_media_type(&request_body.content) else {
        return Ok((empty_body(), None));
    };

    if is_json_media_type(content_type) {
        let json_body = openapi_media_type_example_value(document, media_type)
            .or_else(|| {
                media_type
                    .schema
                    .as_ref()
                    .and_then(|schema| resolve_openapi_schema(document, schema).ok())
                    .and_then(|schema| openapi_schema_example_value(document, &schema).ok())
            })
            .unwrap_or_else(|| serde_json::Value::Object(Default::default()));

        return Ok((
            RequestBody {
                mode: "json".to_string(),
                raw: serde_json::to_string_pretty(&json_body).unwrap_or_else(|_| "{}".to_string()),
                form: vec![empty_kv()],
                files: vec![],
            },
            Some(content_type.to_string()),
        ));
    }

    if content_type.eq_ignore_ascii_case("application/x-www-form-urlencoded") {
        return Ok((
            RequestBody {
                mode: "form-urlencoded".to_string(),
                raw: String::new(),
                form: openapi_form_rows(document, media_type)?,
                files: vec![],
            },
            Some(content_type.to_string()),
        ));
    }

    if content_type.eq_ignore_ascii_case("multipart/form-data") {
        let (form, files) = openapi_multipart_rows(document, media_type)?;
        return Ok((
            RequestBody {
                mode: "multipart".to_string(),
                raw: String::new(),
                form,
                files,
            },
            Some(content_type.to_string()),
        ));
    }

    let raw_body = openapi_media_type_example_value(document, media_type)
        .or_else(|| {
            media_type
                .schema
                .as_ref()
                .and_then(|schema| resolve_openapi_schema(document, schema).ok())
                .and_then(|schema| openapi_schema_example_value(document, &schema).ok())
        })
        .map(|value| raw_body_value_to_string(content_type, &value))
        .unwrap_or_default();

    Ok((
        RequestBody {
            mode: "raw".to_string(),
            raw: raw_body,
            form: vec![empty_kv()],
            files: vec![],
        },
        Some(content_type.to_string()),
    ))
}

fn openapi_form_rows(
    document: &OpenApiDocument,
    media_type: &OpenApiMediaType,
) -> AppResult<Vec<KeyValueRow>> {
    let example_object = openapi_media_type_example_value(document, media_type);
    let mut rows = Vec::new();

    if let Some(serde_json::Value::Object(values)) = example_object {
        for (key, value) in values {
            rows.push(KeyValueRow {
                id: Uuid::new_v4().to_string(),
                key,
                value: json_value_to_input_string(&value),
                enabled: true,
            });
        }
    }

    if !rows.is_empty() {
        return Ok(rows);
    }

    let Some(schema) = media_type
        .schema
        .as_ref()
        .map(|schema| resolve_openapi_schema(document, schema))
        .transpose()?
    else {
        return Ok(vec![empty_kv()]);
    };

    let mut properties: Vec<_> = schema.properties.into_iter().collect();
    properties.sort_by(|(left, _), (right, _)| left.cmp(right));

    for (name, property_schema) in properties {
        let property_schema = resolve_openapi_schema(document, &property_schema)?;
        if openapi_schema_is_binary(&property_schema) {
            continue;
        }

        let value = openapi_schema_example_value(document, &property_schema)
            .map(|value| json_value_to_input_string(&value))
            .unwrap_or_default();

        rows.push(KeyValueRow {
            id: Uuid::new_v4().to_string(),
            key: name,
            value,
            enabled: true,
        });
    }

    Ok(if rows.is_empty() {
        vec![empty_kv()]
    } else {
        rows
    })
}

fn openapi_multipart_rows(
    document: &OpenApiDocument,
    media_type: &OpenApiMediaType,
) -> AppResult<(Vec<KeyValueRow>, Vec<FileRow>)> {
    let mut form = Vec::new();
    let mut files = Vec::new();

    let Some(schema) = media_type
        .schema
        .as_ref()
        .map(|schema| resolve_openapi_schema(document, schema))
        .transpose()?
    else {
        return Ok((vec![empty_kv()], vec![]));
    };

    let example_object = openapi_media_type_example_value(document, media_type);
    let example_values = match example_object {
        Some(serde_json::Value::Object(values)) => values,
        _ => serde_json::Map::new(),
    };

    let mut properties: Vec<_> = schema.properties.into_iter().collect();
    properties.sort_by(|(left, _), (right, _)| left.cmp(right));

    for (name, property_schema) in properties {
        let property_schema = resolve_openapi_schema(document, &property_schema)?;
        if openapi_schema_is_binary(&property_schema) {
            files.push(FileRow {
                id: Uuid::new_v4().to_string(),
                name,
                path: String::new(),
                enabled: true,
            });
            continue;
        }

        let value = example_values
            .get(&name)
            .map(json_value_to_input_string)
            .or_else(|| {
                openapi_schema_example_value(document, &property_schema)
                    .map(|value| json_value_to_input_string(&value))
                    .ok()
            })
            .unwrap_or_default();

        form.push(KeyValueRow {
            id: Uuid::new_v4().to_string(),
            key: name,
            value,
            enabled: true,
        });
    }

    Ok((
        if form.is_empty() {
            vec![empty_kv()]
        } else {
            form
        },
        files,
    ))
}

fn openapi_media_type_example_value(
    document: &OpenApiDocument,
    media_type: &OpenApiMediaType,
) -> Option<serde_json::Value> {
    media_type.example.clone().or_else(|| {
        media_type
            .examples
            .values()
            .next()
            .and_then(|example| resolve_openapi_example(document, example).ok())
            .and_then(|example| example.value)
    })
}

fn openapi_schema_example_value(
    document: &OpenApiDocument,
    schema: &OpenApiSchema,
) -> AppResult<serde_json::Value> {
    if let Some(example) = schema.example.clone() {
        return Ok(example);
    }
    if let Some(default) = schema.default.clone() {
        return Ok(default);
    }
    if let Some(enum_value) = schema.enum_values.first().cloned() {
        return Ok(enum_value);
    }

    if let Some(composed_schema) = schema
        .all_of
        .first()
        .or_else(|| schema.one_of.first())
        .or_else(|| schema.any_of.first())
    {
        return resolve_openapi_schema(document, composed_schema)
            .and_then(|schema| openapi_schema_example_value(document, &schema));
    }

    match schema.schema_type.as_str() {
        "object" => {
            let mut object = serde_json::Map::new();
            let mut properties: Vec<_> = schema.properties.iter().collect();
            properties.sort_by_key(|(name, _)| *name);

            for (name, property_schema) in properties {
                let property_schema = resolve_openapi_schema(document, property_schema)?;
                if schema.required.contains(name)
                    || property_schema.default.is_some()
                    || property_schema.example.is_some()
                {
                    object.insert(
                        name.clone(),
                        openapi_schema_example_value(document, &property_schema)?,
                    );
                }
            }

            Ok(serde_json::Value::Object(object))
        }
        "array" => {
            let item = schema
                .items
                .as_ref()
                .map(|item| resolve_openapi_schema(document, item))
                .transpose()?
                .map(|item| openapi_schema_example_value(document, &item))
                .transpose()?
                .unwrap_or(serde_json::Value::Null);

            Ok(serde_json::Value::Array(vec![item]))
        }
        "integer" => Ok(serde_json::json!(0)),
        "number" => Ok(serde_json::json!(0)),
        "boolean" => Ok(serde_json::json!(false)),
        "string" => Ok(serde_json::Value::String(String::new())),
        _ => Ok(serde_json::Value::String(String::new())),
    }
}

fn select_openapi_media_type(
    content: &HashMap<String, OpenApiMediaType>,
) -> Option<(&str, &OpenApiMediaType)> {
    let mut entries: Vec<_> = content.iter().collect();
    entries.sort_by_key(|(media_type, _)| *media_type);

    let preferred = [
        "application/json",
        "application/x-www-form-urlencoded",
        "multipart/form-data",
        "text/plain",
    ];

    for preferred_content_type in preferred {
        if let Some((content_type, media_type)) = entries
            .iter()
            .find(|(content_type, _)| content_type.eq_ignore_ascii_case(preferred_content_type))
        {
            return Some((content_type.as_str(), *media_type));
        }
    }

    entries
        .iter()
        .find(|(content_type, _)| is_json_media_type(content_type))
        .map(|(content_type, media_type)| (content_type.as_str(), *media_type))
        .or_else(|| {
            entries
                .first()
                .map(|(content_type, media_type)| (content_type.as_str(), *media_type))
        })
}

fn map_openapi_auth(document: &OpenApiDocument, operation: &OpenApiOperation) -> RequestAuth {
    let security_sets = operation
        .security
        .as_ref()
        .or(document.security.as_ref())
        .cloned()
        .unwrap_or_default();

    for security_set in security_sets {
        for scheme_name in security_set.keys() {
            let Some(scheme) = resolve_openapi_security_scheme(document, scheme_name) else {
                continue;
            };

            match scheme.scheme_type.as_str() {
                "http" if scheme.scheme.eq_ignore_ascii_case("basic") => {
                    return RequestAuth {
                        auth_type: "basic".to_string(),
                        basic_username: String::new(),
                        basic_password: String::new(),
                        ..empty_auth()
                    };
                }
                "http" if scheme.scheme.eq_ignore_ascii_case("bearer") => {
                    return RequestAuth {
                        auth_type: "bearer".to_string(),
                        bearer_token: String::new(),
                        ..empty_auth()
                    };
                }
                "apiKey" => {
                    return RequestAuth {
                        auth_type: "api-key".to_string(),
                        api_key_name: scheme.name,
                        api_key_value: String::new(),
                        api_key_in: if scheme.location.eq_ignore_ascii_case("query") {
                            "query".to_string()
                        } else {
                            "header".to_string()
                        },
                        ..empty_auth()
                    };
                }
                _ => {}
            }
        }
    }

    empty_auth()
}

fn openapi_request_name(method: &str, path: &str, operation: &OpenApiOperation) -> String {
    if !operation.summary.trim().is_empty() {
        return operation.summary.trim().to_string();
    }
    if !operation.operation_id.trim().is_empty() {
        return operation.operation_id.trim().to_string();
    }
    if !operation.description.trim().is_empty() {
        let first_line = operation
            .description
            .lines()
            .next()
            .unwrap_or_default()
            .trim();
        if !first_line.is_empty() {
            return first_line.to_string();
        }
    }

    format!("{method} {path}")
}

fn server_url_to_string(server: &OpenApiServer) -> String {
    let mut url = server.url.trim().to_string();
    for (name, variable) in &server.variables {
        let replacement = if variable.default.is_empty() {
            format!("{{{{{name}}}}}")
        } else {
            variable.default.clone()
        };

        url = url.replace(&format!("{{{name}}}"), &replacement);
    }

    url
}

fn join_openapi_server_and_path(server_url: &str, path: &str) -> String {
    let server_url = server_url.trim_end_matches('/');
    let resolved_path = path_parameterized_path(path);
    if server_url.is_empty() {
        return resolved_path;
    }

    if resolved_path.starts_with('/') {
        format!("{server_url}{resolved_path}")
    } else {
        format!("{server_url}/{resolved_path}")
    }
}

fn path_parameterized_path(path: &str) -> String {
    let mut result = String::with_capacity(path.len());
    let mut chars = path.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '{' {
            let mut name = String::new();
            for next in chars.by_ref() {
                if next == '}' {
                    break;
                }
                name.push(next);
            }

            if name.trim().is_empty() {
                result.push_str("{}");
            } else {
                result.push_str("{{");
                result.push_str(name.trim());
                result.push_str("}}");
            }
            continue;
        }

        result.push(ch);
    }

    result
}

fn upsert_header(headers: &mut Vec<KeyValueRow>, key: &str, value: &str) {
    if let Some(existing) = headers
        .iter_mut()
        .find(|header| header.key.eq_ignore_ascii_case(key))
    {
        existing.value = value.to_string();
        existing.enabled = true;
        return;
    }

    headers.push(KeyValueRow {
        id: Uuid::new_v4().to_string(),
        key: key.to_string(),
        value: value.to_string(),
        enabled: true,
    });
}

fn is_json_media_type(content_type: &str) -> bool {
    content_type.eq_ignore_ascii_case("application/json")
        || content_type.to_ascii_lowercase().contains("+json")
}

fn openapi_schema_is_binary(schema: &OpenApiSchema) -> bool {
    schema.schema_type.eq_ignore_ascii_case("string")
        && (schema.format.eq_ignore_ascii_case("binary")
            || schema.format.eq_ignore_ascii_case("base64"))
}

fn raw_body_value_to_string(content_type: &str, value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(value) if !is_json_media_type(content_type) => value.clone(),
        _ => {
            if is_json_media_type(content_type) {
                serde_json::to_string_pretty(value).unwrap_or_default()
            } else {
                json_value_to_input_string(value)
            }
        }
    }
}
