# PostNot Agent Guide

Operational context for coding agents and contributor tooling. This file is the agent-facing companion to the public [README.md](README.md).

## Repository Role

PostNot is a local-first desktop API client built with:

- Rust
- Tauri 2
- SvelteKit
- TypeScript
- SQLite

The app already supports request execution, resolved request preview before send, full response body reads, history, collections with nested folders and drag-and-drop request/folder moves, sidebar collection search, raw WebSocket and Socket.IO connection workspaces with session-only transcripts, playbooks for sequential saved-request execution, environments, secret environment storage, mixed PostNot collection portability, import/export flows including OpenAPI 3 import, broader cURL flag coverage, redacted-by-default single-request cURL/JSON export with optional non-secret environment variable inclusion, OAuth2 bearer auth helpers with client-credentials token fetch, notifications, settings, signed in-app update checks with download progress, inherited collection/folder/saved-request pre-request and test scripts (worker-backed frontend JavaScript execution around the native send), async script helper requests through `pn.http.send(...)`, script-driven active-environment variable writes, and a local authoring-only MCP server with Agent Activity.

## Canonical Working Directory

Use this repo path as the project root:

`/home/tansdf/gitreps/PostNot`

Windows-side equivalent:

`\\wsl.localhost\Ubuntu\home\tansdf\gitreps\PostNot`

Do not use malformed WindowsApps-style paths if they appear in broken thread context.

## Start Here

When onboarding into a fresh task, read these first:

- [docs/tech-design.md](docs/tech-design.md): architecture, runtime behavior, persistence, command boundaries, and design trade-offs
- [docs/design-system.md](docs/design-system.md): application design language, reusable UI patterns, accessibility contract, and feature design checklist
- [src/routes/+page.svelte](src/routes/+page.svelte): main request runner UI
- [src/routes/websockets/+page.svelte](src/routes/websockets/+page.svelte): WebSocket and Socket.IO connection workspace
- [src/routes/settings/+page.svelte](src/routes/settings/+page.svelte): persisted settings UI and updater surface
- [src-tauri/src/lib.rs](src-tauri/src/lib.rs): Tauri startup and command registration
- [src-tauri/src/services/http_client.rs](src-tauri/src/services/http_client.rs): native request execution
- [src-tauri/src/services/realtime_service.rs](src-tauri/src/services/realtime_service.rs): app-wide raw WebSocket session manager
- [src-tauri/src/services/realtime_socketio_service.rs](src-tauri/src/services/realtime_socketio_service.rs): Socket.IO transport adapter
- [src-tauri/src/services/settings_service.rs](src-tauri/src/services/settings_service.rs): persisted settings
- [src-tauri/src/services/history_service.rs](src-tauri/src/services/history_service.rs): request history
- [src-tauri/src/services/environments_service.rs](src-tauri/src/services/environments_service.rs): environments, secret redaction, variable resolution
- [src-tauri/src/mcp.rs](src-tauri/src/mcp.rs): headless stdio MCP tools, safe request projection, and mutation auditing

## Application Capabilities

Implemented now:

- headless `PostNot --mcp` authoring and inspection tools with no listening port
- persistent Agent Activity, live collection invalidation, and optimistic saved-request updates

- Tauri desktop shell
- native HTTP execution in Rust
- native raw WebSocket and Socket.IO 3.x/4.x execution with application-wide session ownership
- persistent disconnected WebSockets tab workspace, bounded session-only transcripts, file-backed large payloads, and opt-in reconnect
- saved HTTP, WebSocket, and Socket.IO definitions in shared collection trees with protocol-aware routing
- lossless mixed PostNot collection import/export and explicit realtime omissions from Postman export
- persisted settings
- persisted history with detail inspection
- restoring stored requests from history into new request tabs
- request cancellation
- resolved outgoing request preview before send, with private values masked
- collections and saved requests with nested folders and drag-and-drop request/folder moves across the sidebar and Collections page
- sidebar collection search across collections, folders, and saved requests
- playbooks with ordered saved-request steps, delays, stop-on-failure execution, and grouped run logs
- environments with variable resolution
- OS-backed secret storage for secret environment variables
- Postman collection import/export
- Postman environment import/export
- OpenAPI 3 collection import and single-request draft import
- cURL import
- broader cURL import coverage for common flags such as `--url`, `--get`, repeated `--data`, `--form`, cookies, compression, redirects, and shell continuations
- single-request export from the Requests page as cURL or PostNot request JSON, redacted by default with optional active non-secret environment variable inclusion
- OAuth2 bearer auth fields and a request-editor client-credentials token fetch action
- multipart request composition with local file uploads
- built-in dynamic request variables
- pre-request and test scripts on collections, folders, and saved requests (`request-scripts.ts`, `request-script-worker.ts`, `ScriptEditor.svelte`)
- async script helper requests through `await pn.http.send(...)`
- script-driven active-environment variable writes, including persisted secret writes
- full response body reads with JSON-friendly response rendering
- floating notifications
- signed in-app update checks with silent startup refresh and download progress
- window size and position restore
- collections sidebar and collections panel share folder tree guides and `FolderGlyph` / `folderPaths` icons

## Validation

Frontend:

```bash
npm run check
```

Browser-mode application UX:

```bash
npm run test:app-e2e
```

Marketing site:

```bash
npm run docs:validate
npm run docs:test-site
npm run docs:check-screenshots
```

Rust:

```bash
source "$HOME/.cargo/env"
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
```

The Socket.IO integration test starts the pinned Node fixture in `src-tauri/tests/fixtures/socketio-server.mjs`, so install the repository's npm dependencies before running the full Rust suite.

Backend quality gate (required before publishing a release):

```bash
source "$HOME/.cargo/env"
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- \
  -D warnings \
  -A clippy::manual_map \
  -A clippy::new_without_default \
  -A clippy::manual_is_multiple_of \
  -A clippy::items_after_test_module \
  -W clippy::await_holding_lock \
  -W clippy::large_futures \
  -W clippy::needless_collect
```

Rustup automatically uses the repository's pinned toolchain from `rust-toolchain.toml`.

Run the desktop app locally with:

```bash
npm run tauri dev
```

## Windows Dev Run

When a task needs a true native Windows run instead of WSL or WSLg:

- install the standard Windows toolchain first: Node.js, Rust via `rustup`, and Visual Studio Build Tools with the C++ workload
- avoid running the Windows toolchain directly against the WSL UNC repo path; instead, copy or mirror the repo into a normal local Windows working directory
- open a regular PowerShell session, or a Visual Studio developer shell if the compiler environment is missing
- from that local Windows copy, run `npm install`, then `npm run check`, then `npm run tauri dev`
- if Tauri fails looking for the MSVC toolchain, verify `cl.exe` is available in the current shell before retrying

This approach is mainly for native Windows verification such as drag-and-drop, windowing, or WebView behavior that can differ from WSLg.

## Runtime Notes

- SQLite data lives under the Tauri app data directory.
- Secret environment values are stored in the OS credential store, not SQLite.
- History persists requests that use secret environment variables, but stores unresolved `{{variable}}` text instead of resolved secret values.
- Single-request exports redact credential-looking values by default, including bearer tokens, OAuth2 access tokens, client secrets, API keys, cookies, and basic-auth passwords; the export dialog can include active non-secret environment variables while keeping secrets redacted.
- Realtime connection definitions and open tabs are managed on `/websockets`; navigation preserves live native sessions, while app restart restores drafts disconnected and clears transcripts.
- Realtime transcripts are bounded, process-scoped, and never written to SQLite history. Payloads over 256 KiB use temporary opaque handles that are cleared on release or startup.
- Realtime v1 does not run collection/folder/request scripts or Playbook steps and does not support durable history, legacy Socket.IO 2.x, custom CA/mTLS/proxy settings, `permessage-deflate`, server-requested ACK replies, or mixed binary placeholder arrays.
- Collections are managed on `/collections`.
- Playbooks are managed on `/playbooks`.
- Environments are managed on `/environments`.
- Settings are managed on `/settings`.
- The updater is wired to GitHub Releases at `https://github.com/tansdf/PostNot/releases/latest/download/latest.json`.
- Because that endpoint follows GitHub's stable `latest` release, prereleases are not discovered by the in-app updater.

## Release Notes For Agents

- Keep versions in sync across:
  - [package.json](package.json)
  - [package-lock.json](package-lock.json)
  - [src-tauri/Cargo.toml](src-tauri/Cargo.toml)
  - [src-tauri/tauri.conf.json](src-tauri/tauri.conf.json)
  - [vite.config.ts](vite.config.ts) for `__APP_VERSION__`
- [CHANGELOG.md](CHANGELOG.md) is the source of truth for release history.
- The release workflow is in [.github/workflows/release.yml](.github/workflows/release.yml).
- Signed updater artifacts require the `TAURI_SIGNING_PRIVATE_KEY` GitHub secret, plus `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` if the key is password protected.

## Repo Hygiene

- Use [README.md](README.md) for customer-facing messaging.
- Keep [AGENTS.md](AGENTS.md) focused on implementation, release, and workspace context.
- Keep [docs/tech-design.md](docs/tech-design.md) aligned with actual code behavior, not aspirational architecture.
