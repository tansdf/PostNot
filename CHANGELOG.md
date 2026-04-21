# Changelog

This changelog was reconstructed from the project chat history and is now the tracked release history for PostNot.

The project currently uses pre-1.0 semantic versioning. Minor versions mark meaningful product milestones, while patch versions should be used for smaller fixes and polish.

## [Unreleased]

## [0.17.4] - 2026-04-21

### Changed

- Unified the hydration-flash strategy across pages by introducing a shared synchronous `localStorage` cache (`src/lib/ui-cache.ts`) that seeds state on first paint instead of gating UI behind an `isHydrated` prop. This replaces the earlier "hide until hydrated" approach with "render with last-known-good state immediately, reconcile from backend."

### Fixed

- Requests page profile bar (request timeout, redirects, TLS, history limit) now renders with the last-known persisted values from the very first paint, eliminating the empty-to-populated flash users were still seeing after the 0.17.3 release.
- Active environment selector and variable-count hint on the Requests page now paint from a persisted cache, so the previously selected environment and its enabled-variable count are visible immediately on launch instead of appearing once the backend list loads.
- Request tabs strip and the request editor now paint from a cached workspace snapshot on launch, removing the brief "no tabs" and blank-editor flashes that remained after the 0.17.3 tab-strip gate.
- Sidebar collections list, its expand/collapse state, and items for previously expanded collections now render from cache on first paint, so the sidebar no longer flashes from a collapsed or empty state to the persisted layout.
- Collections page empty states (`Pick a collection…` and `Select a collection…`) no longer appear for a frame when a collection was already selected previously; the collections store is now seeded from cache so the correct selection paints immediately.
- Environments page list and active status badges now paint from cache on launch, eliminating the flicker of the empty `Create an environment…` state when environments are already present.
- History panel's collapsed/expanded state continues to be cached (now via the shared settings cache) and paints correctly on first frame.

## [0.17.3] - 2026-04-20

### Fixed

- History panel no longer briefly renders in the default expanded state on the Requests page before the persisted collapse setting is applied, eliminating the visible flash on launch.
- Sidebar collections no longer flash from fully collapsed to their persisted expanded state on launch; the expand/collapse chevrons and expanded folder trees are only rendered once the saved sidebar state has loaded.
- Request tabs strip no longer shows an empty "+" control and then pops in the restored tabs on launch; the strip now waits for the persisted workspace to load before rendering chips, and the request editor and response viewer are suppressed until the active tab is available, avoiding a flash of the default blank request.
- Theme and UI scale are now applied synchronously from a cached value at page load via an inline bootstrap script, preventing the brief flash of the wrong theme or zoom level on launch when the persisted preference differs from the system default.

## [0.17.2] - 2026-04-20

### Added

- Collapsible History panel on the Requests page, with the expanded/collapsed state persisted in app settings between launches.
- `Ctrl+S` / `Cmd+S` save shortcuts on the Requests and Environments pages, reusing the existing request save flow and manual environment save action.

### Changed

- Environment editing now autosaves by default after changes, with a persisted Settings toggle to disable autosave and keep manual saves only.

### Fixed

- Unsaved environment confirmation now also covers browser back/forward and other same-page environment switches when autosave is disabled.
- Clicking the currently selected environment no longer reloads it from storage without confirming that unsaved edits should be discarded.
- Environment detail loading no longer gets stuck after switching environments because no-op route sync passes no longer invalidate the active detail fetch.

## [0.17.1] - 2026-04-20

### Added

- Restore actions in the History panel and detail view that open a stored request snapshot in a new standalone request tab.

### Fixed

- History Detail no longer introduces an unwanted horizontal scrollbar after adding restore actions.

## [0.17.0] - 2026-04-17

### Added

- Restored multitab request workspace on the Requests page, including tab-local drafts, responses, and script output that persist between launches.

### Changed

- Opening saved requests from the sidebar or Collections page now activates an existing tab for that request or opens it in a new tab instead of replacing the current editor.
- The Requests page now keeps one native send in flight globally while disabling send in other tabs until the active request completes or is canceled.

## [0.16.0] - 2026-04-16

### Added

- OpenAPI 3 JSON/YAML import for collections from the Collections page and for single-operation request drafts from the main request editor.

### Changed

- Split the Rust import service into format-focused modules so Postman, OpenAPI, cURL, and shared helpers are easier to maintain without changing import behavior.

## [0.15.1] - 2026-04-16

### Added

- Shared `createStaleGuard()` helper for overlapping async route and detail loads.
- Modal focus management: initial focus, Tab cycle inside the dialog, Escape to close, and focus restore when the modal closes (save request, cURL import, collection import, environment import).
- Reuse of native `reqwest::Client` instances keyed by TLS, redirect, and timeout settings, with a bounded cache size.
- `log::warn` when rolling back the OS secret store after environment update/delete errors fails, so divergence is visible in diagnostics.

### Changed

- Collections, environments, and saved-request deep links coordinate in-flight loads so stale responses do not overwrite the UI; clearing `savedRequestId` from the URL resets deep-link tracking so the same request can load again.

## [0.15.0] - 2026-04-16

### Added

- Async scripting helper requests through `await pn.http.send(...)`, so inherited pre-request and test scripts can call the native sender for token/bootstrap workflows without polluting request history.
- Active-environment script writes through `await pn.variables.set(...)` and `await pn.variables.remove(...)`, including persisted secret writes through the existing environment storage path.

### Changed

- Pre-request and test scripts now run as awaited JavaScript in inheritance order, and `pn.test(...)` accepts async assertion callbacks.

## [0.14.1] - 2026-04-15

### Changed

- Simplified the request Scripts tab by removing inline API hint descriptions now covered by the scripting documentation.
- JSON response and history body viewers now soft-wrap long tokens instead of forcing horizontal scrolling.
- Tightened History panel spacing so the clear action, detail header, and close action align cleanly.

### Fixed

- Environment selection on the Environments page now keeps non-active environments open for editing instead of snapping back to the active environment.

## [0.14.0] - 2026-04-15

### Added

- Collection-level and folder-level pre-request and test scripts that run before saved-request scripts, with Postman top-level and folder event import/export support.
- Public GitHub Pages scripting documentation covering inherited execution order, current `pn` APIs, examples, Postman event portability, and current async-helper limitations.

## [0.13.1] - 2026-04-14

### Changed

- Tightened the saved-request Scripts editor layout with denser pre-request and test script headers plus side-by-side script cards on wider screens.
- Environments now use a more compact card-based browser with responsive multi-column behavior on larger windows and clean stacking on smaller widths.
- Settings now use a hand-shaped responsive layout with larger primary cards and a clearer secondary settings row instead of one generic grid.
- Reduced the desktop minimum window size by about 30% to make the shell usable in smaller windowed layouts.

## [0.13.0] - 2026-04-14

### Added

- Drag-and-drop request management in collection trees, including reordering saved requests, moving them into folders, and moving them across collections from the sidebar or Collections page.

### Changed

- Collection request moves now use one shared interaction model across the sidebar and Collections page, with matching root, folder, and sibling drop targets.

### Fixed

- Desktop collection drag-and-drop no longer depends on native HTML5 drag ghost behavior, avoiding broken visuals in WSLg/Linux webviews and unreliable drag starts in native Windows dev runs.

## [0.12.1] - 2026-04-13

### Added

- Shared folder SVG definitions in `src/lib/icons/folderPaths.ts` and a reusable `FolderGlyph` component for the sidebar and collection views.

### Changed

- Collection items UI distinguishes folders from saved requests more clearly: folder header block, tree guide lines, item counts, and reduced duplicate method/URL lines for unnamed requests.
- Collection detail editing resets reliably when switching collections via a keyed `CollectionDetailForm` instead of syncing drafts in an effect.
- Sidebar collections: nested folder contents use the same vertical guide style as the Collection Items panel, with tighter padding so content sits closer to the rule; expanded folder rows use the same left accent treatment as folder cards in Collection Items.
- Sidebar folder rows use a single open/closed folder icon (chevron removed); save-request folder targets use clearer root vs folder styling.
- Collection sidebar state uses `SvelteSet` without an extra `$state` wrapper, with hydration applied via set mutation instead of wholesale replacement.

### Fixed

- Script editor environment-variable deduplication avoids a raw `Set` instance in component scope (Svelte 5 autofixer hygiene).
- Request editor script placeholders use module constants so multiline examples parse correctly in markup.

## [0.12.0] - 2026-04-12

### Added

- Request scripting with saved pre-request and test script fields on requests.
- A PostNot scripting runtime exposed as `pn`, including request mutation helpers, environment variable access, response helpers, and basic assertion/test APIs.
- Script editing autocomplete for common `pn` APIs and active environment variable names.

### Changed

- Postman collection import and export now round-trip request pre-request and test scripts through item `event` blocks.
- Script authoring uses PostNot-branded `pn` helpers instead of Postman-style `pm` naming.

### Fixed

- Request sending no longer fails with `The object can not be cloned.` when pre-request scripts run against the current Svelte request draft state.
- Script autocomplete keyboard navigation now keeps the current selection stable while moving through suggestions.
- Script autocomplete now matches the active script context so test-only helpers are not suggested in pre-request scripts, and expectation-chain completions are reachable.
- Collection request detail loading now includes persisted script fields consistently across save/load and export paths.

## [0.11.0] - 2026-04-09

### Added

- Collection folders with nested sidebar and collection-view browsing, including root folders and subfolders for organizing saved requests.
- Folder-aware request saving so new saved requests can be placed directly at the collection root or inside a chosen folder.
- Persisted collection sidebar expansion state, so open and closed collections and folders are restored after restarting the app.

### Changed

- Postman collection import now recreates folder structure instead of flattening folder names into saved request titles.
- Postman collection export now preserves nested folders when writing Collection v2.1 JSON.

## [0.10.3] - 2026-04-07

### Fixed

- The Settings updater card now keeps the ready-to-install state visible during refreshes and preserves the backend install handoff if a later update check fails.
- The Settings page no longer fails to open when updater metadata includes an unexpected publish date value.

## [0.10.2] - 2026-04-07

### Added

- PostNot now performs a silent update check when the app opens, and the sidebar version pill shows a small upward indicator when a newer signed release is ready to install.

### Changed

- The Settings updates card now uses the shared updater state, so if a startup check already found a release it opens directly in the ready-to-install state instead of waiting for another manual check.

### Fixed

- The updater's `Last checked` timestamp is now persisted in app settings after successful checks instead of resetting every time the app restarts.

## [0.10.1] - 2026-04-06

### Fixed

- The Settings updater card now keeps the available-update details and install action in one explicit UI state, preventing Windows builds from getting stuck showing `Checking...` while the release text says an update is available but the `Install update` button is missing.

## [0.10.0] - 2026-04-06

### Added

- Variable-aware token highlighting in the request URL input, matching the overlay-based editor behavior used for the JSON body editor.
- Dedicated variable token coloring in the JSON body editor so `{{variable}}` placeholders are highlighted separately from normal JSON strings.
- Postman-style built-in dynamic variables at request runtime, including `$guid`, `$randomUUID`, `$timestamp`, `$isoTimestamp`, `$randomBoolean`, `$randomInt`, common random network/text helpers, and an extended `$randomAlphaNumeric[length]` form for explicit output length.

### Fixed

- Variable suggestion popovers now respect the viewport with their own scrolling instead of overflowing off-screen for longer variable lists.
- Undo and redo shortcuts now work in variable-aware editors, including the URL input plus raw and JSON request body fields.

## [0.9.3] - 2026-04-05

### Fixed

- The Settings updater card now uses one explicit UI phase for idle, checking, and installing states, preventing `Checking...` from getting stuck while update availability text is already shown.
- The `Install update` action is now rendered only when a newer version has actually been found, instead of appearing in a permanently disabled state before any check completes.

## [0.9.2] - 2026-04-05

### Changed

- Successful HTTP responses are always returned to the UI even when saving the run to history fails; a warning notification explains that history was not updated, including the underlying error text.
- When a request fails and logging that failure to history also fails, the UI still shows the original request error while a warning notification reports the history write problem (via a Tauri event from the shell).

### Fixed

- Release automation now publishes one combined `latest.json` updater manifest after all platform builds finish, preventing matrix jobs from overwriting each other with single-platform updater metadata.

## [0.9.1] - 2026-04-04

### Changed

- Internal navigation and sidebar links now use SvelteKit `resolve()` so URLs stay correct when the app is served under a subpath.
- Request body mode changes clear JSON and multipart field errors directly instead of using separate reactive effects.
- The environment variable field autocomplete uses declarative mirror text for caret measurement and tighter suggestion-index handling, improving alignment with Svelte 5 runes practice.
- Collection detail draft fields sync from the selected collection with a single consolidated effect.

## [0.9.0] - 2026-04-04

### Added

- In-app updater integration on the Settings page, including signed GitHub Release checks and native install handoff when a newer desktop build is available.
- Apache 2.0 licensing across the repository, package metadata, and desktop crate metadata.
- A dedicated [AGENTS.md](AGENTS.md) guide for coding agents and contributor tooling, separating implementation context from the public README.

### Changed

- Release builds are now configured to produce signed updater artifacts so GitHub Releases can feed the desktop updater.
- The Settings updater area now keeps persistent updater-specific error feedback inside the Updates card instead of duplicating it as a page-level error block.
- The public-facing README was rewritten for human readers and open-source discovery, while operational workspace details moved into `AGENTS.md`.

## [0.8.0] - 2026-03-31

### Added

- Paste-based and file-based import for Postman environment JSON on the dedicated `/environments` page, including an option to make the imported environment active immediately.
- One-click export for collections to Postman Collection v2.1 JSON and for environments to Postman environment JSON through native save dialogs.
- Global floating notifications with timed dismissal, hover pause/resume, progress indicators, manual close, and max-visible queueing for action feedback across the app.
- OS-backed secure storage for environment variables marked as secret, with masked editing controls plus reveal/copy actions in the environments editor.

### Changed

- Postman collection import now preserves exported query parameters more predictably when both `raw` URLs and structured query arrays are present.
- Postman collection import now recognizes JSON raw-body language metadata and multipart `formdata`, improving round-tripping for exported requests.
- Collections, environments, settings, request save/import flows, and history clearing now report completion through the shared notification system instead of page-local success text.
- The desktop window now restores its last size and position on launch, and reopens maximized if that was its last state when closed.
- Requests that use secret environment variables still execute with resolved values, but stored history snapshots now keep the original unresolved `{{variable}}` text instead of persisting the secret.
- Postman environment import/export now understands secret variables and exports them as named placeholders with blank values instead of plaintext.

### Fixed

- Saving settings now preserves the full `60-150%` interface zoom range instead of clamping back to the older `80-120%` limits.
- Refined secret-variable controls in the environments editor with better row alignment, clearer key/visibility icon states, and smoother inline action hierarchy.

## [0.7.0] - 2026-03-27

### Added

- Multipart request composition with text fields plus local file attachments selected through a native picker or entered as file paths.

### Changed

- History detail inspection now renders stored `form-urlencoded` and multipart request bodies as structured fields instead of falling back to the raw-body empty state.
- Updated the project docs to reflect that manual `tauri dev` verification has already been completed and multipart uploads are now part of the implemented request surface.

## [0.6.2] - 2026-03-26

### Added

- JSON body validation on blur: when the editor loses focus, invalid JSON is flagged with a compact error message below the editor that clears on re-focus.
- JSON syntax highlighting in the body editor using a transparent-textarea overlay technique with token-level coloring for keys, strings, numbers, booleans, null, and punctuation.
- Auto-indent on Enter and Tab indentation support in the JSON body editor.
- Format button for the JSON body that pretty-prints with two-space indentation.

### Fixed

- Eliminated doubled rounded corners on the JSON body editor by removing the overlay's inherited border and making the textarea the sole border source.

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
