<script lang="ts">
  import { createKeyValueRow, type AuthType, type BodyMode, type KeyValueRow, type RequestDraft } from "$lib/api/types";
  import VariableField from "$lib/components/request/VariableField.svelte";

  let {
    request = $bindable(),
    isSending = false,
    isCanceling = false,
    isSaving = false,
    saveLabel = "Save",
    saveDisabled = false,
    environmentVariables = [],
    onNewRequest = () => {},
    onOpenCurlImport = () => {},
    onSend = () => {},
    onCancel = () => {},
    onSave = () => {}
  }: {
    request: RequestDraft;
    isSending?: boolean;
    isCanceling?: boolean;
    isSaving?: boolean;
    saveLabel?: string;
    saveDisabled?: boolean;
    environmentVariables?: KeyValueRow[];
    onNewRequest?: () => Promise<void> | void;
    onOpenCurlImport?: () => Promise<void> | void;
    onSend?: () => Promise<void> | void;
    onCancel?: () => Promise<void> | void;
    onSave?: () => Promise<void> | void;
  } = $props();

  let activePanel: "query" | "headers" | "body" | "auth" = $state("query");

  const panels = [
    { id: "query", label: "Query" },
    { id: "headers", label: "Headers" },
    { id: "body", label: "Body" },
    { id: "auth", label: "Auth" }
  ] as const;

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

  function addRow(kind: "queryParams" | "headers") {
    request = { ...request, [kind]: [...request[kind], createKeyValueRow()] };
  }

  function removeRow(kind: "queryParams" | "headers", id: string) {
    const nextRows = request[kind].length === 1 ? [createKeyValueRow()] : request[kind].filter((row) => row.id !== id);
    request = { ...request, [kind]: nextRows };
  }

  function updateBodyMode(mode: BodyMode) {
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

  function updateAuthType(type: AuthType) {
    request = {
      ...request,
      auth: {
        ...request.auth,
        type
      }
    };
  }
</script>

<section class="panel request-panel">
  <div class="request-section-header">
    <div class="request-section-title">
      <h2>Request</h2>
      <button class="ghost-button request-header-button" type="button" onclick={onNewRequest}>New</button>
      <button class="ghost-button request-header-button" type="button" onclick={onOpenCurlImport}>Import</button>
    </div>
  </div>

  <div class="request-header-grid">
    <div class="request-name-block">
      <input
        id="request-name"
        class="text-input"
        bind:value={request.name}
        placeholder="Untitled request"
      />
    </div>

    <button class="ghost-button request-save-control" type="button" onclick={onSave} disabled={saveDisabled || isSaving}>
      {isSaving ? "Saving..." : saveLabel}
    </button>

    <select class="method-select" bind:value={request.method}>
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
        placeholder="https://api.example.com/resource"
        spellcheck={false}
        onValueInput={syncUrlInput}
      />
    </div>

    <button
      class={["send-button request-send-control", isSending && "cancel-button"]}
      type="button"
      onclick={() => (isSending ? onCancel() : onSend())}
      disabled={isCanceling}
    >
      {#if isSending}
        {isCanceling ? "Canceling..." : "Cancel"}
      {:else}
        Send
      {/if}
    </button>
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
        <button class="ghost-button" type="button" onclick={() => addRow("queryParams")}>Add row</button>
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
            <button class="icon-button" type="button" onclick={() => removeRow("queryParams", row.id)}>Remove</button>
          </div>
        {/each}
      </div>
    </div>
  {/if}

  {#if activePanel === "headers"}
    <div class="editor-block">
      <div class="editor-header">
        <h2>Headers</h2>
        <button class="ghost-button" type="button" onclick={() => addRow("headers")}>Add row</button>
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
            <input class="text-input" value={row.key} placeholder="Header" oninput={(event) => updateRows("headers", index, { key: event.currentTarget.value })} />
            <VariableField
              className="text-input"
              value={row.value}
              variables={environmentVariables}
              placeholder="Value"
              onValueInput={(nextValue) => updateRows("headers", index, { value: nextValue })}
            />
            <button class="icon-button" type="button" onclick={() => removeRow("headers", row.id)}>Remove</button>
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
      </div>

      {#if request.body.mode === "none"}
        <div class="empty-state body-empty-state">
          This request will be sent without a body.
        </div>
      {/if}

      {#if request.body.mode === "json" || request.body.mode === "raw"}
        <VariableField
          className="body-textarea"
          multiline={true}
          value={request.body.raw}
          variables={environmentVariables}
          placeholder={request.body.mode === "json" ? '{"hello":"world"}' : "Raw request body"}
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
              <button class="icon-button" type="button" onclick={() => removeFormRow(row.id)}>Remove</button>
            </div>
          {/each}

          <button class="ghost-button" type="button" onclick={addFormRow}>Add field</button>
        </div>
      {/if}

      {#if request.body.mode === "multipart"}
        <div class="callout">
          Multipart request composition will be wired to the native file picker in the next pass.
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
            <input class="text-input" bind:value={request.auth.apiKeyName} />
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
            <select class="text-input" bind:value={request.auth.apiKeyIn}>
              <option value="header">Header</option>
              <option value="query">Query parameter</option>
            </select>
          </label>
        </div>
      {/if}
    </div>
  {/if}
</section>
