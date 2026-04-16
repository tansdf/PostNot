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
- Async runtime: `tokio`
- Serialization: `serde`
- SQL layer: `sqlx`

Why this stack:

- Rust + Tauri keeps the app lighter than Electron-based alternatives
- SvelteKit gives a productive UI layer without browser-side request constraints
- SQLite gives durable local persistence without introducing a separate service
- `reqwest` keeps request execution in the native layer, which avoids browser CORS behavior and makes TLS and redirect settings controllable

## 3. Current Application State

This section reflects the code currently implemented in the repository, including the shipped `0.15.0` scripting update and later patch-level behavior (for example `0.15.1` route and modal polish—see [CHANGELOG.md](../CHANGELOG.md)).

### Implemented

- Tauri application shell with SvelteKit frontend
- SQLite initialization on app startup
- SQL migrations applied automatically at launch
- Single request editor
- Supported request methods: `GET`, `POST`, `PUT`, `PATCH`, `DELETE`, `HEAD`, `OPTIONS`
- Request editing for:
  - URL
  - query parameters
  - headers
  - auth: none, basic, bearer, API key
  - body: none, JSON, raw, form-urlencoded, multipart with file uploads
- Native request execution through Rust
- Response viewer with:
  - status
  - duration
  - size
  - headers
  - body text / JSON pretty rendering
- Persisted application settings in SQLite
- Persisted request history in SQLite
- Cancel in-flight request
- Collections and saved requests
- Collection folders with nested request organization
- Drag-and-drop saved-request moves across collection trees, including reorder, folder moves, and cross-collection placement
- Environments and variable resolution
- OS-backed secret storage for secret environment variables
- Postman collection JSON import
- Postman environment JSON import
- Postman collection JSON export
- Postman environment JSON export
- cURL command import
- Multipart request composition with native file selection
- Built-in dynamic variables at request runtime
- App-level floating notification system for action feedback
- Settings page wired to backend persistence
- Signed in-app update checks, startup refresh, and install handoff
- History panel wired to backend persistence
- History detail inspection from persisted snapshots
- Clear history action
- Pre-request scripts and test scripts for collections, folders, and saved requests (executed in the frontend as sandboxed JavaScript before send and after response)
- Async scripting helper requests through `await pn.http.send(...)`
- Active-environment variable reads and writes from scripts, including persisted secret writes through the OS credential store path
- URL-driven selection for the main saved request (`savedRequestId`), collections (`collectionId`), and environments (`environmentId`) uses generation guards so overlapping async loads from rapid navigation do not apply stale UI state; clearing `savedRequestId` from the URL resets deep-link tracking so the same request can load again from the query string
- Save-request, cURL import, collection import, and environment import modals trap focus (initial focus, Tab cycle within the dialog, Escape to close, prior focus restored on close) for keyboard and assistive technology users
- Native `reqwest::Client` instances are reused per TLS/redirect/timeout fingerprint with a bounded in-memory cache so settings changes do not rebuild a client on every send

### Not Yet Implemented

- Multi-tab workflow

## 4. High-Level Architecture

The app is split into two layers.

### Frontend

Responsibilities:

- Render request editor (including script editors), response viewer, settings page, and history panel
- Manage page-level UI state
- Render global floating notifications for cross-screen action feedback
- Coordinate shared pointer-driven collection drag-and-drop interactions across the sidebar and Collections page
- Keep URL query parameters for saved requests, collections, and environments in sync with the visible editor or browser, including canceling stale async work when the user navigates quickly
- Run inherited collection, folder, and saved-request pre-request and test scripts in JavaScript before and after invoking `send_request`
- Invoke typed Tauri commands for persistence and request execution
- Provide a desktop-oriented workflow without browser networking

The frontend does not execute HTTP requests directly. All network traffic goes through Rust.

### Native Layer

Responsibilities:

- Initialize SQLite database and run migrations
- Execute HTTP requests
- Load and persist settings
- Persist request history
- Load environment metadata from SQLite while storing secret environment values in the OS credential store
- Coordinate signed release checks and install handoff for the Settings updater flow
- Resolve app data paths
- Expose a stable Tauri command surface to the UI

### Data Flow

1. User edits a request in the UI
2. On send, the frontend runs inherited collection, folder, and saved-request pre-request scripts (if any) against a draft copy and either stops with a script error surface or proceeds with the mutated draft as the payload
3. Frontend invokes `send_request` with that payload
4. Rust loads persisted request settings from SQLite
5. Rust resolves environment variables and built-in dynamic variables
6. Rust executes the request with `reqwest`
7. Rust returns response metadata and body to the UI
8. Rust writes a history entry to SQLite, redacting secret-derived environment substitutions back to their original `{{variable}}` form
9. Frontend runs inherited collection, folder, and saved-request test scripts (if any) against the returned response for assertion output
10. Frontend reloads history and renders the latest response and script results

## 5. Actual Folder Structure

This is the meaningful structure currently present in the repo.

```text
PostNot/
  docs/
    tech-design.md
  src/
    app.html
    app.d.ts
    hooks.client.ts
    lib/
      api/
        commands.ts
        types.ts
      components/
        collections/
          CollectionDetailForm.svelte
          CollectionsPanel.svelte
        icons/
          FolderGlyph.svelte
        history/
          HistoryDetail.svelte
          HistoryPanel.svelte
        layout/
          AppShell.svelte
          NotificationHost.svelte
          SidebarCollections.svelte
        request/
          RequestEditor.svelte
          ScriptEditor.svelte
          VariableField.svelte
        response/
          JsonViewer.svelte
          ResponseViewer.svelte
      icons/
        folderPaths.ts
      request-scripts.ts
      stores/
        collections.svelte.ts
        notifications.svelte.ts
        updater.svelte.ts
      theme.ts
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
      settings/
        +page.svelte
  src-tauri/
    Cargo.toml
    tauri.conf.json
    build.rs
    capabilities/
      default.json
    icons/
      icon.png
    migrations/
      0001_init.sql
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
        imports.rs
        updates.rs
        mod.rs
        requests.rs
        settings.rs
        history.rs
      services/
        collections_service.rs
        environments_service.rs
        exports_service.rs
        imports_service.rs
        mod.rs
        http_client.rs
        secret_store_service.rs
        settings_service.rs
        history_service.rs
        updates_service.rs
        window_state_service.rs
      storage/
        mod.rs
        paths.rs
  build/
    .gitkeep
  static/
  package.json
  svelte.config.js
  tsconfig.json
  vite.config.ts
```

## 6. Core Domain Model

The current implementation uses these core entities.

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

### App Settings

Represents persisted request behavior settings.

Fields:

- theme
- interface zoom
- request timeout in milliseconds
- follow redirects flag
- validate TLS flag
- history limit
- notification timeout in milliseconds
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

## 7. SQLite Storage Design

The schema currently matches the initial migration in `src-tauri/migrations/0001_init.sql`.

### Database Location

The database is created under the Tauri app data directory:

- database file: `<app_data_dir>/postnot.sqlite`

### Tables Currently Used

#### `app_settings`

Used actively by the application.

```sql
CREATE TABLE app_settings (
  key TEXT PRIMARY KEY,
  value_json TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
```

Current keys written by the app:

- `theme`
- `ui_scale`
- `request_timeout_ms`
- `follow_redirects`
- `validate_tls`
- `history_limit`
- `notification_timeout_ms`
- `last_update_checked_at`
- `collection_sidebar_state`

#### `history_entries`

Used actively by the application.

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
- successful requests also persist the full response body to a file path referenced by `response_body_path`
- failed requests are also persisted with `error_text`
- history is pruned based on the persisted `history_limit` setting

### Other Actively Used Tables

#### `collections`

Stores saved request collections.

#### `collection_items`

Stores both saved requests and folders within a collection tree.

Implementation notes:

- `kind` distinguishes folders from saved requests
- `parent_id` allows nested folders and request placement inside folders
- `prerequest_script` and `test_script` are persisted per collection, folder, and saved request; the UI runs inherited collection scripts first, then ancestor folder scripts from root to leaf, then saved-request scripts in the frontend (`request-scripts.ts`) before invoking Rust for send (pre-request) and after the response returns (tests), not inside the native HTTP layer

#### `environments`

Stores environment metadata, active-state, and non-secret variable definitions. Secret values are kept in the OS credential store.

## 8. Runtime Behavior

### Startup

At startup, the Tauri app:

1. resolves the app data directory
2. creates the SQLite database if missing
3. applies SQL migrations
4. ensures default settings exist
5. initializes the OS-backed secret store
6. stores the SQLite pool and secret store in app state
7. restores and tracks the main window size and position

### Request Execution

For each request send, Rust currently applies these persisted settings:

- `request_timeout_ms`
- `follow_redirects`
- `validate_tls`

This means the settings page already changes actual network behavior, not just UI state.

Rust builds or reuses a `reqwest::Client` for the active combination of `validate_tls`, `follow_redirects`, and `request_timeout_ms` (cached up to a fixed number of distinct fingerprints) instead of constructing a new client on every request.

For each saved request send, the frontend may first run the collection pre-request script, then each ancestor folder pre-request script from root to leaf, and then the saved request's pre-request script against a draft copy (with the active environment's variables) to mutate headers, query params, URL, and related fields. Those scripts can also await helper HTTP calls through `pn.http.send(...)` and persist active-environment variable changes before the main request continues. Errors from that step surface in the UI without calling Rust.

For each request send, Rust then:

- loads the currently active environment, if one exists
- resolves `{{variable}}` placeholders in URL, query params, headers, body text, form fields, and auth values
- expands built-in dynamic variables such as `$guid`, `$timestamp`, and related runtime helpers
- sends the resolved request payload

After Rust returns a response (or error), the frontend may run the collection test script, ancestor folder test scripts from root to leaf, and then the saved test script, recording assertion results for display in the response panel.

### History Persistence

On successful request execution:

- the request snapshot is stored
- response summary metadata is stored
- response preview text is stored
- full response bodies are stored on disk for detail inspection
- history is pruned to the configured limit

On failed request execution:

- the request snapshot is stored
- error text is stored
- history is pruned to the configured limit

On canceled request execution:

- the in-flight native request is aborted
- no history entry is written

## 9. Current Tauri Command Boundary

Commands currently exposed to the frontend:

- `send_request`
- `cancel_active_request`
- `pick_multipart_files`
- `get_settings`
- `update_settings`
- `check_for_updates`
- `install_update`
- `list_history`
- `get_history_entry`
- `clear_history`
- `list_collections`
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
- `delete_saved_request`
- `export_collection`
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

### Command Roles

- `send_request`: executes the request using persisted settings and records history
- `cancel_active_request`: aborts the currently active native request, if one exists
- `pick_multipart_files`: opens a native file picker and returns selected local file paths for multipart requests
- `get_settings`: loads current settings from SQLite
- `update_settings`: persists settings and returns the saved values
- `check_for_updates`: checks the configured signed updater feed for a newer release
- `install_update`: hands the available signed update off to the native installer
- `list_history`: returns recent history entries ordered by execution time descending
- `get_history_entry`: returns a stored request snapshot and response metadata for one history entry
- `clear_history`: deletes all stored history entries
- `list_collections`: returns saved request collections with request counts and collection-level scripts
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
- `delete_saved_request`: removes one saved request from a collection
- `export_collection`: exports one collection to Postman Collection v2.1 JSON through a native save dialog
- `list_environments`: returns saved environments with active-state and variable counts
- `create_environment`: creates a blank environment draft
- `get_environment`: returns one environment and its variables
- `update_environment`: persists environment name and variables
- `delete_environment`: removes one environment
- `set_active_environment`: marks one environment active or clears the active environment
- `import_postman_environment`: imports a Postman environment JSON file or payload into a new PostNot environment
- `export_environment`: exports one environment to Postman environment JSON through a native save dialog
- `import_requests`: imports requests from Postman collection JSON or cURL into PostNot collections
- `import_curl_request_to_draft`: parses a cURL command into an editable request draft without saving it yet

## 10. Frontend Screens

### Main Page

Current UI sections:

- request profile summary using persisted settings
- active environment selector
- request editor and collection editor with pre-request and test script editors (`ScriptEditor.svelte`)
- save flow with collection and folder target selection
- cURL import modal
- request-level save/update action
- response viewer
- history panel
- history detail inspector

### Settings Page

Current UI sections:

- theme selector
- interface zoom selector
- request timeout input
- history limit input
- notification timeout input
- follow redirects toggle
- validate TLS toggle
- updater status and install surface
- persisted save action

### Collections Page

Current UI sections:

- collection browser with nested folders and saved requests
- dedicated collection editor view (`CollectionDetailForm.svelte` for metadata drafts)
- root-folder and subfolder creation
- collection import/export actions
- selected collection tree for folders and saved requests with vertical tree guides and folder open/closed icons (`FolderGlyph.svelte` + shared SVG paths in `folderPaths.ts`)
- drag-and-drop saved-request management that matches the sidebar tree: reorder among siblings, move into folders, move across collections, and move back to collection root
- matching sidebar tree styling for nested collections (see `SidebarCollections.svelte` and `app.css`)
- open-in-requests and delete actions for saved requests

### Environments Page

Current UI sections:

- environment list
- active/inactive environment controls
- environment variable editor
- Postman environment import
- Postman environment export
- variable usage hint for `{{name}}` syntax

## 11. Security and Persistence Notes

Current state:

- the app is fully local
- requests are executed in Rust, not the browser
- secret environment values are stored in the OS credential store, while SQLite keeps only non-secret environment metadata
- history snapshots redact resolved values that came from secret environment variables
- if an environment update or delete fails after partially changing the credential store, rollback of secrets is attempted; failure to roll back is logged with `log::warn` for diagnostics (the primary error still returns to the UI)

This is the current security posture for environment variables; broader secret handling beyond environment-backed values remains future work.

## 12. Milestone Status

### Milestone 1 Goal

Ship a usable desktop app that can compose and execute HTTP requests locally, persist request behavior settings, and preserve request history across restarts.

### Milestone 1 Implemented So Far

- Tauri + SvelteKit app shell
- single request editor
- auth support for none/basic/bearer/API key
- request execution through Rust
- request cancellation
- collections and saved requests
- collection folders with nested browsing
- sidebar-first collection browsing and dedicated collection editing
- environments and variable resolution
- built-in dynamic request variables
- Postman collection JSON import
- Postman environment JSON import
- Postman collection JSON export
- Postman environment JSON export
- cURL command import
- multipart request composition with local file uploads
- response viewer
- SQLite initialization and migrations
- persisted settings
- persisted history
- settings page
- history panel
- history detail inspection
- clear history action
- signed updater checks with startup refresh and install flow
- pre-request and test scripts on collections, folders, and saved requests (frontend execution around the native send)
- shipped async scripting helper requests and active-environment variable writes in `0.15.0`
- collections sidebar and collections panel folder trees with shared `FolderGlyph` styling
- route/query stale-load guards, modal focus trapping, bounded `reqwest` client cache, and secret rollback warning logs as in `0.15.1`

### Current Scripting Boundary

The async scripting milestone shipped in `0.15.0`.

Scripts now run as awaited frontend JavaScript around one request send. Pre-request scripts can read and write active environment variables, mutate the outgoing request draft, and call `await pn.http.send(...)` to perform helper HTTP requests before the main request runs. Test scripts can inspect the returned response, register sync or async assertions through `pn.test(...)`, and also call helper requests when needed.

Helper script requests reuse the native request pipeline and active environment resolution, but they do not write separate history entries. Active-environment variable writes are buffered while scripts run and then persisted through the normal environment update path before the main request continues. The current runtime still allows only one native request in flight at a time, so helper requests should be awaited sequentially instead of fired concurrently.

### Milestone 1 Remaining

- tighter error handling and UX polish
- multi-request workflow decisions
- richer scripting surface (broader `pn` API parity, execution model hardening, and richer inherited execution controls)

Manual end-to-end verification via `tauri dev` has already been completed for the current milestone state.

## 13. Next Recommended Steps

Recommended implementation order from the current state:

1. Continue tightening error handling and desktop UX polish
2. Decide whether updater discovery should stay on GitHub's stable-only `/latest` endpoint or move to a custom prerelease-aware manifest
3. Evaluate multi-tab workflow and other request-level productivity features
4. Improve import/export compatibility and remaining desktop polish
5. Extend request scripting beyond the shipped `0.15.0` async-helper release with broader API surface, safety, and inherited execution controls

## 14. Open Decisions

These are still unresolved:

- whether large response bodies should spill to files instead of SQLite preview-only storage
- whether tabs should persist in SQLite or only in frontend state at first
- exact import/export format for PostNot bundles
- how far secret redaction should extend beyond environment-backed variables

## 15. Recommendation

Treat the repository as being in an active Milestone 1 state, not full MVP completion.

The design is now grounded in what the code actually does: persisted settings influence request execution, history is stored in SQLite with secret-derived environment values redacted, secret environment values live in the OS credential store, environments resolve variables at send time, collections support nested folders in the working UI with consistent sidebar and collections-panel tree affordances, saved requests can be reordered or moved across folders and collections through a shared drag-and-drop model, collections, folders, and saved requests can run inherited pre-request and test scripts in the frontend before and after native execution, scripts can await helper HTTP requests through `pn.http.send(...)` without polluting history and can persist active-environment variable updates during script execution, import can pull requests in from Postman collections and cURL, multipart requests can attach local files, built-in dynamic variables resolve at runtime, and the desktop shell can check GitHub Releases for signed updater builds both on launch and from Settings. The next work should stay focused on updater channel decisions, multi-tab decisions, scripting depth, and remaining UX polish.
