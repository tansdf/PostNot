<script lang="ts">
  import { pickMultipartFiles } from "$lib/api/commands";
  import {
    cloneRealtimeRequestDraft,
    createKeyValueRow,
    createRealtimeRequestDraft,
    type EnvironmentVariable,
    type RealtimeBinaryPayload,
    type RealtimeProtocol,
    type RealtimeRequestDraft,
    type RequestAuth
  } from "$lib/api/types";
  import RealtimeKeyValueEditor from "$lib/components/realtime/RealtimeKeyValueEditor.svelte";
  import VariableField from "$lib/components/request/VariableField.svelte";

  let {
    draft = $bindable(),
    variables = [],
    status = "disconnected",
    statusMessage = "Disconnected",
    reconnectRequired = false,
    isSaving = false,
    onConnect = () => {},
    onDisconnect = () => {},
    onPing = () => {},
    onClose = () => {},
    onSend = () => {},
    onSave = () => {},
    onSaveAs = () => {},
    onValidityChange = () => {}
  }: {
    draft: RealtimeRequestDraft;
    variables?: EnvironmentVariable[];
    status?: "disconnected" | "connecting" | "connected" | "reconnecting" | "disconnecting" | "failed";
    statusMessage?: string;
    reconnectRequired?: boolean;
    isSaving?: boolean;
    onConnect?: () => Promise<void> | void;
    onDisconnect?: () => Promise<void> | void;
    onPing?: () => Promise<void> | void;
    onClose?: (code: number, reason: string) => Promise<void> | void;
    onSend?: () => Promise<void> | void;
    onSave?: () => Promise<void> | void;
    onSaveAs?: () => Promise<void> | void;
    onValidityChange?: (valid: boolean) => void;
  } = $props();

  let activePanel: "query" | "headers" | "auth" | "protocol" | "reconnect" = $state("query");
  let composerError = $state("");
  let protocolJsonError = $state("");
  let closeCode = $state(1000);
  let closeReason = $state("");
  let showCloseOptions = $state(false);
  let authPayloadText = $state("{}");
  let argumentsText = $state("[]");
  let authPayloadFingerprint = "";
  let argumentsFingerprint = "";
  let authPayloadError = $state("");
  let argumentsError = $state("");
  let isPickingBinaryFile = $state(false);

  const panels = [
    { id: "query", label: "Query" },
    { id: "headers", label: "Headers & cookies" },
    { id: "auth", label: "Auth" },
    { id: "protocol", label: "Protocol" },
    { id: "reconnect", label: "Reconnect" }
  ] as const;

  let isConnected = $derived(status === "connected");
  let hasLiveSession = $derived(status === "connected" || status === "reconnecting");
  let isBusy = $derived(status === "connecting" || status === "disconnecting");
  let structuredJsonValid = $derived(!authPayloadError && !argumentsError);
  type WebSocketDraft = Extract<RealtimeRequestDraft, { requestType: "websocket" }>;
  type SocketIoDraft = Extract<RealtimeRequestDraft, { requestType: "socketio" }>;

  $effect(() => {
    if (draft.requestType !== "socketio") return;
    const nextAuthFingerprint = JSON.stringify(draft.authPayload);
    const nextArgumentsFingerprint = JSON.stringify(draft.composer.arguments);
    if (nextAuthFingerprint !== authPayloadFingerprint) {
      authPayloadFingerprint = nextAuthFingerprint;
      authPayloadText = JSON.stringify(draft.authPayload, null, 2);
      authPayloadError = "";
    }
    if (nextArgumentsFingerprint !== argumentsFingerprint) {
      argumentsFingerprint = nextArgumentsFingerprint;
      argumentsText = JSON.stringify(draft.composer.arguments, null, 2);
      argumentsError = "";
    }
  });

  $effect(() => {
    onValidityChange(structuredJsonValid);
  });

  function patchCommon(patch: Partial<Pick<RealtimeRequestDraft, "name" | "url" | "queryParams" | "headers" | "auth" | "reconnect">>) {
    draft = { ...draft, ...patch } as RealtimeRequestDraft;
  }

  function switchProtocol(protocol: RealtimeProtocol) {
    if (draft.requestType === protocol) return;
    const next = createRealtimeRequestDraft(protocol);
    draft = {
      ...next,
      name: draft.name,
      url: draft.url,
      queryParams: draft.queryParams,
      headers: draft.headers,
      auth: draft.auth,
      reconnect: draft.reconnect
    } as RealtimeRequestDraft;
    composerError = "";
    protocolJsonError = "";
  }

  function patchAuth(patch: Partial<RequestAuth>) {
    patchCommon({ auth: { ...draft.auth, ...patch } });
  }

  function patchWebSocket(patch: Partial<WebSocketDraft>) {
    if (draft.requestType === "websocket") draft = { ...draft, ...patch };
  }

  function patchSocketIo(patch: Partial<SocketIoDraft>) {
    if (draft.requestType === "socketio") draft = { ...draft, ...patch };
  }

  function patchRawComposer(patch: Partial<WebSocketDraft["composer"]>) {
    if (draft.requestType === "websocket") draft = { ...draft, composer: { ...draft.composer, ...patch } };
  }

  function patchSocketIoComposer(patch: Partial<SocketIoDraft["composer"]>) {
    if (draft.requestType === "socketio") draft = { ...draft, composer: { ...draft.composer, ...patch } };
  }

  function addCookieHeader() {
    patchCommon({
      headers: [...draft.headers, { ...createKeyValueRow(), key: "Cookie" }]
    });
  }

  function binarySource(binary: RealtimeBinaryPayload | null | undefined) {
    return binary?.source ?? "file";
  }

  function binaryValue(binary: RealtimeBinaryPayload | null | undefined) {
    if (!binary) return "";
    return binary.source === "file" ? binary.path : binary.value;
  }

  function buildBinary(source: "file" | "hex" | "base64", value: string): RealtimeBinaryPayload {
    return source === "file" ? { source, path: value } : { source, value };
  }

  function setSocketIoJson(field: "authPayload" | "arguments", value: string) {
    if (field === "authPayload") authPayloadText = value;
    else argumentsText = value;
    try {
      const parsed = JSON.parse(value);
      if (field === "authPayload" && draft.requestType === "socketio") {
        if (!parsed || Array.isArray(parsed) || typeof parsed !== "object") {
          throw new Error("Auth payload must be a JSON object.");
        }
        authPayloadFingerprint = JSON.stringify(parsed);
        draft = { ...draft, authPayload: parsed };
        authPayloadError = "";
      } else if (draft.requestType === "socketio") {
        if (!Array.isArray(parsed)) {
          throw new Error("Event arguments must be a JSON array.");
        }
        argumentsFingerprint = JSON.stringify(parsed);
        draft = { ...draft, composer: { ...draft.composer, arguments: parsed } };
        argumentsError = "";
      }
      protocolJsonError = "";
    } catch (error) {
      const message = error instanceof Error ? error.message : "Invalid JSON.";
      if (field === "authPayload") authPayloadError = message;
      else argumentsError = message;
      protocolJsonError = message;
    }
  }

  async function pickBinaryFile() {
    isPickingBinaryFile = true;
    try {
      const [path] = await pickMultipartFiles();
      if (!path) return;
      if (draft.requestType === "websocket") {
        draft = { ...draft, composer: { ...draft.composer, binary: { source: "file", path } } };
      } else {
        draft = { ...draft, composer: { ...draft.composer, binary: { source: "file", path } } };
      }
      composerError = "";
    } catch (error) {
      composerError = error instanceof Error ? error.message : String(error);
    } finally {
      isPickingBinaryFile = false;
    }
  }

  function validateComposer() {
    composerError = "";
    if (draft.requestType === "websocket") {
      if (draft.composer.mode === "json") {
        try {
          JSON.parse(draft.composer.content);
        } catch {
          composerError = "Message body must be valid JSON.";
        }
      } else if (draft.composer.mode === "binary" && !binaryValue(draft.composer.binary).trim()) {
        composerError = "Choose a file or enter binary data before sending.";
      }
    } else if (!draft.composer.event.trim()) {
      composerError = "Enter a Socket.IO event name.";
    } else if (draft.composer.binary && !binaryValue(draft.composer.binary).trim()) {
      composerError = "Choose a file or enter binary data before sending.";
    }
    return !composerError && !protocolJsonError;
  }

  async function send() {
    if (validateComposer()) await onSend();
  }

  function formatJson() {
    if (draft.requestType !== "websocket") return;
    try {
      draft = {
        ...draft,
        composer: { ...draft.composer, content: JSON.stringify(JSON.parse(draft.composer.content), null, 2) }
      };
      composerError = "";
    } catch {
      composerError = "Message body must be valid JSON.";
    }
  }
</script>

<section class="panel realtime-editor" aria-labelledby="realtime-editor-title">
  <div class="editor-header">
    <div>
      <p class="eyebrow">Realtime request</p>
      <h1 id="realtime-editor-title">{draft.requestType === "socketio" ? "Socket.IO connection" : "WebSocket connection"}</h1>
    </div>
    <div class="request-actions">
      <button class="button-secondary" type="button" onclick={onSaveAs} disabled={isSaving || !structuredJsonValid}>Save as…</button>
      <button class="button-secondary" type="button" onclick={onSave} disabled={isSaving || !structuredJsonValid}>
        {isSaving ? "Saving…" : "Save"}
      </button>
    </div>
  </div>

  <div class="realtime-connection-header">
    <label class="request-name-block">
      <span class="field-label">Name</span>
      <input class="text-input" value={draft.name} oninput={(event) => patchCommon({ name: event.currentTarget.value })} />
    </label>
    <label>
      <span class="field-label">Mode</span>
      <select class="method-select realtime-protocol-select" value={draft.requestType} onchange={(event) => switchProtocol(event.currentTarget.value as RealtimeProtocol)}>
        <option value="websocket">WebSocket</option>
        <option value="socketio">Socket.IO</option>
      </select>
    </label>
    <label class="realtime-url-field">
      <span class="field-label">Connection URL</span>
      <VariableField
        value={draft.url}
        {variables}
        className="text-input url-input"
        placeholder={draft.requestType === "websocket" ? "wss://api.example.com/socket" : "https://api.example.com"}
        onValueInput={(value) => patchCommon({ url: value })}
      />
    </label>
    <div class="realtime-connect-actions">
      {#if hasLiveSession}
        <button class="button-danger button-large" type="button" onclick={onDisconnect} disabled={isBusy}>Disconnect</button>
      {:else}
        <button class="button-primary button-large" type="button" onclick={onConnect} disabled={isBusy || !draft.url.trim() || !structuredJsonValid}>
          {status === "connecting" ? "Connecting…" : "Connect"}
        </button>
      {/if}
    </div>
  </div>

  <div class="realtime-status-row" aria-live="polite">
    <span class={["realtime-status-dot", `realtime-status-${status}`]} aria-hidden="true"></span>
    <strong>{statusMessage}</strong>
    {#if reconnectRequired}<span class="status-pill status-warning">Reconnect required</span>{/if}
  </div>

  <div class="panel-tabs" role="tablist" aria-label="Connection settings">
    {#each panels as panel (panel.id)}
      <button class:active={activePanel === panel.id} class="tab-button" type="button" role="tab" aria-selected={activePanel === panel.id} onclick={() => (activePanel = panel.id)}>
        {panel.label}
      </button>
    {/each}
  </div>

  <div class="realtime-settings-panel" role="tabpanel">
    {#if activePanel === "query"}
      <RealtimeKeyValueEditor bind:rows={draft.queryParams} {variables} keyLabel="Parameter" valueLabel="Value" addLabel="Add parameter" />
    {:else if activePanel === "headers"}
      <div class="realtime-section-toolbar">
        <p class="field-help">Handshake headers are resolved when you connect. Add cookies using a standard Cookie header.</p>
        <button class="button-secondary button-compact" type="button" onclick={addCookieHeader}>Add Cookie header</button>
      </div>
      <RealtimeKeyValueEditor bind:rows={draft.headers} {variables} keyLabel="Header" valueLabel="Value" addLabel="Add header" />
    {:else if activePanel === "auth"}
      <div class="auth-grid realtime-auth-grid">
        <label>
          <span class="field-label">Authentication</span>
          <select class="text-input" value={draft.auth.type} onchange={(event) => patchAuth({ type: event.currentTarget.value as RequestAuth["type"] })}>
            <option value="none">None</option>
            <option value="basic">Basic auth</option>
            <option value="bearer">Bearer token</option>
            <option value="api-key">API key</option>
          </select>
        </label>
        {#if draft.auth.type === "basic"}
          <label><span class="field-label">Username</span><VariableField value={draft.auth.basicUsername} {variables} className="text-input" onValueInput={(value) => patchAuth({ basicUsername: value })} /></label>
          <label><span class="field-label">Password</span><VariableField value={draft.auth.basicPassword} {variables} className="text-input" type="password" onValueInput={(value) => patchAuth({ basicPassword: value })} /></label>
        {:else if draft.auth.type === "bearer"}
          <label class="realtime-wide-field"><span class="field-label">Bearer token</span><VariableField value={draft.auth.bearerToken} {variables} className="text-input" type="password" onValueInput={(value) => patchAuth({ bearerToken: value })} /></label>
        {:else if draft.auth.type === "api-key"}
          <label><span class="field-label">Key name</span><VariableField value={draft.auth.apiKeyName} {variables} className="text-input" onValueInput={(value) => patchAuth({ apiKeyName: value })} /></label>
          <label><span class="field-label">Key value</span><VariableField value={draft.auth.apiKeyValue} {variables} className="text-input" type="password" onValueInput={(value) => patchAuth({ apiKeyValue: value })} /></label>
          <label><span class="field-label">Placement</span><select class="text-input" value={draft.auth.apiKeyIn} onchange={(event) => patchAuth({ apiKeyIn: event.currentTarget.value as "header" | "query" })}><option value="header">Header</option><option value="query">Query</option></select></label>
        {/if}
      </div>
    {:else if activePanel === "protocol"}
      {#if draft.requestType === "websocket"}
        <label>
          <span class="field-label">Requested subprotocols</span>
          <VariableField
            value={draft.subprotocols.join(", ")}
            {variables}
            className="text-input"
            placeholder="graphql-transport-ws, chat"
            onValueInput={(value) => patchWebSocket({ subprotocols: value.split(",").map((item) => item.trim()).filter(Boolean) })}
          />
          <span class="field-help">Comma-separated, in preference order.</span>
        </label>
      {:else}
        <div class="realtime-protocol-grid">
          <label><span class="field-label">Engine.IO path</span><VariableField value={draft.path} {variables} className="text-input" onValueInput={(value) => patchSocketIo({ path: value })} /></label>
          <label><span class="field-label">Namespace</span><VariableField value={draft.namespace} {variables} className="text-input" onValueInput={(value) => patchSocketIo({ namespace: value })} /></label>
          <label><span class="field-label">Transport</span><select class="text-input" value={draft.transport} onchange={(event) => patchSocketIo({ transport: event.currentTarget.value as "auto" | "websocketOnly" })}><option value="auto">Auto (polling + upgrade)</option><option value="websocketOnly">WebSocket only</option></select></label>
          <label class="realtime-wide-field"><span class="field-label">Auth payload (JSON object)</span><textarea class="body-textarea realtime-json-input" spellcheck="false" bind:value={authPayloadText} oninput={(event) => setSocketIoJson("authPayload", event.currentTarget.value)} aria-invalid={Boolean(authPayloadError)}></textarea>{#if authPayloadError}<span class="field-error" role="alert">{authPayloadError}</span>{/if}</label>
        </div>
      {/if}
    {:else}
      <div class="realtime-reconnect-grid">
        <label class="settings-checkbox-line">
          <input type="checkbox" checked={draft.reconnect.enabled} onchange={(event) => patchCommon({ reconnect: { ...draft.reconnect, enabled: event.currentTarget.checked } })} />
          <span><strong>Reconnect automatically</strong><small>Retry abnormal transport failures only.</small></span>
        </label>
        <label><span class="field-label">Maximum attempts</span><input class="text-input" type="number" min="1" max="20" value={draft.reconnect.maxAttempts} disabled={!draft.reconnect.enabled} oninput={(event) => patchCommon({ reconnect: { ...draft.reconnect, maxAttempts: event.currentTarget.valueAsNumber || 1 } })} /></label>
        <label><span class="field-label">Initial delay (ms)</span><input class="text-input" type="number" min="100" value={draft.reconnect.initialDelayMs} disabled={!draft.reconnect.enabled} oninput={(event) => patchCommon({ reconnect: { ...draft.reconnect, initialDelayMs: event.currentTarget.valueAsNumber || 500 } })} /></label>
        <label><span class="field-label">Maximum delay (ms)</span><input class="text-input" type="number" min="100" value={draft.reconnect.maxDelayMs} disabled={!draft.reconnect.enabled} oninput={(event) => patchCommon({ reconnect: { ...draft.reconnect, maxDelayMs: event.currentTarget.valueAsNumber || 10000 } })} /></label>
      </div>
    {/if}
    {#if protocolJsonError}<p class="feedback feedback-error" role="alert">{protocolJsonError}</p>{/if}
  </div>

  <section class="realtime-composer" aria-labelledby="realtime-composer-title">
    <div class="request-section-header">
      <div><h2 id="realtime-composer-title">Message composer</h2><p class="field-help">Composer values resolve against the active environment when sent.</p></div>
      <div class="request-actions">
        {#if isConnected && draft.requestType === "websocket"}
          <button class="button-secondary button-compact" type="button" onclick={onPing}>Ping</button>
          <button class="button-ghost button-compact" type="button" onclick={() => (showCloseOptions = !showCloseOptions)}>Close options</button>
        {/if}
        <button class="button-primary" type="button" onclick={send} disabled={!isConnected || !structuredJsonValid}>Send</button>
      </div>
    </div>

    {#if showCloseOptions && draft.requestType === "websocket"}
      <div class="realtime-close-row">
        <label><span class="field-label">Close code</span><input class="text-input" type="number" min="1000" max="4999" bind:value={closeCode} /></label>
        <label><span class="field-label">Reason</span><input class="text-input" maxlength="123" bind:value={closeReason} /></label>
        <button class="button-danger" type="button" onclick={() => onClose(closeCode, closeReason)}>Close gracefully</button>
      </div>
    {/if}

    {#if draft.requestType === "websocket"}
      <div class="realtime-composer-toolbar">
        <label><span class="field-label">Payload type</span><select class="body-mode-select" value={draft.composer.mode} onchange={(event) => patchRawComposer({ mode: event.currentTarget.value as "text" | "json" | "binary" })}><option value="text">Text</option><option value="json">JSON</option><option value="binary">Binary</option></select></label>
        {#if draft.composer.mode === "json"}<button class="button-secondary button-compact" type="button" onclick={formatJson}>Format JSON</button>{/if}
      </div>
      {#if draft.composer.mode === "binary"}
        <div class="realtime-binary-grid">
          <label><span class="field-label">Binary source</span><select class="text-input" value={binarySource(draft.composer.binary)} onchange={(event) => patchRawComposer({ binary: buildBinary(event.currentTarget.value as "file" | "hex" | "base64", binaryValue(draft.requestType === "websocket" ? draft.composer.binary : null)) })}><option value="file">Local file path</option><option value="hex">Hex</option><option value="base64">Base64</option></select></label>
          <label><span class="field-label">{binarySource(draft.composer.binary) === "file" ? "File" : "Binary value"}</span><div class="realtime-file-control"><VariableField value={binaryValue(draft.composer.binary)} {variables} className="text-input" disabled={binarySource(draft.composer.binary) === "file"} placeholder={binarySource(draft.composer.binary) === "file" ? "Choose a local file" : ""} onValueInput={(value) => patchRawComposer({ binary: buildBinary(binarySource(draft.requestType === "websocket" ? draft.composer.binary : null), value) })} />{#if binarySource(draft.composer.binary) === "file"}<button class="button-secondary" type="button" onclick={pickBinaryFile} disabled={isPickingBinaryFile}>{isPickingBinaryFile ? "Choosing…" : "Choose file…"}</button>{/if}</div></label>
        </div>
      {:else}
        <VariableField value={draft.composer.content} {variables} className="body-textarea realtime-message-input" multiline={true} spellcheck={false} placeholder={draft.composer.mode === "json" ? '{\n  \"message\": \"hello\"\n}' : "Type a message…"} onValueInput={(value) => patchRawComposer({ content: value })} />
      {/if}
    {:else}
      <div class="realtime-event-grid">
        <label><span class="field-label">Event</span><VariableField value={draft.composer.event} {variables} className="text-input" placeholder="message" onValueInput={(value) => patchSocketIoComposer({ event: value })} /></label>
        <label><span class="field-label">Payload type</span><select class="text-input" value={draft.composer.binary ? "binary" : "json"} onchange={(event) => patchSocketIoComposer({ binary: event.currentTarget.value === "binary" ? { source: "file", path: "" } : null })}><option value="json">JSON arguments</option><option value="binary">One binary payload</option></select></label>
        {#if draft.composer.binary}
          <label><span class="field-label">Binary source</span><select class="text-input" value={binarySource(draft.composer.binary)} onchange={(event) => patchSocketIoComposer({ binary: buildBinary(event.currentTarget.value as "file" | "hex" | "base64", binaryValue(draft.requestType === "socketio" ? draft.composer.binary : null)) })}><option value="file">Local file</option><option value="hex">Hex</option><option value="base64">Base64</option></select></label>
          <label class="realtime-wide-field"><span class="field-label">{binarySource(draft.composer.binary) === "file" ? "File" : "Binary value"}</span><div class="realtime-file-control"><VariableField value={binaryValue(draft.composer.binary)} {variables} className="text-input" disabled={binarySource(draft.composer.binary) === "file"} placeholder={binarySource(draft.composer.binary) === "file" ? "Choose a local file" : ""} onValueInput={(value) => patchSocketIoComposer({ binary: buildBinary(binarySource(draft.requestType === "socketio" ? draft.composer.binary : null), value) })} />{#if binarySource(draft.composer.binary) === "file"}<button class="button-secondary" type="button" onclick={pickBinaryFile} disabled={isPickingBinaryFile}>{isPickingBinaryFile ? "Choosing…" : "Choose file…"}</button>{/if}</div></label>
        {:else}
          <label class="realtime-wide-field"><span class="field-label">Arguments (JSON array)</span><textarea class="body-textarea realtime-message-input" spellcheck="false" bind:value={argumentsText} oninput={(event) => setSocketIoJson("arguments", event.currentTarget.value)} aria-invalid={Boolean(argumentsError)}></textarea>{#if argumentsError}<span class="field-error" role="alert">{argumentsError}</span>{/if}</label>
        {/if}
        <label class="settings-checkbox-line"><input type="checkbox" checked={draft.composer.waitForAck} onchange={(event) => patchSocketIoComposer({ waitForAck: event.currentTarget.checked })} /><span><strong>Wait for acknowledgement</strong><small>Show the server ACK or timeout.</small></span></label>
        <label><span class="field-label">ACK timeout (ms)</span><input class="text-input" type="number" min="100" max="60000" value={draft.composer.ackTimeoutMs} disabled={!draft.composer.waitForAck} oninput={(event) => patchSocketIoComposer({ ackTimeoutMs: event.currentTarget.valueAsNumber || 5000 })} /></label>
      </div>
    {/if}
    {#if composerError}<p class="feedback feedback-error" role="alert">{composerError}</p>{/if}
  </section>
</section>
