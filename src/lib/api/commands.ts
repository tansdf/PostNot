import { invoke } from "@tauri-apps/api/core";
import { readCachedJson, UI_CACHE_KEYS } from "$lib/ui-cache";
import {
  type CollectionItemSummary,
  type CollectionSearchResult,
  type CollectionSidebarState,
  type CollectionSummary,
  type CreateCollectionFolderInput,
  type CreateCollectionInput,
  type MoveCollectionItemInput,
  type AddPlaybookStepInput,
  type UpdateCollectionFolderInput,
  type UpdatePlaybookStepInput,
  type ReorderPlaybookStepsInput,
  type CurlImportInput,
  type OpenApiRequestImportInput,
  createDefaultSettings,
  type AppSettings,
  type CreatePlaybookRunInput,
  type UpdateCheckResult,
  type EnvironmentDetail,
  type EnvironmentInput,
  type EnvironmentVariable,
  type FinishPlaybookRunInput,
  type ExportResult,
  type ImportEnvironmentInput,
  type ImportEnvironmentResult,
  type EnvironmentSummary,
  type HistoryEntryDetail,
  type HistoryEntrySummary,
  type ImportRequestInput,
  type ImportResult,
  type ImportedRequestDraft,
  type PlaybookDetail,
  type PlaybookExecutionContext,
  type PlaybookInput,
  type PlaybookRunDetail,
  type PlaybookRunStep,
  type PlaybookRunSummary,
  type PlaybookStep,
  type PlaybookSummary,
  type RequestWorkspaceState,
  type RequestWorkspaceTab,
  type RequestDraft,
  type RequestPreview,
  type RecordPlaybookRunStepInput,
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

export type ResponseBodyRow = {
  key: string;
  rowIndex: number;
  sourceLine: number;
  segmentIndex: number;
  text: string;
  continues: boolean;
};

export type ResponseBodyWindow = {
  startRow: number;
  totalRows: number;
  rows: ResponseBodyRow[];
};

export type ResponseSearchResult = {
  totalMatches: number;
  capped: boolean;
  matches: { byteOffset: number; byteLength: number; rowIndex: number }[];
};
export type ResponseSearchMatch = ResponseSearchResult["matches"][number];

function createMockResponse(payload: RequestDraft): ResponsePayload {
  return {
    statusCode: payload.method === "POST" ? 201 : 200,
    statusText: payload.method === "POST" ? "Created" : "OK",
    durationMs: 128,
    sizeBytes: 214,
    headers: [
      {
        id: "mock-header",
        key: "content-type",
        value: "application/json",
        enabled: true
      }
    ],
    body: {
      mode: "inline",
      text: JSON.stringify(
        {
          id: "note_42",
          title: payload.name || "Sample request",
          status: "draft",
          savedLocally: true
        },
        null,
        2
      ),
      sizeBytes: 214,
      contentType: "application/json",
      charset: "utf-8",
      presentation: "json"
    },
    errorText: "",
    executedAt: new Date().toISOString()
  };
}

function createMockHistory(limit = 10): HistoryEntrySummary[] {
  return [
    {
      id: "mock-history-1",
      requestName: "Create onboarding note",
      method: "POST" as const,
      url: "{{base_url}}/notes",
      statusCode: 201,
      durationMs: 128,
      responseBodyPreview: '{\n  "id": "note_42",\n  "status": "draft"\n}',
      errorText: "",
      executedAt: new Date().toISOString()
    },
    {
      id: "mock-history-2",
      requestName: "List notes",
      method: "GET" as const,
      url: "{{base_url}}/notes",
      statusCode: 200,
      durationMs: 84,
      responseBodyPreview: '[\n  { "id": "note_42", "title": "Welcome packet" }\n]',
      errorText: "",
      executedAt: new Date(Date.now() - 1000 * 60 * 12).toISOString()
    }
  ].slice(0, limit);
}

function createMockCollections(): CollectionSummary[] {
  return [
    {
      id: "mock-collection-1",
      name: "PostNot API",
      description: "Saved local-first API workflows",
      preRequestScript: "await pn.variables.set('run_started_at', new Date().toISOString());",
      testScript: "pn.test('response finished', () => pn.expect(pn.response.durationMs).toBeLessThan(1000));",
      requestCount: 4,
      updatedAt: new Date().toISOString()
    },
    {
      id: "mock-collection-2",
      name: "Import samples",
      description: "Postman, OpenAPI, and cURL examples",
      preRequestScript: "",
      testScript: "",
      requestCount: 3,
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
      name: "Create onboarding note",
      method: "POST" as const,
      url: "{{base_url}}/notes",
      updatedAt: new Date().toISOString()
    },
    {
      id: "mock-saved-request-2",
      collectionId: "mock-collection-1",
      parentId: "mock-folder-1",
      name: "List notes",
      method: "GET" as const,
      url: "{{base_url}}/notes",
      updatedAt: new Date().toISOString()
    },
    {
      id: "mock-saved-request-3",
      collectionId: "mock-collection-1",
      parentId: "mock-folder-2",
      name: "Client credentials token",
      method: "POST" as const,
      url: "{{base_url}}/oauth/token",
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
      preRequestScript: "await pn.variables.set('folder', 'notes');",
      testScript: "",
      updatedAt: new Date().toISOString(),
      children: [
        {
          id: "mock-saved-request-1",
          collectionId: "mock-collection-1",
          parentId: "mock-folder-1",
          kind: "request",
          name: "Create onboarding note",
          method: "POST",
          url: "{{base_url}}/notes",
          preRequestScript: "await pn.variables.set('request_nonce', 'docs-preview');",
          testScript: "pn.test('created note', () => pn.expect(pn.response.code).toBe(201));",
          updatedAt: new Date().toISOString(),
          children: []
        },
        {
          id: "mock-saved-request-2",
          collectionId: "mock-collection-1",
          parentId: "mock-folder-1",
          kind: "request",
          name: "List notes",
          method: "GET",
          url: "{{base_url}}/notes",
          preRequestScript: "",
          testScript: "",
          updatedAt: new Date().toISOString(),
          children: []
        }
      ]
    },
    {
      id: "mock-folder-2",
      collectionId: "mock-collection-1",
      parentId: null,
      kind: "folder",
      name: "Auth",
      method: null,
      url: null,
      preRequestScript: "",
      testScript: "",
      updatedAt: new Date().toISOString(),
      children: [
        {
          id: "mock-saved-request-3",
          collectionId: "mock-collection-1",
          parentId: "mock-folder-2",
          kind: "request",
          name: "Client credentials token",
          method: "POST",
          url: "{{base_url}}/oauth/token",
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
      name: "Local dark demo",
      isActive: true,
      variableCount: 5,
      updatedAt: new Date().toISOString()
    },
    {
      id: "mock-environment-2",
      name: "Staging",
      isActive: false,
      variableCount: 4,
      updatedAt: new Date().toISOString()
    }
  ];
}

function createMockEnvironmentDetail(id: string): EnvironmentDetail {
  const variables: EnvironmentVariable[] = [
    {
      id: "env-1",
      key: "base_url",
      value: "https://api.post-not.local",
      enabled: true,
      isSecret: false
    },
    {
      id: "env-2",
      key: "access_token",
      value: "demo-token",
      enabled: true,
      isSecret: true
    },
    {
      id: "env-3",
      key: "client_secret",
      value: "demo-client-secret",
      enabled: true,
      isSecret: true
    },
    {
      id: "env-4",
      key: "workspace_id",
      value: "wrk_local_docs",
      enabled: true,
      isSecret: false
    },
    {
      id: "env-5",
      key: "request_nonce",
      value: "generated-by-script",
      enabled: true,
      isSecret: false
    }
  ];

  return {
    id,
    name: "Local dark demo",
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
        apiKeyIn: "header" as const,
        oauth2AccessToken: "",
        oauth2TokenUrl: "",
        oauth2ClientId: "",
        oauth2ClientSecret: "",
        oauth2Scope: ""
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
    responseBody: {
      mode: "inline",
      text: request.responseBodyPreview,
      sizeBytes: request.responseBodyPreview.length,
      contentType: "application/json",
      charset: "utf-8",
      presentation: "json"
    },
    errorText: request.errorText,
    executedAt: request.executedAt
  };
}

function createMockSavedRequestDetail(id: string): SavedRequestDetail {
  return {
    id,
    collectionId: "mock-collection-1",
    parentId: "mock-folder-1",
    name: "Create onboarding note",
    updatedAt: new Date().toISOString(),
    request: {
      name: "Create onboarding note",
      method: "POST",
      url: "{{base_url}}/notes",
      queryParams: [
        {
          id: "mock-query-1",
          key: "include",
          value: "author,workspace",
          enabled: true
        }
      ],
      headers: [
        {
          id: "mock-header-1",
          key: "Accept",
          value: "application/json",
          enabled: true
        }
      ],
      body: {
        mode: "json",
        raw: '{\n  "title": "Welcome packet",\n  "labels": ["onboarding", "local-first"],\n  "published": false\n}',
        form: [],
        files: []
      },
      auth: {
        type: "oauth2",
        basicUsername: "",
        basicPassword: "",
        bearerToken: "",
        apiKeyName: "",
        apiKeyValue: "",
        apiKeyIn: "header",
        oauth2AccessToken: "{{access_token}}",
        oauth2TokenUrl: "{{base_url}}/oauth/token",
        oauth2ClientId: "postnot-desktop",
        oauth2ClientSecret: "{{client_secret}}",
        oauth2Scope: "notes:write"
      },
      preRequestScript: "await pn.variables.set('request_nonce', 'docs-preview');",
      testScript: "pn.test('created note', () => {\n  pn.expect(pn.response.code).toBe(201);\n});"
    }
  };
}

function createMockPlaybooks(): PlaybookSummary[] {
  return [
    {
      id: "mock-playbook-1",
      name: "Smoke check",
      description: "Sample sequential workflow",
      defaultDelayMs: 250,
      stopOnFailure: true,
      failOnHttpError: true,
      stepCount: 1,
      updatedAt: new Date().toISOString()
    }
  ];
}

function createMockPlaybookStep(playbookId = "mock-playbook-1"): PlaybookStep {
  const request = createMockSavedRequests()[0];
  return {
    id: "mock-playbook-step-1",
    playbookId,
    savedRequestId: request.id,
    savedRequestName: request.name,
    collectionName: "Examples",
    method: request.method,
    url: request.url,
    nameOverride: "",
    notes: "",
    enabled: true,
    sortOrder: 0,
    delayAfterMs: null,
    missingSavedRequest: false,
    updatedAt: new Date().toISOString()
  };
}

function createMockPlaybookDetail(id = "mock-playbook-1"): PlaybookDetail {
  const summary = createMockPlaybooks()[0];
  return {
    id,
    name: summary.name,
    description: summary.description,
    defaultDelayMs: summary.defaultDelayMs,
    stopOnFailure: summary.stopOnFailure,
    failOnHttpError: summary.failOnHttpError,
    steps: [
      createMockPlaybookStep(id),
      {
        ...createMockPlaybookStep(id),
        id: "mock-playbook-step-2",
        savedRequestId: "mock-saved-request-2",
        savedRequestName: "List notes",
        method: "GET",
        url: "{{base_url}}/notes",
        nameOverride: "Verify note listing",
        notes: "Runs after the create step to check the collection endpoint.",
        sortOrder: 1,
        delayAfterMs: 500
      }
    ],
    updatedAt: summary.updatedAt
  };
}

function createMockPlaybookRun(playbookId = "mock-playbook-1"): PlaybookRunSummary {
  return {
    id: `mock-playbook-run-${Date.now()}`,
    playbookId,
    status: "passed",
    startedAt: new Date().toISOString(),
    finishedAt: new Date().toISOString(),
    totalSteps: 1,
    passedSteps: 1,
    failedSteps: 0,
    skippedSteps: 0,
    totalDurationMs: 42,
    stoppedReason: ""
  };
}

function createMockPlaybookRunStep(runId: string): PlaybookRunStep {
  const request = createMockSavedRequests()[0];
  return {
    id: `mock-playbook-run-step-${Date.now()}`,
    runId,
    stepId: "mock-playbook-step-1",
    savedRequestId: request.id,
    savedRequestName: request.name,
    method: request.method,
    url: request.url,
    status: "passed",
    statusCode: 200,
    durationMs: 42,
    responseSizeBytes: 256,
    testPassedCount: 1,
    testFailedCount: 0,
    testErrorText: "",
    errorText: "",
    executedAt: new Date().toISOString()
  };
}

function waitForMockLatency(ms = 450) {
  return new Promise<void>((resolve) => {
    globalThis.setTimeout(resolve, ms);
  });
}

function createMockAvailableUpdate(): UpdateCheckResult {
  return {
    configured: true,
    update: {
      currentVersion: __APP_VERSION__,
      version: "99.0.0-dev",
      date: new Date().toISOString(),
      body: [
        "**Mock updater release**",
        "",
        "- Exercises the available update state in the dev browser.",
        "- Runs a fake download and install without restarting PostNot.",
        "- Leaves the real signed updater path untouched for Tauri builds."
      ].join("\n")
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

export async function previewRequest(payload: RequestDraft): Promise<RequestPreview> {
  if (!hasTauriRuntime()) {
    return {
      name: payload.name,
      method: payload.method,
      finalUrl: payload.url,
      queryParams: payload.queryParams.filter((row) => row.enabled && row.key.trim()),
      headers: payload.headers.filter((row) => row.enabled && row.key.trim()),
      body: payload.body,
      auth: {
        ...payload.auth,
        basicPassword: payload.auth.basicPassword ? "{{redacted}}" : "",
        bearerToken: payload.auth.bearerToken ? "{{redacted}}" : "",
        apiKeyValue: payload.auth.apiKeyValue ? "{{redacted}}" : "",
        oauth2AccessToken: payload.auth.oauth2AccessToken ? "{{redacted}}" : "",
        oauth2ClientSecret: payload.auth.oauth2ClientSecret ? "{{redacted}}" : ""
      },
      settings: {
        requestTimeoutMs: 30_000,
        followRedirects: true,
        validateTls: true,
        activeEnvironmentName: null
      },
      warnings: [],
      notes: ["Preview is read-only and does not execute pre-request scripts."]
    };
  }

  return invoke<RequestPreview>("preview_request", { payload });
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
    return {
      ...createDefaultSettings(),
      ...(readCachedJson<Partial<AppSettings>>(UI_CACHE_KEYS.settings) ?? {})
    };
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
    await waitForMockLatency();
    return createMockAvailableUpdate();
  }

  return invoke<UpdateCheckResult>("check_for_updates");
}

export async function installUpdate(): Promise<void> {
  if (!hasTauriRuntime()) {
    await waitForMockLatency(250);
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

export async function readResponseBodyWindow(input: {
  handleId: string;
  startRow: number;
  rowCount: number;
  maxBytes?: number;
  representation?: "raw" | "formatted" | "hex";
}): Promise<ResponseBodyWindow> {
  return invoke<ResponseBodyWindow>("read_response_body_window", {
    input: { ...input, maxBytes: input.maxBytes ?? 1024 * 1024, representation: input.representation ?? "raw" }
  });
}

export async function searchResponseBody(input: {
  handleId: string;
  query: string;
  caseSensitive: boolean;
  searchId: string;
  representation: "raw" | "formatted" | "hex";
}): Promise<ResponseSearchResult> {
  return invoke<ResponseSearchResult>("search_response_body", { input });
}

export async function cancelResponseSearch(searchId: string): Promise<void> {
  if (!hasTauriRuntime()) return;
  await invoke("cancel_response_search", { searchId });
}

export async function findResponseMatch(input: {
  handleId: string;
  query: string;
  caseSensitive: boolean;
  fromOffset: number;
  direction: "next" | "previous";
  wrap: boolean;
  representation: "raw" | "formatted" | "hex";
}): Promise<ResponseSearchMatch | null> {
  return invoke("find_response_match", { input });
}

export async function readResponseBodyText(handleId: string): Promise<string> {
  return invoke<string>("read_response_body_text", { handleId });
}

export async function retainResponseBody(handleId: string): Promise<void> {
  if (!hasTauriRuntime()) return;
  await invoke("retain_response_body", { handleId });
}

export async function releaseResponseBody(handleId: string): Promise<void> {
  if (!hasTauriRuntime()) return;
  await invoke("release_response_body", { handleId });
}

export async function getResponseBodyPath(handleId: string): Promise<string> {
  return invoke<string>("get_response_body_path", { handleId });
}

export async function saveResponseBody(handleId: string, suggestedName?: string): Promise<string | null> {
  return invoke<string | null>("save_response_body", { handleId, suggestedName });
}

export async function formatResponseBody(handleId: string, jobId: string): Promise<import("$lib/api/types").ResponseBody> {
  return invoke("format_response_body", { handleId, jobId });
}

export async function cancelResponseBodyJob(jobId: string): Promise<void> {
  await invoke("cancel_response_body_job", { jobId });
}

export async function getRequestWorkspaceState(): Promise<RequestWorkspaceState | null> {
  if (!hasTauriRuntime()) {
    const cachedTabs = readCachedJson<RequestWorkspaceTab[]>(UI_CACHE_KEYS.workspaceTabs);
    const cachedActiveTabId = readCachedJson<string>(UI_CACHE_KEYS.workspaceActiveTabId);

    if (cachedTabs?.length) {
      return {
        tabs: cachedTabs,
        activeTabId: cachedActiveTabId && cachedTabs.some((tab) => tab.id === cachedActiveTabId)
          ? cachedActiveTabId
          : cachedTabs[0].id
      };
    }

    const savedRequest = createMockSavedRequestDetail("mock-saved-request-1");
    return {
      tabs: [
        {
          id: "mock-workspace-tab-1",
          source: "saved",
          savedRequestId: savedRequest.id,
          collectionId: savedRequest.collectionId,
          parentId: savedRequest.parentId ?? null,
          request: savedRequest.request,
          baselineRequest: savedRequest.request,
          response: createMockResponse(savedRequest.request),
          scriptExecution: {
            preRequestErrorText: "",
            testScriptErrorText: "",
            tests: [
              { id: "mock-test-1", name: "created note", status: "passed", errorText: "" },
              { id: "mock-test-2", name: "response is JSON", status: "passed", errorText: "" }
            ]
          },
          errorText: ""
        }
      ],
      activeTabId: "mock-workspace-tab-1"
    };
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
): Promise<CollectionItemSummary> {
  if (!hasTauriRuntime()) {
    return {
      id: itemId,
      collectionId: input.targetCollectionId,
      parentId: input.targetParentId ?? null,
      kind: "request",
      name: "Moved request",
      method: "GET",
      url: "https://example.com",
      preRequestScript: "",
      testScript: "",
      updatedAt: new Date().toISOString(),
      children: []
    };
  }

  return invoke<CollectionItemSummary>("move_collection_item", { itemId, input });
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

export async function listPlaybooks(): Promise<PlaybookSummary[]> {
  if (!hasTauriRuntime()) {
    return createMockPlaybooks();
  }

  return invoke<PlaybookSummary[]>("list_playbooks");
}

export async function createPlaybook(input: PlaybookInput): Promise<PlaybookDetail> {
  if (!hasTauriRuntime()) {
    return {
      ...createMockPlaybookDetail(`mock-playbook-${Date.now()}`),
      ...input,
      steps: []
    };
  }

  return invoke<PlaybookDetail>("create_playbook", { input });
}

export async function getPlaybook(playbookId: string): Promise<PlaybookDetail> {
  if (!hasTauriRuntime()) {
    return createMockPlaybookDetail(playbookId);
  }

  return invoke<PlaybookDetail>("get_playbook", { playbookId });
}

export async function updatePlaybook(playbookId: string, input: PlaybookInput): Promise<PlaybookDetail> {
  if (!hasTauriRuntime()) {
    return {
      ...createMockPlaybookDetail(playbookId),
      ...input
    };
  }

  return invoke<PlaybookDetail>("update_playbook", { playbookId, input });
}

export async function duplicatePlaybook(playbookId: string): Promise<PlaybookDetail> {
  if (!hasTauriRuntime()) {
    return {
      ...createMockPlaybookDetail(`mock-playbook-copy-${Date.now()}`),
      name: "Smoke check copy"
    };
  }

  return invoke<PlaybookDetail>("duplicate_playbook", { playbookId });
}

export async function deletePlaybook(playbookId: string): Promise<void> {
  if (!hasTauriRuntime()) {
    return;
  }

  await invoke("delete_playbook", { playbookId });
}

export async function addPlaybookStep(
  playbookId: string,
  input: AddPlaybookStepInput
): Promise<PlaybookStep> {
  if (!hasTauriRuntime()) {
    return {
      ...createMockPlaybookStep(playbookId),
      id: `mock-playbook-step-${Date.now()}`,
      savedRequestId: input.savedRequestId,
      nameOverride: input.nameOverride,
      notes: input.notes,
      enabled: input.enabled,
      delayAfterMs: input.delayAfterMs ?? null
    };
  }

  return invoke<PlaybookStep>("add_playbook_step", { playbookId, input });
}

export async function updatePlaybookStep(
  stepId: string,
  input: UpdatePlaybookStepInput
): Promise<PlaybookStep> {
  if (!hasTauriRuntime()) {
    return {
      ...createMockPlaybookStep(),
      id: stepId,
      nameOverride: input.nameOverride,
      notes: input.notes,
      enabled: input.enabled,
      delayAfterMs: input.delayAfterMs ?? null
    };
  }

  return invoke<PlaybookStep>("update_playbook_step", { stepId, input });
}

export async function reorderPlaybookSteps(
  playbookId: string,
  input: ReorderPlaybookStepsInput
): Promise<PlaybookStep[]> {
  if (!hasTauriRuntime()) {
    return input.stepIds.map((id, index) => ({
      ...createMockPlaybookStep(playbookId),
      id,
      sortOrder: index
    }));
  }

  return invoke<PlaybookStep[]>("reorder_playbook_steps", { playbookId, input });
}

export async function deletePlaybookStep(stepId: string): Promise<void> {
  if (!hasTauriRuntime()) {
    return;
  }

  await invoke("delete_playbook_step", { stepId });
}

export async function getPlaybookExecutionContext(stepId: string): Promise<PlaybookExecutionContext> {
  if (!hasTauriRuntime()) {
    return {
      stepId,
      savedRequest: createMockSavedRequestDetail("mock-saved-request-1"),
      inheritedScripts: {
        preRequestScript: "",
        testScript: "",
        folderScripts: []
      }
    };
  }

  return invoke<PlaybookExecutionContext>("get_playbook_execution_context", { stepId });
}

export async function createPlaybookRun(input: CreatePlaybookRunInput): Promise<PlaybookRunSummary> {
  if (!hasTauriRuntime()) {
    return {
      ...createMockPlaybookRun(input.playbookId),
      status: "running",
      finishedAt: null,
      totalSteps: input.totalSteps,
      passedSteps: 0,
      failedSteps: 0,
      skippedSteps: 0,
      totalDurationMs: 0
    };
  }

  return invoke<PlaybookRunSummary>("create_playbook_run", { input });
}

export async function finishPlaybookRun(
  runId: string,
  input: FinishPlaybookRunInput
): Promise<PlaybookRunSummary> {
  if (!hasTauriRuntime()) {
    return {
      ...createMockPlaybookRun(),
      id: runId,
      status: input.status,
      stoppedReason: input.stoppedReason,
      totalDurationMs: input.totalDurationMs
    };
  }

  return invoke<PlaybookRunSummary>("finish_playbook_run", { runId, input });
}

export async function recordPlaybookRunStep(
  runId: string,
  input: RecordPlaybookRunStepInput
): Promise<PlaybookRunStep> {
  if (!hasTauriRuntime()) {
    return {
      ...createMockPlaybookRunStep(runId),
      ...input,
      id: `mock-playbook-run-step-${Date.now()}`,
      runId,
      executedAt: new Date().toISOString()
    };
  }

  return invoke<PlaybookRunStep>("record_playbook_run_step", { runId, input });
}

export async function listPlaybookRuns(playbookId: string, limit = 20): Promise<PlaybookRunSummary[]> {
  if (!hasTauriRuntime()) {
    return [createMockPlaybookRun(playbookId)];
  }

  return invoke<PlaybookRunSummary[]>("list_playbook_runs", { playbookId, limit });
}

export async function getPlaybookRun(runId: string): Promise<PlaybookRunDetail> {
  if (!hasTauriRuntime()) {
    const summary = createMockPlaybookRun();
    return {
      ...summary,
      id: runId,
      steps: [createMockPlaybookRunStep(runId)]
    };
  }

  return invoke<PlaybookRunDetail>("get_playbook_run", { runId });
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
      createdCollection: !input.targetCollectionId,
      details: {
        format: input.format,
        summary: `Mock ${input.format} import completed.`,
        importedItems: ["Example request"],
        warnings: [],
        errors: []
      }
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
          apiKeyIn: "header",
          oauth2AccessToken: "",
          oauth2TokenUrl: "",
          oauth2ClientId: "",
          oauth2ClientSecret: "",
          oauth2Scope: ""
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
          apiKeyIn: "header",
          oauth2AccessToken: "",
          oauth2TokenUrl: "",
          oauth2ClientId: "",
          oauth2ClientSecret: "",
          oauth2Scope: ""
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
