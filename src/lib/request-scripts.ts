import {
  createKeyValueRow,
  type EnvironmentVariable,
  type KeyValueRow,
  type RequestDraft,
  type RequestScriptExecution,
  type ResponsePayload,
  type ScriptTestResult
} from "$lib/api/types";

const VALID_METHODS = new Set(["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"]);

let scriptRowCounter = 0;
let scriptTestCounter = 0;

class ScriptAssertionError extends Error {}

type VariableMap = Map<string, string>;

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

function findRowIndex(rows: KeyValueRow[], key: string) {
  const normalizedKey = key.trim().toLowerCase();
  return rows.findIndex((row) => row.key.trim().toLowerCase() === normalizedKey);
}

function buildRow(key: string, value: string): KeyValueRow {
  return {
    ...createKeyValueRow(),
    id: nextScriptRowId("kv"),
    key,
    value,
    enabled: true
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
        apiKeyIn: "header"
      };
    },
    getHeader(name: string) {
      const index = findRowIndex(request.headers, name);
      return index >= 0 ? request.headers[index]?.value ?? null : null;
    },
    variables
  };
}

function createVariableFacade(variables: VariableMap) {
  return {
    get(name: string) {
      return variables.get(asString(name).trim()) ?? null;
    },
    has(name: string) {
      return variables.has(asString(name).trim());
    },
    all() {
      return Object.fromEntries(variables.entries());
    }
  };
}

function createResponseFacade(response: ResponsePayload) {
  return {
    code: response.statusCode,
    status: response.statusText,
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
    }))
  };
}

export function createEmptyRequestScriptExecution() {
  return createEmptyExecution();
}

export function runPreRequestScript(
  request: RequestDraft,
  environmentVariables: EnvironmentVariable[]
): { request: RequestDraft; errorText: string } {
  if (!request.preRequestScript.trim()) {
    return {
      request: cloneRequestDraft(request),
      errorText: ""
    };
  }

  const preparedRequest = cloneRequestDraft(request);
  const variables = buildVariableMap(environmentVariables);
  const pn = {
    variables: createVariableFacade(variables),
    request: createRequestFacade(preparedRequest, variables),
    expect: createExpectation
  };

  try {
    const execute = new Function("pn", '"use strict";\n' + request.preRequestScript);
    execute(pn);
    return {
      request: preparedRequest,
      errorText: ""
    };
  } catch (error) {
    return {
      request: preparedRequest,
      errorText: normalizeScriptError(error)
    };
  }
}

export function runTestScript(
  request: RequestDraft,
  response: ResponsePayload,
  environmentVariables: EnvironmentVariable[]
): RequestScriptExecution {
  if (!request.testScript.trim()) {
    return createEmptyExecution();
  }

  const execution = createEmptyExecution();
  const variables = buildVariableMap(environmentVariables);
  const tests: ScriptTestResult[] = [];

  const pn = {
    variables: createVariableFacade(variables),
    response: createResponseFacade(response),
    expect: createExpectation,
    test(name: string, assertion: () => void) {
      try {
        assertion();
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
    }
  };

  try {
    const execute = new Function("pn", '"use strict";\n' + request.testScript);
    execute(pn);
  } catch (error) {
    execution.testScriptErrorText = normalizeScriptError(error);
  }

  execution.tests = tests;
  return execution;
}
