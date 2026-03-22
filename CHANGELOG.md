# Changelog

This changelog was reconstructed from the project chat history and is now the tracked release history for PostNot.

The project currently uses pre-1.0 semantic versioning. Minor versions mark meaningful product milestones, while patch versions should be used for smaller fixes and polish.

## [Unreleased]

### Added

- Paste-based import for Postman Collection v2.1 JSON on the dedicated `/collections` page.
- Paste-based import for single cURL commands into the selected collection or a fallback imported collection.

### Changed

- Updated the project docs to reflect that request import now supports Postman collections and cURL, while export remains future work.

## [0.4.0] - 2026-03-21

### Added

- Dedicated `/collections` view for collection editing and saved request management.
- Sidebar-first collections browser with expandable collection stacks.
- In-editor save flow with `Save` for new requests and `Update` for already saved requests.
- Save dialog collection picker that avoids native select popup issues.
- Dedicated `/environments` view for environment editing and activation.
- Active environment selection on the request page.
- Runtime `{{variable}}` resolution across URL, query params, headers, body, and auth fields.
- Reconstructed project changelog for ongoing release tracking.

### Changed

- Moved collections browsing out of the main request page and into the app sidebar.
- Changed collection creation to a Postman-style flow: create blank collection from the sidebar, then edit it in the collection view.
- Increased the default desktop window size from `1440x920` to `1520x980`.
- Improved sidebar request-card truncation so long URLs no longer stretch the layout or hide controls.
- Added generated multi-platform Tauri icon assets and refreshed them from the current app icon source.

### Fixed

- Multiple request/response layout issues in windowed and fullscreen states.
- Theme application across the app shell and settings flow.
- Sidebar expand/collapse behavior so browsing saved requests does not force navigation.
- Save-request UI alignment and modal interaction polish.

## [0.3.0] - 2026-03-20

### Added

- Flat collections model backed by SQLite.
- Saved request create, load, list, and delete flows.
- Collection create, list, and delete flows.
- Frontend collections panel and typed command bindings.

### Changed

- Expanded the Tauri command surface to support collection and saved request workflows.
- Updated the docs to treat collections and saved requests as part of the shipped Milestone 1 experience.

## [0.2.0] - 2026-03-20

### Added

- Native request cancellation with frontend `Cancel` and `Canceling...` states.
- History detail inspection, clear-history action, and split inspector UI.
- Full response body persistence for new history entries.

### Changed

- Improved response and history body panels with contained scrolling and more stable resizing behavior.
- Refined theme support for dark mode controls and selected states.

### Fixed

- Request cancellation no longer writes canceled entries to history.
- History inspection no longer jumps the page unexpectedly.
- Response and history viewers handle large content more predictably.

## [0.1.0] - 2026-03-20

### Added

- Initial tracked baseline for the Tauri desktop shell and SvelteKit frontend.
- Single-request editor with method, URL, query, headers, body, and auth controls.
- Native Rust HTTP execution.
- SQLite-backed settings and request history.
- Settings page and history panel.

### Notes

- This version reflects the baseline state found at the start of tracked chat-based development.
