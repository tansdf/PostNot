<script lang="ts">
  import { onMount } from "svelte";

  import AppShell from "$lib/components/layout/AppShell.svelte";
  import { getSettings, updateSettings } from "$lib/api/commands";
  import { createDefaultSettings, type AppSettings } from "$lib/api/types";
  import { applyTheme, applyUiScale } from "$lib/theme";

  const uiScaleOptions = [
    { value: 0.8, label: "80%" },
    { value: 0.85, label: "85%" },
    { value: 0.9, label: "90%" },
    { value: 0.95, label: "95%" },
    { value: 1, label: "100%" },
    { value: 1.05, label: "105%" },
    { value: 1.1, label: "110%" },
    { value: 1.15, label: "115%" },
    { value: 1.2, label: "120%" }
  ];

  let settings: AppSettings = $state(createDefaultSettings());
  let isLoading = $state(true);
  let isSaving = $state(false);
  let errorText = $state("");
  let saveMessage = $state("");

  onMount(loadSettings);

  async function loadSettings() {
    isLoading = true;

    try {
      settings = await getSettings();
      applyTheme(settings.theme);
      applyUiScale(settings.uiScale);
      errorText = "";
    } catch (error) {
      errorText = error instanceof Error ? error.message : String(error);
    } finally {
      isLoading = false;
    }
  }

  async function handleSubmit() {
    isSaving = true;
    saveMessage = "";

    try {
      settings = await updateSettings(settings);
      applyTheme(settings.theme);
      applyUiScale(settings.uiScale);
      errorText = "";
      saveMessage = "Settings saved.";
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

<AppShell title="PostNot" subtitle="Local settings are stored in SQLite and applied on send.">
  <section class="settings-page panel">
    <div class="editor-header">
      <h1>Settings</h1>
      {#if isLoading}
        <span class="history-meta">Loading...</span>
      {/if}
    </div>

    <form class="settings-form" onsubmit={(e) => { e.preventDefault(); handleSubmit(); }}>
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

      <label>
        <span class="field-label">Request timeout (ms)</span>
        <input class="text-input" type="number" min="1000" step="1000" bind:value={settings.requestTimeoutMs} />
      </label>

      <label>
        <span class="field-label">History limit</span>
        <input class="text-input" type="number" min="1" step="1" bind:value={settings.historyLimit} />
      </label>

      <label class="settings-toggle">
        <input class="row-toggle settings-checkbox" type="checkbox" bind:checked={settings.followRedirects} />
        <span>Follow redirects automatically</span>
      </label>

      <label class="settings-toggle">
        <input class="row-toggle settings-checkbox" type="checkbox" bind:checked={settings.validateTls} />
        <span>Validate TLS certificates</span>
      </label>

      <div class="settings-actions">
        <button class="send-button" type="submit" disabled={isSaving || isLoading}>
          {isSaving ? "Saving..." : "Save settings"}
        </button>
        {#if saveMessage}
          <span class="history-meta">{saveMessage}</span>
        {/if}
      </div>
    </form>

    {#if errorText}
      <div class="response-error">{errorText}</div>
    {/if}
  </section>
</AppShell>
