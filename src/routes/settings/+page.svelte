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
  const updatesStatusText = $derived.by(() => {
    if (updater.phase === "installing") {
      return "Downloading and applying the available update...";
    }

    if (updater.availableUpdate) {
      return `Version ${updater.availableUpdate.version} is available and ready to install.`;
    }

    if (updater.phase === "checking") {
      return "Checking GitHub Releases for a newer signed build...";
    }

    if (updater.errorText) {
      return updater.errorText;
    }

    if (updater.configured === null) {
      return "Check manually whenever you want to look for a newer desktop build.";
    }

    if (!updater.configured) {
      return "Updater support is not configured for this build yet.";
    }

    return `You're already on the latest signed release (${currentVersion}).`;
  });
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

  onMount(loadSettings);

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
          <div class="settings-primary-grid">
            <section class="settings-section-card">
              <div class="settings-section-heading">
                <div>
                  <h2>General</h2>
                  <p class="settings-section-lede">Desktop look and feel across the entire shell.</p>
                </div>
              </div>

              <div class="settings-field-grid">
                <label>
                  <span class="field-label">Theme</span>
                  <select class="text-input" bind:value={settings.theme}>
                    <option value="system">System</option>
                    <option value="light">Light</option>
                    <option value="dark">Dark</option>
                  </select>
                </label>

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
                  <h2>Updates</h2>
                  <p class="settings-section-lede">Check for newer signed PostNot builds published to the latest stable GitHub Release.</p>
                </div>

                <button
                  class="system-button"
                  type="button"
                  disabled={updater.isChecking || updater.isInstalling}
                  onclick={() => updater.checkManually()}
                >
                  {updater.checkButtonLabel}
                </button>
              </div>

              <div class="settings-field-grid">
                <div class="settings-status-item">
                  <span class="field-label">Current version</span>
                  <strong>v{currentVersion}</strong>
                </div>

                <div class="settings-status-item">
                  <span class="field-label">Last checked</span>
                  <strong>{checkedAtLabel || "Not checked yet"}</strong>
                </div>
              </div>

              <div class="settings-updates-summary">
                {#if updater.errorText && !updater.availableUpdate}
                  <div class="settings-update-feedback settings-update-feedback-error">
                    <strong>Update check failed</strong>
                    <p>{updater.errorText}</p>
                  </div>
                {:else}
                  <p>{updatesStatusText}</p>
                {/if}

                {#if updater.availableUpdate}
                  <div class="settings-update-meta">
                    <strong>Available: v{updater.availableUpdate.version}</strong>
                    {#if formatDateTime(updater.availableUpdate.date)}
                      <span>Published {formatDateTime(updater.availableUpdate.date)}</span>
                    {/if}
                  </div>

                  {#if updatesSecondaryText}
                    <div class="settings-update-feedback">
                      <strong>Refresh issue</strong>
                      <p>{updatesSecondaryText}</p>
                    </div>
                  {/if}

                  {#if updater.availableUpdate.body}
                    <div class="history-preview settings-update-notes settings-update-markdown">
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
                  {/if}
                {/if}
              </div>

              {#if updater.availableUpdate}
                <div class="settings-inline-actions">
                  <button
                    class="send-button"
                    type="button"
                    disabled={updater.isChecking || updater.isInstalling}
                    onclick={() => updater.installAvailableUpdate()}
                  >
                    {updater.isInstalling ? "Installing..." : "Install update"}
                  </button>
                </div>
              {/if}
            </section>
          </div>

          <div class="settings-secondary-grid">
            <section class="settings-section-card settings-section-card-emphasis">
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
          <button class="send-button" type="submit" disabled={isSaving}>
            {isSaving ? "Saving..." : "Save settings"}
          </button>
        </div>
      </form>
    {/if}

    {#if errorText}
      <div class="response-error">{errorText}</div>
    {/if}
  </section>
