import { invoke } from "@tauri-apps/api/core";
import {
  type CollectionItemSummary,
  type CollectionSearchResult,
  type CollectionSidebarState,
  type CollectionSummary,
  type CreateCollectionFolderInput,
  type CreateCollectionInput,
  type MoveCollectionItemInput,
  type UpdateCollectionFolderInput,
  type CurlImportInput,
  type OpenApiRequestImportInput,
  createDefaultSettings,
  type AppSettings,
  type UpdateCheckResult,
  type EnvironmentDetail,
  type EnvironmentInput,
  type EnvironmentVariable,
  type ExportResult,
  type ImportEnvironmentInput,
  type ImportEnvironmentResult,
  type EnvironmentSummary,
  type HistoryEntryDetail,
  type HistoryEntrySummary,
  type ImportRequestInput,
  type ImportResult,
  type ImportedRequestDraft,
  type RequestWorkspaceState,
  type RequestDraft,
  type ResponsePayload,
  type SendRequestResult,
  type SavedRequestDetail,
  type SavedRequestSummary
} from "$lib/api/types";

export function hasTauriRuntime() {
  return typeof window !== "undefined" && ("__TAURI_INTERNALS__" in window || "__TAURI__" in window);
}

export type SendRequestOptions = {
  persistHistory?: boolean;
};

function createMockResponse(payload: RequestDraft): ResponsePayload {
  return {
    statusCode: 200,
    statusText: "Frontend mock",
    durationMs: 42,
    sizeBytes: payload.url.length,
    headers: [
      {
        id: "mock-header",
        key: "content-type",
        value: "application/json",
        enabled: true
      }
    ],
    bodyText: JSON.stringify(
      {
        message: "Tauri backend is not connected yet in this environment.",
        request: payload
      },
      null,
      2
    ),
    bodyBase64: "",
    bodyContentType: "application/json",
    bodyIsBinary: false,
    bodyIsTruncated: false,
    bodyTruncatedAtBytes: null,
    bodyEncoding: "utf-8",
    errorText: "",
    executedAt: new Date().toISOString()
  };
}

function createMockHistory(limit = 10): HistoryEntrySummary[] {
  return [
    {
      id: "mock-history-1",
      requestName: "Sample request",
      method: "GET" as const,
      url: "https://jsonplaceholder.typicode.com/todos/1",
      statusCode: 200,
      durationMs: 42,
      responseBodyPreview: '{\n  "message": "Tauri backend is not connected yet in this environment."\n}',
      errorText: "",
      executedAt: new Date().toISOString()
    }
  ].slice(0, limit);
}

function createMockCollections(): CollectionSummary[] {
  return [
    {
      id: "mock-collection-1",
      name: "Examples",
      description: "Sample saved requests",
      preRequestScript: "",
      testScript: "",
      requestCount: 1,
      updatedAt: new Date().toISOString()
    }
  ];
}

function createMockSavedRequests(): SavedRequestSummary[] {
  return [
    {
      id: "mock-saved-request-1",
      collectionId: "mock-collection-1",
      parentId: "mock-folder-1",
      name: "Sample request",
      method: "GET" as const,
      url: "https://jsonplaceholder.typicode.com/todos/1",
      updatedAt: new Date().toISOString()
    }
  ];
}

function createMockCollectionItems(): CollectionItemSummary[] {
  return [
    {
      id: "mock-folder-1",
      collectionId: "mock-collection-1",
      parentId: null,
      kind: "folder",
      name: "Examples",
      method: null,
      url: null,
      preRequestScript: "",
      testScript: "",
      updatedAt: new Date().toISOString(),
      children: [
        {
          id: "mock-saved-request-1",
          collectionId: "mock-collection-1",
          parentId: "mock-folder-1",
          kind: "request",
          name: "Sample request",
          method: "GET",
          url: "https://jsonplaceholder.typicode.com/todos/1",
          preRequestScript: "",
          testScript: "",
          updatedAt: new Date().toISOString(),
          children: []
        }
      ]
    }
  ];
}

function createMockEnvironments(): EnvironmentSummary[] {
  return [
    {
      id: "mock-environment-1",
      name: "Local",
      isActive: true,
      variableCount: 2,
      updatedAt: new Date().toISOString()
    }
  ];
}

function createMockEnvironmentDetail(id: string): EnvironmentDetail {
  const variables: EnvironmentVariable[] = [
    {
      id: "env-1",
      key: "base_url",
      value: "https://jsonplaceholder.typicode.com",
      enabled: true,
      isSecret: false
    },
    {
      id: "env-2",
      key: "api_token",
      value: "demo-token",
      enabled: true,
      isSecret: true
    }
  ];

  return {
    id,
    name: "Local",
    isActive: true,
    updatedAt: new Date().toISOString(),
    variables
  };
}

function createMockHistoryDetail(id: string): HistoryEntryDetail {
  const request = {
    ...createMockHistory(1)[0],
    requestSnapshot: {
      name: "Sample request",
      method: "GET" as const,
      url: "https://jsonplaceholder.typicode.com/todos/1",
      queryParams: [],
      headers: [],
      body: {
        mode: "none" as const,
        raw: "",
        form: [],
        files: []
      },
      auth: {
        type: "none" as const,
        basicUsername: "",
        basicPassword: "",
        bearerToken: "",
        apiKeyName: "",
        apiKeyValue: "",
        apiKeyIn: "header" as const
      },
      preRequestScript: "",
      testScript: ""
    },
    responseHeaders: [
      {
        id: "mock-header",
        key: "content-type",
        value: "application/json",
        enabled: true
      }
    ]
  };

  return {
    id,
    requestName: request.requestName,
    method: request.method,
    url: request.url,
    statusCode: request.statusCode,
    durationMs: request.durationMs,
    requestSnapshot: request.requestSnapshot,
    responseHeaders: request.responseHeaders,
    responseBodyText: request.responseBodyPreview,
    errorText: request.errorText,
    executedAt: request.executedAt
  };
}

function createMockSavedRequestDetail(id: string): SavedRequestDetail {
  return {
    id,
    collectionId: "mock-collection-1",
    parentId: "mock-folder-1",
    name: "Sample request",
    updatedAt: new Date().toISOString(),
    request: {
      name: "Sample request",
      method: "GET",
      url: "https://jsonplaceholder.typicode.com/todos/1",
      queryParams: [],
      headers: [],
      body: {
        mode: "none",
        raw: "",
        form: [],
        files: []
      },
      auth: {
        type: "none",
        basicUsername: "",
        basicPassword: "",
        bearerToken: "",
        apiKeyName: "",
        apiKeyValue: "",
        apiKeyIn: "header"
      },
      preRequestScript: "",
      testScript: ""
    }
  };
}

export async function sendRequest(
  payload: RequestDraft,
  options: SendRequestOptions = {}
): Promise<SendRequestResult> {
  if (!hasTauriRuntime()) {
    return {
      response: createMockResponse(payload),
      historyPersistenceError: null
    };
  }

  return invoke<SendRequestResult>("send_request", {
    payload,
    persistHistory: options.persistHistory ?? true
  });
}

export async function cancelActiveRequest(): Promise<boolean> {
  if (!hasTauriRuntime()) {
    return true;
  }

  return invoke<boolean>("cancel_active_request");
}

export async function pickMultipartFiles(): Promise<string[]> {
  if (!hasTauriRuntime()) {
    return [];
  }

  return invoke<string[]>("pick_multipart_files");
}

export async function getSettings(): Promise<AppSettings> {
  if (!hasTauriRuntime()) {
    return createDefaultSettings();
  }

  return invoke<AppSettings>("get_settings");
}

export async function updateSettings(settings: AppSettings): Promise<AppSettings> {
  if (!hasTauriRuntime()) {
    return settings;
  }

  return invoke<AppSettings>("update_settings", { settings });
}

export async function checkForUpdates(): Promise<UpdateCheckResult> {
  if (!hasTauriRuntime()) {
    return {
      configured: false,
      update: null
    };
  }

  return invoke<UpdateCheckResult>("check_for_updates");
}

export async function installUpdate(): Promise<void> {
  if (!hasTauriRuntime()) {
    return;
  }

  return invoke<void>("install_update");
}

export async function listHistory(limit = 25): Promise<HistoryEntrySummary[]> {
  if (!hasTauriRuntime()) {
    return createMockHistory(limit);
  }

  return invoke<HistoryEntrySummary[]>("list_history", { limit });
}

export async function getHistoryEntry(id: string): Promise<HistoryEntryDetail> {
  if (!hasTauriRuntime()) {
    return createMockHistoryDetail(id);
  }

  return invoke<HistoryEntryDetail>("get_history_entry", { id });
}

export async function clearHistory(): Promise<void> {
  if (!hasTauriRuntime()) {
    return;
  }

  await invoke("clear_history");
}

export async function getRequestWorkspaceState(): Promise<RequestWorkspaceState | null> {
  if (!hasTauriRuntime()) {
    return null;
  }

  return invoke<RequestWorkspaceState | null>("get_request_workspace_state");
}

export async function saveRequestWorkspaceState(state: RequestWorkspaceState): Promise<void> {
  if (!hasTauriRuntime()) {
    return;
  }

  await invoke("save_request_workspace_state", { state });
}

export async function listCollections(): Promise<CollectionSummary[]> {
  if (!hasTauriRuntime()) {
    return createMockCollections();
  }

  return invoke<CollectionSummary[]>("list_collections");
}

export async function searchCollectionEntities(
  query: string,
  limit = 30
): Promise<CollectionSearchResult[]> {
  if (!hasTauriRuntime()) {
    const collection = createMockCollections()[0];
    const item = createMockCollectionItems()[0]?.children[0];
    const results: CollectionSearchResult[] = item
      ? [
          {
            id: collection.id,
            kind: "collection",
            collectionId: collection.id,
            parentId: null,
            name: collection.name,
            method: null,
            url: null,
            updatedAt: collection.updatedAt,
            collectionName: collection.name,
            ancestorIds: [],
            ancestorNames: [],
            requestCount: collection.requestCount
          },
          {
            id: item.id,
            kind: "request",
            collectionId: item.collectionId,
            parentId: item.parentId ?? null,
            name: item.name,
            method: item.method,
            url: item.url,
            updatedAt: item.updatedAt,
            collectionName: collection.name,
            ancestorIds: item.parentId ? [item.parentId] : [],
            ancestorNames: ["Examples"],
            requestCount: null
          }
        ]
      : [];

    return results
      .filter((result) =>
        [result.name, result.url ?? "", result.method ?? ""]
          .join(" ")
          .toLowerCase()
          .includes(query.trim().toLowerCase())
      )
      .slice(0, limit);
  }

  return invoke<CollectionSearchResult[]>("search_collection_entities", { query, limit });
}

export async function getCollectionSidebarState(): Promise<CollectionSidebarState> {
  if (!hasTauriRuntime()) {
    return {
      expandedCollectionIds: [],
      expandedFolderIds: []
    };
  }

  return invoke<CollectionSidebarState>("get_collection_sidebar_state");
}

export async function saveCollectionSidebarState(sidebarState: CollectionSidebarState): Promise<void> {
  if (!hasTauriRuntime()) {
    return;
  }

  await invoke("save_collection_sidebar_state", { sidebarState });
}

export async function createCollection(input: CreateCollectionInput): Promise<CollectionSummary> {
  if (!hasTauriRuntime()) {
    return {
      id: `mock-collection-${Date.now()}`,
      name: input.name,
      description: input.description,
      preRequestScript: input.preRequestScript,
      testScript: input.testScript,
      requestCount: 0,
      updatedAt: new Date().toISOString()
    };
  }

  return invoke<CollectionSummary>("create_collection", { input });
}

export async function listCollectionItems(collectionId: string): Promise<CollectionItemSummary[]> {
  if (!hasTauriRuntime()) {
    return createMockCollectionItems().filter((item) => item.collectionId === collectionId);
  }

  return invoke<CollectionItemSummary[]>("list_collection_items", { collectionId });
}

export async function createCollectionFolder(
  collectionId: string,
  input: CreateCollectionFolderInput
): Promise<CollectionItemSummary> {
  if (!hasTauriRuntime()) {
    return {
      id: `mock-folder-${Date.now()}`,
      collectionId,
      parentId: input.parentId ?? null,
      kind: "folder",
      name: input.name,
      method: null,
      url: null,
      preRequestScript: input.preRequestScript,
      testScript: input.testScript,
      updatedAt: new Date().toISOString(),
      children: []
    };
  }

  return invoke<CollectionItemSummary>("create_collection_folder", { collectionId, input });
}

export async function updateCollectionFolder(
  itemId: string,
  input: UpdateCollectionFolderInput
): Promise<CollectionItemSummary> {
  if (!hasTauriRuntime()) {
    return {
      id: itemId,
      collectionId: "mock-collection-1",
      parentId: null,
      kind: "folder",
      name: input.name,
      method: null,
      url: null,
      preRequestScript: input.preRequestScript,
      testScript: input.testScript,
      updatedAt: new Date().toISOString(),
      children: []
    };
  }

  return invoke<CollectionItemSummary>("update_collection_folder", { itemId, input });
}

export async function moveCollectionItem(
  itemId: string,
  input: MoveCollectionItemInput
): Promise<SavedRequestSummary> {
  if (!hasTauriRuntime()) {
    return {
      id: itemId,
      collectionId: input.targetCollectionId,
      parentId: input.targetParentId ?? null,
      name: "Moved request",
      method: "GET",
      url: "https://example.com",
      updatedAt: new Date().toISOString()
    };
  }

  return invoke<SavedRequestSummary>("move_collection_item", { itemId, input });
}

export async function updateCollection(collectionId: string, input: CreateCollectionInput): Promise<CollectionSummary> {
  if (!hasTauriRuntime()) {
    return {
      id: collectionId,
      name: input.name,
      description: input.description,
      preRequestScript: input.preRequestScript,
      testScript: input.testScript,
      requestCount: 1,
      updatedAt: new Date().toISOString()
    };
  }

  return invoke<CollectionSummary>("update_collection", { collectionId, input });
}

export async function deleteCollection(collectionId: string): Promise<void> {
  if (!hasTauriRuntime()) {
    return;
  }

  await invoke("delete_collection", { collectionId });
}

export async function listSavedRequests(collectionId: string): Promise<SavedRequestSummary[]> {
  if (!hasTauriRuntime()) {
    return createMockSavedRequests().filter((item) => item.collectionId === collectionId);
  }

  return invoke<SavedRequestSummary[]>("list_saved_requests", { collectionId });
}

export async function saveRequestToCollection(
  collectionId: string,
  request: RequestDraft,
  parentId?: string | null
): Promise<SavedRequestSummary> {
  if (!hasTauriRuntime()) {
    return {
      id: `mock-saved-request-${Date.now()}`,
      collectionId,
      parentId: parentId ?? null,
      name: request.name,
      method: request.method,
      url: request.url,
      updatedAt: new Date().toISOString()
    };
  }

  return invoke<SavedRequestSummary>("save_request_to_collection", { collectionId, parentId, request });
}

export async function updateSavedRequest(itemId: string, request: RequestDraft): Promise<SavedRequestSummary> {
  if (!hasTauriRuntime()) {
    return {
      id: itemId,
      collectionId: "mock-collection-1",
      name: request.name,
      method: request.method,
      url: request.url,
      updatedAt: new Date().toISOString()
    };
  }

  return invoke<SavedRequestSummary>("update_saved_request", { itemId, request });
}

export async function getSavedRequest(itemId: string): Promise<SavedRequestDetail> {
  if (!hasTauriRuntime()) {
    return createMockSavedRequestDetail(itemId);
  }

  return invoke<SavedRequestDetail>("get_saved_request", { itemId });
}

export async function deleteSavedRequest(itemId: string): Promise<void> {
  if (!hasTauriRuntime()) {
    return;
  }

  await invoke("delete_saved_request", { itemId });
}

export async function deleteCollectionItem(itemId: string): Promise<void> {
  if (!hasTauriRuntime()) {
    return;
  }

  await invoke("delete_collection_item", { itemId });
}

export async function exportCollection(collectionId: string): Promise<ExportResult | null> {
  if (!hasTauriRuntime()) {
    return {
      filePath: `/tmp/${collectionId}.postman_collection.json`
    };
  }

  return invoke<ExportResult | null>("export_collection", { collectionId });
}

export async function importRequests(input: ImportRequestInput): Promise<ImportResult> {
  if (!hasTauriRuntime()) {
    return {
      collectionId: input.targetCollectionId ?? `mock-imported-${Date.now()}`,
      collectionName:
        input.format === "curl"
          ? "Imported cURL"
          : input.format === "openapi"
            ? "Imported OpenAPI collection"
            : "Imported Postman collection",
      importedRequestCount: input.format === "curl" ? 1 : 3,
      createdCollection: !input.targetCollectionId
    };
  }

  return invoke<ImportResult>("import_requests", { input });
}

export async function importCurlRequestToDraft(input: CurlImportInput): Promise<ImportedRequestDraft> {
  if (!hasTauriRuntime()) {
    const url = input.source.match(/https?:\/\/\S+/)?.[0] ?? "https://example.com";
    return {
      request: {
        name: `GET ${url}`,
        method: "GET",
        url,
        queryParams: [
          {
            id: `mock-query-${Date.now()}`,
            key: "",
            value: "",
            enabled: true
          }
        ],
        headers: [
          {
            id: `mock-header-${Date.now()}`,
            key: "",
            value: "",
            enabled: true
          }
        ],
        body: {
          mode: "none",
          raw: "",
          form: [
            {
              id: `mock-form-${Date.now()}`,
              key: "",
              value: "",
              enabled: true
            }
          ],
          files: []
        },
        auth: {
          type: "none",
          basicUsername: "",
          basicPassword: "",
          bearerToken: "",
          apiKeyName: "",
          apiKeyValue: "",
          apiKeyIn: "header"
        },
        preRequestScript: "",
        testScript: ""
      }
    };
  }

  return invoke<ImportedRequestDraft>("import_curl_request_to_draft", { input });
}

export async function importOpenApiRequestToDraft(
  input: OpenApiRequestImportInput
): Promise<ImportedRequestDraft> {
  if (!hasTauriRuntime()) {
    return {
      request: {
        name: "List items",
        method: "GET",
        url: "https://api.example.com/items/{{itemId}}",
        queryParams: [
          {
            id: `mock-query-${Date.now()}`,
            key: "limit",
            value: "25",
            enabled: true
          }
        ],
        headers: [
          {
            id: `mock-header-${Date.now()}`,
            key: "Accept",
            value: "application/json",
            enabled: true
          }
        ],
        body: {
          mode: "none",
          raw: "",
          form: [
            {
              id: `mock-form-${Date.now()}`,
              key: "",
              value: "",
              enabled: true
            }
          ],
          files: []
        },
        auth: {
          type: "none",
          basicUsername: "",
          basicPassword: "",
          bearerToken: "",
          apiKeyName: "",
          apiKeyValue: "",
          apiKeyIn: "header"
        },
        preRequestScript: "",
        testScript: ""
      }
    };
  }

  return invoke<ImportedRequestDraft>("import_openapi_request_to_draft", { input });
}

export async function listEnvironments(): Promise<EnvironmentSummary[]> {
  if (!hasTauriRuntime()) {
    return createMockEnvironments();
  }

  return invoke<EnvironmentSummary[]>("list_environments");
}

export async function createEnvironment(): Promise<EnvironmentDetail> {
  if (!hasTauriRuntime()) {
    return createMockEnvironmentDetail(`mock-environment-${Date.now()}`);
  }

  return invoke<EnvironmentDetail>("create_environment");
}

export async function getEnvironment(environmentId: string): Promise<EnvironmentDetail> {
  if (!hasTauriRuntime()) {
    return createMockEnvironmentDetail(environmentId);
  }

  return invoke<EnvironmentDetail>("get_environment", { environmentId });
}

export async function updateEnvironment(environmentId: string, input: EnvironmentInput): Promise<EnvironmentDetail> {
  if (!hasTauriRuntime()) {
    return {
      id: environmentId,
      name: input.name,
      isActive: false,
      updatedAt: new Date().toISOString(),
      variables: input.variables
    };
  }

  return invoke<EnvironmentDetail>("update_environment", { environmentId, input });
}

export async function deleteEnvironment(environmentId: string): Promise<void> {
  if (!hasTauriRuntime()) {
    return;
  }

  await invoke("delete_environment", { environmentId });
}

export async function setActiveEnvironment(environmentId: string | null): Promise<void> {
  if (!hasTauriRuntime()) {
    return;
  }

  await invoke("set_active_environment", { environmentId });
}

export async function importPostmanEnvironment(input: ImportEnvironmentInput): Promise<ImportEnvironmentResult> {
  if (!hasTauriRuntime()) {
    return {
      environmentId: `mock-environment-${Date.now()}`,
      environmentName: "Imported Postman environment",
      importedVariableCount: 2,
      activated: input.setActive
    };
  }

  return invoke<ImportEnvironmentResult>("import_postman_environment", { input });
}

export async function exportEnvironment(environmentId: string): Promise<ExportResult | null> {
  if (!hasTauriRuntime()) {
    return {
      filePath: `/tmp/${environmentId}.postman_environment.json`
    };
  }

  return invoke<ExportResult | null>("export_environment", { environmentId });
}
