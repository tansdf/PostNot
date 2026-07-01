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
      stores/
        collection-dnd.svelte.ts
        collection-search.svelte.ts
        collections.svelte.ts
        notifications.svelte.ts
        request-workspace.svelte.ts
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
      0002_collection_scripts.sql
      0003_playbooks.sql
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
        history.rs
        imports.rs
        playbooks.rs
        mod.rs
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
          shared.rs
        playbooks_service.rs
        playbooks_service_tests.rs
        request_preview_service.rs
        secret_store_service.rs
        settings_service.rs
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

### App Settings

Represents persisted request behavior settings.

Fields:

- theme
- interface zoom
- request timeout in milliseconds
- follow redirects flag
- validate TLS flag
- history limit
- Requests-page history collapsed flag
- environment autosave flag
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

## 6. SQLite Storage Design

The schema is created by the migrations in `src-tauri/migrations/`: `0001_init.sql` for the original app tables, `0002_collection_scripts.sql` for collection-level scripts, and `0003_playbooks.sql` for playbook definitions and grouped run logs.

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
- `is_history_collapsed`
- `environment_autosave`
- `notification_timeout_ms`
- `last_update_checked_at`
- `collection_sidebar_state`
- `request_workspace_state`

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
- successful responses with decoded text also persist the full response body to a file path referenced by `response_body_path`
- failed requests are also persisted with `error_text`
- history is pruned based on the persisted `history_limit` setting

### Other Tables

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

## 7. Runtime Behavior

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

For each request send, Rust applies these persisted settings:

- `request_timeout_ms`
- `follow_redirects`
- `validate_tls`

This means the settings page already changes actual network behavior, not just UI state.

Rust builds or reuses a `reqwest::Client` for the active combination of `validate_tls`, `follow_redirects`, and `request_timeout_ms` (cached up to a fixed number of distinct fingerprints) instead of constructing a new client on every request.

For each saved request send, the frontend may first run the collection pre-request script, then each ancestor folder pre-request script from root to leaf, and then the saved request's pre-request script against a draft copy (with the active environment's variables) to mutate headers, query params, URL, and related fields. Those scripts can also await helper HTTP calls through `pn.http.send(...)` and persist active-environment variable changes before the main request continues. Errors from that step surface in the UI without calling Rust.

Helper HTTP calls are guarded by the script runtime: only one `pn.http.send(...)` helper request may be active at a time, and helper calls must be awaited before a script source finishes. This keeps scripts aligned with the native single-request boundary and prevents the main request from racing an unfinished helper request.

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
- history limit input
- notification timeout input
- follow redirects toggle
- validate TLS toggle
- updater status and install surface
- live download progress with byte counts when content length is known
- available-update notes rendered from the signed updater metadata
- persisted save action

### Collections Page

UI responsibilities:

- collection browser with nested folders and saved requests
- dedicated collection editor view (`CollectionDetailForm.svelte` for metadata drafts)
- root-folder and subfolder creation
- collection import/export actions
- selected collection tree for folders and saved requests with vertical tree guides and folder open/closed icons (`FolderGlyph.svelte` + shared SVG paths in `folderPaths.ts`)
- drag-and-drop saved-request and folder management that matches the sidebar tree: reorder among siblings, move into folders, move across collections, and move back to collection root
- matching sidebar tree styling for nested collections (see `SidebarCollections.svelte` and `app.css`)
- open-in-requests and delete actions for saved requests

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
- Run `npm run docs:check-screenshots` to verify the checked-in screenshots are fresh enough for release. The guard compares the screenshot manifest against tracked UI/docs inputs and expected asset names; it intentionally avoids pixel diffs so normal rendering differences do not make CI brittle. The dedicated docs screenshot workflow can fail on PRs or `main` pushes, while the tag-triggered release workflow treats the check as advisory so a stale screenshot warning does not require tag recreation.

## 10. Security and Persistence Notes

- the app is fully local
- requests are executed in Rust, not the browser
- secret environment values are stored in the OS credential store, while SQLite keeps only non-secret environment metadata
- history snapshots redact resolved values that came from secret environment variables
- single-request cURL and PostNot JSON exports redact credential-looking literal values, including bearer tokens, OAuth2 access tokens, client secrets, API keys, cookies, and basic-auth passwords; the export dialog can inline active non-secret environment variables, while secret variables remain parameterized or are replaced with `***`
- resolved request preview masks credential-looking values and secret-derived environment substitutions before showing outgoing request data
- decoded response bodies are persisted as full text history body files
- if an environment update or delete fails after partially changing the credential store, rollback of secrets is attempted; failure to roll back is logged with `log::warn` for diagnostics (the primary error still returns to the UI)

Environment-backed secrets are protected in storage and history, while single-request export uses local pattern-based redaction for credential-looking values before users copy cURL or PostNot JSON.

## 11. Design Trade-Offs

### Local-First Storage

SQLite plus OS credential storage keeps the app offline-capable and avoids operating a backend service. The trade-off is that cross-device sync, collaboration, and centralized audit features are outside the core architecture.

If the product grows toward multi-device workflows, the persistence boundary should be revisited before adding sync directly into feature code.

### Native Request Execution

Routing all HTTP traffic through Rust avoids browser CORS limits and keeps TLS, redirect, timeout, cancellation, multipart file access, response decoding, and history persistence under one native pipeline. The trade-off is that browser-mode development needs mocks or degraded behavior for desktop-only capabilities.

The command boundary should stay narrow: frontend code prepares drafts and renders results, while native services own network and durable persistence concerns.

### Frontend Script Runtime

Running pre-request and test scripts in a short-lived worker-backed frontend sandbox keeps scripting close to the request editor and Playbook orchestration. The trade-off is that scripts are intentionally scoped to the documented `pn` API instead of attempting full Postman runtime compatibility.

If scripting grows substantially, the runtime API, concurrency model, and isolation guarantees should be treated as an explicit subsystem design rather than as incremental helper additions.

### Response Body Persistence

Persisting decoded response bodies as history body files keeps detail inspection available without inflating the main SQLite rows. The trade-off is an additional file-retention responsibility tied to history pruning and app-data storage size.

Retention controls, migration behavior, and body-size policy should be revisited if response history becomes a storage-pressure source.

### Stable Updater Feed

Using GitHub Releases' stable `latest` updater manifest keeps update discovery predictable for normal users. The trade-off is that prerelease discovery is not part of the default update path.

Any prerelease channel should remain opt-in and should preserve the signed-update and target-selection guarantees already used by the stable path.
