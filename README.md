# PostNot

![Built With AI Agents](https://img.shields.io/badge/built%20with-AI%20agents-ff9b54)
![Code Authorship](https://img.shields.io/badge/code%20authorship-100%25%20AI%20generated-2f855a)

Local-first desktop API client for people who want a fast desktop tool for APIs without living in a browser tab.

> PostNot is a fully AI-generated software project. The codebase was produced end-to-end by AI agents, with no human-written code in the repository.

PostNot is built for working with HTTP APIs on your own machine, with local persistence, collections and folders, environments, secret handling, import/export, and a focused desktop workflow.

## What It Does

- Compose and send HTTP requests from a desktop-native UI
- Save requests into collections and nested folders
- Browse those folders in the sidebar and collections page with aligned tree guides and folder icons
- Work with environments and `{{variable}}` substitution
- Use built-in dynamic variables like Postman-style `$guid` and `$timestamp`
- Store secret environment values in the OS credential store
- Inspect request history locally
- Import from Postman collections and environments
- Import from cURL
- Export collections and environments back to Postman-compatible JSON
- Attach local files to multipart requests
- Run pre-request and test scripts on saved requests (JavaScript in the UI around each send)
- Use floating notifications, persisted settings, and signed in-app update checks

## Why PostNot

- Local-first: request data and app state live on your machine
- Desktop-native: Rust request execution and Tauri packaging instead of a browser-only shell
- Practical portability: bring data in from Postman and export it back out
- Safer environment handling: secrets are kept out of SQLite and redacted from stored history snapshots

## Current Status

PostNot is still pre-1.0 and evolving deliberately. The core workflow is already real and usable, but the product is still growing in areas like richer scripting, multi-request workflow, broader polish, and updater channel decisions.

## Privacy And Storage

- SQLite-backed app data is stored under the Tauri app data directory.
- Secret environment variables are stored in the operating system credential store instead of SQLite.
- When requests use secret environment variables, PostNot keeps the unresolved `{{variable}}` references in history snapshots rather than persisting the resolved secret values.

## Building From Source

Prerequisites:

- Node.js
- Rust
- Tauri system dependencies for your platform

Install dependencies:

```bash
npm install
```

Run the desktop app in development:

```bash
npm run tauri dev
```

Validate the project:

Frontend:

```bash
npm run check
```

Rust:

```bash
source "$HOME/.cargo/env"
cargo check --manifest-path src-tauri/Cargo.toml
```

## Releases

- The changelog is tracked in [CHANGELOG.md](CHANGELOG.md).
- Signed release artifacts are published through GitHub Releases.
- The in-app updater currently checks the latest stable GitHub Release.

## License

PostNot is licensed under the [Apache License 2.0](LICENSE).

## For Contributors And Agents

- Implementation and workspace guidance now lives in [AGENTS.md](AGENTS.md).
- Architecture and deeper technical notes live in [docs/tech-design.md](docs/tech-design.md).
