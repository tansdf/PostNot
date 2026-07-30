<script lang="ts">
  import { pickMultipartFiles } from "$lib/api/commands";
  import {
    createFileRow,
    createKeyValueRow,
    type BodyMode,
    type EnvironmentVariable,
    type FileRow,
    type KeyValueRow,
    type RequestDraft
  } from "$lib/api/types";
  import AuthEditor from "$lib/components/request/AuthEditor.svelte";
  import JsonEditor from "$lib/components/request/JsonEditor.svelte";
  import KeyValueEditor from "$lib/components/request/KeyValueEditor.svelte";
  import ScriptEditor from "$lib/components/request/ScriptEditor.svelte";
  import VariableField from "$lib/components/request/VariableField.svelte";
  import { getHeaderNameSuggestions, getHeaderValueSuggestions } from "$lib/header-suggestions";
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

  function toggleFormEnabled(index: number, enabled: boolean) {
    updateFormRow(index, { enabled });
  }

  let displayUrl = $derived(buildDisplayUrl(request.url, request.queryParams));

  function updateKeyValueRows(kind: "queryParams" | "headers", rows: KeyValueRow[]) {
    request = { ...request, [kind]: rows };
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

  let urlTokens = $derived(pushUrlTokens(displayUrl));

  function pushUrlTokens(url: string) {
    const tokens: HighlightToken[] = [];
    pushVariableAwareText(tokens, url, "text");
    return tokens;
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

</script>

<svelte:window onkeydown={closeSaveMenuOnWindowKeydown} />
<svelte:document onclickcapture={closeSaveMenuOnDocumentClick} />

<section class="panel panel-inset request-panel">
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
    <KeyValueEditor
      rows={request.queryParams}
      variables={environmentVariables}
      title="Query Parameters"
      keyLabel="Key"
      rowLabel="query parameter"
      onRowsChange={(rows) => updateKeyValueRows("queryParams", rows)}
    />
  {/if}

  {#if activePanel === "headers"}
    <KeyValueEditor
      rows={request.headers}
      variables={environmentVariables}
      title="Headers"
      keyLabel="Header"
      keySuggestions={headerNameSuggestions}
      getValueSuggestions={(key) => getHeaderValueSuggestions(key, request.headers)}
      onRowsChange={(rows) => updateKeyValueRows("headers", rows)}
    />
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
        <JsonEditor
          value={request.body.raw}
          variables={environmentVariables}
          placeholder={'{"hello":"world"}'}
          ariaLabel="JSON request body"
          onValueInput={(nextValue) => updateBodyField("raw", nextValue)}
          onBlur={validateJsonOnBlur}
          onFocus={() => { jsonValidationError = ""; }}
        />
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
    <AuthEditor
      auth={request.auth}
      variables={environmentVariables}
      {activeEnvironmentName}
      {handleFetchOAuth2Token}
      onAuthChange={(auth) => (request = { ...request, auth })}
    />
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
