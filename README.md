# PostNot

Local-first desktop API client built with Rust, Tauri, SvelteKit, TypeScript, and SQLite.

## Start Here

If a new chat needs to recover context, use these files first:

- [docs/tech-design.md](docs/tech-design.md): current source of truth for implemented architecture and planned next steps
- [src/routes/+page.svelte](src/routes/+page.svelte): main request runner UI
- [src/routes/settings/+page.svelte](src/routes/settings/+page.svelte): persisted settings UI
- [src-tauri/src/lib.rs](src-tauri/src/lib.rs): Tauri startup and command registration
- [src-tauri/src/services/http_client.rs](src-tauri/src/services/http_client.rs): native request execution
- [src-tauri/src/services/settings_service.rs](src-tauri/src/services/settings_service.rs): SQLite-backed settings
- [src-tauri/src/services/history_service.rs](src-tauri/src/services/history_service.rs): SQLite-backed request history

## Correct Working Directory

Use the WSL repo path as the project root:

`/home/tansdf/gitreps/PostNot`

Windows-side equivalent:

`\\wsl.localhost\Ubuntu\home\tansdf\gitreps\PostNot`

Do not use the malformed WindowsApps resource path that may appear in some broken thread contexts.

## Current State

Implemented now:

- Tauri app shell
- SvelteKit frontend
- single request editor
- native HTTP execution in Rust
- SQLite initialization and migrations on startup
- persisted request settings
- persisted request history
- settings page wired to SQLite
- history panel wired to SQLite
- history detail inspection
- clear history action

Not implemented yet:

- request cancellation
- collections and saved requests
- environments and variable substitution
- Postman import/export
- scripts

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

## Runtime Notes

- The app stores SQLite data under the Tauri app data directory.
- Saved settings currently affect request timeout, redirect behavior, and TLS validation.
- Request history is persisted after each send and pruned using the configured history limit.
- History summaries keep a preview in SQLite, and full response bodies for new entries are stored under the app data directory for detail inspection.

## Recommended Next Steps

1. Run the app with `tauri dev` and verify end-to-end behavior.
2. Add request cancellation.
3. Add collections and saved requests.
4. Add environments and variable resolution.
5. Add Postman import/export.
