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

This section reflects the code currently implemented in the repository, including the shipped `0.15.0` scripting update, the `0.15.1` route/modal polish work, the `0.16.0` OpenAPI 3 import release, the `0.17.0` multitab request workspace release, the `0.17.1` history-restore patch, the `0.17.2` requests/environments workflow follow-up, the `0.17.3` hydration-flash polish release, the `0.17.4` follow-up that unifies hydration-flash handling through a shared synchronous paint cache, the `0.18.0` sidebar collection search release, the `0.18.1` scripting/workspace hardening patch, the `0.19.0` cURL/OAuth2 import-auth and request-export polish, the `0.19.2` full response body follow-up, the `0.19.3` updater download progress patch, the `0.19.4` revert of the 0.18.2 binary response preview layer, the `0.19.5` compressed-response decoding and error-details patch, the `0.19.6` OAuth2 token-fetch helper release, the `0.19.7` redacted single-request export patch, the `0.20.0` Playbooks release, the `0.20.1` Linux updater target patch, the `0.20.2` invisible scrollbar polish patch, the `0.20.3` resolved request preview release, the `0.20.5` landing-page copy refresh, the `0.20.7` Requests header autocomplete patch, and the current `0.20.8` collection folder ordering and drag-reorder patch (see [CHANGELOG.md](../CHANGELOG.md)).

### Implemented

- Tauri application shell with SvelteKit frontend
- SQLite initialization on app startup
- SQL migrations applied automatically at launch
- Multi-tab request workspace with restored local tabs between launches
- Supported request methods: `GET`, `POST`, `PUT`, `PATCH`, `DELETE`, `HEAD`, `OPTIONS`
- Request editing for:
  - URL
  - query parameters
  - headers with common-name autocomplete and name-aware value suggestions
  - auth: none, basic, bearer, API key, OAuth2 bearer with client-credentials token fetch
  - body: none, JSON, raw, form-urlencoded, multipart with file uploads
- Read-only resolved request preview before send, built by the native environment/settings pipeline, including outgoing URL/query/header/auth/body data, active settings, validation warnings, dynamic-variable notes, and masked credential-looking values
- Native request execution through Rust
- Response viewer with:
  - status
  - duration
  - size
  - headers
  - body text / JSON pretty rendering
  - full response body reads
- Persisted application settings in SQLite
- Persisted request history in SQLite, including a collapsible Requests-page history panel state stored in app settings
- Cancel in-flight request
- Restoring stored history requests into new request tabs
- Collections and saved requests
- Collection folders with nested request organization
- Sidebar collection search across collections, folders, and saved requests
- Drag-and-drop saved-request moves across collection trees, including reorder, folder moves, and cross-collection placement
- Playbooks for sequential saved-request execution with ordered steps, per-step/default delays, stop-on-failure policy, grouped run logs, and normal per-step request history
- Environments and variable resolution
- OS-backed secret storage for secret environment variables
- Postman collection JSON import
- Postman environment JSON import
- OpenAPI 3 JSON/YAML import for collections and single-request drafts
- Postman collection JSON export
- Postman environment JSON export
- cURL command import, including `--url`, `--get`, repeated `--data`, multipart `--form`, cookies, compression, redirect flags, and shell continuation cleanup
- Single-request cURL and PostNot request JSON export from the Requests page, with credential-looking values redacted by default and an explicit full-export toggle
- Multipart request composition with native file selection
- Built-in dynamic variables at request runtime
- App-level floating notification system for action feedback
- Settings page wired to backend persistence
- Environment autosave preference stored in persisted settings and enabled by default
- Signed in-app update checks, startup refresh, download progress reporting, and install handoff
- Static GitHub Pages landing site in `docs/`, served at `post-not.com`, with checked-in WebP product screenshots and copy aligned with the current app surface
- History panel wired to backend persistence
- History detail inspection from persisted snapshots
- Restore action that opens a stored history request snapshot as a new standalone tab
- Clear history action
- Pre-request scripts and test scripts for collections, folders, and saved requests (executed in a short-lived worker-backed frontend sandbox before send and after response)
- Async scripting helper requests through `await pn.http.send(...)`
- Active-environment variable reads and writes from scripts, including persisted secret writes through the OS credential store path
- OAuth2 bearer tokens can be fetched from the request editor through the client-credentials flow, sourced from environment variables, optionally persisted to the active environment as a secret `oauth_access_token`, and set at runtime with `pn.request.setOAuth2Token(...)`
- Request tabs persist through the existing `app_settings` store, keeping draft state and active tab selection across restarts without persisting response bodies, script output, or transient tab errors
- URL-driven selection for the main saved request (`savedRequestId`), collections (`collectionId`), and environments (`environmentId`) uses generation guards so overlapping async loads from rapid navigation do not apply stale UI state; clearing `savedRequestId` from the URL resets deep-link tracking so the same request can load again from the query string
- Save-request, cURL import, collection import, and environment import modals trap focus (initial focus, Tab cycle within the dialog, Escape to close, prior focus restored on close) for keyboard and assistive technology users
- `Ctrl+S` / `Cmd+S` shortcuts save the active request on the Requests page and the selected environment on the Environments page
- Native `reqwest::Client` instances are reused per TLS/redirect/timeout fingerprint with a bounded in-memory cache so settings changes do not rebuild a client on every send

## 4. High-Level Architecture

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
- Build resolved request previews without executing scripts or network traffic, using the same environment and dynamic-variable resolution path as sends and masking credential-looking values before returning data to the UI
- Coordinate signed release checks against GitHub Releases' stable `latest` updater manifest, Linux installer-target selection, download progress events, retryable failure handling, and install handoff for the Settings updater flow
- Resolve app data paths
- Expose a stable Tauri command surface to the UI

### Data Flow

1. User edits a request in the UI
2. The frontend keeps that draft inside the active request tab and persists workspace changes locally through the settings-backed workspace store
3. Before sending, the user may open a read-only resolved request preview; the frontend invokes the native `preview_request` command, which resolves the active environment and settings, assembles outgoing query/auth/header/body data, adds auth/body-generated headers, masks credential-looking values, validates URL/header/body/file state, and returns warnings and notes without executing scripts, helper requests, environment writes, or network traffic
4. On send, the frontend runs inherited collection, folder, and saved-request pre-request scripts (if any) against a draft copy and either stops with a script error surface or proceeds with the mutated draft as the payload
5. Frontend invokes `send_request` with that payload
6. Rust loads persisted request settings from SQLite
7. Rust resolves environment variables and built-in dynamic variables
8. Rust executes the request with `reqwest`
9. Rust returns response metadata plus the decoded response body to the UI
10. Rust writes a history entry to SQLite, redacting secret-derived environment substitutions back to their original `{{variable}}` form and persisting decoded response text when available
11. Frontend runs inherited collection, folder, and saved-request test scripts (if any) against the returned response for assertion output
12. Frontend reloads history, updates the originating tab, and persists the refreshed workspace state

### Playbook Data Flow

1. User creates a playbook on `/playbooks` and adds existing saved requests as ordered live references
2. The frontend loads each enabled step immediately before execution through `get_playbook_execution_context`
3. The context returns the latest saved request plus inherited collection and folder scripts
4. The frontend runs pre-request scripts, sends through the existing `send_request` command, then runs test scripts
5. Each step still writes normal request history through the existing send path
6. The frontend records grouped playbook run and step summaries in SQLite
7. Stop-on-failure and non-2xx/3xx failure policy are enforced by the playbook runner, with remaining enabled steps recorded as skipped

## 5. Actual Folder Structure

This is the meaningful structure currently present in the repo.

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
          RequestTabs.svelte
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
        request-workspace.svelte.ts
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
      playbooks/
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
        playbooks.rs
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
        playbooks.rs
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
        playbooks_service.rs
        request_preview_service.rs
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

The schema is created by the migrations in `src-tauri/migrations/`: `0001_init.sql` for the original app tables, `0002_collection_scripts.sql` for collection-level scripts, and `0003_playbooks.sql` for playbook definitions and grouped run logs.

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
- `request_workspace_state`
- `environment_autosave`

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
- successful responses with decoded text also persist the full response body to a file path referenced by `response_body_path`
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

#### `playbooks`

Stores Playbook metadata, default delay, stop-on-failure policy, and HTTP error failure policy.

#### `playbook_steps`

Stores ordered saved-request references for a Playbook, including per-step enabled state, optional name/notes, and optional delay override.

#### `playbook_runs` and `playbook_run_steps`

Store grouped Playbook execution summaries and per-step outcomes. Individual step sends still go through the normal request execution path and write normal request history entries.

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

### Resolved Request Preview

The Requests page can call `preview_request` before send. The command loads persisted settings, resolves the active environment and built-in dynamic variables, and passes both the original and resolved request through `request_preview_service`.

The preview response is intentionally read-only. It does not execute pre-request scripts, helper HTTP calls, active-environment writes, or the main network request. It shows the final URL with enabled query parameters, auth-generated and body-generated headers, resolved auth/body data, active request settings, warnings for invalid URL/header/body/file state, unresolved-variable warnings, and notes about generated transport headers and sampled dynamic variables. Secret-derived values and credential-looking keys are masked before they reach the UI.

### Updater

The updater uses Tauri's signed updater plugin with a bundled public key and the stable GitHub Releases endpoint at `https://github.com/tansdf/PostNot/releases/latest/download/latest.json`. The frontend runs a silent startup check when Tauri is available and exposes manual checks from Settings.

On Linux, update checks request a target matching the detected install type (`deb`, `rpm`, or `appimage`) and architecture. Debian and RPM installs download the package, verify the expected package magic bytes, hand installation to `pkexec`, and time out instead of waiting indefinitely for a missing PolicyKit prompt. Other targets use the plugin `download_and_install` path. Download progress is emitted as `update-download-progress` and surfaced in Settings; failed downloads or installer handoffs leave the pending update available for retry.

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
- `preview_request`
- `cancel_active_request`
- `pick_multipart_files`
- `get_settings`
- `update_settings`
- `get_request_workspace_state`
- `save_request_workspace_state`
- `check_for_updates`
- `install_update`
- `list_history`
- `get_history_entry`
- `clear_history`
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
- `delete_collection_item`
- `delete_saved_request`
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
- `check_for_updates`: checks the configured signed updater feed for a newer stable GitHub Release and stores a pending update when available
- `install_update`: downloads the pending signed update with progress events and hands it off to the native installer, using the detected Debian/RPM/AppImage install type on Linux
- `list_history`: returns recent history entries ordered by execution time descending
- `get_history_entry`: returns a stored request snapshot and response metadata for one history entry
- `clear_history`: deletes all stored history entries
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
- `delete_collection_item`: removes a folder or saved request item from a collection tree
- `delete_saved_request`: removes one saved request from a collection
- `export_collection`: exports one collection to Postman Collection v2.1 JSON through a native save dialog
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
- `import_requests`: imports requests from Postman collection JSON or cURL into PostNot collections
- `import_curl_request_to_draft`: parses a cURL command into an editable request draft without saving it yet
- `import_openapi_request_to_draft`: parses one OpenAPI operation into an editable request draft without saving it yet

## 10. Frontend Screens

### Main Page

Current UI sections:

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

Current UI sections:

- theme selector
- interface zoom selector
- request timeout input
- history limit input
- notification timeout input
- follow redirects toggle
- validate TLS toggle
- updater status and install surface
- live download progress with byte counts when content length is known
- available-update notes rendered from the signed updater metadata
- persisted save action

### Collections Page

Current UI sections:

- collection browser with nested folders and saved requests
- dedicated collection editor view (`CollectionDetailForm.svelte` for metadata drafts)
- root-folder and subfolder creation
- collection import/export actions
- selected collection tree for folders and saved requests with vertical tree guides and folder open/closed icons (`FolderGlyph.svelte` + shared SVG paths in `folderPaths.ts`)
- drag-and-drop saved-request and folder management that matches the sidebar tree: reorder among siblings, move into folders, move across collections, and move back to collection root
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

### Playbooks Page

Current UI sections:

- Playbook list and editor
- ordered saved-request steps with enable/disable controls, per-step notes, and delay overrides
- default delay, stop-on-failure, and fail-on-HTTP-error policy controls
- sequential run controls that reuse the normal request send, scripting, environment, and history paths
- grouped run log summaries and per-step outcomes

### Public Landing Site

Current static site sections:

- GitHub Pages landing page under `docs/index.html`, served with `docs/CNAME` for `post-not.com`
- product copy updated for the current local-first app state, including Playbooks, resolved previews, OAuth2 token fetching, redacted exports, scripting, secrets, and imports
- screenshot gallery using checked-in WebP captures under `docs/images/`
- scripting reference page under `docs/scripting.html`

Screenshot workflow note:

- There is no checked-in screenshot automation script in the current repository. Public screenshots are static WebP assets committed under `docs/images/`; when the UI changes visibly, refresh those assets manually or add a documented capture workflow before relying on automated screenshot updates.

## 11. Security and Persistence Notes

Current state:

- the app is fully local
- requests are executed in Rust, not the browser
- secret environment values are stored in the OS credential store, while SQLite keeps only non-secret environment metadata
- history snapshots redact resolved values that came from secret environment variables
- single-request cURL and PostNot JSON exports redact credential-looking values by default, including bearer tokens, OAuth2 access tokens, client secrets, API keys, cookies, and basic-auth passwords; full-value export requires an explicit toggle and shows a warning
- resolved request preview masks credential-looking values and secret-derived environment substitutions before showing outgoing request data
- decoded response bodies are persisted as full text history body files
- if an environment update or delete fails after partially changing the credential store, rollback of secrets is attempted; failure to roll back is logged with `log::warn` for diagnostics (the primary error still returns to the UI)

This is the current security posture: environment-backed secrets are protected in storage and history, while single-request export uses local pattern-based redaction for credential-looking values before users copy cURL or PostNot JSON.

## 12. Release Progress

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
- signed updater checks with startup refresh, Linux package-target matching, and install flow
- pre-request and test scripts on collections, folders, and saved requests (frontend execution around the native send)
- shipped async scripting helper requests and active-environment variable writes in `0.15.0`
- collections sidebar and collections panel folder trees with shared `FolderGlyph` styling
- route/query stale-load guards, modal focus trapping, bounded `reqwest` client cache, and secret rollback warning logs as in `0.15.1`
- OpenAPI 3 collection import plus single-operation draft import, with the Rust importer split into format-focused modules, as in `0.16.0`
- restored multitab request workspaces with tab-local drafts persisted through `app_settings`, while response bodies and script output remain session-local
- Requests history restore, environment autosave, hydration-flash avoidance through local first-paint caches, and sidebar collection search
- cURL/OAuth2 import-auth improvements, OAuth2 client-credentials token fetching, redacted single-request export, compressed response decoding, full response body persistence, and retryable updater progress
- Playbooks with ordered saved-request steps, run policies, grouped logs, and normal request-history integration
- native resolved request preview before send, with validation notes and credential masking
- static GitHub Pages landing page and scripting reference under `docs/`, with checked-in product screenshots
- Requests header autocomplete for common names, existing draft names, name-aware common values, and matching values from the current draft

### Current Scripting Boundary

The async scripting milestone shipped in `0.15.0`.

Scripts now run as awaited frontend JavaScript inside a short-lived worker-backed sandbox around one request send. Pre-request scripts can read and write active environment variables, mutate the outgoing request draft, and call `await pn.http.send(...)` to perform helper HTTP requests before the main request runs. Test scripts can inspect the returned response, register sync or async assertions through `pn.test(...)`, and also call helper requests when needed.

Helper script requests reuse the native request pipeline and active environment resolution, but they do not write separate history entries. Active-environment variable writes are buffered while scripts run and then persisted through the normal environment update path before the main request continues. The current runtime still allows only one native request in flight at a time, so helper requests should be awaited sequentially instead of fired concurrently.

Manual end-to-end verification via `tauri dev` remains the expected release-confidence check for scripting and request-send behavior.

### Current Position

The project is no longer at the "prove the app works at all" stage. The implemented `0.20.8` surface already covers the primary local API-client workflow: request composition and execution, persisted settings, restored local request tabs, resolved previews before send, history restore, collections, nested folders, folder/request drag management, sidebar search, Playbooks, environments, secret storage, import/export, multipart, OAuth2 helpers, header autocomplete, scripting helpers, signed stable-release updater checks, and a static public landing/scripting site.

The remaining work is primarily about closing the last daily-driver gaps, tightening reliability, and deciding which behaviors are part of the supported `1.x` contract.

## 13. v1.0.0 Criteria

`v1.0.0` should mark product confidence, not just a large feature batch. It does not need to be the single biggest release in terms of visible surface area. It should be the release where PostNot can be described as a stable, trustworthy local desktop API client for the intended solo-user workflow.

### Must Be True For v1.0.0

- the current core workflow remains stable: native request sending, resolved previews, auth modes, body modes, environments, secrets, history, collections, Playbooks, import/export, scripting, and updater flows
- multi-tab workflow remains behaviorally intentional enough to stand behind as part of the product, even if future releases expand it
- history restore and collection search stay reliable as hardening continues
- error handling and desktop UX are hardened enough that the app feels dependable in normal daily use
- the codebase receives a focused hardening and optimization pass rather than only feature additions

### Release Gates For v1.0.0

Use concrete acceptance gates instead of a vague "more polish" bar:

- no known data-loss bugs in saved requests, collections, environments, or history
- no known request-corruption bugs caused by scripting, environment resolution, drag-and-drop moves, or restore flows
- startup, request send, navigation, and collection interactions feel consistently responsive on normal desktop hardware
- `npm run check` passes cleanly
- `cargo check --manifest-path src-tauri/Cargo.toml` passes cleanly
- main workflows are re-verified through `npm run tauri dev`
- Windows behavior that matters for release confidence receives a smoke test in a native Windows environment, not only WSLg

### Explicitly Not Required For v1.0.0

The following can remain post-`1.0` work unless they become necessary for the primary workflow:

- full Postman scripting parity
- a very broad `pn` runtime API
- every possible import/export format beyond the current Postman/OpenAPI/cURL coverage
- collaboration or cloud-sync features
- advanced bundle/export formats for PostNot-specific interchange

## 14. Versioning Strategy Toward v1.0.0

The project should continue shipping meaningful pre-`1.0` minor releases while the `v1` feature set is being completed. `1.0.0` should be used as the maturity and support marker once the intended workflow is complete and hardened.

This means the project does not need to hold all remaining `v1` features for one giant release. Shipping them incrementally is preferred because it keeps changes smaller, validation easier, and regressions easier to isolate.

### Recommended Approach

1. Ship major `v1` features as normal `0.x` minor releases when they are ready.
2. Once the agreed `v1` scope is feature-complete, declare a short stabilization phase.
3. Use `1.0.0` for the release that combines the completed scope with the final hardening, verification, and release-signoff pass.

### Example Sequence

One reasonable path from the current state is:

1. `0.20.8`: current app baseline with Playbooks, resolved previews, updater progress/targeting, landing-site refresh, Requests header autocomplete, and collection folder drag/order polish
2. `0.21.0`: hardening, optimization, and UX/error-handling improvements
3. `1.0.0`: release-signoff build after the `v1` scope is complete and verified

The exact version numbers are less important than the policy: `1.0.0` is allowed to be a smaller visible release than earlier `0.x` milestones if it represents a meaningful jump in confidence and support commitment.

## 15. Open Decisions

These decisions remain relevant as the app approaches `1.0.0`:

- whether full response body files need retention controls, size management, or migration beyond the current history-body file storage
- exact import/export format for PostNot bundles
- how far pattern-based export redaction should extend beyond the current credential-looking request fields
- whether updater discovery should remain stable-only on GitHub's `/latest` endpoint or later grow an opt-in prerelease channel

## 16. Recommendation

Treat the repository as being on the path from active Milestone 1 delivery to a deliberate `v1.0.0`, not as a project that still needs a broad product rethink.

The current design is already grounded in a real desktop workflow: persisted settings influence request execution, history is stored in SQLite with secret-derived environment values redacted, decoded response bodies are persisted as history body files, secret environment values live in the OS credential store, environments resolve variables at send time, the Requests page can ask Rust for a masked resolved preview before send, header rows provide lightweight autocomplete, collections support nested folders in the working UI with consistent sidebar and collections-panel tree affordances, saved requests can be searched, reordered, or moved across folders and collections through a shared drag-and-drop model, Playbooks can run saved requests sequentially with grouped logs, collections, folders, and saved requests can run inherited pre-request and test scripts in the frontend before and after native execution, scripts can await helper HTTP requests through `pn.http.send(...)` without polluting history and can persist active-environment variable updates during script execution, import can pull requests in from Postman collections, OpenAPI 3 documents, and cURL, single-request exports can produce redacted-by-default cURL or PostNot JSON with explicit full export, multipart requests can attach local files, built-in dynamic variables resolve at runtime, the desktop shell can check GitHub Releases' stable `latest` feed for signed updater builds both on launch and from Settings, and the public `docs/` site reflects the current shipped workflow with static screenshot assets.

The remaining `v1` work should stay focused on release-quality steps that close the last day-to-day gaps: strong hardening of correctness, UX, and performance, plus any small workflow fixes that surface during validation.
