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
- request cancellation
- collections and saved requests
- collections sidebar and dedicated collection view
- environments, variable resolution, and OS-backed secret storage for secret environment values
- Postman collection JSON import
- Postman environment JSON import
- Postman collection JSON export
- Postman environment JSON export
- cURL command import
- multipart request composition with file uploads
- app-level floating notifications for action feedback
- settings page wired to SQLite
- history panel wired to SQLite
- history detail inspection
- clear history action

Not implemented yet:

- Tauri updater integration
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

Manual end-to-end verification in `tauri dev` has already been completed for the current milestone state.

## Runtime Notes

- The app stores SQLite data under the Tauri app data directory.
- Saved settings currently affect request timeout, redirect behavior, and TLS validation.
- Request history is persisted after each send and pruned using the configured history limit.
- History summaries keep a preview in SQLite, and full response bodies for new entries are stored under the app data directory for detail inspection.
- Request cancellation is available while a request is in flight and canceled requests are not written to history.
- Collections are browsed from the sidebar, edited on `/collections`, and the request editor can save new requests or update the currently loaded saved request.
- Environments are managed on `/environments`, one environment can be active at a time, and `{{variable}}` placeholders are resolved during request execution.
- Environment variables marked as secret are stored in the OS credential store instead of SQLite, stay masked in the editor by default, and export with blank placeholder values instead of plaintext.
- `/environments` also supports paste-based and file-based import of Postman environment JSON, optional activation during import, and one-click export back to Postman environment JSON.
- `/collections` also supports paste-based import of Postman Collection v2.1 JSON, one-click export back to Postman Collection v2.1 JSON, and single cURL commands into PostNot collections.
- Request history still records requests that use secret environment variables, but the stored snapshot keeps the original unresolved `{{variable}}` text instead of persisting the resolved secret value.

## Versioning Policy

- PostNot uses pre-1.0 semantic versioning.
- Patch versions (`0.x.Y`) are for bug fixes, UI polish, and small internal improvements.
- Minor versions (`0.X.0`) are for meaningful user-facing milestones such as new workflows, command-surface growth, or persistence features.
- The changelog in [CHANGELOG.md](CHANGELOG.md) is the source of truth for release history.
- When bumping versions, keep `package.json`, `package-lock.json`, `src-tauri/Cargo.toml`, and `src-tauri/tauri.conf.json` in sync.

## Recommended Next Steps

1. Add Tauri updater integration.
2. Add collection folders and richer request organization.
3. Continue tightening error handling and desktop UX polish.
4. Evaluate multi-tab workflow and request-level productivity features.
5. Keep tightening import/export compatibility and desktop UX polish.
