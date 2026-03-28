export type HttpMethod = "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "HEAD" | "OPTIONS";
export type BodyMode = "none" | "json" | "raw" | "form-urlencoded" | "multipart";
export type AuthType = "none" | "basic" | "bearer" | "api-key";
export type ApiKeyPlacement = "header" | "query";

export type KeyValueRow = {
  id: string;
  key: string;
  value: string;
  enabled: boolean;
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
};

export type RequestDraft = {
  name: string;
  method: HttpMethod;
  url: string;
  queryParams: KeyValueRow[];
  headers: KeyValueRow[];
  body: RequestBody;
  auth: RequestAuth;
};

export type ResponsePayload = {
  statusCode: number | null;
  statusText: string;
  durationMs: number;
  sizeBytes: number;
  headers: KeyValueRow[];
  bodyText: string;
  errorText: string;
  executedAt: string;
};

export type AppSettings = {
  theme: string;
  uiScale: number;
  requestTimeoutMs: number;
  followRedirects: boolean;
  validateTls: boolean;
  historyLimit: number;
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
  responseBodyText: string;
  errorText: string;
  executedAt: string;
};

export type CollectionSummary = {
  id: string;
  name: string;
  description: string;
  requestCount: number;
  updatedAt: string;
};

export type CreateCollectionInput = {
  name: string;
  description: string;
};

export type ImportFormat = "postman" | "curl";

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
};

export type CurlImportInput = {
  source: string;
};

export type ImportedRequestDraft = {
  request: RequestDraft;
};

export type SavedRequestSummary = {
  id: string;
  collectionId: string;
  name: string;
  method: HttpMethod;
  url: string;
  updatedAt: string;
};

export type SavedRequestDetail = {
  id: string;
  collectionId: string;
  name: string;
  updatedAt: string;
  request: RequestDraft;
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
  variables: KeyValueRow[];
  updatedAt: string;
};

export type EnvironmentInput = {
  name: string;
  variables: KeyValueRow[];
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
      apiKeyIn: "header"
    }
  };
}

export function createDefaultSettings(): AppSettings {
  return {
    theme: "system",
    uiScale: 1,
    requestTimeoutMs: 30_000,
    followRedirects: true,
    validateTls: true,
    historyLimit: 200
  };
}
