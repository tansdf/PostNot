<script lang="ts">
  import { pickMultipartFiles } from "$lib/api/commands";
  import {
    createRealtimeConnectionDraft,
    createRealtimeMessageDraft,
    type EnvironmentVariable,
    type RealtimeBinaryPayload,
    type RealtimeConnectionDraft,
    type RealtimeConnectionProfileSummary,
    type RealtimeMessageDraft,
    type RealtimeProtocol,
  } from "$lib/api/types";
  import AuthEditor from "$lib/components/request/AuthEditor.svelte";
  import JsonEditor from "$lib/components/request/JsonEditor.svelte";
  import KeyValueEditor from "$lib/components/request/KeyValueEditor.svelte";
  import SaveSplitControl from "$lib/components/request/SaveSplitControl.svelte";
  import SendControl from "$lib/components/request/SendControl.svelte";
  import VariableField from "$lib/components/request/VariableField.svelte";
  import { getHeaderNameSuggestions, getHeaderValueSuggestions } from "$lib/header-suggestions";

  let {
    connection = $bindable(),
    message = $bindable(),
    profiles = [],
    selectedProfileId = null,
    selectedMessageId = null,
    connectionDirty = false,
    messageDirty = false,
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
    onNewMessage = () => {},
    onSave = () => {},
    onSaveAs = () => {},
    onSelectProfile = () => {},
    onNewProfile = () => {},
    onSaveProfile = () => {},
    onSaveProfileAs = () => {},
    onDeleteProfile = () => {},
    onImportProfiles = () => {},
    onExportProfile = () => {},
    onValidityChange = () => {}
  }: {
    connection: RealtimeConnectionDraft;
    message: RealtimeMessageDraft;
    profiles?: RealtimeConnectionProfileSummary[];
    selectedProfileId?: string | null;
    selectedMessageId?: string | null;
    connectionDirty?: boolean;
    messageDirty?: boolean;
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
    onNewMessage?: () => Promise<void> | void;
    onSave?: () => Promise<void> | void;
    onSaveAs?: () => Promise<void> | void;
    onSelectProfile?: (profileId: string) => Promise<void> | void;
    onNewProfile?: () => Promise<void> | void;
    onSaveProfile?: () => Promise<void> | void;
    onSaveProfileAs?: () => Promise<void> | void;
    onDeleteProfile?: () => Promise<void> | void;
    onImportProfiles?: () => Promise<void> | void;
    onExportProfile?: () => Promise<void> | void;
    onValidityChange?: (valid: boolean) => void;
  } = $props();

  let activePanel: "query" | "headers" | "auth" | "protocol" | "reconnect" = $state("query");
  let composerError = $state("");
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
  let hasLiveSession = $derived(status !== "disconnected" && status !== "failed");
  let isBusy = $derived(status === "connecting" || status === "disconnecting");
  let structuredJsonValid = $derived(!authPayloadError && !argumentsError);
  let protocolCompatible = $derived(connection.protocol === message.protocol);
  let connectionStateHelp = $derived(
    hasLiveSession
      ? "Connection settings are locked while this session is active. Disconnect to edit them."
      : "Choose a saved profile or edit the connection details, then connect."
  );
  let sendAvailabilityText = $derived(
    !protocolCompatible
      ? "Match the message protocol to this connection before sending."
      : reconnectRequired
        ? "Reconnect to apply the latest environment values before sending."
        : !isConnected
          ? "Connect this tab to send the message."
          : "Ready to send on the current session."
  );
  type CombinedDraft =
    | (Extract<RealtimeConnectionDraft, { protocol: "websocket" }> & { requestType: "websocket"; composer: Extract<RealtimeMessageDraft, { protocol: "websocket" }>["composer"] })
    | (Extract<RealtimeConnectionDraft, { protocol: "socketio" }> & { requestType: "socketio"; composer: Extract<RealtimeMessageDraft, { protocol: "socketio" }>["composer"] });
  let draft = $derived({ ...connection, requestType: connection.protocol, composer: message.protocol === connection.protocol ? message.composer : createRealtimeMessageDraft(connection.protocol).composer } as CombinedDraft);
  let headerNameSuggestions = $derived(getHeaderNameSuggestions(draft.headers));
  type WebSocketDraft = Extract<CombinedDraft, { requestType: "websocket" }>;
  type SocketIoDraft = Extract<CombinedDraft, { requestType: "socketio" }>;

  function panelDomId(panelId: (typeof panels)[number]["id"]) {
    return `realtime-settings-tab-${panelId}`;
  }

  function handlePanelKeydown(event: KeyboardEvent, panelIndex: number) {
    let nextIndex = panelIndex;
    if (event.key === "ArrowRight") nextIndex = (panelIndex + 1) % panels.length;
    else if (event.key === "ArrowLeft") nextIndex = (panelIndex - 1 + panels.length) % panels.length;
    else if (event.key === "Home") nextIndex = 0;
    else if (event.key === "End") nextIndex = panels.length - 1;
    else return;
    event.preventDefault();
    activePanel = panels[nextIndex].id;
    document.getElementById(panelDomId(activePanel))?.focus();
  }

  $effect(() => {
    if (connection.protocol === "socketio") {
      const nextAuthFingerprint = JSON.stringify(connection.authPayload);
      if (nextAuthFingerprint !== authPayloadFingerprint) {
        authPayloadFingerprint = nextAuthFingerprint;
        authPayloadText = JSON.stringify(connection.authPayload, null, 2);
        authPayloadError = "";
      }
    }
    if (message.protocol === "socketio") {
      const nextArgumentsFingerprint = JSON.stringify(message.composer.arguments);
      if (nextArgumentsFingerprint !== argumentsFingerprint) {
        argumentsFingerprint = nextArgumentsFingerprint;
        argumentsText = JSON.stringify(message.composer.arguments, null, 2);
        argumentsError = "";
      }
    }
  });

  $effect(() => {
    onValidityChange(structuredJsonValid);
  });

  $effect(() => {
    if (!isConnected) showCloseOptions = false;
  });

  function patchCommon(patch: Partial<Pick<RealtimeConnectionDraft, "name" | "url" | "queryParams" | "headers" | "auth" | "reconnect">>) {
    connection = { ...connection, ...patch } as RealtimeConnectionDraft;
  }

  function switchProtocol(protocol: RealtimeProtocol) {
    if (connection.protocol === protocol) return;
    const next = createRealtimeConnectionDraft(protocol);
    connection = {
      ...next,
      name: connection.name,
      url: connection.url,
      queryParams: connection.queryParams,
      headers: connection.headers,
      auth: connection.auth,
      reconnect: connection.reconnect
    } as RealtimeConnectionDraft;
    composerError = "";
  }

  function switchMessageProtocol(protocol: RealtimeProtocol) {
    if (message.protocol === protocol) return;
    const next = createRealtimeMessageDraft(protocol);
    next.name = message.name;
    message = next;
    composerError = "";
  }

  function patchWebSocket(patch: Partial<WebSocketDraft>) {
    if (connection.protocol === "websocket") connection = { ...connection, ...patch };
  }

  function patchSocketIo(patch: Partial<SocketIoDraft>) {
    if (connection.protocol === "socketio") connection = { ...connection, ...patch };
  }

  function patchRawComposer(patch: Partial<WebSocketDraft["composer"]>) {
    if (message.protocol === "websocket") message = { ...message, composer: { ...message.composer, ...patch } };
  }

  function patchSocketIoComposer(patch: Partial<SocketIoDraft["composer"]>) {
    if (message.protocol === "socketio") message = { ...message, composer: { ...message.composer, ...patch } };
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
      if (field === "authPayload" && connection.protocol === "socketio") {
        if (!parsed || Array.isArray(parsed) || typeof parsed !== "object") {
          throw new Error("Auth payload must be a JSON object.");
        }
        authPayloadFingerprint = JSON.stringify(parsed);
        connection = { ...connection, authPayload: parsed };
        authPayloadError = "";
      } else if (message.protocol === "socketio") {
        if (!Array.isArray(parsed)) {
          throw new Error("Event arguments must be a JSON array.");
        }
        argumentsFingerprint = JSON.stringify(parsed);
        message = { ...message, composer: { ...message.composer, arguments: parsed } };
        argumentsError = "";
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : "Invalid JSON.";
      if (field === "authPayload") authPayloadError = message;
      else argumentsError = message;
    }
  }

  async function pickBinaryFile() {
    isPickingBinaryFile = true;
    try {
      const [path] = await pickMultipartFiles();
      if (!path) return;
      if (message.protocol === "websocket") message = { ...message, composer: { ...message.composer, binary: { source: "file", path } } };
      else message = { ...message, composer: { ...message.composer, binary: { source: "file", path } } };
      composerError = "";
    } catch (error) {
      composerError = error instanceof Error ? error.message : String(error);
    } finally {
      isPickingBinaryFile = false;
    }
  }

  function validateComposer() {
    composerError = "";
    if (message.protocol === "websocket") {
      if (message.composer.mode === "json") {
        try {
          JSON.parse(message.composer.content);
        } catch {
          composerError = "Message body must be valid JSON.";
        }
      } else if (message.composer.mode === "binary" && !binaryValue(message.composer.binary).trim()) {
        composerError = "Choose a file or enter binary data before sending.";
      }
    } else if (!message.composer.event.trim()) {
      composerError = "Enter a Socket.IO event name.";
    } else if (message.composer.binary && !binaryValue(message.composer.binary).trim()) {
      composerError = "Choose a file or enter binary data before sending.";
    }
    return !composerError && structuredJsonValid;
  }

  async function send() {
    if (validateComposer()) await onSend();
  }

  function formatJson() {
    if (message.protocol !== "websocket") return;
    try {
      if (message.protocol === "websocket") message = { ...message, composer: { ...message.composer, content: JSON.stringify(JSON.parse(message.composer.content), null, 2) } };
      composerError = "";
    } catch {
      composerError = "Message body must be valid JSON.";
    }
  }
</script>

<section class="panel panel-inset realtime-editor" aria-labelledby="realtime-editor-title">
  <div class="realtime-resource-header">
    <div class="panel-heading realtime-resource-heading">
      <p class="eyebrow">Connection profile</p>
      <h1 class="panel-title" id="realtime-editor-title">{draft.requestType === "socketio" ? "Socket.IO connection" : "WebSocket connection"}</h1>
      <p class="field-help">The connection belongs to this tab. Messages can change without interrupting the session.</p>
      {#if connectionDirty}<span class="status-pill status-unsaved">Unsaved connection changes</span>{/if}
    </div>
    <div class="realtime-profile-manager" aria-label="Connection profile management">
      <label class="realtime-profile-picker">
        <span class="field-label">Profile</span>
        <select class="text-input" aria-label="Connection profile" value={selectedProfileId ?? ""} disabled={hasLiveSession} onchange={(event) => onSelectProfile(event.currentTarget.value)}>
          <option value="" disabled={Boolean(selectedProfileId)}>Unsaved connection</option>
          {#each profiles as profile (profile.id)}<option value={profile.id}>{profile.name}</option>{/each}
        </select>
      </label>
      <div class="realtime-profile-actions">
        <button class="button-secondary" type="button" onclick={onNewProfile} disabled={hasLiveSession}>New</button>
        <button class="button-secondary" type="button" onclick={onSaveProfile} disabled={hasLiveSession || isSaving}>{isSaving ? "Saving…" : "Save"}</button>
        <details class="request-actions-menu">
          <summary class="button-secondary">More</summary>
          <div class="request-actions-menu-popover">
            <button class="button-ghost" type="button" onclick={onSaveProfileAs} disabled={hasLiveSession || isSaving}>Save as…</button>
            <button class="button-ghost" type="button" onclick={onImportProfiles} disabled={hasLiveSession}>Import…</button>
            <button class="button-ghost" type="button" onclick={onExportProfile} disabled={!selectedProfileId}>Export…</button>
            <span class="menu-separator" aria-hidden="true"></span>
            <button class="button-ghost menu-danger" type="button" onclick={onDeleteProfile} disabled={hasLiveSession || !selectedProfileId}>Delete…</button>
          </div>
        </details>
      </div>
    </div>
  </div>

  <div class="realtime-connection-header">
    <label class="request-name-block">
      <span class="field-label">Name</span>
      <input class="text-input" value={draft.name} disabled={hasLiveSession} oninput={(event) => patchCommon({ name: event.currentTarget.value })} />
    </label>
    <label>
      <span class="field-label">Connection protocol</span>
      <select class="method-select realtime-protocol-select" value={draft.requestType} disabled={hasLiveSession} onchange={(event) => switchProtocol(event.currentTarget.value as RealtimeProtocol)}>
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
        disabled={hasLiveSession}
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
    <span class="realtime-status-help">{connectionStateHelp}</span>
  </div>

  <div class="panel-tabs" role="tablist" aria-label="Connection settings">
    {#each panels as panel, index (panel.id)}
      <button
        id={panelDomId(panel.id)}
        class:active={activePanel === panel.id}
        class="tab-button"
        type="button"
        role="tab"
        aria-selected={activePanel === panel.id}
        aria-controls="realtime-settings-panel"
        tabindex={activePanel === panel.id ? 0 : -1}
        onclick={() => (activePanel = panel.id)}
        onkeydown={(event) => handlePanelKeydown(event, index)}
      >
        {panel.label}
      </button>
    {/each}
  </div>

  <div
    id="realtime-settings-panel"
    class="realtime-settings-panel"
    class:realtime-settings-locked={hasLiveSession}
    role="tabpanel"
    aria-labelledby={panelDomId(activePanel)}
    tabindex="0"
    inert={hasLiveSession ? true : undefined}
  >
    {#if activePanel === "query"}
      <KeyValueEditor
        rows={draft.queryParams}
        {variables}
        title="Query Parameters"
        keyLabel="Parameter"
        valueLabel="Value"
        onRowsChange={(queryParams) => patchCommon({ queryParams })}
      />
    {:else if activePanel === "headers"}
      <KeyValueEditor
        rows={draft.headers}
        {variables}
        title="Headers"
        description="Handshake headers are resolved when you connect. Add cookies using a standard Cookie header."
        keyLabel="Header"
        valueLabel="Value"
        keySuggestions={headerNameSuggestions}
        getValueSuggestions={(key) => getHeaderValueSuggestions(key, draft.headers)}
        onRowsChange={(headers) => patchCommon({ headers })}
      />
    {:else if activePanel === "auth"}
      <AuthEditor
        auth={draft.auth}
        {variables}
        emptyMessage="This connection will be opened without authentication."
        onAuthChange={(auth) => patchCommon({ auth })}
      />
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
          <label class="realtime-wide-field">
            <span class="field-label">Auth payload (JSON object)</span>
            <JsonEditor
              value={authPayloadText}
              {variables}
              className="body-textarea realtime-json-input"
              ariaLabel="Auth payload (JSON object)"
              ariaInvalid={Boolean(authPayloadError)}
              onValueInput={(value) => setSocketIoJson("authPayload", value)}
            />
            {#if authPayloadError}<span class="field-error" role="alert">{authPayloadError}</span>{/if}
          </label>
        </div>
      {/if}
    {:else}
      <div class="realtime-reconnect-grid">
        <label class="settings-checkbox-line">
          <input class="row-toggle settings-checkbox" type="checkbox" checked={draft.reconnect.enabled} onchange={(event) => patchCommon({ reconnect: { ...draft.reconnect, enabled: event.currentTarget.checked } })} />
          <span><strong>Reconnect automatically</strong><small>Retry abnormal transport failures only.</small></span>
        </label>
        <label><span class="field-label">Maximum attempts</span><input class="text-input" type="number" min="1" max="20" value={draft.reconnect.maxAttempts} disabled={!draft.reconnect.enabled} oninput={(event) => patchCommon({ reconnect: { ...draft.reconnect, maxAttempts: event.currentTarget.valueAsNumber || 1 } })} /></label>
        <label><span class="field-label">Initial delay (ms)</span><input class="text-input" type="number" min="100" value={draft.reconnect.initialDelayMs} disabled={!draft.reconnect.enabled} oninput={(event) => patchCommon({ reconnect: { ...draft.reconnect, initialDelayMs: event.currentTarget.valueAsNumber || 500 } })} /></label>
        <label><span class="field-label">Maximum delay (ms)</span><input class="text-input" type="number" min="100" value={draft.reconnect.maxDelayMs} disabled={!draft.reconnect.enabled} oninput={(event) => patchCommon({ reconnect: { ...draft.reconnect, maxDelayMs: event.currentTarget.valueAsNumber || 10000 } })} /></label>
      </div>
    {/if}
  </div>

  <section class="realtime-composer" aria-labelledby="realtime-composer-title">
    <div class="request-section-header">
      <div class="panel-heading">
        <p class="eyebrow">Collection message</p>
        <h2 id="realtime-composer-title">Message composer</h2>
        <p class="field-help">Choose or edit a message without reconnecting. Values resolve against the active environment when sent.</p>
        {#if messageDirty}<span class="status-pill status-unsaved">Unsaved message changes</span>{/if}
      </div>
      {#if isConnected && draft.requestType === "websocket"}
        <div class="request-actions">
          <button class="button-secondary button-compact" type="button" onclick={onPing}>Ping</button>
          <button
            class="button-ghost button-compact"
            type="button"
            aria-expanded={showCloseOptions}
            aria-controls="realtime-close-options"
            onclick={() => (showCloseOptions = !showCloseOptions)}
          >Close options</button>
        </div>
      {/if}
    </div>
    <div class="realtime-message-header">
      <label class="request-name-block"><span class="field-label">Message name</span><input class="text-input" value={message.name} oninput={(event) => (message = { ...message, name: event.currentTarget.value })} /></label>
      <label><span class="field-label">Message protocol</span><select class="method-select realtime-protocol-select" value={message.protocol} onchange={(event) => switchMessageProtocol(event.currentTarget.value as RealtimeProtocol)}><option value="websocket">WebSocket</option><option value="socketio">Socket.IO</option></select></label>
      <div class="realtime-message-primary-actions">
        <button class="button-secondary button-large" type="button" onclick={onNewMessage} disabled={isSaving}>New</button>
        <SaveSplitControl
          label={selectedMessageId ? "Update" : "Save"}
          {isSaving}
          disabled={!structuredJsonValid}
          showMenu={true}
          onSave={onSave}
          onSaveAs={onSaveAs}
        />
        <SendControl onSend={send} disabled={!isConnected || reconnectRequired || !structuredJsonValid || !protocolCompatible} />
      </div>
    </div>
    {#if !protocolCompatible}<p class="feedback feedback-error" role="alert">This {message.protocol === "socketio" ? "Socket.IO" : "WebSocket"} message is incompatible with the selected {connection.protocol === "socketio" ? "Socket.IO" : "WebSocket"} connection.</p>{/if}
    <p class="realtime-send-hint" class:realtime-send-hint-ready={isConnected && !reconnectRequired && protocolCompatible}>{sendAvailabilityText}</p>

    {#if showCloseOptions && draft.requestType === "websocket"}
      <div id="realtime-close-options" class="realtime-close-row">
        <label><span class="field-label">Close code</span><input class="text-input" type="number" min="1000" max="4999" bind:value={closeCode} /></label>
        <label><span class="field-label">Reason</span><input class="text-input" maxlength="123" bind:value={closeReason} /></label>
        <button class="button-danger" type="button" onclick={() => onClose(closeCode, closeReason)}>Close gracefully</button>
      </div>
    {/if}

    {#if message.protocol === "websocket"}
      <div class="realtime-composer-toolbar">
        <label><span class="field-label">Payload type</span><select class="body-mode-select" value={message.composer.mode} onchange={(event) => patchRawComposer({ mode: event.currentTarget.value as "text" | "json" | "binary" })}><option value="text">Text</option><option value="json">JSON</option><option value="binary">Binary</option></select></label>
        {#if message.composer.mode === "json"}<button class="button-secondary button-compact" type="button" onclick={formatJson}>Format</button>{/if}
      </div>
      {#if message.composer.mode === "binary"}
        <div class="realtime-binary-grid">
          <label><span class="field-label">Binary source</span><select class="text-input" value={binarySource(message.composer.binary)} onchange={(event) => patchRawComposer({ binary: buildBinary(event.currentTarget.value as "file" | "hex" | "base64", binaryValue(message.composer.binary)) })}><option value="file">Local file path</option><option value="hex">Hex</option><option value="base64">Base64</option></select></label>
          <label><span class="field-label">{binarySource(message.composer.binary) === "file" ? "File" : "Binary value"}</span><div class="realtime-file-control"><VariableField value={binaryValue(message.composer.binary)} {variables} className="text-input" disabled={binarySource(message.composer.binary) === "file"} placeholder={binarySource(message.composer.binary) === "file" ? "Choose a local file" : ""} onValueInput={(value) => patchRawComposer({ binary: buildBinary(binarySource(message.composer.binary), value) })} />{#if binarySource(message.composer.binary) === "file"}<button class="button-secondary" type="button" onclick={pickBinaryFile} disabled={isPickingBinaryFile}>{isPickingBinaryFile ? "Choosing…" : "Choose file…"}</button>{/if}</div></label>
        </div>
      {:else if message.composer.mode === "json"}
        <JsonEditor
          value={message.composer.content}
          {variables}
          className="body-textarea realtime-message-input"
          placeholder={'{\n  "message": "hello"\n}'}
          ariaLabel="JSON message"
          ariaInvalid={Boolean(composerError)}
          onValueInput={(value) => patchRawComposer({ content: value })}
        />
      {:else}
        <VariableField value={message.composer.content} {variables} className="body-textarea realtime-message-input" multiline={true} spellcheck={false} placeholder="Type a message…" onValueInput={(value) => patchRawComposer({ content: value })} />
      {/if}
    {:else}
      <div class="realtime-event-grid">
        <label><span class="field-label">Event</span><VariableField value={message.composer.event} {variables} className="text-input" placeholder="message" onValueInput={(value) => patchSocketIoComposer({ event: value })} /></label>
        <label><span class="field-label">Payload type</span><select class="text-input" value={message.composer.binary ? "binary" : "json"} onchange={(event) => patchSocketIoComposer({ binary: event.currentTarget.value === "binary" ? { source: "file", path: "" } : null })}><option value="json">JSON arguments</option><option value="binary">One binary payload</option></select></label>
        {#if message.composer.binary}
          <label><span class="field-label">Binary source</span><select class="text-input" value={binarySource(message.composer.binary)} onchange={(event) => patchSocketIoComposer({ binary: buildBinary(event.currentTarget.value as "file" | "hex" | "base64", binaryValue(message.composer.binary)) })}><option value="file">Local file</option><option value="hex">Hex</option><option value="base64">Base64</option></select></label>
          <label class="realtime-wide-field"><span class="field-label">{binarySource(message.composer.binary) === "file" ? "File" : "Binary value"}</span><div class="realtime-file-control"><VariableField value={binaryValue(message.composer.binary)} {variables} className="text-input" disabled={binarySource(message.composer.binary) === "file"} placeholder={binarySource(message.composer.binary) === "file" ? "Choose a local file" : ""} onValueInput={(value) => patchSocketIoComposer({ binary: buildBinary(binarySource(message.composer.binary), value) })} />{#if binarySource(message.composer.binary) === "file"}<button class="button-secondary" type="button" onclick={pickBinaryFile} disabled={isPickingBinaryFile}>{isPickingBinaryFile ? "Choosing…" : "Choose file…"}</button>{/if}</div></label>
        {:else}
          <label class="realtime-wide-field">
            <span class="field-label">Arguments (JSON array)</span>
            <JsonEditor
              value={argumentsText}
              {variables}
              className="body-textarea realtime-message-input"
              ariaLabel="Arguments (JSON array)"
              ariaInvalid={Boolean(argumentsError)}
              onValueInput={(value) => setSocketIoJson("arguments", value)}
            />
            {#if argumentsError}<span class="field-error" role="alert">{argumentsError}</span>{/if}
          </label>
        {/if}
        <label class="settings-checkbox-line"><input class="row-toggle settings-checkbox" type="checkbox" checked={message.composer.waitForAck} onchange={(event) => patchSocketIoComposer({ waitForAck: event.currentTarget.checked })} /><span><strong>Wait for acknowledgement</strong><small>Show the server ACK or timeout.</small></span></label>
        <label><span class="field-label">ACK timeout (ms)</span><input class="text-input" type="number" min="100" max="60000" value={message.composer.ackTimeoutMs} disabled={!message.composer.waitForAck} oninput={(event) => patchSocketIoComposer({ ackTimeoutMs: event.currentTarget.valueAsNumber || 5000 })} /></label>
      </div>
    {/if}
    {#if composerError}<p class="feedback feedback-error" role="alert">{composerError}</p>{/if}
  </section>
</section>
