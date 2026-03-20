import { invoke } from "@tauri-apps/api/core";
import {
  type CollectionSummary,
  type CreateCollectionInput,
  createDefaultSettings,
  type AppSettings,
  type HistoryEntryDetail,
  type HistoryEntrySummary,
  type RequestDraft,
  type ResponsePayload,
  type SavedRequestDetail,
  type SavedRequestSummary
} from "$lib/api/types";

function hasTauriRuntime() {
  return typeof window !== "undefined" && ("__TAURI_INTERNALS__" in window || "__TAURI__" in window);
}

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
      name: "Sample request",
      method: "GET" as const,
      url: "https://jsonplaceholder.typicode.com/todos/1",
      updatedAt: new Date().toISOString()
    }
  ];
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
      }
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
      }
    }
  };
}

export async function sendRequest(payload: RequestDraft): Promise<ResponsePayload> {
  if (!hasTauriRuntime()) {
    return createMockResponse(payload);
  }

  return invoke<ResponsePayload>("send_request", { payload });
}

export async function cancelActiveRequest(): Promise<boolean> {
  if (!hasTauriRuntime()) {
    return true;
  }

  return invoke<boolean>("cancel_active_request");
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

export async function listCollections(): Promise<CollectionSummary[]> {
  if (!hasTauriRuntime()) {
    return createMockCollections();
  }

  return invoke<CollectionSummary[]>("list_collections");
}

export async function createCollection(input: CreateCollectionInput): Promise<CollectionSummary> {
  if (!hasTauriRuntime()) {
    return {
      id: `mock-collection-${Date.now()}`,
      name: input.name,
      description: input.description,
      requestCount: 0,
      updatedAt: new Date().toISOString()
    };
  }

  return invoke<CollectionSummary>("create_collection", { input });
}

export async function updateCollection(collectionId: string, input: CreateCollectionInput): Promise<CollectionSummary> {
  if (!hasTauriRuntime()) {
    return {
      id: collectionId,
      name: input.name,
      description: input.description,
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

export async function saveRequestToCollection(collectionId: string, request: RequestDraft): Promise<SavedRequestSummary> {
  if (!hasTauriRuntime()) {
    return {
      id: `mock-saved-request-${Date.now()}`,
      collectionId,
      name: request.name,
      method: request.method,
      url: request.url,
      updatedAt: new Date().toISOString()
    };
  }

  return invoke<SavedRequestSummary>("save_request_to_collection", { collectionId, request });
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
