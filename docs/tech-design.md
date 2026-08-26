# PostNot Technical Design

## 1. Product Goal

PostNot is a local-first desktop API client for users who want the useful parts of Postman without cloud accounts, collaboration features, or webserver dependence.

Core principles:

- Native desktop app
- Fully usable offline
- Small memory footprint and fast startup
- Local storage owned by the user
- Predictable import/export via files

## 2. Chosen Stack

- Core runtime: Rust
- Desktop shell: Tauri v2
- Frontend: SvelteKit in SPA mode
- UI language: TypeScript
- Local database: SQLite
- HTTP client: `reqwest`
- Raw WebSocket client: `tokio-tungstenite`
- Socket.IO client: pinned `rust_socketio` 0.6 adapter
- Async runtime: `tokio`
- Serialization: `serde`
- SQL layer: `sqlx`

Why this stack:

- Rust + Tauri keeps the app lighter than Electron-based alternatives
- SvelteKit gives a productive UI layer without browser-side request constraints
- SQLite gives durable local persistence without introducing a separate service
- `reqwest` keeps request execution in the native layer, which avoids browser CORS behavior and makes TLS and redirect settings controllable

`rust_socketio` is pinned to 0.6.0 and vendored under `src-tauri/vendor/rust_socketio-0.6.0` with two targeted corrections documented in [`POSTNOT_PATCH.md`](../src-tauri/vendor/rust_socketio-0.6.0/POSTNOT_PATCH.md). First, an outgoing binary event that requests an ACK remains a `BinaryEvent` packet carrying an ACK id instead of being encoded as a client-originated `BinaryAck`. Second, the asynchronous client exposes terminal callbacks for reconnect exhaustion and a closed packet stream without reconnect; upstream otherwise only logs exhaustion or leaves consumers without a terminal signal. PostNot uses those callbacks to move dead sessions to Failed or Disconnected and release their runtime task. Remove the vendor override only after an upstream release contains both behaviors and the pinned Socket.IO 4.8.1 fixture still passes Auto, WebSocket-only, JSON/binary ACK, successful reconnect, reconnect exhaustion, and no-reconnect transport-loss cases.

## 3. High-Level Architecture

The app is split into two layers.

### Frontend

Responsibilities:

- Render request editor (including script editors), response viewer, settings page, and history panel
- Manage page-level UI state
- Persist and restore request-tab workspace state, including active tab selection and per-tab drafts
- Render global floating notifications for cross-screen action feedback
- Provide Requests header datalist suggestions from common HTTP header names, existing header names in the draft, name-aware common values, and matching values already used for that header name in the same draft
- Open the resolved request preview from the Requests send controls and render the native preview result as read-only outgoing URL, query, header, auth, body, settings, warning, and note sections
- Coordinate shared pointer-driven collection drag-and-drop interactions for saved requests and folders across the sidebar and Collections page
- Provide sidebar collection search that navigates directly to matching collections, folders, or saved requests
- Render and execute playbooks in the frontend so sequential runs reuse worker-backed request scripting, active environment writes, cancellation, notifications, and normal request-history behavior
- Keep URL query parameters for saved requests, collections, and environments in sync with the visible editor or browser, including canceling stale async work when the user navigates quickly
- Persist and restore the dedicated WebSockets tab workspace while deliberately restoring every tab disconnected and without a transcript
- Keep live realtime sessions available while the user navigates between routes, reconcile ordered native events by connection generation and sequence, and request a native snapshot when an event gap is detected
- Render raw WebSocket and Socket.IO definitions through one protocol-aware editor, while routing saved definitions from collection trees to `/websockets`
- Run inherited collection, folder, and saved-request pre-request and test scripts in a worker-backed JavaScript sandbox before and after invoking `send_request`
- Invoke typed Tauri commands for persistence and request execution
- Provide a desktop-oriented workflow without browser networking

The frontend does not execute HTTP requests directly. All network traffic goes through Rust.

### Native Layer

Responsibilities:

- Initialize SQLite database and run migrations
- Execute HTTP requests
- Load and persist settings
- Persist request history
- Persist request workspace state through `app_settings`
- Load environment metadata from SQLite while storing secret environment values in the OS credential store
- Build one canonical prepared-request model for both sends and resolved previews after environment and dynamic-variable resolution, then mask credential-looking values before preview data returns to the UI
- Coordinate signed release checks against GitHub Releases' stable `latest` updater manifest, Linux installer-target selection, download progress events, retryable failure handling, and install handoff for the Settings updater flow
- Resolve app data paths
- Own application-wide realtime connection state, bounded session transcripts, and temporary file-backed payload handles
- Expose a stable Tauri command surface to the UI

### Data Flow

1. User edits a request in the UI
2. The frontend keeps that draft inside the active request tab and persists workspace changes locally through the settings-backed workspace store
3. Before sending, the user may open a read-only resolved request preview; the frontend invokes the native `preview_request` command, which resolves the active environment and settings, prepares the same normalized URL, enabled query rows, generated auth/content headers, and body mode used by send, then masks credential-looking values and returns warnings and notes without executing scripts, helper requests, environment writes, or network traffic
4. On send, the frontend runs inherited collection, folder, and saved-request pre-request scripts (if any) against a draft copy and either stops with a script error surface or proceeds with the mutated draft as the payload
5. Frontend invokes `send_request` with that payload
6. Rust loads persisted request settings from SQLite
7. Rust resolves environment variables and built-in dynamic variables
8. Rust creates a request guard, prepares the canonical outgoing request, and executes it with `reqwest`; dropping the guard releases only that matching active-request slot on success, failure, or cancellation
9. Rust returns response metadata plus either an inline body (up to 1 MiB) or a managed file handle; large bodies never cross Tauri IPC in full
10. Rust copies the downloaded response into its history path, inserts the history row, adopts the copied file only after the insert succeeds, and redacts secret-derived environment substitutions back to their original `{{variable}}` form
11. Frontend runs inherited collection, folder, and saved-request test scripts (if any) against the returned response for assertion output
12. Frontend reloads history, updates the originating tab, and persists the refreshed workspace state

### Realtime Data Flow

1. The user opens or creates a raw WebSocket or Socket.IO definition in the dedicated `/websockets` workspace
2. The frontend persists open tab definitions and active-tab selection through `realtime_workspace_state`; connection status, generation counters, errors, and transcripts are stripped to a disconnected empty state on every persistence boundary
3. On Connect, Rust resolves the active environment and built-in dynamic variables across the URL, enabled query and header rows, authentication, protocol fields, and the saved composer snapshot
4. The application-wide connection manager creates a new generation for that tab ID, applies the persisted connect timeout, TLS policy, concurrent-session limit, and message-size limit, and owns the transport task independently of the current route
5. The manager publishes status and transcript updates over a Tauri `Channel`; every update carries `connectionId`, `generation`, and a monotonically increasing `sequence`
6. The frontend rejects stale generations and requests `get_realtime_session_snapshot` if it detects a sequence gap
7. Outgoing composer values are resolved again when Send is invoked, so an active environment change affects the next message; changing connection fields or the environment while connected marks the tab as requiring a reconnect
8. The native manager records sent, received, lifecycle, ping/pong, ACK, and error entries in a bounded session-only transcript. Payloads over 256 KiB are stored in temporary app-data files and cross IPC as a handle plus a 4 KiB preview
9. Closing a tab disconnects and releases its native session and temporary payload handles. Restart restores its editable definition but never reconnects or restores its transcript

Realtime definitions do not enter HTTP history and do not run collection, folder, or saved-request pre-request/test scripts in v1.

### Playbook Data Flow

1. User creates a playbook on `/playbooks` and adds existing saved requests as ordered live references
2. The frontend loads each enabled step immediately before execution through `get_playbook_execution_context`
3. The context returns the latest saved request plus inherited collection and folder scripts
4. The frontend runs pre-request scripts, sends through the existing `send_request` command, then runs test scripts
5. Each step still writes normal request history through the existing send path
6. The frontend records grouped playbook run and step summaries in SQLite
7. Stop-on-failure and non-2xx/3xx failure policy are enforced by the playbook runner, with remaining enabled steps recorded as skipped

## 4. Repository Structure

This is the meaningful structure for the application and documentation code.

```text
PostNot/
  docs/
    CNAME
    index.html
    scripting.html
    site.css
    tech-design.md
    images/
      collections-page.webp
      environments-page.webp
      playbooks-page.webp
      request-preview.webp
      requests-page.webp
      settings-page.webp
  src/
    app.html
    app.d.ts
    hooks.client.ts
    lib/
      api/
        commands.ts
        realtime.ts
        types.ts
      components/
        collections/
          CollectionDetailForm.svelte
          CollectionsPanel.svelte
          FolderScriptForm.svelte
        icons/
          FolderGlyph.svelte
        history/
          HistoryDetail.svelte
          HistoryPanel.svelte
        layout/
          AppShell.svelte
          CollectionDragController.svelte
          DialogShell.svelte
          NotificationHost.svelte
          SidebarCollections.svelte
        realtime/
          RealtimeEditor.svelte
          RealtimeKeyValueEditor.svelte
          RealtimeTabs.svelte
          RealtimeTranscript.svelte
        request/
          RequestEditor.svelte
          RequestTabs.svelte
          ScriptEditor.svelte
          VariableField.svelte
        response/
          JsonViewer.svelte
          ResponseViewer.svelte
      icons/
        folderPaths.ts
      collections/
        drag-and-drop.ts
      request-scripts.ts
      request-script-worker.ts
      realtime-workspace.ts
      stores/
        collection-dnd.svelte.ts
        collection-search.svelte.ts
        collections.svelte.ts
        notifications.svelte.ts
        request-workspace.svelte.ts
        realtime-workspace.svelte.ts
        updater.svelte.ts
      async-stale-guard.ts
      modal-focus-trap.ts
      theme.ts
      ui-cache.ts
      styles/
        tokens.css
        app.css
    routes/
      +layout.svelte
      +layout.ts
      +page.svelte
      collections/
        +page.svelte
      environments/
        +page.svelte
      playbooks/
        +page.svelte
      settings/
        +page.svelte
      websockets/
        +page.svelte
  src-tauri/
    Cargo.toml
    tauri.conf.json
    build.rs
    capabilities/
      default.json
    vendor/
      rust_socketio-0.6.0/
    icons/
      icon.png
    migrations/
      0001_init.sql
      0002_collection_scripts.sql
      0003_playbooks.sql
      0004_collection_search_fts.sql
      0005_response_body_metadata.sql
      0006_collection_integrity.sql
      0007_playbook_integrity.sql
      0008_environment_integrity.sql
      0009_agent_activity.sql
      0010_realtime_requests.sql
      0011_realtime_connections_and_messages.sql
    src/
      main.rs
      lib.rs
      app_state.rs
      error.rs
      commands/
        mod.rs
        collections.rs
        environments.rs
        imports.rs
        playbooks.rs
        realtime.rs
        requests.rs
        settings.rs
        history.rs
        updates.rs
      db/
        mod.rs
      domain/
        collections.rs
        environments.rs
        exports.rs
        history.rs
        imports.rs
        playbooks.rs
        portability.rs
        mod.rs
        realtime.rs
        requests.rs
        settings.rs
        updates.rs
        workspace.rs
      services/
        collections_service.rs
        environments_service.rs
        environments_service_tests.rs
        exports_service.rs
        history_service.rs
        mod.rs
        http_client.rs
        imports_service.rs
        imports_service/
          curl.rs
          openapi.rs
          postman.rs
          postnot.rs
          shared.rs
        playbooks_service.rs
        playbooks_service_tests.rs
        request_plan_service.rs
        request_preview_service.rs
        realtime_payload_service.rs
        realtime_connections_service.rs
        realtime_resolution_service.rs
        realtime_service.rs
        realtime_socketio_service.rs
        response_body_service.rs
        response_body_service_tests.rs
        secret_store_service.rs
        settings_service.rs
        updates_service.rs
        window_state_service.rs
      storage/
        mod.rs
        paths.rs
    tests/
      fixtures/
        socketio-server.mjs
  build/
    .gitkeep
  static/
  package.json
  svelte.config.js
  tsconfig.json
  vite.config.ts
```

## 5. Core Domain Model

The application centers on these persisted and transport entities.

### Send Request Payload

Represents the editable request state sent from the frontend to Rust.

Fields:

- name
- method
- url
- query params
- headers
- body
- auth

### Response Payload

Represents a completed request result returned from Rust to the frontend.

Fields:

- status code
- status text
- duration
- size
- headers
- body text
- error text
- executed at timestamp

### Realtime Connections and Messages

Realtime persistence is split into two independently selected resources. Global connection profiles use a versioned `RealtimeConnectionDraft`; collection items use a distinct versioned `RealtimeMessageDraft`. Both carry a `protocol` discriminator of `websocket` or `socketio`, but messages never store a profile reference.

Connection profile fields:

- name and connection URL
- enabled query parameters and handshake headers
- Basic, Bearer, or API-key authentication
- opt-in reconnect policy with attempt and delay bounds

Raw WebSocket fields:

- requested subprotocols
- requested subprotocols

Raw WebSocket message fields:

- message name and one composer in text, JSON, or binary mode
- binary source as a local file, hexadecimal text, or base64 text

Socket.IO fields:

- connection profiles store the Engine.IO path, namespace, auth JSON object, and transport selection
- messages store an event composer with a JSON argument array or one binary payload
- optional client-requested ACK and timeout

The serialized object carries `version: 1`, allowing later readers to reject unsupported schema revisions rather than guessing how to interpret them.

### App Settings

Represents persisted request behavior settings.

Fields:

- theme
- interface zoom
- request timeout in milliseconds
- follow redirects flag
- validate TLS flag
- history limit
- optional history age limit in days
- optional history response-body storage limit in bytes
- Requests-page history collapsed flag
- environment autosave flag
- notification timeout in milliseconds
- realtime connect timeout in milliseconds
- maximum concurrent realtime sessions
- maximum realtime message size
- per-session transcript entry limit
- per-session transcript byte limit
- last successful update check timestamp

### History Entry Summary

Represents a persisted request execution summary shown in the UI.

Fields:

- id
- request name
- method
- url
- status code
- duration
- response preview
- error text
- executed at timestamp

## 6. SQLite Storage Design

The schema is created by forward-only migrations in `src-tauri/migrations/`. Migrations `0001` through `0003` define the original tables, collection scripts, and playbooks; `0005` adds response-body metadata. Migration `0006` removes the superseded collection FTS shadow table and adds sibling-ordering support, `0007` adds the folder-chain index used by playbooks, `0008` repairs competing active environments before enforcing the one-active-environment invariant, and `0009` adds MCP Agent Activity. Migration `0010` added the original combined realtime definition. Migration `0011` creates standalone connection profiles and directly renames `realtime_request_json` to `realtime_message_json`. A transactional, shape-based Rust upgrader then splits legacy JSON and can safely run again until no combined records remain. Released migrations remain unchanged so existing databases upgrade safely.

### Database Location

The database is created under the Tauri app data directory:

- database file: `<app_data_dir>/postnot.sqlite`

### Core Tables

#### `app_settings`

Stores key/value application preferences and serialized UI state.

```sql
CREATE TABLE app_settings (
  key TEXT PRIMARY KEY,
  value_json TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
```

Keys written by the app:

- `theme`
- `ui_scale`
- `request_timeout_ms`
- `follow_redirects`
- `validate_tls`
- `history_limit`
- `history_retention_days`
- `history_storage_limit_bytes`
- `is_history_collapsed`
- `environment_autosave`
- `notification_timeout_ms`
- `realtime_connect_timeout_ms`
- `realtime_max_concurrent_sessions`
- `realtime_max_message_bytes`
- `realtime_transcript_max_entries`
- `realtime_transcript_max_bytes`
- `last_update_checked_at`
- `collection_sidebar_state`
- `request_workspace_state`
- `realtime_workspace_state`

Settings reads load the stored rows once and merge them over Rust defaults without rewriting defaults on the read path. A full settings save upserts all normalized values in one transaction. History pruning reads only the count, age, and response-body byte retention keys; age and byte limits default to disabled so existing installations keep their previous behavior.

`realtime_workspace_state` stores each tab's independent selected profile/message IDs, connection/message drafts and baselines, plus active-tab selection. It does not store a durable session. The native save boundary normalizes status to disconnected, clears generation and sequence state, and removes transcript and error content. This guarantees that application restart never reconnects implicitly or resurrects received data.

#### `history_entries`

Stores request execution summaries and links to full response-body files.

```sql
CREATE TABLE history_entries (
  id TEXT PRIMARY KEY,
  request_name TEXT NOT NULL DEFAULT '',
  method TEXT NOT NULL,
  url TEXT NOT NULL,
  request_snapshot_json TEXT NOT NULL,
  status_code INTEGER NULL,
  duration_ms INTEGER NOT NULL,
  response_headers_json TEXT NOT NULL DEFAULT '[]',
  response_body_path TEXT NULL,
  response_body_preview TEXT NOT NULL DEFAULT '',
  error_text TEXT NOT NULL DEFAULT '',
  executed_at TEXT NOT NULL
);
```

Implementation notes:

- successful requests are persisted with a response preview
- successful responses persist exact response bytes to a file path referenced by `response_body_path`; file-backed responses use copy-insert-adopt ordering so a failed insert removes the staged history copy without invalidating the live response handle
- file-backed bodies include content type, charset, presentation, actual byte size, and a sparse row index for bounded visible-range reads
- failed requests are also persisted with `error_text`
- history retention applies the persisted entry-count, optional age, and optional response-body byte limits together
- clear and prune commit row deletion in a transaction before scheduling the corresponding files for lease-aware removal

### Other Tables

#### `collections`

Stores saved request collections.

#### `collection_items`

Stores both saved requests and folders within a collection tree.

Implementation notes:

- `kind` distinguishes folders from saved requests
- `request_type` distinguishes `http`, `websocket`, and `socketio` while preserving the existing `folder`/`request` tree shape
- HTTP rows keep using the normalized HTTP columns; realtime message rows keep a versioned message-only wrapper in `realtime_message_json`, leave HTTP URL/method/body fields empty, and retain the item name for tree/search projection
- HTTP service queries explicitly filter `request_type = 'http'`, while realtime CRUD accepts only the two realtime discriminators and uses revision-checked updates
- `parent_id` allows nested folders and request placement inside folders
- `prerequest_script` and `test_script` are persisted per collection, folder, and saved request; the UI runs inherited collection scripts first, then ancestor folder scripts from root to leaf, then saved-request scripts in the frontend (`request-scripts.ts`) before invoking Rust for send (pre-request) and after the response returns (tests), not inside the native HTTP layer
- collection search loads current collection and item rows directly, builds ancestor paths from that snapshot, classifies and ranks matches in memory, and applies the result limit; committed renames and moves therefore require no shadow-index rebuild
- collection trees and search results project protocol badges and route HTTP items to `/`, but route WebSocket and Socket.IO items to `/websockets`

#### `realtime_connections`

Stores global connection profiles with `id`, display `name`, `protocol`, versioned `config_json`, and timestamps. Profiles are selected independently from collection messages. Exact legacy connection matches reuse a single profile; deleting a profile never mutates collection messages.

#### `environments`

Stores environment metadata, active-state, and non-secret variable definitions. Secret values are kept in the OS credential store. Activation validates the target, clears the previous active row, and sets the new row in one immediate transaction; a partial unique index enforces at most one `is_active = 1` row even across competing connections.

#### `playbooks`

Stores Playbook metadata, default delay, stop-on-failure policy, and HTTP error failure policy.

#### `playbook_steps`

Stores ordered saved-request references for a Playbook, including per-step enabled state, optional name/notes, and optional delay override.

#### `playbook_runs` and `playbook_run_steps`

Store grouped Playbook execution summaries and per-step outcomes. Individual step sends still go through the normal request execution path and write normal request history entries. Duplicate, reorder, delete-and-renumber, and run-step/counter updates commit as logical transactions, while inherited folder scripts are loaded with one recursive parent-chain query.

## 7. Runtime Behavior

### Startup

At startup, the Tauri app:

1. resolves the app data directory
2. creates the SQLite database if missing
3. applies SQL migrations
4. ensures default settings exist
5. initializes the OS-backed secret store
6. clears the previous process's temporary realtime payload directory and initializes the application-wide realtime connection manager
7. stores the SQLite pool, secret store, and realtime manager in app state
8. restores and tracks the main window size and position

### Request Execution

For each request send, Rust applies these persisted settings:

- `request_timeout_ms`
- `follow_redirects`
- `validate_tls`

This means the settings page already changes actual network behavior, not just UI state.

Rust builds or reuses a `reqwest::Client` for the active combination of `validate_tls`, `follow_redirects`, and `request_timeout_ms` (cached up to a fixed number of distinct fingerprints) instead of constructing a new client on every request.

`request_plan_service::prepare_request` is the single assembly boundary for send and preview. It normalizes bare localhost URLs, filters disabled query/header/body rows, applies generated authentication and content headers with deterministic precedence, parses the public string modes into private enums, and rejects unknown modes. The sender alone opens multipart files; preview never touches the filesystem.

For each saved request send, the frontend may first run the collection pre-request script, then each ancestor folder pre-request script from root to leaf, and then the saved request's pre-request script against a draft copy (with the active environment's variables) to mutate headers, query params, URL, and related fields. Those scripts can also await helper HTTP calls through `pn.http.send(...)` and persist active-environment variable changes before the main request continues. Errors from that step surface in the UI without calling Rust.

Helper HTTP calls are guarded by the script runtime: only one `pn.http.send(...)` helper request may be active at a time, and helper calls must be awaited before a script source finishes. This keeps scripts aligned with the native single-request boundary and prevents the main request from racing an unfinished helper request.

For each request send, Rust then:

- loads the currently active environment, if one exists
- resolves `{{variable}}` placeholders in URL, query params, headers, body text, form fields, and auth values
- expands built-in dynamic variables such as `$guid`, `$timestamp`, and related runtime helpers
- prepares and sends the resolved request payload

`AppState::start_request` returns an owned `RequestGuard` and cancellation receiver. The guard's `Drop` implementation clears only its matching request ID, so early returns cannot strand the active slot and an older guard cannot clear a newer request.

After Rust returns a response (or error), the frontend may run the collection test script, ancestor folder test scripts from root to leaf, and then the saved test script, recording assertion results for display in the response panel.

### Resolved Request Preview

The Requests page can call `preview_request` before send. The command loads persisted settings, resolves the active environment and built-in dynamic variables, and passes both the original and resolved request through `request_preview_service`, which consumes the same canonical prepared request as the native sender.

The preview response is intentionally read-only. It does not execute pre-request scripts, helper HTTP calls, active-environment writes, or the main network request. Canonical preparation rejects an invalid method or URL and unknown auth, API-key-location, or body modes with the same error used by send. A successful preview shows the final URL with enabled query parameters, auth-generated and body-generated headers, resolved auth/body data, active request settings, non-fatal warnings for invalid headers or JSON, missing multipart files, missing OAuth tokens, and unresolved variables, plus notes about generated transport headers and sampled dynamic variables. Secret-derived values and credential-looking keys are masked before they reach the UI.

### Updater

The updater uses Tauri's signed updater plugin with a bundled public key and the stable GitHub Releases endpoint at `https://github.com/tansdf/PostNot/releases/latest/download/latest.json`. The frontend runs a silent startup check when Tauri is available and exposes manual checks from Settings.

On Linux, update checks request a target matching the detected install type (`deb`, `rpm`, or `appimage`) and architecture. Debian and RPM installs download the package, verify the expected package magic bytes, hand installation to `pkexec`, and time out instead of waiting indefinitely for a missing PolicyKit prompt. Other targets use the plugin `download_and_install` path. Download progress is emitted as `update-download-progress` and surfaced in Settings; failed downloads or installer handoffs leave the pending update available for retry.

### History Persistence

On successful request execution:

- the request snapshot is stored
- response summary metadata is stored
- response preview text is stored
- full response bodies are stored on disk for virtualized detail inspection, whole-document search, formatting, binary hex display, image preview, and streamed Save as
- active tabs and history details lease body handles so pruning or clearing history cannot invalidate an open response
- responses at or below 1 MiB remain inline; larger responses stream to disk and expose only bounded windows to the WebView
- history is pruned against the configured entry-count, optional age, and optional response-body byte limits together

History recording never moves the live response before persistence succeeds. It copies file-backed responses to the final history path, inserts the database row, then marks the copied file history-owned; insertion failure removes the copy and its sidecar while the live handle remains readable. Clear and prune first commit database deletion, then remove committed paths only when active leases permit it.

On failed request execution:

- the request snapshot is stored
- error text is stored
- history is pruned against the configured entry-count, optional age, and optional response-body byte limits together

On canceled request execution:

- the in-flight native request is aborted
- no history entry is written

### Realtime Sessions

Realtime connections are app-wide native resources keyed by the frontend tab ID. A route change does not destroy a connection; closing its tab calls both disconnect and release. Reconnecting the same ID increments its generation and disconnects the superseded task so late events from an older transport cannot mutate the current tab.

Raw WebSocket execution uses `tokio-tungstenite` and supports:

- `ws://` and `wss://`
- enabled query parameters and custom handshake headers
- Basic, Bearer, and API-key header or query authentication
- a standard `Cookie` header
- ordered requested subprotocols
- text, validated JSON text, binary file, hexadecimal, and base64 sends
- received text/JSON classification, binary frames, ping, pong, and close lifecycle entries
- explicit Ping and graceful Close controls

Socket.IO execution uses the pinned `rust_socketio` 0.6 adapter for protocol v5 / Engine.IO v4 servers (Socket.IO 3.x and 4.x). The adapter accepts HTTP(S) and WS(S) connection URLs, a configurable Engine.IO path, one namespace, enabled query and opening-header rows, Basic/Bearer/API-key authentication, an auth JSON object, and the shared TLS validation policy. Auto transport begins with HTTP polling and permits WebSocket upgrade; WebSocket-only skips polling. The composer emits an event with either a JSON argument array or one binary payload and may request a client-side ACK with a bounded timeout. Incoming events and ACKs enter the same sequenced transcript, while Engine.IO ping/pong remains library-managed.

The connection manager applies:

- connect timeout: default 30 seconds, normalized to 1–120 seconds
- concurrent live sessions: default 20, normalized to 1–100
- message size: default 64 MiB, normalized to 64 KiB–256 MiB
- transcript entries: default 2,000, normalized to 1–10,000 per session
- transcript retained bytes: default 64 MiB, normalized to 64 KiB–512 MiB per session
- the shared `validate_tls` setting for secure transports

Reconnect is opt-in and defaults to five attempts with a 500 ms initial delay and 10 second maximum delay. The manager uses capped exponential backoff with jitter only after connection/network/abnormal transport failures. Manual disconnect, graceful close, and a server close frame stop the session. Commands sent during reconnect are not queued. The retry loop uses the resolved definition snapshot from the last explicit Connect; edits or active-environment changes are applied only after the user reconnects.

The native transcript is an ordered, bounded `VecDeque`, not SQLite history. When an entry or byte bound is crossed, old entries and their payload handles are released and a visible trim marker is inserted. Text and binary payloads at or below 256 KiB remain inline; larger payloads are written under the process-scoped realtime payload directory and represented over IPC by an opaque handle, a 4 KiB preview, size, encoding, and truncation flag. The UI can copy bounded payloads, save a complete file-backed payload, clear the transcript, or export a session transcript. Restart clears the temporary directory.

All `Channel` status and transcript events carry a `sessionId`, connection generation, and monotonic sequence. One native transport task belongs to each workspace tab. Only explicit Connect/Reconnect replaces it and advances its generation; selecting or editing messages does not reconnect or clear its transcript. The runtime records the connected protocol and rejects incompatible messages before enqueueing them. A transcript trim or clear emits a reset event; the frontend requests a complete session snapshot if it observes an event gap.

### Realtime v1 Boundaries

The first realtime release intentionally excludes:

- collection/folder/saved-request pre-request and test scripts for realtime definitions
- Playbook steps for realtime definitions
- durable or cross-session WebSocket/Socket.IO history
- legacy Socket.IO 2.x / Engine.IO 3
- custom CA bundles, mutual TLS, and explicit proxy configuration
- WebSocket `permessage-deflate`
- replies to server-requested Socket.IO ACKs
- multiple or mixed JSON/binary Socket.IO placeholder arguments
- STOMP, GraphQL subscription, WebTransport, and other protocol adapters

These are product boundaries, not fields silently ignored by the runtime. Unsupported handshake headers, URL schemes, payload structures, and protocol combinations are rejected with actionable errors.

## 8. Tauri Command Boundary

Commands exposed to the frontend:

- `send_request`
- `preview_request`
- `cancel_active_request`
- `pick_multipart_files`
- `get_settings`
- `update_settings`
- `get_request_workspace_state`
- `save_request_workspace_state`
- `get_realtime_workspace_state`
- `save_realtime_workspace_state`
- `connect_realtime_connection`
- `disconnect_realtime_connection`
- `release_realtime_connection`
- `send_realtime_message`
- `ping_realtime_connection`
- `close_realtime_connection`
- `get_realtime_session_snapshot`
- `clear_realtime_transcript`
- `read_realtime_payload`
- `save_realtime_payload`
- `export_realtime_transcript`
- `list_realtime_connection_profiles`
- `get_realtime_connection_profile`
- `create_realtime_connection_profile`
- `update_realtime_connection_profile`
- `delete_realtime_connection_profile`
- `import_realtime_connection_profiles`
- `export_realtime_connection_profiles`
- `check_for_updates`
- `install_update`
- `list_history`
- `get_history_entry`
- `clear_history`
- `apply_history_retention`
- `get_storage_summary`
- `export_portable_workspace`
- `inspect_portable_workspace`
- `import_portable_workspace`
- `list_collections`
- `search_collection_entities`
- `get_collection_sidebar_state`
- `save_collection_sidebar_state`
- `create_collection`
- `list_collection_items`
- `create_collection_folder`
- `update_collection_folder`
- `move_collection_item`
- `update_collection`
- `delete_collection`
- `list_saved_requests`
- `save_request_to_collection`
- `update_saved_request`
- `get_saved_request`
- `list_saved_realtime_messages`
- `save_realtime_message_to_collection`
- `update_saved_realtime_message`
- `get_saved_realtime_message`
- `delete_collection_item`
- `delete_saved_request`
- `delete_saved_realtime_message`
- `export_collection`
- `list_playbooks`
- `create_playbook`
- `get_playbook`
- `update_playbook`
- `duplicate_playbook`
- `delete_playbook`
- `add_playbook_step`
- `update_playbook_step`
- `reorder_playbook_steps`
- `delete_playbook_step`
- `get_playbook_execution_context`
- `create_playbook_run`
- `finish_playbook_run`
- `record_playbook_run_step`
- `list_playbook_runs`
- `get_playbook_run`
- `list_environments`
- `create_environment`
- `get_environment`
- `update_environment`
- `delete_environment`
- `set_active_environment`
- `import_postman_environment`
- `export_environment`
- `import_requests`
- `import_curl_request_to_draft`
- `import_openapi_request_to_draft`

### Command Roles

- `send_request`: executes the request using persisted settings and records history
- `preview_request`: resolves the current draft against the active environment and persisted settings, then returns a read-only masked outgoing request preview without executing scripts, helper requests, environment writes, or network traffic
- `cancel_active_request`: aborts the currently active native request, if one exists
- `pick_multipart_files`: opens a native file picker and returns selected local file paths for multipart requests
- `get_settings`: loads current settings from SQLite
- `update_settings`: persists settings and returns the saved values
- `get_request_workspace_state`: loads the persisted Requests tab workspace snapshot from `app_settings`
- `save_request_workspace_state`: stores the Requests tab workspace snapshot in `app_settings`
- `get_realtime_workspace_state`: loads the normalized WebSockets tab workspace snapshot from `app_settings`
- `save_realtime_workspace_state`: stores editable WebSockets tabs after forcing session state and transcripts to a disconnected empty form
- `connect_realtime_connection`: resolves an immutable connection snapshot against the active environment, applies persisted limits, subscribes a Tauri event channel, and starts or replaces the tab's native session generation
- `disconnect_realtime_connection`: manually stops a native realtime session without scheduling reconnect
- `release_realtime_connection`: removes a session from the manager and releases its transcript payload handles
- `send_realtime_message`: resolves the current composer against the active environment and sends one protocol-matched message without queueing
- `ping_realtime_connection`: sends a raw WebSocket Ping frame with an optional payload
- `close_realtime_connection`: validates and sends a raw WebSocket graceful close code and reason
- `get_realtime_session_snapshot`: returns the authoritative status, generation, sequence, transcript, and retained byte count for event-gap recovery
- `clear_realtime_transcript`: clears a session transcript, releases payload handles, and emits a transcript reset
- `read_realtime_payload`: reads a bounded temporary payload by opaque handle for expanded UI display
- `save_realtime_payload`: copies a complete temporary payload to a user-selected path
- `export_realtime_transcript`: writes the current session transcript to a user-selected JSON file
- `check_for_updates`: checks the configured signed updater feed for a newer stable GitHub Release and stores a pending update when available
- `install_update`: downloads the pending signed update with progress events and hands it off to the native installer, using the detected Debian/RPM/AppImage install type on Linux
- `list_history`: returns recent history entries ordered by execution time descending
- `get_history_entry`: returns a stored request snapshot and response metadata for one history entry
- `clear_history`: deletes all stored history entries
- `apply_history_retention`: immediately applies count, age, and on-disk response-body limits, committing row deletion before releasing body files
- `get_storage_summary`: reports the app-data directory, database and managed-file sizes, and durable entity counts without exposing record contents
- `export_portable_workspace`: builds a versioned, redacted authoring-data document and writes it through a native save dialog
- `inspect_portable_workspace`: parses and validates schema version, IDs, hierarchy, references, protocols, and secret invariants without mutating local state
- `import_portable_workspace`: additively inserts a validated document in one immediate transaction, remaps internal references, reuses exact realtime-profile matches, and creates blank local secret placeholders
- `list_collections`: returns saved request collections with request counts and collection-level scripts
- `search_collection_entities`: searches collections, folders, and saved requests for sidebar quick navigation
- `get_collection_sidebar_state`: loads persisted sidebar expansion state for collections and folders
- `save_collection_sidebar_state`: persists sidebar expansion state for collections and folders
- `create_collection`: creates a new collection for saved requests, including empty or provided collection-level scripts
- `list_collection_items`: returns the nested folder and request tree for one collection
- `create_collection_folder`: creates a folder at the collection root or inside another folder
- `update_collection_folder`: updates one folder's name and inherited scripts
- `move_collection_item`: reorders or relocates a saved request within the collection tree, including moves across folders and collections
- `update_collection`: updates one collection's name, description, and collection-level scripts
- `delete_collection`: removes a collection and its saved requests
- `list_saved_requests`: lists saved requests within one collection
- `save_request_to_collection`: stores the current request draft in a collection
- `update_saved_request`: updates an existing saved request in place
- `get_saved_request`: loads one saved request back into the editor
- `list_saved_realtime_messages`: lists WebSocket and Socket.IO messages within one collection
- `save_realtime_message_to_collection`: validates and stores a versioned message-only definition in a collection or folder
- `update_saved_realtime_message`: revision-checks and replaces a realtime message in place
- `get_saved_realtime_message`: loads one message into the WebSockets message block without changing the connection/session
- `delete_collection_item`: removes a folder or saved request item from a collection tree
- `delete_saved_request`: removes one saved request from a collection
- `delete_saved_realtime_message`: removes one realtime message from a collection
- `export_collection`: exports a lossless mixed PostNot collection or an HTTP-only Postman Collection v2.1 file through a native save dialog; Postman results report omitted realtime definitions
- `list_playbooks`: returns Playbook summaries
- `create_playbook`: creates a Playbook
- `get_playbook`: returns one Playbook with its steps
- `update_playbook`: updates Playbook metadata and execution policy
- `duplicate_playbook`: copies a Playbook and its steps
- `delete_playbook`: removes a Playbook and run history
- `add_playbook_step`: appends a saved-request step to a Playbook
- `update_playbook_step`: updates step metadata, enabled state, delay, or saved-request reference
- `reorder_playbook_steps`: persists Playbook step ordering
- `delete_playbook_step`: removes one Playbook step
- `get_playbook_execution_context`: loads the latest saved request and inherited scripts for a step immediately before execution
- `create_playbook_run`: creates a grouped Playbook run record
- `finish_playbook_run`: finalizes a grouped Playbook run summary
- `record_playbook_run_step`: stores one Playbook run step outcome
- `list_playbook_runs`: returns grouped Playbook run summaries
- `get_playbook_run`: returns one grouped Playbook run with step outcomes
- `list_environments`: returns saved environments with active-state and variable counts
- `create_environment`: creates a blank environment draft
- `get_environment`: returns one environment and its variables
- `update_environment`: persists environment name and variables
- `delete_environment`: removes one environment
- `set_active_environment`: marks one environment active or clears the active environment
- `import_postman_environment`: imports a Postman environment JSON file or payload into a new PostNot environment
- `export_environment`: exports one environment to Postman environment JSON through a native save dialog
- `import_requests`: imports PostNot mixed collection JSON, Postman collection JSON, OpenAPI, or cURL into PostNot collections
- `import_curl_request_to_draft`: parses a cURL command into an editable request draft without saving it yet
- `import_openapi_request_to_draft`: parses one OpenAPI operation into an editable request draft without saving it yet

## 9. Frontend Screens

### Main Page

UI responsibilities:

- request profile summary using persisted settings
- active environment selector
- request editor and collection editor with pre-request and test script editors (`ScriptEditor.svelte`)
- header name autocomplete for common HTTP headers plus names already present in the draft
- header value autocomplete based on the current header name, common values, and values already used by matching header rows in the draft
- save flow with collection and folder target selection
- request import modal for cURL and OpenAPI 3 single-request drafts
- request export modal for cURL and PostNot request JSON
- resolved request preview modal opened from a compact icon beside Send, showing native-resolved URL, query params, headers, auth, body, active settings, warnings, and notes while keeping credential-looking values masked
- request-level save/update action
- response viewer
- history panel
- history detail inspector

### Settings Page

UI responsibilities:

- theme selector
- interface zoom selector
- request timeout input
- history count, age, and response-body storage inputs
- portable workspace export, preflight inspection, and additive import controls
- durable/temporary storage ownership summary
- notification timeout input
- follow redirects toggle
- validate TLS toggle
- realtime connect timeout, concurrent-session, message-size, transcript-entry, and transcript-byte limits
- updater status and install surface
- live download progress with byte counts when content length is known
- available-update notes rendered from the signed updater metadata
- persisted save action

### Collections Page

UI responsibilities:

- collection browser with nested folders and saved requests
- dedicated collection editor view (`CollectionDetailForm.svelte` for metadata drafts)
- root-folder and subfolder creation
- collection import/export actions, with lossless mixed PostNot JSON and an explicit realtime-omission warning for HTTP-only Postman export
- selected collection tree for folders and saved requests with vertical tree guides and folder open/closed icons (`FolderGlyph.svelte` + shared SVG paths in `folderPaths.ts`)
- drag-and-drop saved-request and folder management that matches the sidebar tree: reorder among siblings, move into folders, move across collections, and move back to collection root
- matching sidebar tree styling for nested collections (see `SidebarCollections.svelte` and `app.css`)
- `WS` and `S.IO` badges for realtime definitions
- protocol-aware open actions that route HTTP definitions to Requests and realtime definitions to WebSockets
- open and delete actions for saved definitions

### WebSockets Page

UI responsibilities:

- horizontal persistent connection tabs with protocol, dirty, and connection-status indicators
- raw WebSocket and Socket.IO mode selection
- active environment selection plus reconnect-required feedback after connection-affecting edits
- connection URL, enabled query parameters, headers/cookies, authentication, and protocol configuration
- opt-in reconnect controls
- text/JSON/binary raw WebSocket composer with JSON formatting, Ping, and graceful Close controls
- Socket.IO event composer with JSON argument-array or single-binary mode and optional client-requested ACK
- connection status and actionable failure feedback, including background failure notifications that return to the affected tab
- ordered searchable session transcript with follow/pause behavior, copy/expand/save actions, trim markers, clear, and JSON export
- save, update, save-as, external-change, and dirty-close flows shared with collection persistence
- route-addressable opening through `savedRequestId` and `tabId`, with stale generation and sequence-gap handling delegated to the workspace store

### Environments Page

UI responsibilities:

- environment list
- active/inactive environment controls
- environment variable editor
- Postman environment import
- Postman environment export
- variable usage hint for `{{name}}` syntax

### Playbooks Page

UI responsibilities:

- Playbook list and editor
- ordered saved-request steps with enable/disable controls, per-step notes, and delay overrides
- default delay, stop-on-failure, and fail-on-HTTP-error policy controls
- sequential run controls that reuse the normal request send, scripting, environment, and history paths
- grouped run log summaries and per-step outcomes

### Public Landing Site

Static site responsibilities:

- GitHub Pages landing page under `docs/index.html`, served with `docs/CNAME` for `post-not.com`
- product copy that describes the local-first app model, including Playbooks, resolved previews, OAuth2 token fetching, redacted exports, scripting, secrets, and imports
- screenshot gallery using checked-in Dark-theme WebP captures under `docs/images/`
- scripting reference page under `docs/scripting.html`

Screenshot workflow note:

- Run `npm run docs:capture-screenshots` after visible UI releases. The script starts or reuses the Vite dev server, seeds browser-mode local data, forces the Dark theme, captures the public screenshot set with Playwright, converts PNG captures to WebP with `cwebp`, and writes the checked-in assets under `docs/images/`.
- Run `npm run docs:check-screenshots` to verify the checked-in screenshots are fresh enough for release. The guard compares the screenshot manifest against tracked UI inputs and expected asset names; it intentionally avoids pixel diffs so normal rendering differences do not make CI brittle. Screenshot freshness remains a failing local command, but its step is advisory in both the dedicated docs workflow and the tag-triggered release workflow. Documentation structure, metadata, accessibility, and interaction tests remain required GitHub checks.

## 10. Security and Persistence Notes

- the app is fully local
- requests are executed in Rust, not the browser
- secret environment values are stored in the OS credential store, while SQLite keeps only non-secret environment metadata
- history snapshots redact resolved values that came from secret environment variables
- single-request cURL and PostNot JSON exports redact credential-looking literal values, including bearer tokens, OAuth2 access tokens, client secrets, API keys, cookies, and basic-auth passwords; the export dialog can inline active non-secret environment variables, while secret variables remain parameterized or are replaced with `***`
- portable workspace exports clear credential-looking literals, never read secret values from the credential store, carry an explicit redaction list, and omit history, response bodies, transcripts, temporary payloads, playbook runs, Agent Activity, updater state, window state, and incidental UI state
- resolved request preview masks credential-looking values and secret-derived environment substitutions before showing outgoing request data
- realtime connection and composer templates resolve against the active environment natively; secret-derived outgoing transcript payloads, lifecycle labels, errors, transcript exports, and MCP reads are masked, while server-received payloads remain exactly as received
- raw WebSocket callers cannot override transport-owned handshake headers such as `Host`, `Connection`, `Upgrade`, `Sec-WebSocket-Key`, `Sec-WebSocket-Version`, `Sec-WebSocket-Extensions`, or `Sec-WebSocket-Protocol`; explicit authentication/header conflicts are rejected
- Socket.IO applies the same transport-header and authentication-conflict checks, rejects caller-supplied `EIO`, `transport`, and `sid` query keys (including API-key names), and coalesces repeated opening headers without silently dropping values
- realtime transcript payload handles are process-scoped and opaque, live outside SQLite, and are deleted when evicted, cleared, released, or on the next app startup
- decoded response bodies are persisted as full text history body files
- if an environment update or delete fails after partially changing the credential store, rollback of secrets is attempted; failure to roll back is logged with `log::warn` for diagnostics (the primary error still returns to the UI)
- the installed executable can run as a windowless stdio MCP server with `--mcp`; it resolves the same `data_dir/com.postnot.app` database, runs migrations, and uses the existing native collection and preview services
- MCP environment context includes non-secret values but omits secret values; saved credential literals are returned as `***` with explicit preservation paths for revision-checked updates
- MCP exposes separate authoring-only list/get/create/update/delete tools for standalone realtime connections and collection messages. Reads redact credential-looking literals and preserve-path updates use optimistic revisions; MCP never connects, sends, runs scripts, or reads session transcripts
- `agent_activity` retains the latest 1,000 MCP operation records with actor, target, outcome, and changed field names, never request values

Environment-backed secrets are protected in storage and history, while single-request export uses local pattern-based redaction for credential-looking values before users copy cURL or PostNot JSON.

### Collection Portability

PostNot collection JSON v2 is the lossless mixed format. Its versioned document preserves collection/folder hierarchy and scripts, HTTP definitions and their scripts, and message-only WebSocket/Socket.IO entries. Version 1 combined entries remain importable and are split into standalone profiles plus collection messages. Connection profiles also have a separate versioned PostNot document; exports redact literal credentials by default.

Postman Collection v2.1 export remains an HTTP interoperability format. Realtime messages are omitted rather than misrepresented; the export result returns a warning and exact omission count, and the Collections dialog explains the limitation before export.

### Workspace Portability

Portable workspace JSON v1 is a separate authoring-data boundary. It preserves collections/folders, HTTP requests, realtime messages and standalone profiles, scripts, inactive imported environments, playbooks and step references, plus optional request and realtime drafts. Export IDs exist only to preserve internal references; additive import generates new database IDs, remaps parents and playbook/draft links, and reuses a realtime profile only when its redacted configuration is an exact local match.

The native inspector rejects unsupported schemas and versions, duplicate IDs, folder cycles, broken references, protocol/version mismatches, nonblank secret values, and invalid realtime drafts before opening a write transaction. Import never replaces existing data. Secret environment variables retain their key/enabled/secret metadata but receive blank credential-store placeholders, and the result reports which fields require input. The format is deliberately readable and is not a passworded complete backup.

## 11. Design Trade-Offs

### Local-First Storage

SQLite plus OS credential storage keeps the app offline-capable and avoids operating a backend service. The trade-off is that cross-device sync, collaboration, and centralized audit features are outside the core architecture.

If the product grows toward multi-device workflows, the persistence boundary should be revisited before adding sync directly into feature code.

### Native Request Execution

Routing all HTTP traffic through Rust avoids browser CORS limits and keeps TLS, redirect, timeout, cancellation, multipart file access, response decoding, and history persistence under one native pipeline. The trade-off is that browser-mode development needs mocks or degraded behavior for desktop-only capabilities.

The command boundary should stay narrow: frontend code prepares drafts and renders results, while native services own network and durable persistence concerns.

### Native Realtime Execution

Keeping WebSocket and Socket.IO transports in an application-wide native manager avoids WebView lifetime and browser networking constraints and lets sessions survive route changes. It also gives one place to enforce connection/message/transcript limits, TLS policy, secret masking, ordering, reconnect behavior, and temporary payload ownership.

The trade-off is that realtime session data is deliberately ephemeral and protocol support is intentionally narrower than the editable HTTP pipeline. Durable history, scripts, Playbook execution, proxy/custom-certificate controls, and additional realtime protocols must be designed at the native session boundary rather than added as frontend-only behavior.

### Frontend Script Runtime

Running pre-request and test scripts in a short-lived worker-backed frontend sandbox keeps scripting close to the request editor and Playbook orchestration. The trade-off is that scripts are intentionally scoped to the documented `pn` API instead of attempting full Postman runtime compatibility.

If scripting grows substantially, the runtime API, concurrency model, and isolation guarantees should be treated as an explicit subsystem design rather than as incremental helper additions.

### Response Body Persistence

Persisting decoded response bodies as history body files keeps detail inspection available without inflating the main SQLite rows. The trade-off is an additional file-retention responsibility. Settings makes that ownership visible and enforces count, age, and actual managed-file byte caps; deletion still commits database changes before leased body paths are released.

### Stable Updater Feed

Using GitHub Releases' stable `latest` updater manifest keeps update discovery predictable for normal users. The trade-off is that prerelease discovery is not part of the default update path.

Any prerelease channel should remain opt-in and should preserve the signed-update and target-selection guarantees already used by the stable path.

### Local MCP Authoring

PostNot uses the official Rust MCP SDK over stdio and starts the server before Tauri initialization when the executable receives `--mcp`. Reusing the application binary keeps signing, AppImage behavior, and updater delivery aligned without a second sidecar artifact.

The MCP surface is deliberately authoring-only. Collection reads and masked previews reuse native services, while creates and revision-checked request replacements write the same SQLite database. Request sending and script execution remain outside MCP because the script runtime is worker-backed frontend JavaScript and cannot yet provide identical headless behavior.

Realtime MCP tools follow the same rule: they list, read, create, and revision-check saved definitions but cannot open sessions or send messages.

The desktop polls the monotonic Agent Activity cursor while focused. New successful rows refresh affected collection trees and mark open saved-request drafts as externally changed rather than overwriting them.
