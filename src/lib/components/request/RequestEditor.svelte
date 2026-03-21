<script lang="ts">
  import { createKeyValueRow, type AuthType, type BodyMode, type KeyValueRow, type RequestDraft } from "$lib/api/types";
  import VariableField from "$lib/components/request/VariableField.svelte";

  export let request: RequestDraft;
  export let isSending = false;
  export let isCanceling = false;
  export let isSaving = false;
  export let saveLabel = "Save";
  export let saveDisabled = false;
  export let environmentVariables: KeyValueRow[] = [];
  export let onSend: () => Promise<void> | void = () => {};
  export let onCancel: () => Promise<void> | void = () => {};
  export let onSave: () => Promise<void> | void = () => {};

  let activePanel: "query" | "headers" | "body" | "auth" = "query";

  const panels = [
    { id: "query", label: "Query" },
    { id: "headers", label: "Headers" },
    { id: "body", label: "Body" },
    { id: "auth", label: "Auth" }
  ] as const;

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
  <div class="request-topbar">
    <div class="request-name-block">
      <label class="field-label" for="request-name">Request</label>
      <input
        id="request-name"
        class="text-input"
        bind:value={request.name}
        placeholder="Untitled request"
      />
    </div>

    <div class="request-actions">
      <button class="ghost-button request-save-button" type="button" on:click={onSave} disabled={saveDisabled || isSaving}>
        {isSaving ? "Saving..." : saveLabel}
      </button>

      <button
        class:cancel-button={isSending}
        class="send-button"
        type="button"
        on:click={() => (isSending ? onCancel() : onSend())}
        disabled={isCanceling}
      >
        {#if isSending}
          {isCanceling ? "Canceling..." : "Cancel"}
        {:else}
          Send
        {/if}
      </button>
    </div>
  </div>

  <div class="url-row">
    <select class="method-select" bind:value={request.method}>
      <option value="GET">GET</option>
      <option value="POST">POST</option>
      <option value="PUT">PUT</option>
      <option value="PATCH">PATCH</option>
      <option value="DELETE">DELETE</option>
      <option value="HEAD">HEAD</option>
      <option value="OPTIONS">OPTIONS</option>
    </select>

    <VariableField
      className="text-input url-input"
      value={request.url}
      variables={environmentVariables}
      placeholder="https://api.example.com/resource"
      spellcheck={false}
      onValueInput={(nextValue) => (request = { ...request, url: nextValue })}
    />
  </div>

  <div class="panel-tabs">
    {#each panels as panel}
      <button
        class:active={activePanel === panel.id}
        class="tab-button"
        type="button"
        on:click={() => (activePanel = panel.id)}
      >
        {panel.label}
      </button>
    {/each}
  </div>

  {#if activePanel === "query"}
    <div class="editor-block">
      <div class="editor-header">
        <h2>Query Parameters</h2>
        <button class="ghost-button" type="button" on:click={() => addRow("queryParams")}>Add row</button>
      </div>

      <div class="row-list">
        {#each request.queryParams as row, index (row.id)}
          <div class="kv-row">
            <input type="checkbox" checked={row.enabled} on:change={(event) => updateRows("queryParams", index, { enabled: event.currentTarget.checked })} />
            <input class="text-input" value={row.key} placeholder="Key" on:input={(event) => updateRows("queryParams", index, { key: event.currentTarget.value })} />
            <VariableField
              className="text-input"
              value={row.value}
              variables={environmentVariables}
              placeholder="Value"
              onValueInput={(nextValue) => updateRows("queryParams", index, { value: nextValue })}
            />
            <button class="icon-button" type="button" on:click={() => removeRow("queryParams", row.id)}>Remove</button>
          </div>
        {/each}
      </div>
    </div>
  {/if}

  {#if activePanel === "headers"}
    <div class="editor-block">
      <div class="editor-header">
        <h2>Headers</h2>
        <button class="ghost-button" type="button" on:click={() => addRow("headers")}>Add row</button>
      </div>

      <div class="row-list">
        {#each request.headers as row, index (row.id)}
          <div class="kv-row">
            <input type="checkbox" checked={row.enabled} on:change={(event) => updateRows("headers", index, { enabled: event.currentTarget.checked })} />
            <input class="text-input" value={row.key} placeholder="Header" on:input={(event) => updateRows("headers", index, { key: event.currentTarget.value })} />
            <VariableField
              className="text-input"
              value={row.value}
              variables={environmentVariables}
              placeholder="Value"
              onValueInput={(nextValue) => updateRows("headers", index, { value: nextValue })}
            />
            <button class="icon-button" type="button" on:click={() => removeRow("headers", row.id)}>Remove</button>
          </div>
        {/each}
      </div>
    </div>
  {/if}

  {#if activePanel === "body"}
    <div class="editor-block">
      <div class="editor-header split">
        <h2>Body</h2>
        <select class="body-mode-select" value={request.body.mode} on:change={(event) => updateBodyMode(event.currentTarget.value as BodyMode)}>
          <option value="none">None</option>
          <option value="json">JSON</option>
          <option value="raw">Raw</option>
          <option value="form-urlencoded">Form URL Encoded</option>
          <option value="multipart">Multipart</option>
        </select>
      </div>

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
              <input type="checkbox" checked={row.enabled} on:change={(event) => updateFormRow(index, { enabled: event.currentTarget.checked })} />
              <input class="text-input" value={row.key} placeholder="Field" on:input={(event) => updateFormRow(index, { key: event.currentTarget.value })} />
              <VariableField
                className="text-input"
                value={row.value}
                variables={environmentVariables}
                placeholder="Value"
                onValueInput={(nextValue) => updateFormRow(index, { value: nextValue })}
              />
              <button class="icon-button" type="button" on:click={() => removeFormRow(row.id)}>Remove</button>
            </div>
          {/each}

          <button class="ghost-button" type="button" on:click={addFormRow}>Add field</button>
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
    <div class="editor-block auth-grid">
      <label>
        <span class="field-label">Type</span>
        <select class="text-input" value={request.auth.type} on:change={(event) => updateAuthType(event.currentTarget.value as AuthType)}>
          <option value="none">None</option>
          <option value="basic">Basic</option>
          <option value="bearer">Bearer</option>
          <option value="api-key">API key</option>
        </select>
      </label>

      {#if request.auth.type === "basic"}
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
      {/if}

      {#if request.auth.type === "bearer"}
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
      {/if}

      {#if request.auth.type === "api-key"}
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
      {/if}
    </div>
  {/if}
</section>
