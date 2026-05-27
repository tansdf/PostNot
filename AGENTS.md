# PostNot Agent Guide

Operational context for coding agents and contributor tooling. This file is the agent-facing companion to the public [README.md](README.md).

## Repository Role

PostNot is a local-first desktop API client built with:

- Rust
- Tauri 2
- SvelteKit
- TypeScript
- SQLite

The app already supports request execution, resolved request preview before send, full response body reads, history, collections with nested folders and drag-and-drop request/folder moves, sidebar collection search, playbooks for sequential saved-request execution, environments, secret environment storage, import/export flows including OpenAPI 3 import, broader cURL flag coverage, redacted-by-default single-request cURL/JSON export with explicit full export, OAuth2 bearer auth helpers with client-credentials token fetch, notifications, settings, signed in-app update checks with download progress, inherited collection/folder/saved-request pre-request and test scripts (worker-backed frontend JavaScript execution around the native send), async script helper requests through `pn.http.send(...)`, and script-driven active-environment variable writes.

## Canonical Working Directory

Use this repo path as the project root:

`/home/tansdf/gitreps/PostNot`

Windows-side equivalent:

`\\wsl.localhost\Ubuntu\home\tansdf\gitreps\PostNot`

Do not use malformed WindowsApps-style paths if they appear in broken thread context.

## Start Here

When onboarding into a fresh task, read these first:

- [docs/tech-design.md](docs/tech-design.md): current architecture and implementation state
- [src/routes/+page.svelte](src/routes/+page.svelte): main request runner UI
- [src/routes/settings/+page.svelte](src/routes/settings/+page.svelte): persisted settings UI and updater surface
- [src-tauri/src/lib.rs](src-tauri/src/lib.rs): Tauri startup and command registration
- [src-tauri/src/services/http_client.rs](src-tauri/src/services/http_client.rs): native request execution
- [src-tauri/src/services/settings_service.rs](src-tauri/src/services/settings_service.rs): persisted settings
- [src-tauri/src/services/history_service.rs](src-tauri/src/services/history_service.rs): request history
- [src-tauri/src/services/environments_service.rs](src-tauri/src/services/environments_service.rs): environments, secret redaction, variable resolution

## Current Product State

Implemented now:

- Tauri desktop shell
- native HTTP execution in Rust
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
- single-request export from the Requests page as cURL or PostNot request JSON, redacted by default with an explicit full-export toggle
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

Still intentionally open:

- multi-tab workflow decisions
- deeper scripting beyond the shipped async helper and environment-write surface (broader runtime API, richer inherited execution controls)
- additional UX polish and error handling
- updater channel decision for prereleases vs stable-only discovery

## Validation

Frontend:

```bash
npm run check
```

Rust:

```bash
source "$HOME/.cargo/env"
cargo check --manifest-path src-tauri/Cargo.toml
```

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
- Single-request exports redact credential-looking values by default, including bearer tokens, OAuth2 access tokens, client secrets, API keys, cookies, and basic-auth passwords; the export dialog has an explicit full-export toggle.
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
