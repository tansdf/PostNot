export type HttpMethod = "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "HEAD" | "OPTIONS";
export type BodyMode = "none" | "json" | "raw" | "form-urlencoded" | "multipart";
export type AuthType = "none" | "basic" | "bearer" | "api-key" | "oauth2";
export type ApiKeyPlacement = "header" | "query";

export type KeyValueRow = {
  id: string;
  key: string;
  value: string;
  enabled: boolean;
};

export type EnvironmentVariable = {
  id: string;
  key: string;
  value: string;
  enabled: boolean;
  isSecret: boolean;
};

export type FileRow = {
  id: string;
  name: string;
  path: string;
  enabled: boolean;
};

export type RequestBody = {
  mode: BodyMode;
  raw: string;
  form: KeyValueRow[];
  files: FileRow[];
};

export type RequestAuth = {
  type: AuthType;
  basicUsername: string;
  basicPassword: string;
  bearerToken: string;
  apiKeyName: string;
  apiKeyValue: string;
  apiKeyIn: ApiKeyPlacement;
  oauth2AccessToken: string;
  oauth2TokenUrl: string;
  oauth2ClientId: string;
  oauth2ClientSecret: string;
  oauth2Scope: string;
};

export type RequestDraft = {
  name: string;
  method: HttpMethod;
  url: string;
  queryParams: KeyValueRow[];
  headers: KeyValueRow[];
  body: RequestBody;
  auth: RequestAuth;
  preRequestScript: string;
  testScript: string;
};

export type ResponsePresentation = "text" | "json" | "image" | "binary";

export type ResponseBody =
  | {
      mode: "inline";
      text: string;
      sizeBytes: number;
      contentType: string | null;
      charset: string | null;
      presentation: ResponsePresentation;
    }
  | {
      mode: "file";
      handleId: string;
      previewText: string;
      sizeBytes: number;
      contentType: string | null;
      charset: string | null;
      presentation: ResponsePresentation;
    };

export type ResponsePayload = {
  statusCode: number | null;
  statusText: string;
  durationMs: number;
  sizeBytes: number;
  headers: KeyValueRow[];
  body: ResponseBody;
  errorText: string;
  executedAt: string;
};

export type SendRequestResult = {
  response: ResponsePayload;
  historyPersistenceError: string | null;
};

export type RequestResponseProgress = {
  requestId: string;
  downloadedBytes: number;
  contentLength: number | null;
  finished: boolean;
};

export type RequestPreviewSettings = {
  requestTimeoutMs: number;
  followRedirects: boolean;
  validateTls: boolean;
  activeEnvironmentName: string | null;
};

export type RequestPreview = {
  name: string;
  method: HttpMethod;
  finalUrl: string;
  queryParams: KeyValueRow[];
  headers: KeyValueRow[];
  body: RequestBody;
  auth: RequestAuth;
  settings: RequestPreviewSettings;
  warnings: string[];
  notes: string[];
};

export type AppSettings = {
  theme: string;
  uiScale: number;
  requestTimeoutMs: number;
  followRedirects: boolean;
  validateTls: boolean;
  historyLimit: number;
  isHistoryCollapsed: boolean;
  environmentAutosave: boolean;
  notificationTimeoutMs: number;
  lastUpdateCheckedAt: string | null;
};

export type AvailableUpdate = {
  currentVersion: string;
  version: string;
  date: string | null;
  body: string | null;
};

export type UpdateCheckResult = {
  configured: boolean;
  update: AvailableUpdate | null;
};

export type UpdateDownloadProgress = {
  downloadedBytes: number;
  contentLength: number | null;
  finished: boolean;
};

export type HistoryEntrySummary = {
  id: string;
  requestName: string;
  method: HttpMethod;
  url: string;
  statusCode: number | null;
  durationMs: number;
  responseBodyPreview: string;
  errorText: string;
  executedAt: string;
};

export type HistoryEntryDetail = {
  id: string;
  requestName: string;
  method: HttpMethod;
  url: string;
  statusCode: number | null;
  durationMs: number;
  requestSnapshot: RequestDraft;
  responseHeaders: KeyValueRow[];
  responseBody: ResponseBody;
  errorText: string;
  executedAt: string;
};

export type CollectionSummary = {
  id: string;
  name: string;
  description: string;
  preRequestScript: string;
  testScript: string;
  requestCount: number;
  updatedAt: string;
};

export type CreateCollectionInput = {
  name: string;
  description: string;
  preRequestScript: string;
  testScript: string;
};

export type CreateCollectionFolderInput = {
  name: string;
  parentId?: string | null;
  preRequestScript: string;
  testScript: string;
};

export type UpdateCollectionFolderInput = {
  name: string;
  preRequestScript: string;
  testScript: string;
};

export type MoveCollectionItemInput = {
  targetCollectionId: string;
  targetParentId?: string | null;
  targetIndex?: number | null;
};

export type CollectionItemSummary = {
  id: string;
  collectionId: string;
  parentId?: string | null;
  kind: "folder" | "request";
  name: string;
  method?: HttpMethod | null;
  url?: string | null;
  preRequestScript: string;
  testScript: string;
  updatedAt: string;
  children: CollectionItemSummary[];
};

export type CollectionSidebarState = {
  expandedCollectionIds: string[];
  expandedFolderIds: string[];
};

export type CollectionSearchResult = {
  id: string;
  kind: "collection" | "folder" | "request";
  collectionId: string;
  parentId?: string | null;
  name: string;
  method?: HttpMethod | null;
  url?: string | null;
  updatedAt: string;
  collectionName: string;
  ancestorIds: string[];
  ancestorNames: string[];
  requestCount?: number | null;
};

export type ImportFormat = "postman" | "curl" | "openapi";

export type ImportRequestInput = {
  format: ImportFormat;
  source: string;
  targetCollectionId?: string | null;
};

export type ImportResult = {
  collectionId: string;
  collectionName: string;
  importedRequestCount: number;
  createdCollection: boolean;
  details?: ImportDetails | null;
};

export type ImportDetails = {
  format: string;
  summary: string;
  importedItems: string[];
  warnings: string[];
  errors: string[];
};

export type CurlImportInput = {
  source: string;
};

export type OpenApiRequestImportInput = {
  source: string;
};

export type ImportedRequestDraft = {
  request: RequestDraft;
};

export type SavedRequestSummary = {
  id: string;
  collectionId: string;
  parentId?: string | null;
  name: string;
  method: HttpMethod;
  url: string;
  updatedAt: string;
};

export type SavedRequestDetail = {
  id: string;
  collectionId: string;
  parentId?: string | null;
  name: string;
  updatedAt: string;
  request: RequestDraft;
};

export type PlaybookSummary = {
  id: string;
  name: string;
  description: string;
  defaultDelayMs: number;
  stopOnFailure: boolean;
  failOnHttpError: boolean;
  stepCount: number;
  updatedAt: string;
};

export type PlaybookDetail = {
  id: string;
  name: string;
  description: string;
  defaultDelayMs: number;
  stopOnFailure: boolean;
  failOnHttpError: boolean;
  steps: PlaybookStep[];
  updatedAt: string;
};

export type PlaybookInput = {
  name: string;
  description: string;
  defaultDelayMs: number;
  stopOnFailure: boolean;
  failOnHttpError: boolean;
};

export type AddPlaybookStepInput = {
  savedRequestId: string;
  nameOverride: string;
  notes: string;
  enabled: boolean;
  delayAfterMs?: number | null;
};

export type UpdatePlaybookStepInput = {
  nameOverride: string;
  notes: string;
  enabled: boolean;
  delayAfterMs?: number | null;
};

export type ReorderPlaybookStepsInput = {
  stepIds: string[];
};

export type PlaybookStep = {
  id: string;
  playbookId: string;
  savedRequestId?: string | null;
  savedRequestName: string;
  collectionName?: string | null;
  method?: HttpMethod | null;
  url?: string | null;
  nameOverride: string;
  notes: string;
  enabled: boolean;
  sortOrder: number;
  delayAfterMs?: number | null;
  missingSavedRequest: boolean;
  updatedAt: string;
};

export type PlaybookFolderScripts = {
  name: string;
  preRequestScript: string;
  testScript: string;
};

export type PlaybookInheritedScripts = {
  preRequestScript: string;
  testScript: string;
  folderScripts: PlaybookFolderScripts[];
};

export type PlaybookExecutionContext = {
  stepId: string;
  savedRequest: SavedRequestDetail;
  inheritedScripts: PlaybookInheritedScripts;
};

export type PlaybookRunStatus = "running" | "passed" | "failed" | "canceled";
export type PlaybookRunStepStatus = "passed" | "failed" | "skipped" | "canceled";

export type CreatePlaybookRunInput = {
  playbookId: string;
  totalSteps: number;
};

export type FinishPlaybookRunInput = {
  status: PlaybookRunStatus;
  stoppedReason: string;
  totalDurationMs: number;
};

export type RecordPlaybookRunStepInput = {
  stepId?: string | null;
  savedRequestId?: string | null;
  savedRequestName: string;
  method: string;
  url: string;
  status: PlaybookRunStepStatus;
  statusCode?: number | null;
  durationMs: number;
  responseSizeBytes: number;
  testPassedCount: number;
  testFailedCount: number;
  testErrorText: string;
  errorText: string;
};

export type PlaybookRunSummary = {
  id: string;
  playbookId: string;
  status: PlaybookRunStatus;
  startedAt: string;
  finishedAt?: string | null;
  totalSteps: number;
  passedSteps: number;
  failedSteps: number;
  skippedSteps: number;
  totalDurationMs: number;
  stoppedReason: string;
};

export type PlaybookRunStep = {
  id: string;
  runId: string;
  stepId?: string | null;
  savedRequestId?: string | null;
  savedRequestName: string;
  method: string;
  url: string;
  status: PlaybookRunStepStatus;
  statusCode?: number | null;
  durationMs: number;
  responseSizeBytes: number;
  testPassedCount: number;
  testFailedCount: number;
  testErrorText: string;
  errorText: string;
  executedAt: string;
};

export type PlaybookRunDetail = PlaybookRunSummary & {
  steps: PlaybookRunStep[];
};

export type EnvironmentSummary = {
  id: string;
  name: string;
  isActive: boolean;
  variableCount: number;
  updatedAt: string;
};

export type EnvironmentDetail = {
  id: string;
  name: string;
  isActive: boolean;
  variables: EnvironmentVariable[];
  updatedAt: string;
};

export type EnvironmentInput = {
  name: string;
  variables: EnvironmentVariable[];
};

export type ImportEnvironmentInput = {
  source: string;
  setActive: boolean;
};

export type ImportEnvironmentResult = {
  environmentId: string;
  environmentName: string;
  importedVariableCount: number;
  activated: boolean;
};

export type ExportResult = {
  filePath: string;
};

export type ScriptTestResult = {
  id: string;
  name: string;
  status: "passed" | "failed";
  errorText: string;
};

export type RequestScriptExecution = {
  preRequestErrorText: string;
  testScriptErrorText: string;
  tests: ScriptTestResult[];
};

export type RequestWorkspaceTabSource = "blank" | "saved" | "imported" | "history";

export type RequestWorkspaceTab = {
  id: string;
  source: RequestWorkspaceTabSource;
  savedRequestId: string | null;
  collectionId: string | null;
  parentId: string | null;
  sourceUpdatedAt: string | null;
  externallyChanged: boolean;
  request: RequestDraft;
  baselineRequest: RequestDraft | null;
  response: ResponsePayload | null;
  scriptExecution: RequestScriptExecution;
  errorText: string;
};

export type RequestWorkspaceState = {
  tabs: RequestWorkspaceTab[];
  activeTabId: string;
};

export type AgentActivityEntry = {
  id: number;
  batchId: string;
  occurredAt: string;
  actorName: string;
  actorVersion: string;
  sessionId: string;
  operation: string;
  outcome: "succeeded" | "failed";
  targetKind: string;
  targetId: string | null;
  targetName: string;
  collectionId: string | null;
  changedFields: string[];
  errorCode: string | null;
  errorMessage: string | null;
};

export type AgentActivityPage = {
  entries: AgentActivityEntry[];
  latestId: number;
};

export type McpSetupInfo = {
  executablePath: string;
  arguments: string[];
  genericConfigJson: string;
  codexConfigToml: string;
  claudeConfigJson: string;
  cursorConfigJson: string;
};

function createId() {
  if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
    return crypto.randomUUID();
  }

  return `id-${Date.now()}-${Math.random().toString(36).slice(2, 10)}`;
}

export function createKeyValueRow(): KeyValueRow {
  return {
    id: createId(),
    key: "",
    value: "",
    enabled: true
  };
}

export function createEnvironmentVariable(): EnvironmentVariable {
  return {
    id: createId(),
    key: "",
    value: "",
    enabled: true,
    isSecret: false
  };
}

export function createFileRow(): FileRow {
  return {
    id: createId(),
    name: "file",
    path: "",
    enabled: true
  };
}

export function createRequestDraft(): RequestDraft {
  return {
    name: "Untitled request",
    method: "GET",
    url: "https://jsonplaceholder.typicode.com/todos/1",
    queryParams: [createKeyValueRow()],
    headers: [createKeyValueRow()],
    body: {
      mode: "none",
      raw: "",
      form: [createKeyValueRow()],
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
  };
}

export function createDefaultSettings(): AppSettings {
  return {
    theme: "system",
    uiScale: 1,
    requestTimeoutMs: 30_000,
    followRedirects: true,
    validateTls: true,
    historyLimit: 200,
    isHistoryCollapsed: false,
    environmentAutosave: true,
    notificationTimeoutMs: 5_000,
    lastUpdateCheckedAt: null
  };
}

function deepCloneSerializable<T>(value: T): T {
  return JSON.parse(JSON.stringify(value)) as T;
}

export function cloneRequestDraft(request: RequestDraft): RequestDraft {
  const cloned = deepCloneSerializable(request);
  const defaultAuth = createRequestDraft().auth;

  return {
    ...cloned,
    auth: {
      ...defaultAuth,
      ...(cloned.auth ?? {}),
      apiKeyIn: cloned.auth?.apiKeyIn === "query" ? "query" : "header"
    }
  };
}

export function cloneResponsePayload(response: ResponsePayload): ResponsePayload {
  return {
    ...response,
    body: { ...response.body },
    headers: response.headers.map((header) => ({ ...header }))
  };
}

export function inlineResponseText(response: ResponsePayload): string {
  return response.body.mode === "inline" ? response.body.text : response.body.previewText;
}

export function cloneRequestScriptExecution(execution: RequestScriptExecution): RequestScriptExecution {
  return deepCloneSerializable(execution);
}

export function cloneRequestWorkspaceTab(tab: RequestWorkspaceTab): RequestWorkspaceTab {
  return {
    ...tab,
    request: cloneRequestDraft(tab.request),
    baselineRequest: tab.baselineRequest ? cloneRequestDraft(tab.baselineRequest) : null,
    response: tab.response ? cloneResponsePayload(tab.response) : null,
    scriptExecution: cloneRequestScriptExecution(tab.scriptExecution)
  };
}

export function cloneRequestWorkspaceState(state: RequestWorkspaceState): RequestWorkspaceState {
  return {
    tabs: state.tabs.map(cloneRequestWorkspaceTab),
    activeTabId: state.activeTabId
  };
}
