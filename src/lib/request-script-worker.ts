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
import type { InheritedRequestScripts } from "$lib/request-scripts";

const VALID_METHODS = new Set(["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"]);
const VALID_BODY_MODES = new Set(["none", "json", "raw", "form-urlencoded", "multipart"]);
const VALID_AUTH_TYPES = new Set(["none", "basic", "bearer", "api-key", "oauth2"]);
const VALID_API_KEY_PLACEMENTS = new Set(["header", "query"]);
const AsyncFunction = Object.getPrototypeOf(async function () {
  return undefined;
}).constructor as new (argumentName: string, body: string) => (pn: unknown) => Promise<unknown>;
const SANDBOX_PREAMBLE = `"use strict";
const window = undefined;
const document = undefined;
const localStorage = undefined;
const sessionStorage = undefined;
const indexedDB = undefined;
const caches = undefined;
const fetch = undefined;
const XMLHttpRequest = undefined;
const WebSocket = undefined;
const EventSource = undefined;
const Worker = undefined;
const SharedWorker = undefined;
const importScripts = undefined;
const Function = undefined;
const self = undefined;
const globalThis = undefined;
`;
const CONCURRENT_HELPER_REQUEST_ERROR =
  "pn.http.send helper requests must be awaited sequentially. Await the previous pn.http.send(...) call before starting another one.";
const UNAWAITED_HELPER_REQUEST_ERROR =
  "Unawaited pn.http.send call detected. Use await pn.http.send(...) so the script waits for the helper request before continuing.";

type VariableMap = Map<string, string>;

type WorkerRequest =
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

type WorkerToMainMessage =
  | { type: "done"; id: string; result: unknown }
  | { type: "error"; id: string; errorText: string }
  | { type: "http-send"; id: string; bridgeId: string; request: RequestDraft }
  | { type: "persist-environment"; id: string; bridgeId: string; environment: EnvironmentDetail };

type MainToWorkerMessage =
  | WorkerRequest
  | { type: "bridge-result"; id: string; bridgeId: string; value: unknown }
  | { type: "bridge-error"; id: string; bridgeId: string; errorText: string };
type BridgeRequestMessage =
  | Omit<Extract<WorkerToMainMessage, { type: "http-send" }>, "id" | "bridgeId">
  | Omit<Extract<WorkerToMainMessage, { type: "persist-environment" }>, "id" | "bridgeId">;

type WorkerScope = {
  postMessage: (message: WorkerToMainMessage) => void;
  onmessage: ((event: MessageEvent<MainToWorkerMessage>) => void) | null;
};

const workerScope = globalThis as unknown as WorkerScope;
const bridgeCallbacks = new Map<
  string,
  {
    resolve: (value: unknown) => void;
    reject: (error: Error) => void;
  }
>();

let scriptRowCounter = 0;
let scriptTestCounter = 0;
let bridgeCounter = 0;

class ScriptAssertionError extends Error {}

function nextScriptRowId(prefix: string) {
  scriptRowCounter += 1;
  return `script-${prefix}-${scriptRowCounter}`;
}

function nextScriptTestId() {
  scriptTestCounter += 1;
  return `script-test-${scriptTestCounter}`;
}

function nextBridgeId() {
  bridgeCounter += 1;
  return `bridge-${bridgeCounter}`;
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

function cloneRequestDraft(request: RequestDraft): RequestDraft {
  return JSON.parse(JSON.stringify(request)) as RequestDraft;
}

function cloneEnvironmentDetail(environment: EnvironmentDetail): EnvironmentDetail {
  return JSON.parse(JSON.stringify(environment)) as EnvironmentDetail;
}

function buildVariableMap(environmentVariables: EnvironmentVariable[]) {
  const variables = new Map<string, string>();

  for (const variable of environmentVariables) {
    const key = variable.key.trim();
    if (variable.enabled && key) {
      variables.set(key, variable.value);
    }
  }

  return variables;
}

function syncVariableMap(target: VariableMap, source: VariableMap) {
  target.clear();
  for (const [key, value] of source.entries()) {
    target.set(key, value);
  }
}

function createEmptyExecution(): RequestScriptExecution {
  return {
    preRequestErrorText: "",
    testScriptErrorText: "",
    tests: []
  };
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
        request.queryParams[index] = { ...request.queryParams[index], key: nextKey, value: nextValue, enabled: true };
        return;
      }

      request.queryParams = [...request.queryParams, buildRow(nextKey, nextValue)];
    },
    removeQueryParam(key: string) {
      const normalizedKey = asString(key).trim().toLowerCase();
      request.queryParams = request.queryParams.filter((row) => row.key.trim().toLowerCase() !== normalizedKey);
    },
    setRawBody(value: string) {
      request.body = { ...request.body, mode: "raw", raw: asString(value) };
    },
    setJsonBody(value: unknown) {
      request.body = {
        ...request.body,
        mode: "json",
        raw: typeof value === "string" ? value : JSON.stringify(value, null, 2)
      };
    },
    clearBody() {
      request.body = { ...request.body, mode: "none", raw: "" };
    },
    setBearerToken(token: string) {
      request.auth = { ...request.auth, type: "bearer", bearerToken: asString(token) };
    },
    setOAuth2Token(token: string) {
      request.auth = { ...request.auth, type: "oauth2", oauth2AccessToken: asString(token) };
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

function bridgeToMain<T>(
  requestId: string,
  message: BridgeRequestMessage
): Promise<T> {
  const bridgeId = nextBridgeId();

  return new Promise<T>((resolve, reject) => {
    bridgeCallbacks.set(bridgeId, {
      resolve: (value) => resolve(value as T),
      reject
    });
    workerScope.postMessage({
      ...message,
      id: requestId,
      bridgeId
    } as WorkerToMainMessage);
  });
}

function createHttpFacade(requestId: string) {
  let activeSend: Promise<ResponsePayload> | null = null;

  async function waitForActiveSend() {
    if (!activeSend) {
      return;
    }

    try {
      await activeSend;
    } catch {
      // The original awaited call reports its own failure. This wait only prevents
      // the script phase from racing the native single-request boundary.
    }
  }

  return {
    facade: {
      async send(input: unknown) {
        const preparedRequest = normalizeScriptRequestInput(input);

        if (activeSend) {
          await waitForActiveSend();
          throw new Error(CONCURRENT_HELPER_REQUEST_ERROR);
        }

        activeSend = bridgeToMain<ResponsePayload>(requestId, {
          type: "http-send",
          request: preparedRequest
        });

        try {
          const response = await activeSend;
          return createResponseFacade(response);
        } finally {
          activeSend = null;
        }
      }
    },
    async assertNoPendingHelperRequest() {
      if (!activeSend) {
        return;
      }

      await waitForActiveSend();
      throw new Error(UNAWAITED_HELPER_REQUEST_ERROR);
    },
    waitForPendingHelperRequest: waitForActiveSend
  };
}

type HttpFacade = ReturnType<typeof createHttpFacade>;

async function settlePendingHelperRequest(http: HttpFacade) {
  await http.waitForPendingHelperRequest();
}

async function assertNoPendingHelperRequest(http: HttpFacade) {
  await http.assertNoPendingHelperRequest();
}

async function runScriptSource(
  source: { label: string; script: string },
  pn: unknown,
  http: HttpFacade
) {
  try {
    const execute = new AsyncFunction("pn", SANDBOX_PREAMBLE + source.script);
    await execute(pn);
    await assertNoPendingHelperRequest(http);
  } catch (error) {
    await settlePendingHelperRequest(http);
    throw error;
  }
}

function createVariableFacade(
  requestId: string,
  variables: VariableMap,
  activeEnvironment: EnvironmentDetail | null
) {
  let draftEnvironment = activeEnvironment ? cloneEnvironmentDetail(activeEnvironment) : null;
  let hasPendingWrites = false;

  function assertWritableEnvironment() {
    if (!draftEnvironment) {
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
          environment.variables[existingIndex] = {
            ...existing,
            key,
            value: nextValue,
            enabled: nextEnabled,
            isSecret: typeof options.secret === "boolean" ? options.secret : existing.isSecret
          };
        } else {
          environment.variables = [
            ...environment.variables,
            {
              ...createEnvironmentVariable(),
              key,
              value: nextValue,
              enabled: nextEnabled,
              isSecret: options.secret ?? false
            }
          ];
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
      if (!hasPendingWrites || !draftEnvironment) {
        return;
      }

      const persisted = await bridgeToMain<EnvironmentDetail>(requestId, {
        type: "persist-environment",
        environment: cloneEnvironmentDetail(draftEnvironment)
      });
      draftEnvironment = cloneEnvironmentDetail(persisted);
      syncVariableMap(variables, buildVariableMap(draftEnvironment.variables));
      hasPendingWrites = false;
    }
  };
}

function normalizeKeyValueRows(value: unknown): KeyValueRow[] {
  if (Array.isArray(value)) {
    return value.flatMap((entry) => {
      if (!isRecord(entry)) {
        return [];
      }

      return [buildRow(asString(entry.key ?? ""), asString(entry.value ?? ""), normalizeEnabled(entry.enabled))];
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
    return { mode: "none", raw: "", form: [], files: [] };
  }

  if (typeof value === "string") {
    return { mode: "raw", raw: value, form: [], files: [] };
  }

  if (!isRecord(value)) {
    return { mode: "raw", raw: asString(value), form: [], files: [] };
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

async function runPreRequestScript(request: Extract<WorkerRequest, { kind: "pre-request" }>) {
  const scriptSources = [
    { label: "Collection pre-request script", script: request.inheritedScripts?.preRequestScript ?? "" },
    ...(request.inheritedScripts?.folderScripts ?? []).map((folder) => ({
      label: `Folder pre-request script (${folder.name})`,
      script: folder.preRequestScript
    })),
    { label: "Pre-request script", script: request.request.preRequestScript }
  ].filter((source) => source.script.trim());

  if (scriptSources.length === 0) {
    return {
      request: cloneRequestDraft(request.request),
      errorText: ""
    };
  }

  const preparedRequest = cloneRequestDraft(request.request);
  const variables = buildVariableMap(request.environmentVariables);
  const http = createHttpFacade(request.id);
  const variableFacade = createVariableFacade(request.id, variables, request.activeEnvironment);
  const pn = {
    variables: variableFacade.facade,
    request: createRequestFacade(preparedRequest, variables),
    expect: createExpectation,
    http: http.facade
  };

  for (const source of scriptSources) {
    try {
      await runScriptSource(source, pn, http);
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

async function runTestScript(request: Extract<WorkerRequest, { kind: "test" }>) {
  const scriptSources = [
    { label: "Collection test script", script: request.inheritedScripts?.testScript ?? "" },
    ...(request.inheritedScripts?.folderScripts ?? []).map((folder) => ({
      label: `Folder test script (${folder.name})`,
      script: folder.testScript
    })),
    { label: "Test script", script: request.request.testScript }
  ].filter((source) => source.script.trim());

  if (scriptSources.length === 0) {
    return createEmptyExecution();
  }

  const execution = createEmptyExecution();
  const variables = buildVariableMap(request.environmentVariables);
  const tests: ScriptTestResult[] = [];
  const http = createHttpFacade(request.id);
  const variableFacade = createVariableFacade(request.id, variables, request.activeEnvironment);
  let testChain = Promise.resolve();

  const pn = {
    variables: variableFacade.facade,
    response: createResponseFacade(request.response),
    expect: createExpectation,
    http: http.facade,
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
      const execute = new AsyncFunction("pn", SANDBOX_PREAMBLE + source.script);
      await execute(pn);
      await testChain;
      await assertNoPendingHelperRequest(http);
    } catch (error) {
      await testChain;
      await settlePendingHelperRequest(http);
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

workerScope.onmessage = (event: MessageEvent<MainToWorkerMessage>) => {
  const message = event.data;

  if ("type" in message && (message.type === "bridge-result" || message.type === "bridge-error")) {
    const callback = bridgeCallbacks.get(message.bridgeId);
    if (!callback) {
      return;
    }

    bridgeCallbacks.delete(message.bridgeId);

    if (message.type === "bridge-result") {
      callback.resolve(message.value);
    } else {
      callback.reject(new Error(message.errorText));
    }
    return;
  }

  void (async () => {
    try {
      const result = message.kind === "pre-request"
        ? await runPreRequestScript(message)
        : await runTestScript(message);

      workerScope.postMessage({
        type: "done",
        id: message.id,
        result
      });
    } catch (error) {
      workerScope.postMessage({
        type: "error",
        id: message.id,
        errorText: normalizeScriptError(error)
      });
    }
  })();
};
