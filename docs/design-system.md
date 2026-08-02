# PostNot Design System

Implementation guide for maintaining and extending PostNot's application UI. This document describes the system that exists in the product today, the conventions new work must follow, and the known areas where the system should become more consistent.

This is an internal product and engineering reference. Customer-facing visual language belongs in [README.md](../README.md) and the public site.

## 1. Design Direction

PostNot should feel like a focused desktop workbench: warm rather than clinical, dense enough for technical work, and calm even when the request or response data is complex.

The interface follows these principles:

1. **Clarity before decoration.** Request state, hierarchy, and the next action must be obvious without relying on ornament.
2. **Consistency over novelty.** Reuse existing tokens, controls, panels, and interaction patterns before adding a new variant.
3. **Compact, not cramped.** Preserve the current desktop density while keeping labels, focus states, and click targets usable.
4. **Progressive disclosure.** Keep common request work visible and move advanced settings, scripts, raw details, and destructive choices behind explicit controls.
5. **Local-first confidence.** Explain persistence, secret handling, imports, exports, and irreversible actions at the point where they matter.
6. **State must not depend on color alone.** Pair color with text, icons, shape, or position for status and selection.
7. **Light and dark are one system.** New UI is incomplete until both themes have been considered.

## 2. Source of Truth

Use this order when implementation and documentation disagree:

1. [`src/lib/styles/tokens.css`](../src/lib/styles/tokens.css) for semantic visual values and theme overrides.
2. [`src/lib/styles/app.css`](../src/lib/styles/app.css) for shared components, layouts, states, and responsive behavior.
3. Shared Svelte components under [`src/lib/components`](../src/lib/components) for structure and behavior.
4. This guide for intent, usage rules, and how to extend the system.

Feature-local `<style>` blocks are allowed for genuinely feature-specific composition. They must use shared tokens and should not recreate a shared control, panel, status, modal, or feedback pattern.

## 3. Foundations

### 3.1 Color

Always consume semantic tokens. Do not choose a literal color based only on its appearance in one theme.

| Intent | Token | Use |
|---|---|---|
| Application canvas | `--bg-app` | Root window background |
| Panel | `--bg-panel` | Standard panels and notification surfaces |
| Strong panel | `--bg-panel-strong` | Inputs and surfaces needing separation |
| Sidebar | `--bg-sidebar` | Primary navigation background |
| Primary action | `--bg-accent` | Send, save, and other dominant actions |
| Primary action hover | `--bg-accent-strong` | Hover and stronger accent emphasis |
| Primary button | `--button-primary-bg`, `--button-primary-bg-hover` | Accessible filled-button backgrounds per theme |
| Primary button text | `--button-primary-text` | Theme-specific text with at least 4.5:1 contrast on primary buttons |
| Accent tint | `--bg-accent-soft` | Selected or highlighted surfaces |
| Subtle/quiet surface | `--surface-subtle`, `--surface-muted` | Nested cards and low-emphasis grouping |
| Control surface | `--control-bg`, `--control-hover-bg` | Secondary controls and interactive rows |
| Selected control | `--control-selected-bg`, `--control-selected-text` | Active tabs and selected toggles |
| Primary text | `--text-primary` | Titles and main content |
| Secondary text | `--text-secondary` | Labels, descriptions, and metadata |
| Muted text | `--text-muted` | Tertiary details; never essential information alone |
| Inverse text | `--text-inverse` | Text on the sidebar and other dark surfaces |
| Borders | `--border-soft`, `--border-strong` | Default and emphasized boundaries |
| Success | `--success` | Completed or valid state |
| Warning | `--warning` | Caution and recoverable risk |
| Danger | `--danger` | Failures, destructive actions, and invalid state |
| Destructive action | `--danger-bg`, `--danger-bg-hover`, `--danger-text` | Filled destructive buttons that need theme-specific contrast |
| Code surface | `--bg-code` | JSON, scripts, raw bodies, paths, and logs |
| Overlay | `--overlay-backdrop`, `--shadow-overlay` | Dialog backdrops and floating-layer elevation |
| HTTP methods | `--method-*`, `--method-*-inverse` | Method labels on normal and dark/sidebar surfaces |
| Syntax | `--syntax-*` | JSON, scripts, and variable highlighting |
| Realtime status | `--success`, `--warning`, `--danger`, `--text-muted` | Connected, transitional, failed, and disconnected indicators paired with text |

The light palette uses cream surfaces, deep teal text/navigation, and burnt orange action color. The dark palette preserves those relationships with higher-luminance text and accent values. The optional Forest theme uses the same semantic roles with a deeper green workbench palette and a cooler green accent. Never copy a resolved light-theme value into component CSS.

HTTP method colors are a special categorical palette. Use the existing `.method-get`, `.method-query`, `.method-post`, `.method-put`, `.method-patch`, `.method-delete`, `.method-head`, and `.method-options` classes. Do not use those colors for semantic success or failure.

Realtime protocol labels are compact categorical identifiers, not health states. Use the shared `.protocol-badge` with the exact short labels `WS` and `S.IO`; use `.realtime-status-*` only for connection state. A status dot must always be paired with visible or screen-reader text.

### 3.2 Typography

| Role | Token | Current size |
|---|---|---|
| Page title | `--font-page-title` | `1.45rem` |
| Panel title | `--font-panel-title` | `1.05rem` |
| Section title | `--font-section-title` | `0.95rem` |
| Label | `--font-label` | `0.8rem` |
| Body/control | `--font-body` | `0.9rem` |
| Metadata | `--font-meta` | `0.8rem` |
| Code/data | `--font-code` | `0.86rem` |

- Use `--font-sans` for interface copy and `--font-mono` for code, URLs when scanning benefits, paths, JSON, scripts, and raw payloads.
- Use sentence case for headings, labels, actions, tabs, and menu items.
- Buttons should begin with a verb: “Save request”, “Create environment”, “Clear history”.
- Keep help text direct and explain consequences, not the control label again.
- Use an ellipsis only when an action opens a flow requiring more input.

### 3.3 Spacing and Density

Use the compact spacing scale for shared primitives and new layouts:

| Token | Value | Typical use |
|---|---:|---|
| `--space-1` | `4px` | Icon/text gaps and micro-adjustments |
| `--space-2` | `8px` | Tightly related content |
| `--space-3` | `12px` | Controls in a row and compact lists |
| `--space-4` | `16px` | Cards, form groups, and dialog gaps |
| `--space-5` | `20px` | Panel and dialog padding |
| `--space-6` | `24px` | Major internal separation |

Existing intermediate values may remain where density or geometry requires them. Do not introduce another spacing value without a concrete layout constraint.

The application supports a UI scale from `0.6` to `1.5` through `--ui-scale`. Validate fixed, sticky, and overlay UI at the default and at both extremes.

### 3.4 Shape, Border, and Elevation

| Role | Token |
|---|---|
| Large container | `--radius-lg` (`14px`) |
| Standard group | `--radius-md` (`10px`) |
| Input/small surface | `--radius-sm` (`8px`) |
| Panel | `--radius-panel` (`12px`) |
| Card/control | `--radius-card`, `--radius-control` (`8px`) |
| Compact icon control | `--radius-compact` (`6px`) |
| Chip/pill | `--radius-pill` |

Use `--border-soft` for structure and `--border-strong` for selection or emphasis. Prefer borders and subtle surface shifts to shadows for hierarchy. Reserve `--shadow-soft` and overlay shadows for floating layers such as dialogs and notifications.

### 3.5 Motion

Use `--motion-fast` (`140ms`) for small state changes and `--motion-base` (`180ms`) for controls and overlays, with `--ease-standard`. Motion should explain state change, not delay work. Do not animate large data regions or introduce looping animation except for active progress/loading feedback.

The shared reduced-motion rule removes decorative transitions and pulse/slide animations while preserving static loading state and notification expiry. `NotificationHost.svelte` also disables its JavaScript fly/fade movement when reduced motion is requested; future JavaScript transitions must make the same explicit check because CSS alone does not control them.

## 4. Layout and Information Hierarchy

### Application shell

`AppShell.svelte` owns the two-column desktop frame: a `320px` sidebar and a flexible workspace. At widths below `980px`, it becomes a single flowing column. The sidebar has three stable zones: compact workspace switching for Requests, WebSockets, and Playbooks; a Collections section whose heading opens the full Collections workspace and whose tree owns the remaining height; and anchored utilities for Environments, MCP Activity, and Settings. Every destination requires an explicit non-color-only active state and an accessible name, including icon-only utility controls.

### Pages

Use this hierarchy:

1. Page heading and optional short description.
2. Primary actions aligned with the heading where space allows.
3. Panels for major work areas.
4. Cards or field groups only when they clarify a relationship.

Avoid nesting decorative panels more than two levels deep. A border, section title, or spacing change is often sufficient.

### Responsive behavior

The application is desktop-first but must remain operable in a narrow window. Existing shared breakpoints are `1720`, `1500`, `1220`, `980`, `900`, and `720px`; feature-local layouts may use a nearby constraint only when their content requires it.

- Collapse multi-column detail and form layouts to one column before controls become compressed.
- Allow toolbars to wrap.
- Keep primary actions visible and avoid horizontal page scrolling.
- Give code/data regions their own bounded scrolling rather than expanding the entire page without limit.
- Dialogs must use `dvh` fallbacks and retain reachable headers/actions.

## 5. Shared Components

### 5.1 Buttons

| Class/variant | Use when |
|---|---|
| `.button-primary` | The single dominant action in a section or dialog |
| `.button-secondary` | A supporting action with visible affordance |
| `.button-ghost` | Low-emphasis or tertiary action |
| `.button-danger` | An action is destructive or difficult to reverse |
| `.button-compact` | Small button size for dense lists, rows, and compact toolbars |
| `.button-large` | Large button size for high-emphasis request or dialog actions |
| `.icon-button` / `.row-action-button` | Familiar action where text would add clutter |
| `.tab-button` | Switches a local view; active state uses `.active` |

Button sizing is separate from action intent. The standard control scale is:

| Size | Classes | Height | Use when |
|---|---|---:|---|
| Small | `.button-compact` or `.icon-button.button-compact` | `--control-height-sm` (`32px`) | Dense action groups, table/list rows, panel headers with several controls |
| Medium | no size class | `--control-height-md` (`36px`) | Default forms, dialogs, and page actions |
| Large | `.button-large` or `.icon-button.button-large` | `--control-height-lg` (`40px`) | Primary request/send/save surfaces and rare high-emphasis actions |

Rules:

- Prefer one primary action per visible action group.
- Pair destructive styling with a clear verb; use confirmation for deletion of durable user data.
- Icon-only buttons require an `aria-label` and usually a `title`.
- Keep controls in the same action group on the same size. For example, pair `.button-secondary.button-compact` with `.icon-button.button-compact`, not a default medium icon button.
- Do not introduce feature-local button heights or widths when one of the three shared sizes fits.
- Disabled controls must remain understandable from nearby context. Use a loading label when the action is in progress.
- Use shared heights: `--control-height-sm`, `--control-height-md`, and `--control-height-lg`.
- Preserve the global `:focus-visible` ring. Custom compound controls must provide an equivalent inset or outer ring.

### 5.2 Inputs and Forms

Use `.text-input`, `.method-select`, `.body-mode-select`, and `.body-textarea` rather than creating new base input styling.

- Every input needs a visible label or an accessible name.
- Put `.field-help` after the label and before or after the control consistently within a section.
- Mark optional fields in supporting text; do not mark every required field if most of the form is required.
- Show validation near the affected field when recovery is local. Use a page-level `.feedback.feedback-error` block only for submission or loading failures affecting the whole surface.
- Do not erase user input after a failed action.
- Secret fields must support masking and must not expose their resolved values in previews, history, notifications, or default exports.

Key/value editing uses `KeyValueEditor.svelte`, which owns `.editor-block`, `.editor-header`, `.row-list`, `.kv-row`, `.row-toggle`, the variable-aware value field, and row action controls. Requests and realtime connections use this same component for query parameters and headers. Header consumers also use the shared `header-suggestions.ts` catalog so names and context-sensitive values remain consistent. The title and `Add row` action stay in the header, rows stay in the list, and delete uses the standard icon action. Do not create feature-local key/value editors or add a full-width creation row below the data.

Authentication editors use `AuthEditor.svelte`, which owns the `.editor-block` composition: `Auth` and the `.body-mode-select` belong in `.editor-header`; the selected method's fields use `.auth-grid`; and `None` renders the standard empty state. HTTP requests provide the optional client-credentials token-fetch helper, while realtime connections use the same OAuth2 layout for a manually supplied access token. Protocol-specific workspaces must not introduce a second auth layout.

All interactive checkboxes use the styled `.row-toggle` control. Settings-like checkbox rows also add `.settings-checkbox` so the control aligns with its title and supporting copy. Never rely on the browser-default checkbox in an application workspace.

Use `JsonEditor.svelte` for editable JSON in requests, realtime messages, Socket.IO auth payloads, and Socket.IO argument arrays. It owns syntax tokenization, environment-variable highlighting and suggestions, caret-safe overlay scrolling, and Enter/Tab indentation. Feature code owns schema-specific validation and formatting actions; it must pass error state through `ariaInvalid`.

### 5.3 Panels and Cards

Use `.panel` for a major workspace region. `.panel` is intentionally only the visual shell: it provides the background, border, radius, and shadow, but no content padding. Every panel must declare one inset strategy so a new surface cannot silently render against its border.

| Inset strategy | Class | Spacing | Use |
|---|---|---|---|
| Standard | `.panel-inset` | `--space-5`, reduced to `--space-4` at `720px` | Normal editors, results, settings, collection pages, and feature workspaces |
| Compact | `.panel-inset-compact` | `--space-3` | Tab strips and similarly dense single-row panels |
| Flush | `.panel-flush` | `0` | Deliberately edge-to-edge content whose children own every inset |
| Custom | `.panel-custom-inset` plus a purpose-specific class | Defined by that component | Dialogs or constrained layouts that cannot use a standard density |

Do not put feature-local padding on a standard panel. Add the appropriate modifier in markup and let the shared modifier own its responsive behavior. A custom inset is an exception that must be documented with the component, not a substitute for choosing a density.

Use `.panel-title` for a panel's page-level `h1`. Use `.panel-heading` around stacked eyebrow/title/supporting-copy groups; it removes browser-default child margins and supplies one `--space-1` gap. When a parent already supplies `gap`, keep its child heading margins at zero. Do not combine a parent gap with heading `margin-top` or `margin-bottom` to create the same separation twice.

Use a subtle surface plus a border for nested cards, as demonstrated by `.request-script-card` and `.multipart-file-card`.

A panel should have:

- a concise title;
- an action area only when the action applies to the whole panel;
- a clear loading, error, empty, or content state;
- no duplicate page-level title.

Panel review checklist:

- Does the `.panel` declare standard, compact, flush, or documented custom inset behavior?
- Does a stacked heading use `.panel-heading`, with `.panel-title` for a page-level title?
- Is vertical separation owned by exactly one mechanism: parent `gap`, section margin, or component inset?
- At the compact breakpoint, does the content retain `--space-4` from the panel edge without horizontal overflow?

### 5.4 Tabs and Segmented Views

Use tabs only when views are peers and switching does not submit or navigate through a workflow. Implement `role="tablist"`, `role="tab"`, and `aria-selected`. The request workspace uses `RequestTabs.svelte`; local panel tabs use `.panel-tabs` and `.tab-button`.

Selection must remain visible without hover and should not rely on text color alone.

The WebSockets workspace reuses the request-tab chip geometry through `RealtimeTabs.svelte` and the shared `horizontalWheelScroll` attachment for overflowing strips. Each tab must expose the definition name, protocol badge, connection status text, unsaved marker, and a visible close affordance inside the chip; click that affordance or press Delete on the focused tab to close it. The new-tab action immediately follows the chips but remains outside the semantic `tablist`, because ARIA tablists may own only tabs. Do not move close/new actions into a detached far-edge toolbar. Restored tabs begin disconnected; do not visually imply that a saved/open tab is a live connection.

Realtime workspace tabs, connection-setting tabs, and transcript-filter tabs use roving focus. Left/Right moves to the previous/next peer, Home/End moves to the first/last peer, and focus follows selection. Delete closes the focused workspace tab through the normal dirty/live confirmation flow. Keep the selected tab at `tabindex="0"`, peers at `-1`, and connect each tab to its panel with `aria-controls`/`aria-labelledby`.

### 5.5 Dialogs

Use `DialogShell.svelte` with the standard `save-dialog` size or a purpose-specific size class. Its public properties are `ariaLabelledby`, `onDismiss`, optional `sizeClass`, optional `dismissible`, and the content snippet. It owns the backdrop, dialog role, focus trap/restore, Escape handling, and backdrop dismissal. Every dialog's content must include:

- a heading whose id matches `ariaLabelledby`;
- Escape and backdrop dismissal unless a critical operation sets `dismissible={false}`;
- a visible close/cancel action;
- `.modal-scroll-body` when content can exceed the viewport.

Primary confirmation belongs last in the action row. A destructive confirmation should name the affected object and explain whether recovery is possible.

Saving HTTP and realtime definitions into collection trees uses `CollectionSaveDialog.svelte`. It owns the collection/folder picker geometry, request-count copy, selected state, and action order; callers provide only labels, current targets, and persistence callbacks. New saved-request types must extend this dialog rather than copying its markup into a route.

### 5.6 Notifications and Inline Feedback

Use the `notifications` store for transient confirmation or failure following a user action. Supported tones are info, success, warning, and error. Notifications expose polite live updates and `alert` semantics for errors. Their progress bar controls expiry, pauses while the notification is hovered, and becomes visually static under reduced motion without changing the expiry duration.

Use inline feedback when the message:

- blocks progress in the current form;
- needs to remain until the user fixes something;
- explains an empty or partial state;
- is tied to a specific field or result.

Avoid showing the same message both inline and as a notification unless one communicates global impact and the other gives local recovery guidance.

Block feedback uses `.feedback` plus exactly one tone class: `.feedback-info`, `.feedback-success`, `.feedback-warning`, or `.feedback-error`. Add `role="alert"` only when an error appears dynamically and requires immediate attention. Keep field-level validation text local to its control.

### 5.7 Empty, Loading, and Error States

Every async or data-dependent surface must design all four states: initial/loading, populated, empty, and error.

- `.empty-state` should explain what is absent and, when useful, provide the next action.
- Loading copy should identify what is loading. Use progress indicators for operations with measurable progress.
- `.feedback.feedback-error` is the shared prominent error treatment. Put technical details in a disclosure when the user-facing explanation can be clearer.
- Preserve partial or stale content when it remains useful, and label its state rather than blanking the screen.

### 5.8 Navigation, Trees, and Drag-and-Drop

Primary navigation is route-based and uses `.sidebar-link` plus `.sidebar-link-active`. Collections use a shared folder glyph/path implementation across the sidebar and Collections page.

- Tree rows need a clear selected, expanded, hover, and drop-target state.
- Expand/collapse controls require `aria-expanded` and an accessible name.
- Drag-and-drop must have a non-drag alternative for essential movement or ordering work. Collection folders and saved requests use the Collections-page **Move…** dialog, which exposes destination collection, folder/root, and first/after-sibling position to keyboard users. Destination contents load only when that collection is selected; keep the folder, position, and confirmation controls disabled while loading and guard stale completions when users change destinations quickly.
- Do not invent a second folder icon or tree-guide geometry.

### 5.9 Code and Data Views

Use monospace type and `--bg-code`. Long values must wrap or scroll within their region. JSON should use `JsonViewer.svelte`; request/response displays should reuse existing key/value and detail patterns.

- Do not truncate the only available copy of a value.
- Provide copy actions for high-value identifiers, URLs, paths, tokens, and payload fragments where appropriate.
- Keep secrets masked by default.
- Syntax color is categorical decoration and may not be the only way to identify invalid content.

### 5.10 Realtime Workspaces and Transcripts

`RealtimeEditor.svelte` and `RealtimeTranscript.svelte` establish the shared realtime workbench pattern:

- editor and transcript panels use `.panel-inset`; the connection tab strip uses `.panel-inset-compact`;
- one large Connect or Disconnect action in the connection header;
- a visible status row with text plus `.realtime-status-dot`;
- peer connection settings in semantic local tabs;
- an advanced reconnect section disabled until opt-in;
- a composer grouped separately from handshake settings;
- a bounded transcript using `role="log"` with live announcements disabled so high-volume traffic does not interrupt assistive technology;
- transcript direction written as “Sent”, “Received”, or “Event” and reinforced by a border accent;
- explicit empty, filtered-empty, trim, error, disconnected, reconnect-required, and large-payload states;
- bounded file-backed payload inspection through deliberate Read or Copy actions, plus complete Save, instead of inserting large data into the DOM automatically.

Keep connection definition and session state distinct in copy and hierarchy. “Save” persists a reusable definition; “Connect” starts an ephemeral native session; “Clear” removes only the current session transcript; closing a dirty or live tab explains both consequences.

Realtime failures in a background route use the notification action to return to the affected tab. Do not announce every incoming message globally. When follow mode is paused because the user scrolls away from the bottom, provide an explicit “Follow new messages” action rather than moving their reading position.

## 6. Accessibility Contract

New UI must meet these minimum requirements:

- All actions and fields are reachable and operable by keyboard.
- Focus is always visible.
- DOM order matches visual and interaction order.
- Icon-only controls have accessible names; decorative icons use `aria-hidden="true"`.
- Dynamic status uses an appropriate live region without repeatedly interrupting the user.
- Dialog focus is trapped and restored.
- Tabs, listboxes, trees, disclosures, and progress controls expose their state semantically.
- Realtime tab status and protocol are exposed as text; status dots, direction borders, and badges are supplementary.
- High-volume transcripts use a named `role="log"` with `aria-live="off"`; connection-state changes use a concise polite live region.
- Information is not communicated by color alone.
- Text and interactive elements retain sufficient contrast in light and dark themes.
- Click targets should normally be at least `32px`; use `36–40px` for common actions.
- Zoom/UI scale and a narrow window do not hide essential controls or content.

For a complex custom interaction, document keyboard behavior alongside its implementation. Native elements are preferred when they provide the needed semantics.

## 7. Content and UX Conventions

- Name objects consistently: request, saved request, realtime definition, connection, session transcript, collection, folder, environment, variable, playbook, step, run, and history entry.
- Use “request” for the editable/sendable HTTP object. Use “realtime definition” when distinguishing a saved WebSocket/Socket.IO configuration from its live connection; concise local labels may use “realtime request” where the shared collection model is already clear.
- Use “connection” for a live or connectable WebSocket/Socket.IO tab and “session transcript” for its ephemeral message log. Do not call it durable history.
- Confirm successful persistence with the object name when helpful.
- Error messages should say what failed and what the user can do next. Preserve raw backend details in an expandable technical section when needed.
- State when a value is saved locally, stored in the OS credential store, redacted, unresolved, or exported in full.
- Never imply that previewing a request sends traffic. Preview surfaces must clearly remain read-only.
- For long-running operations, show current activity and expose cancellation when cancellation is safe.

## 8. Adding a New Feature

Before implementation, write down:

1. The user's goal and the primary action.
2. Where the feature fits in the existing page/navigation hierarchy.
3. Which existing panels, controls, form rows, dialogs, notifications, and data views it reuses.
4. Its loading, empty, error, success, disabled, and cancellation states.
5. Keyboard and screen-reader behavior.
6. Light/dark, UI-scale, narrow-window, long-content, and secret-data behavior.

Implementation sequence:

1. Compose existing shared classes and components.
2. Add a semantic token only when an intent is reused or needs theme-specific values.
3. Add a shared class/component when the pattern will recur or already appears more than once.
4. Keep truly local layout rules beside the feature, using tokens for all visual decisions.
5. Validate the state matrix below and run `npm test` and `npm run check`.
6. Update this guide when a new reusable pattern or rule is introduced.

### Feature state matrix

| Area | Questions |
|---|---|
| Default | Is the main task and primary action obvious? |
| Hover/focus/active | Are mouse and keyboard states equally clear? |
| Disabled/loading | Is the reason or current activity understandable? |
| Empty | Does the surface explain what to do next? |
| Error | Is the failure actionable and user input preserved? |
| Success | Is confirmation proportional and non-disruptive? |
| Long content | Do names, URLs, code, and errors wrap or scroll safely? |
| Themes | Does it work in light, dark, and system preference? |
| Scale/layout | Does it work at `0.6`, `1`, `1.5`, and a narrow window? |
| Accessibility | Are semantics, names, focus, keyboard, and announcements correct? |
| Privacy | Are secrets masked and exports explicit? |

## 9. Design System Audit

Audit date: 2026-06-22. Scope: 17 shared frontend components, shared tokens/styles, application shell, and route-level UI patterns after the `0.20.13` remediation.

### Summary

**Components reviewed: 17 | Remaining issue groups: 2 | Score: 94/100**

The system now has semantic method/syntax/overlay/sidebar tokens, a compact spacing scale, canonical text-button names, a shared dialog shell, reduced-motion behavior, standardized block feedback, and a keyboard alternative for collection drag-and-drop. Playbooks now consume the shared spacing and typography scale, sidebar collection disclosures support Arrow/Home/End keyboard traversal, and loading/error updates in the sidebar, Playbooks, and history surfaces use consistent status or alert semantics. Remaining work is mostly opportunistic migration of untouched feature-local values and continued live-region review as async surfaces change.

### Naming consistency

| Issue | Current examples | Direction |
|---|---|---|
| Multiple radius names resolve to the same value | `--radius-sm`, `--radius-card`, `--radius-control` | Keep semantic names; do not replace them with literal `8px` |
| Legacy feature-specific validation names | `.auth-error-text`, `.error-text`, `.run-error-text` | Keep local text errors near fields; use `.feedback-*` for message blocks |
| Shared and route-local component styling coexist | global `app.css` plus Playbooks route styles | Promote a pattern once it is used by a second feature |

### Token coverage

| Category | Coverage | Known gap |
|---|---|---|
| Theme colors | Strong | `app.css` has no hardcoded hex colors; 21 `rgb`/`rgba` occurrences remain in isolated legacy state surfaces |
| Typography | Strong | Playbooks' repeated text roles use the shared typography tokens; isolated component-specific sizes remain where geometry requires them |
| Radius | Strong | Several legacy literal pill radii remain |
| Control height | Good | Specialized controls use fixed heights where composition demands it |
| Spacing | Good | Shared scale exists and Playbooks' repeated gaps and padding now consume it; isolated geometry-specific values remain |
| Elevation | Good | Standard and overlay elevation are tokenized |
| Motion | Good | Shared durations, static reduced-motion expiry, hover pause, and JS notification transition handling are implemented |

### Component completeness

| Pattern | Variants/states | Accessibility | Documentation | Score |
|---|---|---|---|---:|
| Buttons and icon actions | Good | Visible focus; labels depend on call site | Complete | 9/10 |
| Inputs and key/value rows | Good | Mostly native controls and visible labels | Complete | 8/10 |
| Tabs | Good | ARIA implemented in main tab surfaces | Complete | 8/10 |
| Dialogs | Good | Shared `DialogShell`, focus trap, Escape, backdrop, focus restore | Complete | 9/10 |
| Notifications | Good | Live regions, tone roles, reduced motion, hover pause | Complete | 9/10 |
| Panels/cards | Good visually | Semantic structure depends on call site | Complete | 8/10 |
| Empty/loading/error states | Block feedback standardized | Core sidebar, Playbooks, and history updates announce consistently | Partial | 8/10 |
| Trees and drag-and-drop | Pointer drag plus lazy keyboard move dialog | Sidebar disclosures expose state and Arrow/Home/End traversal | Complete | 9/10 |

### Priority actions

1. Continue standardizing live-region behavior when untouched async loading, empty, and error surfaces are changed.
2. Migrate repeated feature-local visual values to shared tokens when a second use establishes reusable intent.

These are improvement directions, not permission for an unrelated sweeping refactor. Feature changes should improve the code they touch while preserving recognizable behavior.

## 10. Review Checklist

Before merging a UI change:

- [ ] Existing tokens and patterns were reused before new ones were added.
- [ ] The UI has intentional loading, empty, error, success, and disabled states.
- [ ] Light, dark, and system themes were checked.
- [ ] Default, minimum, and maximum UI scale were considered.
- [ ] Narrow-window and long-content behavior were checked.
- [ ] Keyboard navigation and visible focus were checked.
- [ ] Accessible names, roles, states, and live announcements are correct.
- [ ] Secrets and credential-looking values remain masked by default.
- [ ] Destructive actions explain impact and require appropriate confirmation.
- [ ] New reusable behavior is documented here.
- [ ] `npm test` passes when shared behavior or interaction helpers change.
- [ ] `npm run check` passes.
