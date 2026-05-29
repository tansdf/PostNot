import { sendRequest } from "$lib/api/commands";
import {
  createEnvironmentVariable,
  createFileRow,
  createKeyValueRow,
  type EnvironmentDetail,
  type EnvironmentVariable,
  type FileRow,
  type KeyValueRow,
  type RequestAuth,
  type RequestBody,
  type RequestDraft,
  type RequestScriptExecution,
  type ResponsePayload,
  type ScriptTestResult
} from "$lib/api/types";

const VALID_METHODS = new Set(["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"]);
const VALID_BODY_MODES = new Set(["none", "json", "raw", "form-urlencoded", "multipart"]);
const VALID_AUTH_TYPES = new Set(["none", "basic", "bearer", "api-key", "oauth2"]);
const VALID_API_KEY_PLACEMENTS = new Set(["header", "query"]);
const AsyncFunction = Object.getPrototypeOf(async function () {
  return undefined;
}).constructor as new (argumentName: string, body: string) => (pn: unknown) => Promise<unknown>;

let scriptRowCounter = 0;
let scriptTestCounter = 0;

class ScriptAssertionError extends Error {}

type VariableMap = Map<string, string>;
export type InheritedRequestScripts = {
  preRequestScript: string;
  testScript: string;
  folderScripts?: {
    name: string;
    preRequestScript: string;
    testScript: string;
  }[];
};

export type ScriptRuntimeContext = {
  activeEnvironment?: EnvironmentDetail | null;
  persistActiveEnvironment?: (environment: EnvironmentDetail) => Promise<EnvironmentDetail>;
};

const SCRIPT_WORKER_TIMEOUT_MS = 60_000;

type ScriptWorkerRequest =
  | {
      id: string;
      kind: "pre-request";
      request: RequestDraft;
      environmentVariables: EnvironmentVariable[];
      inheritedScripts: InheritedRequestScripts | null;
      activeEnvironment: EnvironmentDetail | null;
    }
  | {
      id: string;
      kind: "test";
      request: RequestDraft;
      response: ResponsePayload;
      environmentVariables: EnvironmentVariable[];
      inheritedScripts: InheritedRequestScripts | null;
      activeEnvironment: EnvironmentDetail | null;
    };

type ScriptWorkerToMainMessage =
  | { type: "done"; id: string; result: unknown }
  | { type: "error"; id: string; errorText: string }
  | { type: "http-send"; id: string; bridgeId: string; request: RequestDraft }
  | { type: "persist-environment"; id: string; bridgeId: string; environment: EnvironmentDetail };

type ScriptMainToWorkerMessage =
  | ScriptWorkerRequest
  | { type: "bridge-result"; id: string; bridgeId: string; value: unknown }
  | { type: "bridge-error"; id: string; bridgeId: string; errorText: string };

function nextScriptRowId(prefix: string) {
  scriptRowCounter += 1;
  return `script-${prefix}-${scriptRowCounter}`;
}

function nextScriptTestId() {
  scriptTestCounter += 1;
  return `script-test-${scriptTestCounter}`;
}

function normalizeScriptError(error: unknown) {
  if (error instanceof Error) {
    return error.message || error.name;
  }

  return String(error);
}

function asString(value: unknown) {
  return typeof value === "string" ? value : String(value);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function normalizeEnabled(value: unknown, fallback = true) {
  return typeof value === "boolean" ? value : fallback;
}

function buildVariableMap(environmentVariables: EnvironmentVariable[]) {
  const variables = new Map<string, string>();

  for (const variable of environmentVariables) {
    const key = variable.key.trim();
    if (!variable.enabled || !key) {
      continue;
    }

    variables.set(key, variable.value);
  }

  return variables;
}

function syncVariableMap(target: VariableMap, source: VariableMap) {
  target.clear();
  for (const [key, value] of source.entries()) {
    target.set(key, value);
  }
}

function findEnvironmentVariableIndex(variables: EnvironmentVariable[], name: string) {
  const normalizedName = name.trim().toLowerCase();
  return variables.findIndex((variable) => variable.key.trim().toLowerCase() === normalizedName);
}

function normalizeEnvironmentVariableName(name: string) {
  const normalized = asString(name).trim();
  if (!normalized) {
    throw new Error("Environment variable name is required.");
  }

  return normalized;
}

function findRowIndex(rows: KeyValueRow[], key: string) {
  const normalizedKey = key.trim().toLowerCase();
  return rows.findIndex((row) => row.key.trim().toLowerCase() === normalizedKey);
}

function buildRow(key: string, value: string, enabled = true): KeyValueRow {
  return {
    ...createKeyValueRow(),
    id: nextScriptRowId("kv"),
    key,
    value,
    enabled
  };
}

function buildFileRow(name: string, path: string, enabled = true): FileRow {
  return {
    ...createFileRow(),
    id: nextScriptRowId("file"),
    name,
    path,
    enabled
  };
}

function createEmptyExecution(): RequestScriptExecution {
  return {
    preRequestErrorText: "",
    testScriptErrorText: "",
    tests: []
  };
}

function cloneRequestDraft(request: RequestDraft): RequestDraft {
  return JSON.parse(JSON.stringify(request)) as RequestDraft;
}

function cloneEnvironmentDetail(environment: EnvironmentDetail): EnvironmentDetail {
  return JSON.parse(JSON.stringify(environment)) as EnvironmentDetail;
}

function cloneEnvironmentVariables(variables: EnvironmentVariable[]): EnvironmentVariable[] {
  return JSON.parse(JSON.stringify(variables)) as EnvironmentVariable[];
}

function cloneResponsePayload(response: ResponsePayload): ResponsePayload {
  return {
    ...response,
    headers: response.headers.map((header) => ({ ...header }))
  };
}

function cloneInheritedScripts(scripts: InheritedRequestScripts | null): InheritedRequestScripts | null {
  return scripts ? (JSON.parse(JSON.stringify(scripts)) as InheritedRequestScripts) : null;
}

function cloneScriptWorkerRequest(request: ScriptWorkerRequest): ScriptWorkerRequest {
  if (request.kind === "pre-request") {
    return {
      id: request.id,
      kind: request.kind,
      request: cloneRequestDraft(request.request),
      environmentVariables: cloneEnvironmentVariables(request.environmentVariables),
      inheritedScripts: cloneInheritedScripts(request.inheritedScripts),
      activeEnvironment: request.activeEnvironment ? cloneEnvironmentDetail(request.activeEnvironment) : null
    };
  }

  return {
    id: request.id,
    kind: request.kind,
    request: cloneRequestDraft(request.request),
    response: cloneResponsePayload(request.response),
    environmentVariables: cloneEnvironmentVariables(request.environmentVariables),
    inheritedScripts: cloneInheritedScripts(request.inheritedScripts),
    activeEnvironment: request.activeEnvironment ? cloneEnvironmentDetail(request.activeEnvironment) : null
  };
}

function isDeepEqual(left: unknown, right: unknown): boolean {
  if (Object.is(left, right)) {
    return true;
  }

  if (Array.isArray(left) || Array.isArray(right)) {
    if (!Array.isArray(left) || !Array.isArray(right) || left.length !== right.length) {
      return false;
    }

    return left.every((value, index) => isDeepEqual(value, right[index]));
  }

  if (
    typeof left === "object" &&
    left !== null &&
    typeof right === "object" &&
    right !== null
  ) {
    const leftEntries = Object.entries(left);
    const rightEntries = Object.entries(right);
    if (leftEntries.length !== rightEntries.length) {
      return false;
    }

    return leftEntries.every(([key, value]) =>
      Object.prototype.hasOwnProperty.call(right, key) &&
      isDeepEqual(value, (right as Record<string, unknown>)[key])
    );
  }

  return false;
}

function formatValue(value: unknown) {
  if (typeof value === "string") {
    return value;
  }

  try {
    return JSON.stringify(value);
  } catch {
    return String(value);
  }
}

function createExpectation(actual: unknown) {
  return {
    toBe(expected: unknown) {
      if (!Object.is(actual, expected)) {
        throw new ScriptAssertionError(`Expected ${formatValue(actual)} to be ${formatValue(expected)}.`);
      }
    },
    toEqual(expected: unknown) {
      if (!isDeepEqual(actual, expected)) {
        throw new ScriptAssertionError(`Expected ${formatValue(actual)} to equal ${formatValue(expected)}.`);
      }
    },
    toInclude(expected: unknown) {
      if (typeof actual === "string") {
        if (!actual.includes(asString(expected))) {
          throw new ScriptAssertionError(`Expected "${actual}" to include "${asString(expected)}".`);
        }
        return;
      }

      if (Array.isArray(actual)) {
        if (!actual.some((value) => isDeepEqual(value, expected))) {
          throw new ScriptAssertionError(`Expected array to include ${formatValue(expected)}.`);
        }
        return;
      }

      throw new ScriptAssertionError("toInclude only supports strings and arrays.");
    },
    toMatch(expected: RegExp | string) {
      if (typeof actual !== "string") {
        throw new ScriptAssertionError("toMatch expects a string value.");
      }

      const passed = typeof expected === "string" ? actual.includes(expected) : expected.test(actual);
      if (!passed) {
        throw new ScriptAssertionError(`Expected "${actual}" to match ${formatValue(expected)}.`);
      }
    },
    toBeTruthy() {
      if (!actual) {
        throw new ScriptAssertionError(`Expected ${formatValue(actual)} to be truthy.`);
      }
    },
    toBeFalsy() {
      if (actual) {
        throw new ScriptAssertionError(`Expected ${formatValue(actual)} to be falsy.`);
      }
    },
    toBeGreaterThan(expected: number) {
      if (typeof actual !== "number" || actual <= expected) {
        throw new ScriptAssertionError(`Expected ${formatValue(actual)} to be greater than ${expected}.`);
      }
    },
    toBeLessThan(expected: number) {
      if (typeof actual !== "number" || actual >= expected) {
        throw new ScriptAssertionError(`Expected ${formatValue(actual)} to be less than ${expected}.`);
      }
    }
  };
}

function createRequestFacade(request: RequestDraft, variables: VariableMap) {
  return {
    get name() {
      return request.name;
    },
    set name(value: string) {
      request.name = asString(value);
    },
    get method() {
      return request.method;
    },
    set method(value: string) {
      const normalized = asString(value).trim().toUpperCase();
      if (!VALID_METHODS.has(normalized)) {
        throw new Error(`Unsupported HTTP method: ${value}`);
      }

      request.method = normalized as RequestDraft["method"];
    },
    get url() {
      return request.url;
    },
    set url(value: string) {
      request.url = asString(value);
    },
    addHeader(key: string, value: string) {
      request.headers = [...request.headers, buildRow(asString(key), asString(value))];
    },
    upsertHeader(key: string, value: string) {
      const nextKey = asString(key);
      const nextValue = asString(value);
      const index = findRowIndex(request.headers, nextKey);
      if (index >= 0) {
        request.headers[index] = { ...request.headers[index], key: nextKey, value: nextValue, enabled: true };
        return;
      }

      request.headers = [...request.headers, buildRow(nextKey, nextValue)];
    },
    removeHeader(key: string) {
      const normalizedKey = asString(key).trim().toLowerCase();
      request.headers = request.headers.filter((row) => row.key.trim().toLowerCase() !== normalizedKey);
    },
    addQueryParam(key: string, value: string) {
      request.queryParams = [...request.queryParams, buildRow(asString(key), asString(value))];
    },
    upsertQueryParam(key: string, value: string) {
      const nextKey = asString(key);
      const nextValue = asString(value);
      const index = findRowIndex(request.queryParams, nextKey);
      if (index >= 0) {
        request.queryParams[index] = {
          ...request.queryParams[index],
          key: nextKey,
          value: nextValue,
          enabled: true
        };
        return;
      }

      request.queryParams = [...request.queryParams, buildRow(nextKey, nextValue)];
    },
    removeQueryParam(key: string) {
      const normalizedKey = asString(key).trim().toLowerCase();
      request.queryParams = request.queryParams.filter((row) => row.key.trim().toLowerCase() !== normalizedKey);
    },
    setRawBody(value: string) {
      request.body = {
        ...request.body,
        mode: "raw",
        raw: asString(value)
      };
    },
    setJsonBody(value: unknown) {
      request.body = {
        ...request.body,
        mode: "json",
        raw: typeof value === "string" ? value : JSON.stringify(value, null, 2)
      };
    },
    clearBody() {
      request.body = {
        ...request.body,
        mode: "none",
        raw: ""
      };
    },
    setBearerToken(token: string) {
      request.auth = {
        ...request.auth,
        type: "bearer",
        bearerToken: asString(token)
      };
    },
    setOAuth2Token(token: string) {
      request.auth = {
        ...request.auth,
        type: "oauth2",
        oauth2AccessToken: asString(token)
      };
    },
    setBasicAuth(username: string, password: string) {
      request.auth = {
        ...request.auth,
        type: "basic",
        basicUsername: asString(username),
        basicPassword: asString(password)
      };
    },
    setApiKey(name: string, value: string, placement: "header" | "query" = "header") {
      request.auth = {
        ...request.auth,
        type: "api-key",
        apiKeyName: asString(name),
        apiKeyValue: asString(value),
        apiKeyIn: placement
      };
    },
    clearAuth() {
      request.auth = {
        ...request.auth,
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
      };
    },
    getHeader(name: string) {
      const index = findRowIndex(request.headers, name);
      return index >= 0 ? request.headers[index]?.value ?? null : null;
    },
    variables
  };
}

function createVariableFacade(variables: VariableMap, runtimeContext: ScriptRuntimeContext = {}) {
  let draftEnvironment = runtimeContext.activeEnvironment
    ? cloneEnvironmentDetail(runtimeContext.activeEnvironment)
    : null;
  let hasPendingWrites = false;

  function assertWritableEnvironment() {
    if (!draftEnvironment || !runtimeContext.persistActiveEnvironment) {
      throw new Error("Active environment writes are unavailable because no active environment is selected.");
    }
  }

  function writableEnvironment(): EnvironmentDetail {
    assertWritableEnvironment();
    return draftEnvironment as EnvironmentDetail;
  }

  function syncDraftVariablesToMap() {
    if (!draftEnvironment) {
      return;
    }

    syncVariableMap(variables, buildVariableMap(draftEnvironment.variables));
  }

  return {
    facade: {
      get(name: string) {
        return variables.get(asString(name).trim()) ?? null;
      },
      has(name: string) {
        return variables.has(asString(name).trim());
      },
      all() {
        return Object.fromEntries(variables.entries());
      },
      async set(name: string, value: string, options: { secret?: boolean; enabled?: boolean } = {}) {
        const environment = writableEnvironment();

        const key = normalizeEnvironmentVariableName(name);
        const nextValue = asString(value);
        const nextEnabled = typeof options.enabled === "boolean" ? options.enabled : true;
        const existingIndex = findEnvironmentVariableIndex(environment.variables, key);

        if (existingIndex >= 0) {
          const existing = environment.variables[existingIndex]!;
          const nextVariable: EnvironmentVariable = {
            ...existing,
            key,
            value: nextValue,
            enabled: nextEnabled,
            isSecret: typeof options.secret === "boolean" ? options.secret : existing.isSecret
          };

          environment.variables[existingIndex] = nextVariable;
        } else {
          const nextVariable: EnvironmentVariable = {
            ...createEnvironmentVariable(),
            key,
            value: nextValue,
            enabled: nextEnabled,
            isSecret: options.secret ?? false
          };

          environment.variables = [...environment.variables, nextVariable];
        }

        syncDraftVariablesToMap();
        hasPendingWrites = true;
      },
      async remove(name: string) {
        const environment = writableEnvironment();

        const key = normalizeEnvironmentVariableName(name);
        environment.variables = environment.variables.filter(
          (variable) => variable.key.trim().toLowerCase() !== key.toLowerCase()
        );
        syncDraftVariablesToMap();
        hasPendingWrites = true;
      }
    },
    async flushPendingWrites() {
      if (!hasPendingWrites || !draftEnvironment || !runtimeContext.persistActiveEnvironment) {
        return;
      }

      const persisted = await runtimeContext.persistActiveEnvironment(cloneEnvironmentDetail(draftEnvironment));
      draftEnvironment = cloneEnvironmentDetail(persisted);
      syncVariableMap(variables, buildVariableMap(draftEnvironment.variables));
      hasPendingWrites = false;
    }
  };
}

function createResponseFacade(response: ResponsePayload) {
  return {
    code: response.statusCode,
    status: response.statusText,
    durationMs: response.durationMs,
    sizeBytes: response.sizeBytes,
    errorText: response.errorText,
    executedAt: response.executedAt,
    text() {
      return response.bodyText;
    },
    json() {
      return JSON.parse(response.bodyText);
    },
    header(name: string) {
      const normalizedName = asString(name).trim().toLowerCase();
      return response.headers.find((header) => header.key.trim().toLowerCase() === normalizedName)?.value ?? null;
    },
    headers: response.headers.map((header) => ({
      key: header.key,
      value: header.value
    })),
    raw: response
  };
}

function normalizeKeyValueRows(value: unknown): KeyValueRow[] {
  if (Array.isArray(value)) {
    return value.flatMap((entry) => {
      if (!isRecord(entry)) {
        return [];
      }

      return [
        buildRow(
          asString(entry.key ?? ""),
          asString(entry.value ?? ""),
          normalizeEnabled(entry.enabled)
        )
      ];
    });
  }

  if (!isRecord(value)) {
    return [];
  }

  return Object.entries(value).map(([key, entryValue]) => buildRow(key, asString(entryValue)));
}

function normalizeFileRows(value: unknown): FileRow[] {
  if (!Array.isArray(value)) {
    return [];
  }

  return value.flatMap((entry) => {
    if (typeof entry === "string") {
      return [buildFileRow("file", entry)];
    }

    if (!isRecord(entry)) {
      return [];
    }

    const path = asString(entry.path ?? "").trim();
    if (!path) {
      return [];
    }

    return [buildFileRow(asString(entry.name ?? "file"), path, normalizeEnabled(entry.enabled))];
  });
}

function normalizeBody(value: unknown): RequestBody {
  if (value == null) {
    return {
      mode: "none",
      raw: "",
      form: [],
      files: []
    };
  }

  if (typeof value === "string") {
    return {
      mode: "raw",
      raw: value,
      form: [],
      files: []
    };
  }

  if (!isRecord(value)) {
    return {
      mode: "raw",
      raw: asString(value),
      form: [],
      files: []
    };
  }

  if (typeof value.mode === "string" && VALID_BODY_MODES.has(value.mode)) {
    const raw =
      typeof value.raw === "string"
        ? value.raw
        : value.raw == null
          ? ""
          : JSON.stringify(value.raw, null, 2);

    return {
      mode: value.mode as RequestBody["mode"],
      raw,
      form: normalizeKeyValueRows(value.form),
      files: normalizeFileRows(value.files)
    };
  }

  return {
    mode: "json",
    raw: JSON.stringify(value, null, 2),
    form: [],
    files: []
  };
}

function normalizeAuth(value: unknown): RequestAuth {
  const fallback: RequestAuth = {
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
  };

  if (!isRecord(value)) {
    return fallback;
  }

  const authType: RequestAuth["type"] =
    typeof value.type === "string" && VALID_AUTH_TYPES.has(value.type)
      ? (value.type as RequestAuth["type"])
      : typeof value.oauth2AccessToken === "string"
        ? "oauth2"
        : typeof value.bearerToken === "string"
        ? "bearer"
        : typeof value.basicUsername === "string" || typeof value.basicPassword === "string"
          ? "basic"
          : typeof value.apiKeyName === "string" || typeof value.apiKeyValue === "string"
            ? "api-key"
            : "none";

  return {
    type: authType,
    basicUsername: asString(value.basicUsername ?? ""),
    basicPassword: asString(value.basicPassword ?? ""),
    bearerToken: asString(value.bearerToken ?? ""),
    apiKeyName: asString(value.apiKeyName ?? ""),
    apiKeyValue: asString(value.apiKeyValue ?? ""),
    apiKeyIn:
      typeof value.apiKeyIn === "string" && VALID_API_KEY_PLACEMENTS.has(value.apiKeyIn)
        ? (value.apiKeyIn as RequestAuth["apiKeyIn"])
        : "header",
    oauth2AccessToken: asString(value.oauth2AccessToken ?? ""),
    oauth2TokenUrl: asString(value.oauth2TokenUrl ?? ""),
    oauth2ClientId: asString(value.oauth2ClientId ?? ""),
    oauth2ClientSecret: asString(value.oauth2ClientSecret ?? ""),
    oauth2Scope: asString(value.oauth2Scope ?? "")
  };
}

function normalizeScriptRequestInput(input: unknown): RequestDraft {
  if (!isRecord(input)) {
    throw new Error("pn.http.send expects a request object.");
  }

  const url = asString(input.url ?? "").trim();
  if (!url) {
    throw new Error("pn.http.send requires a request URL.");
  }

  const method = asString(input.method ?? "GET").trim().toUpperCase();
  if (!VALID_METHODS.has(method)) {
    throw new Error(`Unsupported HTTP method: ${method}`);
  }

  return {
    name: asString(input.name ?? "Script helper request"),
    method: method as RequestDraft["method"],
    url,
    queryParams: normalizeKeyValueRows(input.queryParams ?? input.query),
    headers: normalizeKeyValueRows(input.headers),
    body: normalizeBody(input.body),
    auth: normalizeAuth(input.auth),
    preRequestScript: "",
    testScript: ""
  };
}

function createHttpFacade() {
  return {
    async send(input: unknown) {
      const preparedRequest = normalizeScriptRequestInput(input);
      const result = await sendRequest(preparedRequest, { persistHistory: false });
      return createResponseFacade(result.response);
    }
  };
}

export function createEmptyRequestScriptExecution() {
  return createEmptyExecution();
}

async function runScriptInWorker<T>(
  request: ScriptWorkerRequest,
  runtimeContext: ScriptRuntimeContext
): Promise<T> {
  if (typeof Worker === "undefined") {
    throw new Error("Script workers are unavailable in this runtime.");
  }

  const workerRequest = cloneScriptWorkerRequest(request);
  const worker = new Worker(new URL("./request-script-worker.ts", import.meta.url), {
    type: "module"
  });

  return new Promise<T>((resolve, reject) => {
    let settled = false;
    const timeoutId = window.setTimeout(() => {
      finish("reject", new Error("Script execution timed out."));
    }, SCRIPT_WORKER_TIMEOUT_MS);

    function finish(mode: "resolve" | "reject", value: T | Error) {
      if (settled) {
        return;
      }

      settled = true;
      window.clearTimeout(timeoutId);
      worker.terminate();

      if (mode === "resolve") {
        resolve(value as T);
      } else {
        reject(value);
      }
    }

    worker.onmessage = (event: MessageEvent<ScriptWorkerToMainMessage>) => {
      const message = event.data;

      if (message.id !== workerRequest.id) {
        return;
      }

      if (message.type === "done") {
        finish("resolve", message.result as T);
        return;
      }

      if (message.type === "error") {
        finish("reject", new Error(message.errorText));
        return;
      }

      if (message.type === "http-send") {
        void sendRequest(message.request, { persistHistory: false })
          .then((result) => {
            if (settled) {
              return;
            }
            worker.postMessage({
              type: "bridge-result",
              id: workerRequest.id,
              bridgeId: message.bridgeId,
              value: result.response
            } satisfies ScriptMainToWorkerMessage);
          })
          .catch((error) => {
            if (settled) {
              return;
            }
            worker.postMessage({
              type: "bridge-error",
              id: workerRequest.id,
              bridgeId: message.bridgeId,
              errorText: normalizeScriptError(error)
            } satisfies ScriptMainToWorkerMessage);
          });
        return;
      }

      if (message.type === "persist-environment") {
        if (!runtimeContext.persistActiveEnvironment) {
          worker.postMessage({
            type: "bridge-error",
            id: workerRequest.id,
            bridgeId: message.bridgeId,
            errorText: "Active environment writes are unavailable because no active environment is selected."
          } satisfies ScriptMainToWorkerMessage);
          return;
        }

        void runtimeContext.persistActiveEnvironment(message.environment)
          .then((environment) => {
            if (settled) {
              return;
            }
            worker.postMessage({
              type: "bridge-result",
              id: workerRequest.id,
              bridgeId: message.bridgeId,
              value: environment
            } satisfies ScriptMainToWorkerMessage);
          })
          .catch((error) => {
            if (settled) {
              return;
            }
            worker.postMessage({
              type: "bridge-error",
              id: workerRequest.id,
              bridgeId: message.bridgeId,
              errorText: normalizeScriptError(error)
            } satisfies ScriptMainToWorkerMessage);
          });
      }
    };

    worker.onerror = (event) => {
      finish("reject", new Error(event.message || "Script worker failed."));
    };

    worker.postMessage(workerRequest satisfies ScriptMainToWorkerMessage);
  });
}

function scriptWorkerRequestId(prefix: string) {
  return `${prefix}-${Date.now()}-${Math.random().toString(36).slice(2, 10)}`;
}

export async function runPreRequestScript(
  request: RequestDraft,
  environmentVariables: EnvironmentVariable[],
  inheritedScripts: InheritedRequestScripts | null = null,
  runtimeContext: ScriptRuntimeContext = {}
): Promise<{ request: RequestDraft; errorText: string }> {
  try {
    return await runScriptInWorker(
      {
        id: scriptWorkerRequestId("pre-request"),
        kind: "pre-request",
        request,
        environmentVariables,
        inheritedScripts,
        activeEnvironment: runtimeContext.activeEnvironment ?? null
      },
      runtimeContext
    );
  } catch (error) {
    return {
      request: cloneRequestDraft(request),
      errorText: `Script worker: ${normalizeScriptError(error)}`
    };
  }
}

async function runPreRequestScriptInPage(
  request: RequestDraft,
  environmentVariables: EnvironmentVariable[],
  inheritedScripts: InheritedRequestScripts | null = null,
  runtimeContext: ScriptRuntimeContext = {},
  workerError: unknown = null
): Promise<{ request: RequestDraft; errorText: string }> {
  const scriptSources = [
    { label: "Collection pre-request script", script: inheritedScripts?.preRequestScript ?? "" },
    ...(inheritedScripts?.folderScripts ?? []).map((folder) => ({
      label: `Folder pre-request script (${folder.name})`,
      script: folder.preRequestScript
    })),
    { label: "Pre-request script", script: request.preRequestScript }
  ].filter((source) => source.script.trim());

  if (scriptSources.length === 0) {
    return {
      request: cloneRequestDraft(request),
      errorText: workerError ? `Script worker: ${normalizeScriptError(workerError)}` : ""
    };
  }

  const preparedRequest = cloneRequestDraft(request);
  const variables = buildVariableMap(environmentVariables);
  const http = createHttpFacade();
  const variableFacade = createVariableFacade(variables, runtimeContext);
  const pn = {
    variables: variableFacade.facade,
    request: createRequestFacade(preparedRequest, variables),
    expect: createExpectation,
    http
  };

  for (const source of scriptSources) {
    try {
      const execute = new AsyncFunction("pn", '"use strict";\n' + source.script);
      await execute(pn);
    } catch (error) {
      return {
        request: preparedRequest,
        errorText: `${source.label}: ${normalizeScriptError(error)}`
      };
    }
  }

  try {
    await variableFacade.flushPendingWrites();
  } catch (error) {
    return {
      request: preparedRequest,
      errorText: `Active environment update: ${normalizeScriptError(error)}`
    };
  }

  return {
    request: preparedRequest,
    errorText: ""
  };
}

export async function runTestScript(
  request: RequestDraft,
  response: ResponsePayload,
  environmentVariables: EnvironmentVariable[],
  inheritedScripts: InheritedRequestScripts | null = null,
  runtimeContext: ScriptRuntimeContext = {}
): Promise<RequestScriptExecution> {
  if (!hasTestScriptSources(request, inheritedScripts)) {
    return createEmptyExecution();
  }

  try {
    return await runScriptInWorker(
      {
        id: scriptWorkerRequestId("test"),
        kind: "test",
        request,
        response,
        environmentVariables,
        inheritedScripts,
        activeEnvironment: runtimeContext.activeEnvironment ?? null
      },
      runtimeContext
    );
  } catch (error) {
    return {
      ...createEmptyExecution(),
      testScriptErrorText: `Script worker: ${normalizeScriptError(error)}`
    };
  }
}

function hasTestScriptSources(request: RequestDraft, inheritedScripts: InheritedRequestScripts | null) {
  if (request.testScript.trim() || inheritedScripts?.testScript.trim()) {
    return true;
  }

  return (inheritedScripts?.folderScripts ?? []).some((folder) => folder.testScript.trim());
}

async function runTestScriptInPage(
  request: RequestDraft,
  response: ResponsePayload,
  environmentVariables: EnvironmentVariable[],
  inheritedScripts: InheritedRequestScripts | null = null,
  runtimeContext: ScriptRuntimeContext = {},
  workerError: unknown = null
): Promise<RequestScriptExecution> {
  const scriptSources = [
    { label: "Collection test script", script: inheritedScripts?.testScript ?? "" },
    ...(inheritedScripts?.folderScripts ?? []).map((folder) => ({
      label: `Folder test script (${folder.name})`,
      script: folder.testScript
    })),
    { label: "Test script", script: request.testScript }
  ].filter((source) => source.script.trim());

  if (scriptSources.length === 0) {
    return {
      ...createEmptyExecution(),
      testScriptErrorText: workerError ? `Script worker: ${normalizeScriptError(workerError)}` : ""
    };
  }

  const execution = createEmptyExecution();
  const variables = buildVariableMap(environmentVariables);
  const tests: ScriptTestResult[] = [];
  const http = createHttpFacade();
  const variableFacade = createVariableFacade(variables, runtimeContext);
  let testChain = Promise.resolve();

  const pn = {
    variables: variableFacade.facade,
    response: createResponseFacade(response),
    expect: createExpectation,
    http,
    test(name: string, assertion: () => void | Promise<void>) {
      const promise = testChain.then(async () => {
        try {
          await assertion();
          tests.push({
            id: nextScriptTestId(),
            name: asString(name),
            status: "passed",
            errorText: ""
          });
        } catch (error) {
          tests.push({
            id: nextScriptTestId(),
            name: asString(name),
            status: "failed",
            errorText: normalizeScriptError(error)
          });
        }
      });

      testChain = promise;
      return promise;
    }
  };

  for (const source of scriptSources) {
    try {
      const execute = new AsyncFunction("pn", '"use strict";\n' + source.script);
      await execute(pn);
      await testChain;
    } catch (error) {
      await testChain;
      execution.testScriptErrorText = `${source.label}: ${normalizeScriptError(error)}`;
      break;
    }
  }

  if (!execution.testScriptErrorText) {
    try {
      await variableFacade.flushPendingWrites();
    } catch (error) {
      execution.testScriptErrorText = `Active environment update: ${normalizeScriptError(error)}`;
    }
  }

  execution.tests = tests;
  return execution;
}
