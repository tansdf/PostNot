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

This section reflects the code currently implemented in the repository.

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
  - body: none, JSON, raw, form-urlencoded, multipart placeholder
- Native request execution through Rust
- Response viewer with:
  - status
  - duration
  - size
  - headers
  - body text / JSON pretty rendering
- Persisted application settings in SQLite
- Persisted request history in SQLite
- Settings page wired to backend persistence
- History panel wired to backend persistence
- History detail inspection from persisted snapshots
- Clear history action

### Not Yet Implemented

- Cancel in-flight request
- Collections
- Saved requests
- Environments and variable substitution
- Postman import/export
- Multi-tab workflow
- Pre-request scripts
- Test scripts
- Secret storage outside SQLite

## 4. High-Level Architecture

The app is split into two layers.

### Frontend

Responsibilities:

- Render request editor, response viewer, settings page, and history panel
- Manage page-level UI state
- Invoke typed Tauri commands for persistence and request execution
- Provide a desktop-oriented workflow without browser networking

The frontend does not execute HTTP requests directly. All network traffic goes through Rust.

### Native Layer

Responsibilities:

- Initialize SQLite database and run migrations
- Execute HTTP requests
- Load and persist settings
- Persist request history
- Resolve app data paths
- Expose a stable Tauri command surface to the UI

### Data Flow

1. User edits a request in the UI
2. Frontend builds a typed request payload
3. Frontend invokes `send_request`
4. Rust loads persisted request settings from SQLite
5. Rust executes the request with `reqwest`
6. Rust returns response metadata and body to the UI
7. Rust writes a history entry to SQLite
8. Frontend reloads history and renders the latest response

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
        history/
          HistoryPanel.svelte
        layout/
          AppShell.svelte
        request/
          RequestEditor.svelte
        response/
          ResponseViewer.svelte
      styles/
        tokens.css
        app.css
    routes/
      +layout.svelte
      +layout.ts
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
        requests.rs
        settings.rs
        history.rs
      db/
        mod.rs
      domain/
        mod.rs
        requests.rs
        settings.rs
        history.rs
      services/
        mod.rs
        http_client.rs
        settings_service.rs
        history_service.rs
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
- request timeout in milliseconds
- follow redirects flag
- validate TLS flag
- history limit

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
- `request_timeout_ms`
- `follow_redirects`
- `validate_tls`
- `history_limit`

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

### Tables Present But Not Yet Used By The UI

The initial migration also creates these tables for planned work:

- `collections`
- `collection_items`
- `environments`

They are not yet wired into the runtime UI or command surface.

## 8. Runtime Behavior

### Startup

At startup, the Tauri app:

1. resolves the app data directory
2. creates the SQLite database if missing
3. applies SQL migrations
4. ensures default settings exist
5. stores the SQLite pool in app state

### Request Execution

For each request send, Rust currently applies these persisted settings:

- `request_timeout_ms`
- `follow_redirects`
- `validate_tls`

This means the settings page already changes actual network behavior, not just UI state.

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

## 9. Current Tauri Command Boundary

Commands currently exposed to the frontend:

- `send_request`
- `get_settings`
- `update_settings`
- `list_history`
- `get_history_entry`
- `clear_history`

### Command Roles

- `send_request`: executes the request using persisted settings and records history
- `get_settings`: loads current settings from SQLite
- `update_settings`: persists settings and returns the saved values
- `list_history`: returns recent history entries ordered by execution time descending
- `get_history_entry`: returns a stored request snapshot and response metadata for one history entry
- `clear_history`: deletes all stored history entries

## 10. Frontend Screens

### Main Page

Current UI sections:

- request profile summary using persisted settings
- request editor
- response viewer
- history panel
- history detail inspector

### Settings Page

Current UI sections:

- theme selector
- request timeout input
- history limit input
- follow redirects toggle
- validate TLS toggle
- persisted save action

## 11. Security and Persistence Notes

Current state:

- the app is fully local
- requests are executed in Rust, not the browser
- sensitive values may still be stored in SQLite as plain application data

This is acceptable for the current milestone but not the final security posture.

Planned improvement:

- move secrets to OS keychain storage while leaving non-secret metadata in SQLite

## 12. Milestone Status

### Milestone 1 Goal

Ship a usable desktop app that can compose and execute HTTP requests locally, persist request behavior settings, and preserve request history across restarts.

### Milestone 1 Implemented So Far

- Tauri + SvelteKit app shell
- single request editor
- auth support for none/basic/bearer/API key
- request execution through Rust
- response viewer
- SQLite initialization and migrations
- persisted settings
- persisted history
- settings page
- history panel
- history detail inspection
- clear history action

### Milestone 1 Remaining

- request cancellation
- better multipart/file workflow
- actual runtime verification via `tauri dev`
- tighter error handling and UX polish

## 13. Next Recommended Steps

Recommended implementation order from the current state:

1. Run the app with `tauri dev` and verify end-to-end behavior manually
2. Add request cancellation support
3. Add saved requests and collections using the tables already present in the schema
4. Add environments and variable resolution
5. Add Postman import/export

## 14. Open Decisions

These are still unresolved:

- whether large response bodies should spill to files instead of SQLite preview-only storage
- whether tabs should persist in SQLite or only in frontend state at first
- exact import/export format for PostNot bundles
- how to model secrets before keychain integration

## 15. Recommendation

Treat the repository as being in an active Milestone 1 state, not full MVP completion.

The design is now grounded in what the code actually does: persisted settings influence request execution, history is stored in SQLite, and the frontend surfaces both. The next work should stay focused on completing Milestone 1 before opening collections, environments, or import/export.
