<script lang="ts">
  import { pickMultipartFiles } from "$lib/api/commands";
  import {
    createFileRow,
    createKeyValueRow,
    type AuthType,
    type BodyMode,
    type EnvironmentVariable,
    type FileRow,
    type KeyValueRow,
    type RequestDraft
  } from "$lib/api/types";
  import ScriptEditor from "$lib/components/request/ScriptEditor.svelte";
  import VariableField from "$lib/components/request/VariableField.svelte";
  import { insertTextIntoEditableControl } from "$lib/dom-editing";
  import { formatScript } from "$lib/script-formatting";
  import type { Attachment } from "svelte/attachments";

  let {
    request = $bindable(),
    isSending = false,
    isCanceling = false,
    isSaving = false,
    saveLabel = "Save",
    saveDisabled = false,
    sendDisabled = false,
    environmentVariables = [],
    handleNewRequest = () => {},
    handleOpenImport = () => {},
    handleOpenExport = () => {},
    handleOpenPreview = () => {},
    handleSendRequest = () => {},
    handleCancelRequest = () => {},
    handleSaveRequest = () => {},
    showSaveMenu = false,
    handleSaveAsRequest = () => {},
    activeEnvironmentName = "",
    handleFetchOAuth2Token = undefined
  }: {
    request: RequestDraft;
    isSending?: boolean;
    isCanceling?: boolean;
    isSaving?: boolean;
    saveLabel?: string;
    saveDisabled?: boolean;
    sendDisabled?: boolean;
    environmentVariables?: EnvironmentVariable[];
    handleNewRequest?: () => Promise<void> | void;
    handleOpenImport?: () => Promise<void> | void;
    handleOpenExport?: () => Promise<void> | void;
    handleOpenPreview?: () => Promise<void> | void;
    handleSendRequest?: () => Promise<void> | void;
    handleCancelRequest?: () => Promise<void> | void;
    handleSaveRequest?: () => Promise<void> | void;
    showSaveMenu?: boolean;
    handleSaveAsRequest?: () => Promise<void> | void;
    activeEnvironmentName?: string;
    handleFetchOAuth2Token?: (options: { persistToEnvironment: boolean }) => Promise<{
      accessToken: string;
      persistedToEnvironment: boolean;
      expiresIn: number | null;
      tokenType: string;
    }>;
  } = $props();

  let activePanel: "query" | "headers" | "body" | "auth" | "scripts" = $state("query");
  let isSaveMenuOpen = $state(false);
  let saveSplitRootNode: HTMLDivElement | null = null;

  const attachSaveSplitRoot: Attachment<HTMLDivElement> = (node) => {
    saveSplitRootNode = node;
    return () => {
      if (saveSplitRootNode === node) {
        saveSplitRootNode = null;
      }
    };
  };

  function closeSaveMenuOnDocumentClick(event: MouseEvent) {
    if (!isSaveMenuOpen) {
      return;
    }
    const root = saveSplitRootNode;
    if (!root || root.contains(event.target as Node)) {
      return;
    }
    isSaveMenuOpen = false;
  }

  function closeSaveMenuOnWindowKeydown(event: KeyboardEvent) {
    if (!isSaveMenuOpen) {
      return;
    }
    if (event.key === "Escape") {
      isSaveMenuOpen = false;
    }
  }

  let jsonValidationError = $state("");
  let multipartErrorText = $state("");
  let isPickingMultipartFiles = $state(false);
  let isFetchingOAuth2Token = $state(false);
  let oauth2FetchErrorText = $state("");
  let oauth2FetchStatusText = $state("");
  let shouldPersistOAuth2Token = $state(true);

  const panels = [
    { id: "query", label: "Query" },
    { id: "headers", label: "Headers" },
    { id: "body", label: "Body" },
    { id: "auth", label: "Auth" },
    { id: "scripts", label: "Scripts" }
  ] as const;

  const PRE_REQUEST_SCRIPT_PLACEHOLDER =
    "pn.request.upsertHeader('X-Trace-Id', pn.variables.get('trace_id') ?? 'local-run');";

  const TEST_SCRIPT_PLACEHOLDER = `pn.test('status is 200', () => {
  pn.expect(pn.response.code).toBe(200);
});`;

  const GENERAL_HEADER_NAMES = [
    "Accept",
    "Accept-Encoding",
    "Accept-Language",
    "Authorization",
    "Cache-Control",
    "Connection",
    "Content-Encoding",
    "Content-Length",
    "Content-Type",
    "Cookie",
    "Host",
    "If-Match",
    "If-Modified-Since",
    "If-None-Match",
    "If-Unmodified-Since",
    "Origin",
    "Pragma",
    "Prefer",
    "Range",
    "Referer",
    "User-Agent",
    "X-API-Key",
    "X-Request-ID",
    "X-Trace-ID"
  ];

  const GENERAL_HEADER_VALUE_SUGGESTIONS: Record<string, string[]> = {
    accept: ["application/json", "application/xml", "text/plain", "text/html", "*/*"],
    "accept-encoding": ["gzip, deflate, br", "gzip", "identity"],
    "accept-language": ["en-US,en;q=0.9", "en-US", "en"],
    authorization: ["Bearer {{oauth_access_token}}", "Bearer ", "Basic "],
    "cache-control": ["no-cache", "no-store", "max-age=0", "max-age=3600"],
    connection: ["keep-alive", "close"],
    "content-encoding": ["gzip", "br", "deflate", "identity"],
    "content-type": [
      "application/json",
      "application/x-www-form-urlencoded",
      "multipart/form-data",
      "text/plain",
      "application/xml",
      "text/html"
    ],
    cookie: ["session=", "token="],
    "if-match": ["*"],
    "if-none-match": ["*"],
    origin: ["http://localhost:3000", "http://localhost:5173"],
    pragma: ["no-cache"],
    prefer: ["return=representation", "return=minimal"],
    range: ["bytes=0-"],
    referer: ["http://localhost:3000", "http://localhost:5173"],
    "user-agent": ["PostNot"],
    "x-api-key": ["{{api_key}}"],
    "x-request-id": ["{{$guid}}"],
    "x-trace-id": ["{{$guid}}"]
  };

  let canPersistOAuth2Token = $derived(Boolean(activeEnvironmentName && handleFetchOAuth2Token));
  let headerNameSuggestions = $derived(getHeaderNameSuggestions(request.headers));

  function splitUrlAndQuery(value: string) {
    const hashIndex = value.indexOf("#");
    const hash = hashIndex >= 0 ? value.slice(hashIndex) : "";
    const beforeHash = hashIndex >= 0 ? value.slice(0, hashIndex) : value;
    const queryIndex = beforeHash.indexOf("?");

    if (queryIndex < 0) {
      return {
        baseUrl: value,
        queryString: ""
      };
    }

    return {
      baseUrl: `${beforeHash.slice(0, queryIndex)}${hash}`,
      queryString: beforeHash.slice(queryIndex + 1)
    };
  }

  function safeDecodeQueryValue(value: string) {
    try {
      return decodeURIComponent(value.replace(/\+/g, " "));
    } catch {
      return value;
    }
  }

  function parseQueryRows(queryString: string) {
    if (!queryString.trim()) {
      return [];
    }

    return queryString
      .split("&")
      .filter((segment) => segment.length > 0)
      .map((segment) => {
        const [rawKey, ...rawValueParts] = segment.split("=");
        return {
          id: createKeyValueRow().id,
          key: safeDecodeQueryValue(rawKey ?? ""),
          value: safeDecodeQueryValue(rawValueParts.join("=")),
          enabled: true
        };
      });
  }

  function buildDisplayUrl(baseUrl: string, rows: KeyValueRow[]) {
    const activeRows = rows.filter((row) => row.enabled && row.key.trim());
    if (activeRows.length === 0) {
      return baseUrl;
    }

    const hashIndex = baseUrl.indexOf("#");
    const hash = hashIndex >= 0 ? baseUrl.slice(hashIndex) : "";
    const beforeHash = hashIndex >= 0 ? baseUrl.slice(0, hashIndex) : baseUrl;
    const queryString = activeRows
      .map((row) => `${row.key}${row.value.length > 0 ? `=${row.value}` : ""}`)
      .join("&");

    return `${beforeHash}?${queryString}${hash}`;
  }

  function syncUrlInput(nextValue: string) {
    const { baseUrl, queryString } = splitUrlAndQuery(nextValue);
    const parsedRows = parseQueryRows(queryString);

    request = {
      ...request,
      url: baseUrl,
      queryParams: parsedRows.length > 0 ? parsedRows : [createKeyValueRow()]
    };
  }

  function toggleRow(kind: "queryParams" | "headers", index: number, enabled: boolean) {
    updateRows(kind, index, { enabled });
  }

  function toggleFormEnabled(index: number, enabled: boolean) {
    updateFormRow(index, { enabled });
  }

  let displayUrl = $derived(buildDisplayUrl(request.url, request.queryParams));

  function updateRows(kind: "queryParams" | "headers", index: number, patch: Partial<KeyValueRow>) {
    const nextRows = request[kind].map((row, rowIndex) => (rowIndex === index ? { ...row, ...patch } : row));
    request = { ...request, [kind]: nextRows };
  }

  function normalizeHeaderName(value: string) {
    return value.trim().toLowerCase();
  }

  function uniqueStrings(values: string[]) {
    const seen: Record<string, true> = {};
    const result: string[] = [];

    for (const value of values) {
      const trimmedValue = value.trim();
      const lookupKey = trimmedValue.toLowerCase();

      if (!trimmedValue || seen[lookupKey]) {
        continue;
      }

      seen[lookupKey] = true;
      result.push(trimmedValue);
    }

    return result;
  }

  function getHeaderNameSuggestions(rows: KeyValueRow[]) {
    return uniqueStrings([
      ...GENERAL_HEADER_NAMES,
      ...rows.map((row) => row.key)
    ]).sort((a, b) => a.localeCompare(b));
  }

  function getHeaderValueSuggestions(headerName: string, rows: KeyValueRow[]) {
    const normalizedHeaderName = normalizeHeaderName(headerName);

    if (!normalizedHeaderName) {
      return [];
    }

    return uniqueStrings([
      ...rows
        .filter((row) => normalizeHeaderName(row.key) === normalizedHeaderName)
        .map((row) => row.value),
      ...(GENERAL_HEADER_VALUE_SUGGESTIONS[normalizedHeaderName] ?? [])
    ]);
  }

  function getHeaderNameListId(rowId: string) {
    return `header-name-suggestions-${rowId}`;
  }

  function getHeaderValueListId(rowId: string) {
    return `header-value-suggestions-${rowId}`;
  }

  function addRow(kind: "queryParams" | "headers") {
    request = { ...request, [kind]: [...request[kind], createKeyValueRow()] };
  }

  function removeRow(kind: "queryParams" | "headers", id: string) {
    const nextRows = request[kind].length === 1 ? [createKeyValueRow()] : request[kind].filter((row) => row.id !== id);
    request = { ...request, [kind]: nextRows };
  }

  function updateBodyMode(mode: BodyMode) {
    if (mode !== "json") {
      jsonValidationError = "";
    }
    if (mode !== "multipart") {
      multipartErrorText = "";
    }
    request = {
      ...request,
      body: {
        ...request.body,
        mode
      }
    };
  }

  function updateBodyField(field: "raw", value: string) {
    request = {
      ...request,
      body: {
        ...request.body,
        [field]: value
      }
    };
  }

  function formatJsonBody() {
    try {
      const parsed = JSON.parse(request.body.raw);
      updateBodyField("raw", JSON.stringify(parsed, null, 2));
      jsonValidationError = "";
    } catch {
      // not valid JSON, do nothing
    }
  }

  function validateJsonOnBlur() {
    const raw = request.body.raw.trim();
    if (!raw) {
      jsonValidationError = "";
      return;
    }
    try {
      JSON.parse(raw);
      jsonValidationError = "";
    } catch (error) {
      jsonValidationError = error instanceof SyntaxError ? error.message : "Invalid JSON";
    }
  }

  type HighlightToken = { type: string; value: string };

  const variableTokenPattern = /{{\s*(?:\$[A-Za-z0-9_.-]+(?:\[\d+\])?|[A-Za-z0-9_.-]+)\s*}}/g;

  function matchVariableToken(source: string, start: number) {
    return source.slice(start).match(/^{{\s*(?:\$[A-Za-z0-9_.-]+(?:\[\d+\])?|[A-Za-z0-9_.-]+)\s*}}/)?.[0] ?? null;
  }

  function pushVariableAwareText(tokens: HighlightToken[], value: string, baseType: HighlightToken["type"]) {
    if (!value) {
      return;
    }

    let lastIndex = 0;

    for (const match of value.matchAll(variableTokenPattern)) {
      const index = match.index ?? 0;

      if (index > lastIndex) {
        tokens.push({ type: baseType, value: value.slice(lastIndex, index) });
      }

      tokens.push({ type: "variable", value: match[0] });
      lastIndex = index + match[0].length;
    }

    if (lastIndex < value.length) {
      tokens.push({ type: baseType, value: value.slice(lastIndex) });
    } else if (lastIndex === 0) {
      tokens.push({ type: baseType, value });
    }
  }

  function tokenizeJson(json: string): HighlightToken[] {
    const tokens: HighlightToken[] = [];
    let i = 0;
    while (i < json.length) {
      const variableToken = matchVariableToken(json, i);

      if (variableToken) {
        tokens.push({ type: "variable", value: variableToken });
        i += variableToken.length;
        continue;
      }

      const ch = json[i];
      if (ch === '"') {
        const start = i;
        i++;
        while (i < json.length && json[i] !== '"') { if (json[i] === '\\') i++; i++; }
        i++;
        const raw = json.slice(start, i);
        let j = i;
        while (j < json.length && (json[j] === ' ' || json[j] === '\t')) j++;
        pushVariableAwareText(tokens, raw, json[j] === ':' ? "key" : "string");
        continue;
      }
      if (ch === '-' || (ch >= '0' && ch <= '9')) {
        const start = i;
        while (i < json.length && /[0-9.eE+\-]/.test(json[i])) i++;
        tokens.push({ type: "number", value: json.slice(start, i) });
        continue;
      }
      if (json.startsWith("true", i)) { tokens.push({ type: "bool", value: "true" }); i += 4; continue; }
      if (json.startsWith("false", i)) { tokens.push({ type: "bool", value: "false" }); i += 5; continue; }
      if (json.startsWith("null", i)) { tokens.push({ type: "null", value: "null" }); i += 4; continue; }
      if ('{}[]'.includes(ch)) { tokens.push({ type: "bracket", value: ch }); i++; continue; }
      if (ch === ':') { tokens.push({ type: "colon", value: ":" }); i++; continue; }
      if (ch === ',') { tokens.push({ type: "comma", value: "," }); i++; continue; }
      if (ch === '\n') { tokens.push({ type: "newline", value: "\n" }); i++; continue; }
      if (ch === ' ' || ch === '\t') {
        const start = i;
        while (i < json.length && (json[i] === ' ' || json[i] === '\t')) i++;
        tokens.push({ type: "indent", value: json.slice(start, i) });
        continue;
      }
      tokens.push({ type: "text", value: ch });
      i++;
    }
    return tokens;
  }

  let jsonTokens = $derived(
    request.body.mode === "json" ? tokenizeJson(request.body.raw) : []
  );
  let urlTokens = $derived(pushUrlTokens(displayUrl));

  function pushUrlTokens(url: string) {
    const tokens: HighlightToken[] = [];
    pushVariableAwareText(tokens, url, "text");
    return tokens;
  }

  function handleJsonKeydown(event: KeyboardEvent) {
    if (request.body.mode !== "json") return;
    const textarea = event.target as HTMLTextAreaElement;
    if (textarea.tagName !== "TEXTAREA") return;

    if (event.key === "Enter") {
      event.preventDefault();
      const { selectionStart } = textarea;
      const val = textarea.value;
      const lineStart = val.lastIndexOf('\n', selectionStart - 1) + 1;
      const currentLine = val.slice(lineStart, selectionStart);
      const indent = currentLine.match(/^(\s*)/)?.[1] ?? "";
      const charBefore = val[selectionStart - 1];
      const charAfter = val[selectionStart];
      let insert: string;

      if ((charBefore === '{' || charBefore === '[') && (charAfter === '}' || charAfter === ']')) {
        insert = `\n${indent}  \n${indent}`;
        insertTextIntoEditableControl(textarea, insert, {
          selectionStart,
          selectionEnd: selectionStart,
          cursorOffset: indent.length + 3
        });
      } else if (charBefore === '{' || charBefore === '[') {
        insert = `\n${indent}  `;
        insertTextIntoEditableControl(textarea, insert, {
          selectionStart,
          selectionEnd: selectionStart
        });
      } else {
        insert = `\n${indent}`;
        insertTextIntoEditableControl(textarea, insert, {
          selectionStart,
          selectionEnd: selectionStart
        });
      }
      return;
    }

    if (event.key === "Tab") {
      event.preventDefault();
      const { selectionStart, selectionEnd } = textarea;
      const insert = "  ";
      insertTextIntoEditableControl(textarea, insert, {
        selectionStart,
        selectionEnd
      });
    }
  }


  function updateFormRow(index: number, patch: Partial<KeyValueRow>) {
    const nextRows = request.body.form.map((row, rowIndex) => (rowIndex === index ? { ...row, ...patch } : row));
    request = {
      ...request,
      body: {
        ...request.body,
        form: nextRows
      }
    };
  }

  function addFormRow() {
    request = {
      ...request,
      body: {
        ...request.body,
        form: [...request.body.form, createKeyValueRow()]
      }
    };
  }

  function removeFormRow(id: string) {
    const nextRows = request.body.form.length === 1 ? [createKeyValueRow()] : request.body.form.filter((row) => row.id !== id);
    request = {
      ...request,
      body: {
        ...request.body,
        form: nextRows
      }
    };
  }

  function toggleFileEnabled(index: number, enabled: boolean) {
    updateFileRow(index, { enabled });
  }

  function updateFileRow(index: number, patch: Partial<FileRow>) {
    const nextRows = request.body.files.map((row, rowIndex) => (rowIndex === index ? { ...row, ...patch } : row));
    request = {
      ...request,
      body: {
        ...request.body,
        files: nextRows
      }
    };
  }

  function addFileRow() {
    request = {
      ...request,
      body: {
        ...request.body,
        files: [...request.body.files, createFileRow()]
      }
    };
  }

  function appendPickedFiles(paths: string[]) {
    if (paths.length === 0) {
      return;
    }

    request = {
      ...request,
      body: {
        ...request.body,
        files: [
          ...request.body.files,
          ...paths.map((path) => ({
            ...createFileRow(),
            path
          }))
        ]
      }
    };
  }

  function removeFileRow(id: string) {
    request = {
      ...request,
      body: {
        ...request.body,
        files: request.body.files.filter((row) => row.id !== id)
      }
    };
  }

  function getFileName(path: string) {
    const normalized = path.replace(/\\/g, "/");
    const segments = normalized.split("/");
    return segments[segments.length - 1] || path;
  }

  async function handlePickMultipartFiles() {
    isPickingMultipartFiles = true;

    try {
      const paths = await pickMultipartFiles();
      appendPickedFiles(paths);
      multipartErrorText = "";
    } catch (error) {
      multipartErrorText = error instanceof Error ? error.message : String(error);
    } finally {
      isPickingMultipartFiles = false;
    }
  }

  function updateAuthType(type: AuthType) {
    request = {
      ...request,
      auth: {
        ...request.auth,
        type
      }
    };
  }

  function updateName(name: string) {
    request = {
      ...request,
      name
    };
  }

  function updateMethod(method: RequestDraft["method"]) {
    request = {
      ...request,
      method
    };
  }

  function updateApiKeyName(value: string) {
    request = {
      ...request,
      auth: {
        ...request.auth,
        apiKeyName: value
      }
    };
  }

  function updateApiKeyPlacement(value: RequestDraft["auth"]["apiKeyIn"]) {
    request = {
      ...request,
      auth: {
        ...request.auth,
        apiKeyIn: value
      }
    };
  }

  function updateScriptField(field: "preRequestScript" | "testScript", value: string) {
    request = {
      ...request,
      [field]: value
    };
  }

  function formatScriptField(field: "preRequestScript" | "testScript") {
    updateScriptField(field, formatScript(request[field]));
  }

  function removeActionLabel(label: string) {
    return `Remove ${label}`;
  }

  async function fetchOAuth2Token() {
    if (!handleFetchOAuth2Token || isFetchingOAuth2Token) {
      return;
    }

    isFetchingOAuth2Token = true;
    oauth2FetchErrorText = "";
    oauth2FetchStatusText = "";

    try {
      const result = await handleFetchOAuth2Token({
        persistToEnvironment: canPersistOAuth2Token && shouldPersistOAuth2Token
      });
      request = {
        ...request,
        auth: {
          ...request.auth,
          type: "oauth2",
          oauth2AccessToken: result.persistedToEnvironment ? "{{oauth_access_token}}" : result.accessToken
        }
      };
      const expiryText = result.expiresIn ? ` Expires in ${result.expiresIn}s.` : "";
      oauth2FetchStatusText = result.persistedToEnvironment
        ? `Token saved to ${activeEnvironmentName} as {{oauth_access_token}}.${expiryText}`
        : `Token fetched into this request field.${expiryText}`;
    } catch (error) {
      oauth2FetchErrorText = error instanceof Error ? error.message : String(error);
    } finally {
      isFetchingOAuth2Token = false;
    }
  }
</script>

<svelte:window onkeydown={closeSaveMenuOnWindowKeydown} />
<svelte:document onclickcapture={closeSaveMenuOnDocumentClick} />

<section class="panel request-panel">
  <div class="request-section-header">
    <div class="request-section-title">
      <h2>Request</h2>
      <button class="button-secondary button-compact" type="button" onclick={handleNewRequest}>New</button>
      <button class="button-secondary button-compact" type="button" onclick={handleOpenImport}>Import</button>
      <button class="button-secondary button-compact" type="button" onclick={handleOpenExport}>Export</button>
    </div>
  </div>

  <div class="request-header-grid">
    <div class="request-name-block">
      <input
        id="request-name"
        class="text-input"
        value={request.name}
        placeholder="Untitled request"
        oninput={(event) => updateName(event.currentTarget.value)}
      />
    </div>

    <div
      class={[
        "request-save-split",
        !showSaveMenu && "request-save-split-solo",
        (saveDisabled || isSaving) && "request-save-split-disabled"
      ]}
      {@attach attachSaveSplitRoot}
    >
      <button
        class="request-save-split-main"
        type="button"
        onclick={() => {
          isSaveMenuOpen = false;
          void handleSaveRequest();
        }}
        disabled={saveDisabled || isSaving}
      >
        {isSaving ? "Saving..." : saveLabel}
      </button>
      {#if showSaveMenu}
        <button
          class="request-save-split-chevron"
          type="button"
          aria-expanded={isSaveMenuOpen}
          aria-haspopup="true"
          aria-label="More save actions"
          disabled={saveDisabled || isSaving}
          onclick={(event) => {
            event.stopPropagation();
            isSaveMenuOpen = !isSaveMenuOpen;
          }}
        >
          <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
            <path d="M6 9l6 6 6-6" />
          </svg>
        </button>
        {#if isSaveMenuOpen}
          <div class="request-save-menu" role="menu">
            <button
              class="request-save-menu-item"
              type="button"
              role="menuitem"
              onclick={() => {
                isSaveMenuOpen = false;
                void handleSaveAsRequest();
              }}
            >
              Save as
            </button>
          </div>
        {/if}
      {/if}
    </div>

    <select
      class={`method-select method-${request.method.toLowerCase()}`}
      value={request.method}
      onchange={(event) => updateMethod(event.currentTarget.value as RequestDraft["method"])}
    >
      <option value="GET">GET</option>
      <option value="POST">POST</option>
      <option value="PUT">PUT</option>
      <option value="PATCH">PATCH</option>
      <option value="DELETE">DELETE</option>
      <option value="HEAD">HEAD</option>
      <option value="OPTIONS">OPTIONS</option>
    </select>

    <div class="request-url-field">
      <VariableField
        className="text-input url-input"
        value={displayUrl}
        variables={environmentVariables}
        highlightTokens={urlTokens}
        placeholder="https://api.example.com/resource"
        spellcheck={false}
        onValueInput={syncUrlInput}
      />
    </div>

    <div class={["request-send-actions", isSending && "request-send-actions-cancel"]}>
      <button
        class="request-send-preview-control"
        type="button"
        onclick={handleOpenPreview}
        aria-label="Preview resolved request"
        title="Preview resolved request"
        disabled={isSending || isCanceling}
      >
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <path d="M2.5 12s3.5-6 9.5-6 9.5 6 9.5 6-3.5 6-9.5 6-9.5-6-9.5-6Z" />
          <circle cx="12" cy="12" r="3" />
        </svg>
      </button>

      <button
        class="request-send-main-control"
        type="button"
        onclick={() => (isSending ? handleCancelRequest() : handleSendRequest())}
        disabled={isCanceling || sendDisabled}
      >
        {#if isSending}
          {isCanceling ? "Canceling..." : "Cancel"}
        {:else}
          Send
        {/if}
      </button>
    </div>
  </div>

  <div class="panel-tabs">
    {#each panels as panel (panel.id)}
      <button
        class={["tab-button", activePanel === panel.id && "active"]}
        type="button"
        onclick={() => (activePanel = panel.id)}
      >
        {panel.label}
      </button>
    {/each}
  </div>

  {#if activePanel === "query"}
    <div class="editor-block">
      <div class="editor-header">
        <h2>Query Parameters</h2>
        <button class="button-secondary" type="button" onclick={() => addRow("queryParams")}>Add row</button>
      </div>

      <div class="row-list">
        {#each request.queryParams as row, index (row.id)}
          <div class="kv-row">
            <input
              class="row-toggle"
              type="checkbox"
              checked={row.enabled}
              aria-label="Enable query parameter row"
              onchange={(event) => toggleRow("queryParams", index, event.currentTarget.checked)}
            />
            <input class="text-input" value={row.key} placeholder="Key" oninput={(event) => updateRows("queryParams", index, { key: event.currentTarget.value })} />
            <VariableField
              className="text-input"
              value={row.value}
              variables={environmentVariables}
              placeholder="Value"
              onValueInput={(nextValue) => updateRows("queryParams", index, { value: nextValue })}
            />
            <button
              class="icon-button row-action-button row-action-danger"
              type="button"
              title={removeActionLabel("query parameter row")}
              aria-label={removeActionLabel("query parameter row")}
              onclick={() => removeRow("queryParams", row.id)}
            >
              <svg viewBox="0 0 20 20" aria-hidden="true">
                <path d="M3 5h14" />
                <path d="M8 5V3h4v2" />
                <path d="M6 8v8" />
                <path d="M10 8v8" />
                <path d="M14 8v8" />
                <path d="M5 5l1 12h8l1-12" />
              </svg>
            </button>
          </div>
        {/each}
      </div>
    </div>
  {/if}

  {#if activePanel === "headers"}
    <div class="editor-block">
      <div class="editor-header">
        <h2>Headers</h2>
        <button class="button-secondary" type="button" onclick={() => addRow("headers")}>Add row</button>
      </div>

      <div class="row-list">
        {#each request.headers as row, index (row.id)}
          <div class="kv-row">
            <input
              class="row-toggle"
              type="checkbox"
              checked={row.enabled}
              aria-label="Enable header row"
              onchange={(event) => toggleRow("headers", index, event.currentTarget.checked)}
            />
            <input
              class="text-input"
              value={row.key}
              placeholder="Header"
              list={getHeaderNameListId(row.id)}
              oninput={(event) => updateRows("headers", index, { key: event.currentTarget.value })}
            />
            <datalist id={getHeaderNameListId(row.id)}>
              {#each headerNameSuggestions as headerName (headerName)}
                <option value={headerName}></option>
              {/each}
            </datalist>
            <VariableField
              className="text-input"
              value={row.value}
              variables={environmentVariables}
              placeholder="Value"
              list={getHeaderValueListId(row.id)}
              onValueInput={(nextValue) => updateRows("headers", index, { value: nextValue })}
            />
            <datalist id={getHeaderValueListId(row.id)}>
              {#each getHeaderValueSuggestions(row.key, request.headers) as headerValue (headerValue)}
                <option value={headerValue}></option>
              {/each}
            </datalist>
            <button
              class="icon-button row-action-button row-action-danger"
              type="button"
              title={removeActionLabel("header row")}
              aria-label={removeActionLabel("header row")}
              onclick={() => removeRow("headers", row.id)}
            >
              <svg viewBox="0 0 20 20" aria-hidden="true">
                <path d="M3 5h14" />
                <path d="M8 5V3h4v2" />
                <path d="M6 8v8" />
                <path d="M10 8v8" />
                <path d="M14 8v8" />
                <path d="M5 5l1 12h8l1-12" />
              </svg>
            </button>
          </div>
        {/each}
      </div>
    </div>
  {/if}

  {#if activePanel === "body"}
    <div class="editor-block">
      <div class="editor-header">
        <h2>Body</h2>
        <label class="body-mode-control">
          <span class="sr-only">Body type</span>
          <select class="body-mode-select" value={request.body.mode} onchange={(event) => updateBodyMode(event.currentTarget.value as BodyMode)}>
            <option value="none">None</option>
            <option value="json">JSON</option>
            <option value="raw">Raw</option>
            <option value="form-urlencoded">Form URL Encoded</option>
            <option value="multipart">Multipart</option>
          </select>
        </label>
        {#if request.body.mode === "json"}
          <button class="button-secondary" type="button" onclick={formatJsonBody}>Format</button>
        {/if}
      </div>

      {#if request.body.mode === "none"}
        <div class="empty-state body-empty-state">
          This request will be sent without a body.
        </div>
      {/if}

      {#if request.body.mode === "json"}
        <div class="json-editor-shell" onfocusout={validateJsonOnBlur} onfocusin={() => { jsonValidationError = ""; }}>
          <VariableField
            className="body-textarea json-editor-textarea"
            multiline={true}
            value={request.body.raw}
            variables={environmentVariables}
            highlightTokens={jsonTokens}
            highlightOverlayClassName="json-editor-overlay"
            placeholder={'{"hello":"world"}'}
            spellcheck={false}
            onValueInput={(nextValue) => updateBodyField("raw", nextValue)}
            onExtraKeydown={handleJsonKeydown}
          />
        </div>
        {#if jsonValidationError}
          <p class="json-validation-error">{jsonValidationError}</p>
        {/if}
      {:else if request.body.mode === "raw"}
        <VariableField
          className="body-textarea"
          multiline={true}
          value={request.body.raw}
          variables={environmentVariables}
          placeholder="Raw request body"
          onValueInput={(nextValue) => updateBodyField("raw", nextValue)}
        />
      {/if}

      {#if request.body.mode === "form-urlencoded"}
        <div class="row-list">
          {#each request.body.form as row, index (row.id)}
            <div class="kv-row">
              <input
                class="row-toggle"
                type="checkbox"
                checked={row.enabled}
                aria-label="Enable form field row"
                onchange={(event) => toggleFormEnabled(index, event.currentTarget.checked)}
              />
              <input class="text-input" value={row.key} placeholder="Field" oninput={(event) => updateFormRow(index, { key: event.currentTarget.value })} />
              <VariableField
                className="text-input"
                value={row.value}
                variables={environmentVariables}
                placeholder="Value"
                onValueInput={(nextValue) => updateFormRow(index, { value: nextValue })}
              />
              <button
                class="icon-button row-action-button row-action-danger"
                type="button"
                title={removeActionLabel("form field row")}
                aria-label={removeActionLabel("form field row")}
                onclick={() => removeFormRow(row.id)}
              >
                <svg viewBox="0 0 20 20" aria-hidden="true">
                  <path d="M3 5h14" />
                  <path d="M8 5V3h4v2" />
                  <path d="M6 8v8" />
                  <path d="M10 8v8" />
                  <path d="M14 8v8" />
                  <path d="M5 5l1 12h8l1-12" />
                </svg>
              </button>
            </div>
          {/each}

          <button class="button-secondary" type="button" onclick={addFormRow}>Add field</button>
        </div>
      {/if}

      {#if request.body.mode === "multipart"}
        <div class="multipart-editor">
          <section class="multipart-section">
            <div class="editor-header">
              <h3>Fields</h3>
              <button class="button-secondary" type="button" onclick={addFormRow}>Add field</button>
            </div>

            <div class="row-list">
              {#each request.body.form as row, index (row.id)}
                <div class="kv-row">
                  <input
                    class="row-toggle"
                    type="checkbox"
                    checked={row.enabled}
                    aria-label="Enable multipart field row"
                    onchange={(event) => toggleFormEnabled(index, event.currentTarget.checked)}
                  />
                  <input class="text-input" value={row.key} placeholder="Field" oninput={(event) => updateFormRow(index, { key: event.currentTarget.value })} />
                  <VariableField
                    className="text-input"
                    value={row.value}
                    variables={environmentVariables}
                    placeholder="Value"
                    onValueInput={(nextValue) => updateFormRow(index, { value: nextValue })}
                  />
                  <button
                    class="icon-button row-action-button row-action-danger"
                    type="button"
                    title={removeActionLabel("multipart field row")}
                    aria-label={removeActionLabel("multipart field row")}
                    onclick={() => removeFormRow(row.id)}
                  >
                    <svg viewBox="0 0 20 20" aria-hidden="true">
                      <path d="M3 5h14" />
                      <path d="M8 5V3h4v2" />
                      <path d="M6 8v8" />
                      <path d="M10 8v8" />
                      <path d="M14 8v8" />
                      <path d="M5 5l1 12h8l1-12" />
                    </svg>
                  </button>
                </div>
              {/each}
            </div>
          </section>

          <section class="multipart-section">
            <div class="editor-header">
              <h3>Files</h3>
              <div class="multipart-actions">
                <button class="button-secondary" type="button" onclick={addFileRow}>Add path</button>
                <button class="button-secondary" type="button" onclick={handlePickMultipartFiles} disabled={isPickingMultipartFiles}>
                  {isPickingMultipartFiles ? "Picking..." : "Pick files"}
                </button>
              </div>
            </div>

            {#if request.body.files.length === 0}
              <div class="empty-state body-empty-state">
                Add file rows or pick files to send them as multipart form parts.
              </div>
            {:else}
              <div class="row-list">
                {#each request.body.files as file, index (file.id)}
                  <div class="multipart-file-card">
                    <div class="kv-row">
                      <input
                        class="row-toggle"
                        type="checkbox"
                        checked={file.enabled}
                        aria-label="Enable multipart file row"
                        onchange={(event) => toggleFileEnabled(index, event.currentTarget.checked)}
                      />
                      <input
                        class="text-input"
                        value={file.name}
                        placeholder="Field name"
                        oninput={(event) => updateFileRow(index, { name: event.currentTarget.value })}
                      />
                      <VariableField
                        className="text-input"
                        value={file.path}
                        variables={environmentVariables}
                        placeholder="/path/to/file"
                        onValueInput={(nextValue) => updateFileRow(index, { path: nextValue })}
                      />
                      <button
                        class="icon-button row-action-button row-action-danger"
                        type="button"
                        title={removeActionLabel("multipart file row")}
                        aria-label={removeActionLabel("multipart file row")}
                        onclick={() => removeFileRow(file.id)}
                      >
                        <svg viewBox="0 0 20 20" aria-hidden="true">
                          <path d="M3 5h14" />
                          <path d="M8 5V3h4v2" />
                          <path d="M6 8v8" />
                          <path d="M10 8v8" />
                          <path d="M14 8v8" />
                          <path d="M5 5l1 12h8l1-12" />
                        </svg>
                      </button>
                    </div>
                    <div class="multipart-file-meta">
                      <span class="multipart-file-name">{file.path ? getFileName(file.path) : "No file selected yet"}</span>
                      {#if file.path}
                        <span class="multipart-file-path" title={file.path}>{file.path}</span>
                      {/if}
                    </div>
                  </div>
                {/each}
              </div>
            {/if}
          </section>

          {#if multipartErrorText}
            <div class="feedback feedback-error">{multipartErrorText}</div>
          {/if}
        </div>
      {/if}
    </div>
  {/if}

  {#if activePanel === "auth"}
    <div class="editor-block">
      <div class="editor-header">
        <h2>Auth</h2>
        <label class="body-mode-control">
          <span class="sr-only">Auth type</span>
          <select class="body-mode-select" value={request.auth.type} onchange={(event) => updateAuthType(event.currentTarget.value as AuthType)}>
            <option value="none">None</option>
            <option value="basic">Basic</option>
            <option value="bearer">Bearer</option>
            <option value="api-key">API key</option>
            <option value="oauth2">OAuth2</option>
          </select>
        </label>
      </div>

      {#if request.auth.type === "none"}
        <div class="empty-state body-empty-state">
          This request will be sent without authentication.
        </div>
      {/if}

      {#if request.auth.type === "basic"}
        <div class="auth-grid">
          <label>
            <span class="field-label">Username</span>
            <VariableField
              className="text-input"
              value={request.auth.basicUsername}
              variables={environmentVariables}
              onValueInput={(nextValue) =>
                (request = {
                  ...request,
                  auth: { ...request.auth, basicUsername: nextValue }
                })}
            />
          </label>
          <label>
            <span class="field-label">Password</span>
            <VariableField
              className="text-input"
              type="password"
              value={request.auth.basicPassword}
              variables={environmentVariables}
              onValueInput={(nextValue) =>
                (request = {
                  ...request,
                  auth: { ...request.auth, basicPassword: nextValue }
                })}
            />
          </label>
        </div>
      {/if}

      {#if request.auth.type === "bearer"}
        <div class="auth-grid">
          <label>
            <span class="field-label">Token</span>
            <VariableField
              className="text-input"
              type="password"
              value={request.auth.bearerToken}
              variables={environmentVariables}
              placeholder={"{{api_token}}"}
              onValueInput={(nextValue) =>
                (request = {
                  ...request,
                  auth: { ...request.auth, bearerToken: nextValue }
                })}
            />
          </label>
        </div>
      {/if}

      {#if request.auth.type === "api-key"}
        <div class="auth-grid">
          <label>
            <span class="field-label">Key</span>
            <input
              class="text-input"
              value={request.auth.apiKeyName}
              oninput={(event) => updateApiKeyName(event.currentTarget.value)}
            />
          </label>
          <label>
            <span class="field-label">Value</span>
            <VariableField
              className="text-input"
              type="password"
              value={request.auth.apiKeyValue}
              variables={environmentVariables}
              onValueInput={(nextValue) =>
                (request = {
                  ...request,
                  auth: { ...request.auth, apiKeyValue: nextValue }
                })}
            />
          </label>
          <label>
            <span class="field-label">Send in</span>
            <select
              class="text-input"
              value={request.auth.apiKeyIn}
              onchange={(event) => updateApiKeyPlacement(event.currentTarget.value as RequestDraft["auth"]["apiKeyIn"])}
            >
              <option value="header">Header</option>
              <option value="query">Query parameter</option>
            </select>
          </label>
        </div>
      {/if}

      {#if request.auth.type === "oauth2"}
        <div class="auth-grid">
          <label>
            <span class="field-label">Access token</span>
            <VariableField
              className="text-input"
              type="password"
              value={request.auth.oauth2AccessToken}
              variables={environmentVariables}
              placeholder={"{{oauth_access_token}}"}
              onValueInput={(nextValue) =>
                (request = {
                  ...request,
                  auth: { ...request.auth, oauth2AccessToken: nextValue }
                })}
            />
          </label>
          <label>
            <span class="field-label">Token URL</span>
            <VariableField
              className="text-input"
              value={request.auth.oauth2TokenUrl}
              variables={environmentVariables}
              placeholder={"{{oauth_token_url}}"}
              onValueInput={(nextValue) =>
                (request = {
                  ...request,
                  auth: { ...request.auth, oauth2TokenUrl: nextValue }
                })}
            />
          </label>
          <label>
            <span class="field-label">Client ID</span>
            <VariableField
              className="text-input"
              value={request.auth.oauth2ClientId}
              variables={environmentVariables}
              placeholder={"{{oauth_client_id}}"}
              onValueInput={(nextValue) =>
                (request = {
                  ...request,
                  auth: { ...request.auth, oauth2ClientId: nextValue }
                })}
            />
          </label>
          <label>
            <span class="field-label">Client secret</span>
            <VariableField
              className="text-input"
              type="password"
              value={request.auth.oauth2ClientSecret}
              variables={environmentVariables}
              placeholder={"{{oauth_client_secret}}"}
              onValueInput={(nextValue) =>
                (request = {
                  ...request,
                  auth: { ...request.auth, oauth2ClientSecret: nextValue }
                })}
            />
          </label>
          <label>
            <span class="field-label">Scope</span>
            <VariableField
              className="text-input"
              value={request.auth.oauth2Scope}
              variables={environmentVariables}
              placeholder={"{{oauth_scope}}"}
              onValueInput={(nextValue) =>
                (request = {
                  ...request,
                  auth: { ...request.auth, oauth2Scope: nextValue }
                })}
            />
          </label>
          <div class="auth-action-row">
            <div class="oauth2-actions">
              <button
                class="button-primary"
                type="button"
                onclick={fetchOAuth2Token}
                disabled={!handleFetchOAuth2Token || isFetchingOAuth2Token}
              >
                {isFetchingOAuth2Token ? "Fetching..." : "Fetch token"}
              </button>
              <label class={["inline-checkbox", !canPersistOAuth2Token && "inline-checkbox-disabled"]}>
                <input
                  type="checkbox"
                  checked={shouldPersistOAuth2Token && canPersistOAuth2Token}
                  disabled={!canPersistOAuth2Token || isFetchingOAuth2Token}
                  onchange={(event) => (shouldPersistOAuth2Token = event.currentTarget.checked)}
                />
                <span>
                  {canPersistOAuth2Token
                    ? `Save to ${activeEnvironmentName} as {{oauth_access_token}}`
                    : "Activate an environment to save the token as {{oauth_access_token}}"}
                </span>
              </label>
            </div>
            {#if oauth2FetchStatusText}
              <p class="auth-status-text">{oauth2FetchStatusText}</p>
            {/if}
            {#if oauth2FetchErrorText}
              <p class="auth-error-text">{oauth2FetchErrorText}</p>
            {/if}
          </div>
        </div>
      {/if}
    </div>
  {/if}

  {#if activePanel === "scripts"}
    <div class="editor-block">
      <div class="editor-header">
        <h2>Scripts</h2>
      </div>

      <div class="request-script-grid">
        <section class="request-script-card">
          <div class="request-script-card-header">
            <h3 class="request-script-card-title">Pre-request Script</h3>
            <button class="button-secondary button-compact" type="button" onclick={() => formatScriptField("preRequestScript")}>Format</button>
          </div>
          <ScriptEditor
            value={request.preRequestScript}
            {environmentVariables}
            scriptKind="preRequest"
            placeholder={PRE_REQUEST_SCRIPT_PLACEHOLDER}
            onValueInput={(nextValue) => updateScriptField("preRequestScript", nextValue)}
          />
        </section>

        <section class="request-script-card">
          <div class="request-script-card-header">
            <h3 class="request-script-card-title">Test Script</h3>
            <button class="button-secondary button-compact" type="button" onclick={() => formatScriptField("testScript")}>Format</button>
          </div>
          <ScriptEditor
            value={request.testScript}
            {environmentVariables}
            scriptKind="test"
            placeholder={TEST_SCRIPT_PLACEHOLDER}
            onValueInput={(nextValue) => updateScriptField("testScript", nextValue)}
          />
        </section>
      </div>
    </div>
  {/if}
</section>
