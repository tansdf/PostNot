# Changelog

This changelog was reconstructed from the project chat history and is now the tracked release history for PostNot.

The project currently uses pre-1.0 semantic versioning. Minor versions mark meaningful product milestones, while patch versions should be used for smaller fixes and polish.

## [0.6.1] - 2026-03-26

### Changed

- Moved the AppShell wrapper into the root layout so the sidebar stays mounted across page navigations, preserving expanded collection state and scroll position.
- Applied HTTP method color badges to fallback request names in the sidebar and collections list when no custom name is set.
- Extended the interface zoom range from 80-120% to 60-150% for better flexibility on high-DPI and large displays.
- Added a version pill next to the app title in the sidebar header showing the current release.

### Fixed

- Sidebar method badge colors were overridden by the sidebar link span styles; added higher-specificity selectors with brighter tones tuned for the dark sidebar background.

## [0.6.0] - 2026-03-26

### Changed

- Migrated all Svelte components and stores from Svelte 4 patterns to Svelte 5 runes (`$state`, `$derived`, `$effect`, `$props`, `$bindable`), `onevent` attributes, `{@render}` snippets, and `$app/state`.
- Replaced the writable/derived collections store with a reactive `$state` class in a `.svelte.ts` module.
- Compacted the Request Profile panel into a slim horizontal summary bar above the request editor, freeing vertical space for the main workflow.
- Tightened desktop density across the entire UI: reduced border radii, panel padding, grid gaps, button heights, and input padding.
- Added HTTP method color coding (GET green, POST orange, PUT blue, PATCH purple, DELETE red) across history entries, sidebar collections, saved request lists, and the method selector.
- Strengthened the sidebar active navigation state with a filled background and left accent border replacing the previous subtle box-shadow.
- Compacted the sidebar brand block by removing the subtitle and reducing the heading size.
- Loaded IBM Plex Sans and JetBrains Mono from Google Fonts instead of relying on system fallbacks.

## [0.5.4] - 2026-03-25

### Added

- Persisted interface zoom control in Settings so desktop users can scale the PostNot UI up or down without relying on OS-level display changes.

### Changed

- Reduced the default desktop UI scale so the main request workflow fits more comfortably inside medium Windows window sizes.

### Fixed

- Windows sidebar collections header and cards now align more consistently with the primary page selector buttons.
- Interface zoom now scales the desktop shell inside a bounded viewport instead of causing the app content to overflow the window.

## [0.5.3] - 2026-03-25

### Changed

- Hardened desktop button rendering so shared controls align more consistently with the Linux styling across Windows and Linux builds.
- Refined History Detail section surfaces to reduce accidental-looking inner framing and make the inspector hierarchy feel more intentional.

### Fixed

- History entries no longer collapse into clipped header-only rows in the left split-view pane.
- History empty and selected detail states now size more predictably with full-width empty messaging and steadier scroll behavior.
- Expanded collections in the sidebar are now contained inside the sidebar shell instead of stretching the full app height on desktop layouts.

## [0.5.2] - 2026-03-23

### Changed

- Polished Windows/Tauri scroll areas with theme-aware custom scrollbar styling so dense split views feel more integrated with the app.

### Fixed

- Medium-width history entry overflow caused by long request URLs stretching the card and preview body width.
- Additional history split-view sizing issues in selected and empty detail states on packaged desktop builds.

## [0.5.1] - 2026-03-23

### Changed

- Refined the history split-view behavior so empty and selected detail states behave more intentionally on packaged desktop builds.
- Improved request editor consistency across body, auth, query, and import workflows with additional header and control polish.

### Fixed

- Windows packaged startup and installer behavior by embedding migrations and improving release diagnostics.
- Multi-platform release packaging issues around Tauri bundle icons and generated icon assets.
- History layout issues on Windows fullscreen and windowed builds, including URL truncation, overflow handling, and split-pane sizing.
- Request editor control alignment, custom checkbox styling, and query parameter URL synchronization polish.
- Collections import modal alignment and other packaged-build visual regressions.

## [0.5.0] - 2026-03-22

### Added

- Paste-based and file-based import for Postman Collection v2.1 JSON on the dedicated `/collections` page.
- cURL request import directly into the request editor via a dedicated import modal.
- `New` request action in the request editor header for clearing the current draft and saved-request binding.
- URL and query-parameter synchronization so pasted URLs unpack query params into the UI and active params are reflected back into the URL field.
- Variable-aware autocomplete and preview pills across URL, headers, query values, auth, and request body inputs.

### Changed

- Moved collection import behind a dedicated modal trigger instead of keeping the import block permanently visible on the Collections page.
- Refined the request editor section headers so `Request`, `Body`, and `Auth` follow a more consistent panel pattern.
- Added explicit empty-state messaging for body and auth sections when they are omitted.
- Updated the project docs to reflect that request import now supports Postman collections and cURL, while export remains future work.

### Fixed

- Windows packaged startup by embedding SQL migrations instead of depending on build-machine paths.
- Release packaging by restoring the generated Tauri icon set and explicit bundle icon configuration.
- Query/header/form row toggle alignment with a custom-styled checkbox that fits the app theme.
- Multiple request editor layout issues around save/send actions, body/auth headers, and narrow-width behavior.

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
