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
- SvelteKit gives a productive UI layer without pulling in more framework overhead than needed
- SQLite gives durable local persistence without introducing a separate service
- `reqwest` keeps all HTTP execution in the native layer, which avoids browser CORS constraints and keeps behavior consistent

## 3. High-Level Architecture

The app is split into two layers:

### Frontend

Responsibilities:

- Render request editor and response viewer
- Manage tabs and local UI state
- Trigger Tauri commands for persistence and request execution
- Provide a responsive desktop workflow

The frontend does not perform network requests directly. All request execution flows through Rust.

### Native Layer

Responsibilities:

- Execute HTTP requests
- Read/write SQLite data
- Resolve app data paths
- Handle import/export
- Manage request history
- Provide a stable command API to the UI

### Data Flow

1. User edits a request in the UI
2. Frontend builds a typed request payload
3. Frontend invokes a Tauri command
4. Rust resolves environment variables and auth details
5. Rust executes the request with `reqwest`
6. Rust returns response metadata and body preview to the UI
7. Rust optionally stores a history entry
8. UI renders response and keeps tab state in sync

## 4. MVP Scope

### In Scope for MVP

- Desktop app for Windows, Linux, and macOS
- Request tabs
- HTTP methods: `GET`, `POST`, `PUT`, `PATCH`, `DELETE`, `HEAD`, `OPTIONS`
- URL editor
- Query parameters editor
- Headers editor
- Request body support:
  - none
  - raw text
  - JSON
  - `application/x-www-form-urlencoded`
  - `multipart/form-data` with file attachments
- Auth support:
  - none
  - basic
  - bearer token
  - API key
- Send and cancel request
- Response viewer:
  - status
  - duration
  - size
  - headers
  - body with text/JSON pretty view
- Save requests into collections and folders
- Environments with variable substitution
- Request history
- Import/export in PostNot format
- Import of Postman Collection v2.1 JSON
- Light settings page

### Explicit Non-Goals for MVP

- Cloud sync
- Team collaboration
- Shared workspaces
- Mock servers
- Monitors
- GraphQL-specialized tooling
- WebSocket client
- gRPC client
- OAuth device/browser flows
- Full Postman-compatible scripting runtime
- Plugin marketplace

### Deferred but Planned

- Pre-request scripts
- Test scripts
- Cookie manager
- OAuth2 helper flows
- Keychain-backed secret storage
- OpenAPI import

## 5. Folder Structure

Proposed repository layout:

```text
PostNot/
  docs/
    tech-design.md
  src/
    app.html
    lib/
      components/
        layout/
        request/
        response/
        collections/
        environments/
        shared/
      stores/
        tabs.ts
        collections.ts
        environments.ts
        history.ts
        settings.ts
      api/
        commands.ts
        types.ts
      utils/
        variables.ts
        formatting.ts
        validation.ts
      styles/
        tokens.css
        app.css
    routes/
      +layout.svelte
      +page.svelte
      settings/
        +page.svelte
  src-tauri/
    Cargo.toml
    tauri.conf.json
    build.rs
    capabilities/
    icons/
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
        collections.rs
        environments.rs
        history.rs
        settings.rs
        import_export.rs
      db/
        mod.rs
        migrations.rs
      domain/
        mod.rs
        requests.rs
        collections.rs
        environments.rs
        history.rs
        settings.rs
      services/
        mod.rs
        http_client.rs
        variable_resolver.rs
        collection_service.rs
        environment_service.rs
        history_service.rs
        settings_service.rs
        import_export_service.rs
      storage/
        mod.rs
        paths.rs
  static/
  package.json
  svelte.config.js
  tsconfig.json
  vite.config.ts
```

### Structure Notes

- `src/lib/api` contains the TypeScript boundary for Tauri commands and shared payload types
- `src/lib/stores` contains app state stores used across pages and panels
- `src-tauri/src/commands` is the UI-facing command layer
- `src-tauri/src/domain` contains pure data models and DTOs
- `src-tauri/src/services` contains business logic
- `src-tauri/migrations` keeps SQL schema changes explicit and versioned

## 6. Core Domain Model

Primary entities:

- Collection
- Collection Item
- Environment
- Environment Variable
- Request Snapshot
- History Entry
- App Setting

### Collection

A named container for folders and saved requests.

### Collection Item

A tree node within a collection.

Kinds:

- folder
- request

### Environment

A named set of key/value variables used for substitution in URLs, headers, auth, and bodies.

### Request Snapshot

A full persisted representation of a request at a point in time. This is used both for saved requests and for history entries.

### History Entry

A past execution record containing a request snapshot plus response summary metadata.

## 7. SQLite Storage Design

The schema should stay simple and use JSON columns where nesting is natural. That gives us a stable relational spine without over-normalizing early.

### Database Location

Use the Tauri app data directory, for example:

- database file: `<app_data_dir>/postnot.sqlite`
- exported files: user-selected path
- temporary multipart files: OS temp directory

### Tables

#### `collections`

```sql
CREATE TABLE collections (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  description TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
```

#### `collection_items`

Stores both folders and requests.

```sql
CREATE TABLE collection_items (
  id TEXT PRIMARY KEY,
  collection_id TEXT NOT NULL,
  parent_id TEXT NULL,
  kind TEXT NOT NULL CHECK (kind IN ('folder', 'request')),
  name TEXT NOT NULL,
  sort_order INTEGER NOT NULL DEFAULT 0,

  method TEXT NULL,
  url TEXT NULL,
  query_params_json TEXT NOT NULL DEFAULT '[]',
  headers_json TEXT NOT NULL DEFAULT '[]',
  body_json TEXT NOT NULL DEFAULT '{}',
  auth_json TEXT NOT NULL DEFAULT '{}',
  prerequest_script TEXT NOT NULL DEFAULT '',
  test_script TEXT NOT NULL DEFAULT '',

  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,

  FOREIGN KEY (collection_id) REFERENCES collections(id) ON DELETE CASCADE,
  FOREIGN KEY (parent_id) REFERENCES collection_items(id) ON DELETE CASCADE
);
```

Notes:

- Folder rows keep request-specific fields empty
- Request rows keep all request fields populated
- `body_json` stores body mode and payload metadata
- `auth_json` stores auth mode plus its configuration

#### `environments`

```sql
CREATE TABLE environments (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  is_active INTEGER NOT NULL DEFAULT 0,
  variables_json TEXT NOT NULL DEFAULT '[]',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
```

Notes:

- `variables_json` contains objects like `{ key, value, enabled, secret }`
- Only one environment should be active at a time in MVP

#### `history_entries`

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

Notes:

- Large response bodies should be stored on disk if needed instead of bloating the DB
- `response_body_preview` stores a short preview for fast history rendering

#### `app_settings`

```sql
CREATE TABLE app_settings (
  key TEXT PRIMARY KEY,
  value_json TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
```

Suggested keys:

- `theme`
- `font_size`
- `follow_redirects`
- `validate_tls`
- `request_timeout_ms`
- `history_limit`
- `last_opened_collection_id`

### Indexes

```sql
CREATE INDEX idx_collection_items_collection_id
  ON collection_items(collection_id);

CREATE INDEX idx_collection_items_parent_id
  ON collection_items(parent_id);

CREATE INDEX idx_history_entries_executed_at
  ON history_entries(executed_at DESC);

CREATE INDEX idx_environments_is_active
  ON environments(is_active);
```

## 8. Request Data Shapes

These typed shapes should exist in both Rust and TypeScript.

### Query Param

```json
{
  "key": "page",
  "value": "1",
  "enabled": true
}
```

### Header

```json
{
  "key": "Accept",
  "value": "application/json",
  "enabled": true
}
```

### Body

```json
{
  "mode": "json",
  "raw": "{\"hello\":\"world\"}",
  "form": [],
  "files": []
}
```

### Auth

```json
{
  "type": "bearer",
  "bearerToken": "{{api_token}}",
  "basic": null,
  "apiKey": null
}
```

## 9. Command Boundary

Initial Tauri commands:

- `send_request`
- `cancel_request`
- `list_collections`
- `get_collection`
- `save_collection_item`
- `delete_collection_item`
- `reorder_collection_items`
- `list_environments`
- `save_environment`
- `set_active_environment`
- `delete_environment`
- `list_history`
- `delete_history_entry`
- `clear_history`
- `get_settings`
- `update_setting`
- `import_postman_collection`
- `export_postnot_bundle`

Guidelines:

- UI commands should exchange typed DTOs, not raw SQL-shaped objects
- Commands should be thin and delegate to services
- Services should be testable without the Tauri runtime

## 10. Variable Resolution Rules

Variable syntax:

- `{{variable_name}}`

Resolution order for MVP:

1. Active environment variables
2. Future collection-scoped variables
3. Future globals

Rules:

- Disabled variables are ignored
- Missing variables are left unresolved and highlighted in the UI
- Secret variables are masked in the UI where possible

## 11. Security and Persistence Notes

For MVP:

- The app is fully local
- Requests are executed in Rust, not in a browser context
- Sensitive values may exist in SQLite for now

Follow-up improvement:

- Move secret values to OS keychain storage while keeping non-secret metadata in SQLite

This is the main security compromise in MVP and should be called out clearly.

## 12. Milestone Plan

Implementation should be staged to keep the app runnable early.

### Milestone 1: Local Request Runner

Goal:

Ship a usable desktop app that can compose and execute HTTP requests locally and inspect responses.

Included:

- Tauri + SvelteKit app shell
- Single request tab
- Method, URL, headers, query params, and body editors
- Auth support for none/basic/bearer/API key
- Send request through Rust
- Response viewer with status, time, headers, and body
- Basic app settings in SQLite
- Request history in SQLite

Excluded from Milestone 1:

- Saved collections
- Environments UI
- Postman import
- Multiple tabs
- Scripts

Acceptance criteria:

- User can launch the app without any external server
- User can send a JSON request to a public API endpoint
- User can inspect response status, headers, and prettified JSON body
- User can cancel an in-flight request
- History persists across app restarts
- App settings persist across app restarts

### Milestone 2: Saved Requests and Environments

Included:

- Collections and folders
- Save/update/delete request definitions
- Environments and variable resolution
- Multiple tabs

### Milestone 3: Import/Export

Included:

- PostNot bundle export/import
- Postman Collection v2.1 import
- Better history browsing

### Milestone 4: Scripting and Polish

Included:

- Pre-request scripts
- Test scripts
- Secret storage improvements
- Better UX polish and keyboard flows

## 13. Implementation Order for Scaffold

When we scaffold the repo, build in this order:

1. Create Tauri + SvelteKit + TypeScript base app
2. Add Rust command bridge and typed frontend API layer
3. Add SQLite setup and initial migration
4. Implement `send_request`
5. Implement request editor UI
6. Implement response viewer UI
7. Persist settings and history
8. Add collections and environments in the next pass

## 14. Open Decisions

These can be deferred until after scaffold:

- Whether to use Tailwind or plain CSS
- Whether response bodies over a threshold should always spill to files
- Exact import/export bundle format
- Whether tab state should persist in SQLite or only in frontend state for MVP

## 15. Recommendation

Proceed with scaffolding against Milestone 1 first.

That gives us a thin but real desktop API client quickly, while leaving room for collections and environments without reworking the architecture.
