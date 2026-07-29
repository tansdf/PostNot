<script lang="ts">
  import { onMount } from "svelte";

  import { getSettings, updateSettings } from "$lib/api/commands";
  import { createDefaultSettings, type AppSettings } from "$lib/api/types";
  import { notifications } from "$lib/stores/notifications.svelte";
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
      settings = await getSettings();
      await updater.initialize();
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
      applyTheme(settings.theme);
      applyUiScale(settings.uiScale);
      notifications.setDefaultDuration(settings.notificationTimeoutMs);
      errorText = "";
      notifications.success("Your preferences were saved.", "Settings saved");
    } catch (error) {
      errorText = error instanceof Error ? error.message : String(error);
    } finally {
      isSaving = false;
    }
  }
</script>

<svelte:head>
  <title>PostNot Settings</title>
</svelte:head>

<section class="settings-page panel">
    <div class="request-section-header">
      <h1>Settings</h1>
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
                <div>
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
                        {#each option.swatches as swatch, index (`${option.value}-${index}`)}
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
                <div>
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
                <div>
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
                    step="1"
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
                    step="1"
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
                <div>
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
                      {#each parsedUpdateNotes as line, lineIndex (lineIndex)}
                        <p class="settings-update-markdown-line">
                          {#if line.length === 0}
                            <br />
                          {:else}
                            {#each line as token, tokenIndex (tokenIndex)}
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
                <div>
                  <h2>History</h2>
                  <p class="settings-section-lede">How much recent request activity PostNot keeps on disk.</p>
                </div>
              </div>

              <div class="settings-field-grid">
                <label>
                  <span class="field-label">History limit</span>
                  <input class="text-input" type="number" min="1" step="1" bind:value={settings.historyLimit} />
                </label>
              </div>

            </section>

            <section class="settings-section-card">
              <div class="settings-section-heading">
                <div>
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
