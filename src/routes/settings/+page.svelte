<script lang="ts">
  import { onMount } from "svelte";

  import { checkForUpdates, getSettings, installUpdate, updateSettings } from "$lib/api/commands";
  import { createDefaultSettings, type AppSettings, type UpdateCheckResult } from "$lib/api/types";
  import { notifications } from "$lib/stores/notifications.svelte";
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
  let updateResult = $state<UpdateCheckResult | null>(null);
  let updateCheckedAt = $state<string | null>(null);
  let updateErrorText = $state("");
  let isCheckingUpdates = $state(false);
  let isInstallingUpdate = $state(false);

  const currentVersion = __APP_VERSION__;
  const updatesStatusText = $derived.by(() => {
    if (isCheckingUpdates) {
      return "Checking GitHub Releases for a newer signed build...";
    }

    if (isInstallingUpdate) {
      return "Downloading and applying the available update...";
    }

    if (updateErrorText) {
      return updateErrorText;
    }

    if (!updateResult) {
      return "Check manually whenever you want to look for a newer desktop build.";
    }

    if (!updateResult.configured) {
      return "Updater support is not configured for this build yet.";
    }

    if (updateResult.update) {
      return `Version ${updateResult.update.version} is available and ready to install.`;
    }

    return `You're already on the latest signed release (${currentVersion}).`;
  });
  const checkedAtLabel = $derived.by(() => {
    if (!updateCheckedAt) {
      return "";
    }

    return new Intl.DateTimeFormat(undefined, {
      dateStyle: "medium",
      timeStyle: "short"
    }).format(new Date(updateCheckedAt));
  });

  onMount(loadSettings);

  async function loadSettings() {
    isLoading = true;

    try {
      settings = await getSettings();
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

  async function handleCheckForUpdates() {
    isCheckingUpdates = true;
    updateErrorText = "";

    try {
      updateResult = await checkForUpdates();
      updateCheckedAt = new Date().toISOString();

      if (updateResult.update) {
        notifications.success(`Version ${updateResult.update.version} is available.`, "Update found");
      } else {
        notifications.info(`No newer signed release is available than v${currentVersion}.`, "No update found");
      }
    } catch (error) {
      updateErrorText = error instanceof Error ? error.message : String(error);
      notifications.error(updateErrorText, "Update check failed");
    } finally {
      isCheckingUpdates = false;
    }
  }

  async function handleInstallUpdate() {
    if (!updateResult?.update) {
      return;
    }

    isInstallingUpdate = true;
    updateErrorText = "";

    try {
      notifications.info(
        `Installing v${updateResult.update.version}. PostNot will restart if the platform keeps the app open after install.`,
        "Applying update"
      );
      await installUpdate();
    } catch (error) {
      updateErrorText = error instanceof Error ? error.message : String(error);
      notifications.error(updateErrorText, "Update install failed");
      isInstallingUpdate = false;
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

    <form class="settings-form" onsubmit={(e) => { e.preventDefault(); handleSubmit(); }}>
      <div class="settings-layout">
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
              disabled={isLoading || isCheckingUpdates || isInstallingUpdate}
              onclick={handleCheckForUpdates}
            >
              {isCheckingUpdates ? "Checking..." : "Check now"}
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
            {#if updateErrorText}
              <div class="settings-update-feedback settings-update-feedback-error">
                <strong>Update check failed</strong>
                <p>{updateErrorText}</p>
              </div>
            {:else}
              <p>{updatesStatusText}</p>
            {/if}

            {#if updateResult?.update}
              <div class="settings-update-meta">
                <strong>Available: v{updateResult.update.version}</strong>
                {#if updateResult.update.date}
                  <span>Published {new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" }).format(new Date(updateResult.update.date))}</span>
                {/if}
              </div>

              {#if updateResult.update.body}
                <pre class="history-preview settings-update-notes">{updateResult.update.body}</pre>
              {/if}
            {/if}
          </div>

          <div class="settings-inline-actions">
            <button
              class="send-button"
              type="button"
              disabled={!updateResult?.update || isCheckingUpdates || isInstallingUpdate}
              onclick={handleInstallUpdate}
            >
              {isInstallingUpdate ? "Installing..." : "Install update"}
            </button>
          </div>
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

      <div class="settings-actions">
        <button class="send-button" type="submit" disabled={isSaving || isLoading}>
          {isSaving ? "Saving..." : "Save settings"}
        </button>
      </div>
    </form>

    {#if errorText}
      <div class="response-error">{errorText}</div>
    {/if}
  </section>
