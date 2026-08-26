<script lang="ts">
  import { onMount } from "svelte";

  import {
    applyHistoryRetention,
    exportPortableWorkspace,
    getSettings,
    getStorageSummary,
    importPortableWorkspace,
    inspectPortableWorkspace,
    updateSettings
  } from "$lib/api/commands";
  import {
    createDefaultSettings,
    type AppSettings,
    type PortableWorkspaceExportResult,
    type PortableWorkspaceDrafts,
    type PortableWorkspaceImportPreview,
    type PortableWorkspaceImportResult,
    type StorageSummary
  } from "$lib/api/types";
  import { collections } from "$lib/stores/collections.svelte";
  import { notifications } from "$lib/stores/notifications.svelte";
  import { realtimeWorkspace } from "$lib/stores/realtime-workspace.svelte";
  import { requestWorkspace } from "$lib/stores/request-workspace.svelte";
  import { updater } from "$lib/stores/updater.svelte";
  import { applyTheme, applyUiScale } from "$lib/theme";

  const uiScaleOptions = [
    { value: 0.6, label: "60%" },
    { value: 0.65, label: "65%" },
    { value: 0.7, label: "70%" },
    { value: 0.75, label: "75%" },
    { value: 0.8, label: "80%" },
    { value: 0.85, label: "85%" },
    { value: 0.9, label: "90%" },
    { value: 0.95, label: "95%" },
    { value: 1, label: "100%" },
    { value: 1.05, label: "105%" },
    { value: 1.1, label: "110%" },
    { value: 1.15, label: "115%" },
    { value: 1.2, label: "120%" },
    { value: 1.25, label: "125%" },
    { value: 1.3, label: "130%" },
    { value: 1.4, label: "140%" },
    { value: 1.5, label: "150%" }
  ];
  const themeOptions = [
    { value: "system", label: "System", description: "Follow your desktop preference.", swatches: ["#f2efe7", "#163331", "#d96c3b"] },
    { value: "light", label: "Light", description: "Warm cream surfaces with deep teal structure.", swatches: ["#f2efe7", "#172c2b", "#d96c3b"] },
    { value: "dark", label: "Dark", description: "Low-glare teal surfaces with warm actions.", swatches: ["#111917", "#eaf0ea", "#de7c4f"] },
    { value: "forest", label: "Forest", description: "A deeper green workspace with softer contrast.", swatches: ["#101713", "#eef5ed", "#2f7d55"] }
  ];

  let settings: AppSettings = $state(createDefaultSettings());
  let isLoading = $state(true);
  let isSaving = $state(false);
  let errorText = $state("");
  let storageSummary: StorageSummary | null = $state(null);
  let isLoadingStorage = $state(false);
  let includeOpenDrafts = $state(true);
  let includeImportedDrafts = $state(true);
  let isExportingWorkspace = $state(false);
  let isInspectingWorkspace = $state(false);
  let isImportingWorkspace = $state(false);
  let importSource = $state("");
  let importFileName = $state("");
  let importPreview: PortableWorkspaceImportPreview | null = $state(null);
  let exportResult: PortableWorkspaceExportResult | null = $state(null);
  let importResult: PortableWorkspaceImportResult | null = $state(null);
  type UpdateNoteToken =
    | { kind: "text"; value: string }
    | { kind: "strong"; value: string };

  function formatDateTime(value: string | null | undefined) {
    if (!value) {
      return "";
    }

    const parsed = new Date(value);
    if (Number.isNaN(parsed.getTime())) {
      return "";
    }

    return new Intl.DateTimeFormat(undefined, {
      dateStyle: "medium",
      timeStyle: "short"
    }).format(parsed);
  }

  function formatBytes(value: number) {
    if (!Number.isFinite(value) || value <= 0) return "0 B";
    const units = ["B", "KiB", "MiB", "GiB", "TiB"];
    const exponent = Math.min(Math.floor(Math.log(value) / Math.log(1024)), units.length - 1);
    const amount = value / 1024 ** exponent;
    return `${amount >= 10 || exponent === 0 ? amount.toFixed(0) : amount.toFixed(1)} ${units[exponent]}`;
  }

  function portableCountLabel(preview: PortableWorkspaceImportPreview | null) {
    if (!preview) return "";
    const counts = preview.counts;
    return [
      `${counts.collections} collection${counts.collections === 1 ? "" : "s"}`,
      `${counts.httpRequests} HTTP request${counts.httpRequests === 1 ? "" : "s"}`,
      `${counts.realtimeConnections} realtime profile${counts.realtimeConnections === 1 ? "" : "s"}`,
      `${counts.environments} environment${counts.environments === 1 ? "" : "s"}`,
      `${counts.playbooks} playbook${counts.playbooks === 1 ? "" : "s"}`
    ].join(" · ");
  }

  function parseUpdateNotes(body: string | null | undefined): UpdateNoteToken[][] {
    if (!body) {
      return [];
    }

    return body.split(/\r?\n/).map((line) => {
      const tokens: UpdateNoteToken[] = [];
      const pattern = /(\*\*[^*]+\*\*)/g;
      let lastIndex = 0;

      for (const match of line.matchAll(pattern)) {
        const matched = match[0];
        const start = match.index ?? 0;

        if (start > lastIndex) {
          tokens.push({ kind: "text", value: line.slice(lastIndex, start) });
        }

        if (matched.startsWith("**") && matched.endsWith("**")) {
          tokens.push({ kind: "strong", value: matched.slice(2, -2) });
        }

        lastIndex = start + matched.length;
      }

      if (lastIndex < line.length) {
        tokens.push({ kind: "text", value: line.slice(lastIndex) });
      }

      return tokens;
    });
  }

  const currentVersion = __APP_VERSION__;
  const updatesSecondaryText = $derived.by(() => {
    if (!updater.errorText || !updater.availableUpdate) {
      return "";
    }

    return `${updater.errorText} You can still install v${updater.availableUpdate.version} from the earlier successful check.`;
  });
  const checkedAtLabel = $derived.by(() => {
    return formatDateTime(updater.lastCheckedAt);
  });
  const parsedUpdateNotes = $derived.by(() => parseUpdateNotes(updater.availableUpdate?.body));
  const updateState = $derived.by(() => {
    if (updater.phase === "installing") {
      return {
        tone: "installing",
        label: "Downloading",
        title: "Installing v" + (updater.availableUpdate?.version ?? "next"),
        description: updater.isMockRuntime
          ? "PostNot is running a fake update in the dev browser and will stay open when it finishes."
          : "PostNot will hand the signed update to the installer when the download finishes."
      };
    }

    if (updater.errorText && !updater.availableUpdate) {
      return {
        tone: "error",
        label: "Needs attention",
        title: "Update check failed",
        description: "The last check could not reach the signed release feed. Try again when your connection is ready."
      };
    }

    if (updater.availableUpdate) {
      return {
        tone: updater.errorText ? "warning" : "available",
        label: updater.errorText ? "Retry available" : "Ready",
        title: "Version " + updater.availableUpdate.version + " is available",
        description: updater.errorText
          ? "The saved update can still be installed, but refreshing the release information failed."
          : updater.isMockRuntime
            ? "A fake desktop build is ready to exercise the update flow in the dev browser."
            : "A newer signed desktop build is ready to download and install."
      };
    }

    if (updater.phase === "checking") {
      return {
        tone: "checking",
        label: "Checking",
        title: "Checking for updates",
        description: "PostNot is contacting the latest stable GitHub Release for a signed build."
      };
    }

    if (updater.configured === false) {
      return {
        tone: "muted",
        label: "Unavailable",
        title: "Updater is not configured",
        description: "This build cannot check for signed updates yet."
      };
    }

    if (updater.configured === null) {
      return {
        tone: "muted",
        label: "Manual",
        title: "Ready to check",
        description: "Run a manual check whenever you want to look for a newer signed build."
      };
    }

    return {
      tone: "current",
      label: "Current",
      title: "PostNot is up to date",
      description: "You are already on the latest signed stable release."
    };
  });
  const installProgressText = $derived.by(() => {
    if (!updater.installProgress) {
      return "";
    }

    const percent = updater.installProgressPercent;
    const sizeLabel = updater.installProgressLabel;

    if (typeof percent === "number") {
      return `${percent}% · ${sizeLabel}`;
    }

    return sizeLabel;
  });

  onMount(() => void loadSettings());

  async function loadSettings() {
    isLoading = true;

    try {
      const [nextSettings] = await Promise.all([
        getSettings(),
        updater.initialize(),
        loadStorageSummary()
      ]);
      settings = nextSettings;
      applyTheme(settings.theme);
      applyUiScale(settings.uiScale);
      notifications.setDefaultDuration(settings.notificationTimeoutMs);
      errorText = "";
    } catch (error) {
      errorText = error instanceof Error ? error.message : String(error);
    } finally {
      isLoading = false;
    }
  }

  async function handleSubmit() {
    isSaving = true;

    try {
      settings = await updateSettings(settings);
      const retentionResult = await applyHistoryRetention();
      await loadStorageSummary();
      applyTheme(settings.theme);
      applyUiScale(settings.uiScale);
      notifications.setDefaultDuration(settings.notificationTimeoutMs);
      errorText = "";
      const retentionMessage = retentionResult.removedEntryCount
        ? ` Removed ${retentionResult.removedEntryCount} history entries and released ${formatBytes(retentionResult.releasedResponseBodyBytes)}.`
        : "";
      notifications.success(`Your preferences were saved.${retentionMessage}`, "Settings saved");
    } catch (error) {
      errorText = error instanceof Error ? error.message : String(error);
    } finally {
      isSaving = false;
    }
  }

  async function loadStorageSummary() {
    isLoadingStorage = true;
    try {
      storageSummary = await getStorageSummary();
    } finally {
      isLoadingStorage = false;
    }
  }

  async function handleWorkspaceExport() {
    isExportingWorkspace = true;
    exportResult = null;
    try {
      let drafts: PortableWorkspaceDrafts = { requests: [], realtime: [] };
      if (includeOpenDrafts) {
        await Promise.all([requestWorkspace.ensureInitialized(), realtimeWorkspace.ensureInitialized()]);
        drafts = {
          requests: requestWorkspace.createPortableDrafts(),
          realtime: realtimeWorkspace.createPortableDrafts()
        };
      }
      exportResult = await exportPortableWorkspace(includeOpenDrafts, drafts);
      if (exportResult) {
        notifications.success(
          `${exportResult.redactionCount} credential field${exportResult.redactionCount === 1 ? " was" : "s were"} cleared from the file.`,
          "Portable workspace exported",
          exportResult.warnings.length
            ? { details: { title: "Export notes", warnings: exportResult.warnings } }
            : {}
        );
      }
    } catch (error) {
      errorText = error instanceof Error ? error.message : String(error);
    } finally {
      isExportingWorkspace = false;
    }
  }

  async function handleWorkspaceFile(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    importPreview = null;
    importResult = null;
    importSource = "";
    importFileName = file?.name ?? "";
    if (!file) return;
    isInspectingWorkspace = true;
    try {
      importSource = await file.text();
      importPreview = await inspectPortableWorkspace(importSource);
      errorText = "";
    } catch (error) {
      errorText = error instanceof Error ? error.message : String(error);
      importSource = "";
    } finally {
      isInspectingWorkspace = false;
    }
  }

  async function handleWorkspaceImport() {
    if (!importSource || !importPreview) return;
    isImportingWorkspace = true;
    try {
      importResult = await importPortableWorkspace(importSource, includeImportedDrafts);
      if (includeImportedDrafts) {
        await Promise.all([requestWorkspace.ensureInitialized(), realtimeWorkspace.ensureInitialized()]);
        requestWorkspace.appendPortableDrafts(importResult.requestDrafts);
        realtimeWorkspace.appendPortableDrafts(importResult.realtimeDrafts);
      }
      await Promise.all([collections.loadCollections(), loadStorageSummary()]);
      notifications.success(
        `${importResult.counts.collections} collections and ${importResult.counts.httpRequests} HTTP requests were added. Existing workspace data was not replaced.`,
        "Portable workspace imported",
        {
          details: {
            title: "Import details",
            warnings: [
              ...importResult.warnings,
              ...(importResult.credentialFieldsRequiringInput.length
                ? [`${importResult.credentialFieldsRequiringInput.length} credential fields require input on this device.`]
                : [])
            ]
          }
        }
      );
      importSource = "";
      importPreview = null;
      importFileName = "";
      errorText = "";
    } catch (error) {
      errorText = error instanceof Error ? error.message : String(error);
    } finally {
      isImportingWorkspace = false;
    }
  }
</script>

<svelte:head>
  <title>PostNot Settings</title>
</svelte:head>

<section class="settings-page panel panel-inset">
    <div class="request-section-header">
      <h1 class="panel-title">Settings</h1>
      {#if isLoading}
        <span class="history-meta">Loading...</span>
      {/if}
    </div>

    {#if !isLoading}
      <form class="settings-form" onsubmit={(e) => { e.preventDefault(); handleSubmit(); }}>
        <div class="settings-layout">
          <div class="settings-column">
            <section class="settings-section-card">
              <div class="settings-section-heading">
                <div class="panel-heading">
                  <h2>General</h2>
                  <p class="settings-section-lede">Desktop look and feel across the entire shell.</p>
                </div>
              </div>

              <fieldset class="settings-theme-group">
                <legend class="field-label">Theme</legend>
                <div class="settings-theme-options">
                  {#each themeOptions as option (option.value)}
                    <label class={["settings-theme-option", settings.theme === option.value && "settings-theme-option-active"]}>
                      <input type="radio" name="theme" value={option.value} bind:group={settings.theme} />
                      <span class="settings-theme-preview" aria-hidden="true">
                        {#each option.swatches as swatch (swatch)}
                          <span style:background={swatch}></span>
                        {/each}
                      </span>
                      <span class="settings-theme-copy">
                        <strong>{option.label}</strong>
                        <span>{option.description}</span>
                      </span>
                    </label>
                  {/each}
                </div>
              </fieldset>

              <div class:settings-field-grid-stacked={Boolean(updater.availableUpdate)} class="settings-field-grid">
                <label>
                  <span class="field-label">Interface zoom</span>
                  <select class="text-input" bind:value={settings.uiScale}>
                    {#each uiScaleOptions as option (option.value)}
                      <option value={option.value}>{option.label}</option>
                    {/each}
                  </select>
                </label>
              </div>

              <label class="settings-toggle">
                <input class="row-toggle settings-checkbox" type="checkbox" bind:checked={settings.environmentAutosave} />
                <span>Autosave environment edits immediately</span>
              </label>
            </section>

            <section class="settings-section-card">
              <div class="settings-section-heading">
                <div class="panel-heading">
                  <h2>Portable workspace</h2>
                  <p class="settings-section-lede">Move authoring data between PostNot installations without replacing data already on the destination.</p>
                </div>
              </div>

              <div class="feedback feedback-warning">
                This is a portable JSON export, not an encrypted backup. Known credential literals and all secret environment values are cleared; history, response bodies, transcripts, playbook runs, and Agent Activity are excluded.
              </div>

              <label class="settings-toggle">
                <input class="row-toggle settings-checkbox" type="checkbox" bind:checked={includeOpenDrafts} />
                <span>Include open request and realtime drafts</span>
              </label>

              <div class="settings-inline-actions">
                <button
                  class="button-primary"
                  type="button"
                  disabled={isExportingWorkspace}
                  onclick={handleWorkspaceExport}
                >
                  {isExportingWorkspace ? "Preparing export..." : "Export workspace"}
                </button>
              </div>

              {#if exportResult}
                <div class="feedback feedback-success" aria-live="polite">
                  Exported to {exportResult.filePath}. {exportResult.redactionCount} credential field{exportResult.redactionCount === 1 ? "" : "s"} cleared.
                </div>
              {/if}

              <div class="settings-section-heading settings-portable-import-heading">
                <div class="panel-heading">
                  <h3>Import additively</h3>
                  <p class="settings-section-lede">The file is validated and summarized before anything is written.</p>
                </div>
              </div>

              <label>
                <span class="field-label">Portable workspace file</span>
                <input
                  class="text-input"
                  type="file"
                  accept=".json,.postnot_workspace.json,application/json"
                  onchange={handleWorkspaceFile}
                />
              </label>

              {#if isInspectingWorkspace}
                <p class="field-help">Validating {importFileName}...</p>
              {:else if importPreview}
                <div class="settings-update-panel settings-update-panel-current" aria-live="polite">
                  <div class="settings-update-state">
                    <span class="settings-update-badge">Validated</span>
                    <div>
                      <strong>{importFileName}</strong>
                      <p>{portableCountLabel(importPreview)}</p>
                    </div>
                  </div>
                  <div class="settings-update-facts">
                    <div class="settings-status-item">
                      <span class="field-label">Exported</span>
                      <strong>{formatDateTime(importPreview.exportedAt) || importPreview.exportedAt}</strong>
                    </div>
                    <div class="settings-status-item">
                      <span class="field-label">Credential fields to fill</span>
                      <strong>{importPreview.credentialFieldsRequiringInput}</strong>
                    </div>
                    <div class="settings-status-item">
                      <span class="field-label">Open drafts</span>
                      <strong>{importPreview.counts.requestDrafts + importPreview.counts.realtimeDrafts}</strong>
                    </div>
                  </div>
                </div>

                <label class="settings-toggle">
                  <input class="row-toggle settings-checkbox" type="checkbox" bind:checked={includeImportedDrafts} />
                  <span>Open drafts included in this file after import</span>
                </label>

                <p class="field-help">Import creates new collections, environments, and playbooks. Exact realtime profile matches are reused. Existing records are never overwritten.</p>
                <div class="settings-inline-actions">
                  <button
                    class="button-primary"
                    type="button"
                    disabled={isImportingWorkspace}
                    onclick={handleWorkspaceImport}
                  >
                    {isImportingWorkspace ? "Importing..." : "Import and add to workspace"}
                  </button>
                </div>
              {/if}

              {#if importResult}
                <div class="feedback feedback-success">Import completed. Existing workspace data was preserved.</div>
              {/if}
            </section>

            <section class="settings-section-card">
              <div class="settings-section-heading">
                <div class="panel-heading">
                  <h2>Data &amp; storage</h2>
                  <p class="settings-section-lede">See which data is durable, which files consume disk space, and where PostNot owns it.</p>
                </div>
                <button class="button-secondary button-compact" type="button" disabled={isLoadingStorage} onclick={loadStorageSummary}>
                  {isLoadingStorage ? "Refreshing..." : "Refresh"}
                </button>
              </div>

              {#if storageSummary}
                <div class="settings-update-facts">
                  <div class="settings-status-item">
                    <span class="field-label">SQLite database</span>
                    <strong>{formatBytes(storageSummary.databaseSizeBytes)}</strong>
                  </div>
                  <div class="settings-status-item">
                    <span class="field-label">History response bodies</span>
                    <strong>{formatBytes(storageSummary.historyResponseBodyBytes)}</strong>
                  </div>
                  <div class="settings-status-item">
                    <span class="field-label">Temporary realtime payloads</span>
                    <strong>{formatBytes(storageSummary.realtimeTemporaryBytes)}</strong>
                  </div>
                  <div class="settings-status-item">
                    <span class="field-label">Workspace records</span>
                    <strong>{storageSummary.collectionCount} collections · {storageSummary.collectionItemCount} items</strong>
                  </div>
                  <div class="settings-status-item">
                    <span class="field-label">History</span>
                    <strong>{storageSummary.historyEntryCount} entries</strong>
                  </div>
                  <div class="settings-status-item">
                    <span class="field-label">Operational logs</span>
                    <strong>{storageSummary.playbookRunCount} playbook runs · {storageSummary.agentActivityCount} agent events</strong>
                  </div>
                </div>
                <p class="field-help settings-storage-path">Owned app-data directory: {storageSummary.dataDirectory}</p>
                <p class="field-help">Collections, profiles, environments, playbooks, history metadata, playbook runs, and Agent Activity live in SQLite. Response bodies are separate durable files. Realtime transcripts are process-only; large payload files are temporary.</p>
              {/if}
            </section>

            <section class="settings-section-card">
              <div class="settings-section-heading">
                <div class="panel-heading">
                  <h2>Requests</h2>
                  <p class="settings-section-lede">Default execution behavior for outgoing HTTP requests.</p>
                </div>
              </div>

              <div class="settings-field-grid">
                <label>
                  <span class="field-label">Request timeout (ms)</span>
                  <input class="text-input" type="number" min="1000" step="1000" bind:value={settings.requestTimeoutMs} />
                </label>
              </div>

              <label class="settings-toggle">
                <input class="row-toggle settings-checkbox" type="checkbox" bind:checked={settings.followRedirects} />
                <span>Follow redirects automatically</span>
              </label>

              <label class="settings-toggle">
                <input class="row-toggle settings-checkbox" type="checkbox" bind:checked={settings.validateTls} />
                <span>Validate TLS certificates</span>
              </label>
            </section>

            <section class="settings-section-card">
              <div class="settings-section-heading">
                <div class="panel-heading">
                  <h2>WebSockets</h2>
                  <p class="settings-section-lede">Connection and session transcript retention limits for WebSocket and Socket.IO.</p>
                </div>
              </div>

              <div class="settings-field-grid">
                <label>
                  <span class="field-label">Connect timeout (seconds)</span>
                  <input
                    class="text-input"
                    type="number"
                    min="1"
                    max="120"
                    step="1"
                    value={settings.realtimeConnectTimeoutMs / 1000}
                    oninput={(event) => (settings = { ...settings, realtimeConnectTimeoutMs: Math.max(1, event.currentTarget.valueAsNumber || 30) * 1000 })}
                  />
                </label>
                <label>
                  <span class="field-label">Maximum live sessions</span>
                  <input class="text-input" type="number" min="1" max="100" step="1" bind:value={settings.realtimeMaxConcurrentSessions} />
                </label>
                <label>
                  <span class="field-label">Maximum message (MiB)</span>
                  <input
                    class="text-input"
                    type="number"
                    min="0.0625"
                    max="256"
                    step="0.0625"
                    value={settings.realtimeMaxMessageBytes / (1024 * 1024)}
                    oninput={(event) => (settings = { ...settings, realtimeMaxMessageBytes: Math.round(Math.max(0.0625, event.currentTarget.valueAsNumber || 64) * 1024 * 1024) })}
                  />
                </label>
                <label>
                  <span class="field-label">Transcript entries per session</span>
                  <input class="text-input" type="number" min="1" max="10000" step="1" bind:value={settings.realtimeTranscriptMaxEntries} />
                </label>
                <label>
                  <span class="field-label">Transcript retained data per session (MiB)</span>
                  <input
                    class="text-input"
                    type="number"
                    min="0.0625"
                    max="512"
                    step="0.0625"
                    value={settings.realtimeTranscriptMaxBytes / (1024 * 1024)}
                    oninput={(event) => (settings = { ...settings, realtimeTranscriptMaxBytes: Math.round(Math.max(0.0625, event.currentTarget.valueAsNumber || 64) * 1024 * 1024) })}
                  />
                </label>
              </div>
              <p class="field-help">Transcripts are session-only and never restored; large payloads use temporary files cleared on release or startup.</p>
            </section>
          </div>

          <div class="settings-column">
            <section class="settings-section-card settings-updates-card">
              <div class="settings-section-heading">
                <div class="panel-heading">
                  <h2>Updates</h2>
                  <p class="settings-section-lede">Check for newer signed PostNot builds published to the latest stable GitHub Release.</p>
                </div>

                <button
                  class="button-secondary button-compact"
                  type="button"
                  disabled={updater.isChecking || updater.isInstalling}
                  onclick={() => updater.checkManually()}
                >
                  {updater.checkButtonLabel}
                </button>
              </div>

              <div class={["settings-update-panel", `settings-update-panel-${updateState.tone}`]} aria-live="polite">
                <div class="settings-update-state">
                  <span class="settings-update-badge">{updateState.label}</span>
                  <div>
                    <strong>{updateState.title}</strong>
                    <p>{updateState.description}</p>
                  </div>
                </div>

                <div class="settings-update-facts">
                  <div class="settings-status-item">
                    <span class="field-label">Current version</span>
                    <strong>v{currentVersion}</strong>
                  </div>

                  <div class="settings-status-item">
                    <span class="field-label">Last checked</span>
                    <strong>{checkedAtLabel || "Not checked yet"}</strong>
                  </div>

                  {#if updater.availableUpdate}
                    <div class="settings-status-item">
                      <span class="field-label">Available version</span>
                      <strong>v{updater.availableUpdate.version}</strong>
                    </div>
                  {/if}
                </div>

                {#if updater.availableUpdate && formatDateTime(updater.availableUpdate.date)}
                  <p class="settings-update-published">Published {formatDateTime(updater.availableUpdate.date)}</p>
                {/if}

                {#if updater.isInstalling && updater.installProgress}
                  <div class="settings-update-progress" aria-live="polite">
                    <div class="settings-update-progress-header">
                      <span>Download progress</span>
                      <strong>{installProgressText}</strong>
                    </div>

                    {#if typeof updater.installProgressPercent === "number"}
                      <progress
                        max="100"
                        value={updater.installProgressPercent}
                        aria-label="Update download progress"
                      >
                        {updater.installProgressPercent}%
                      </progress>
                    {:else}
                      <progress aria-label="Update download progress"></progress>
                    {/if}
                  </div>
                {/if}

                {#if updater.errorText}
                  <div class="settings-update-feedback settings-update-feedback-error" role="alert">
                    <strong>{updater.availableUpdate ? "Refresh issue" : "Check failed"}</strong>
                    <p>{updater.availableUpdate ? updatesSecondaryText : updater.errorText}</p>
                  </div>
                {/if}

                {#if updater.availableUpdate?.body}
                  <details class="settings-update-notes">
                    <summary>Release notes</summary>
                    <div class="history-preview settings-update-markdown">
                      {#each parsedUpdateNotes as line}
                        <p class="settings-update-markdown-line">
                          {#if line.length === 0}
                            <br />
                          {:else}
                            {#each line as token}
                              {#if token.kind === "text"}
                                {token.value}
                              {:else}
                                <strong>{token.value}</strong>
                              {/if}
                            {/each}
                          {/if}
                        </p>
                      {/each}
                    </div>
                  </details>
                {/if}

                {#if updater.availableUpdate}
                  <div class="settings-inline-actions">
                    <button
                      class="button-primary"
                      type="button"
                      disabled={updater.isChecking || updater.isInstalling}
                      onclick={() => updater.installAvailableUpdate()}
                    >
                      {updater.isInstalling ? "Downloading..." : "Install update"}
                    </button>
                  </div>
                {/if}
              </div>
            </section>

            <section class="settings-section-card">
              <div class="settings-section-heading">
                <div class="panel-heading">
                  <h2>History</h2>
                  <p class="settings-section-lede">How much recent request activity PostNot keeps on disk.</p>
                </div>
              </div>

              <div class="settings-field-grid">
                <label>
                  <span class="field-label">Maximum entries</span>
                  <input class="text-input" type="number" min="1" step="1" bind:value={settings.historyLimit} />
                </label>
                <label>
                  <span class="field-label">Maximum age (days)</span>
                  <input class="text-input" type="number" min="0" max="3650" step="1" bind:value={settings.historyRetentionDays} />
                  <span class="field-help">0 keeps entries regardless of age.</span>
                </label>
                <label>
                  <span class="field-label">Response-body storage (MiB)</span>
                  <input
                    class="text-input"
                    type="number"
                    min="0"
                    max="1048576"
                    step="1"
                    value={settings.historyStorageLimitBytes / (1024 * 1024)}
                    oninput={(event) => (settings = {
                      ...settings,
                      historyStorageLimitBytes: Math.round(Math.max(0, event.currentTarget.valueAsNumber || 0) * 1024 * 1024)
                    })}
                  />
                  <span class="field-help">0 disables the disk-size cap.</span>
                </label>
              </div>
              <p class="field-help">All enabled limits are enforced together, oldest first. Saving settings applies retention immediately; removed history cannot be restored from a portable workspace export.</p>
            </section>

            <section class="settings-section-card">
              <div class="settings-section-heading">
                <div class="panel-heading">
                  <h2>Notifications</h2>
                  <p class="settings-section-lede">Floating notification behavior for action feedback across the app.</p>
                </div>
              </div>

              <div class="settings-field-grid">
                <label>
                  <span class="field-label">Notification timeout (seconds)</span>
                  <input
                    class="text-input"
                    type="number"
                    min="1"
                    step="1"
                    value={Math.round(settings.notificationTimeoutMs / 1000)}
                    oninput={(event) => {
                      const seconds = Number(event.currentTarget.value);
                      settings = {
                        ...settings,
                        notificationTimeoutMs: Number.isFinite(seconds) ? Math.max(1, seconds) * 1000 : 5000
                      };
                    }}
                  />
                </label>
              </div>
            </section>
          </div>
        </div>

        <div class="settings-actions">
          <button class="button-primary" type="submit" disabled={isSaving}>
            {isSaving ? "Saving..." : "Save settings"}
          </button>
        </div>
      </form>
    {/if}

    {#if errorText}
      <div class="feedback feedback-error">{errorText}</div>
    {/if}
  </section>
