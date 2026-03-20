<script lang="ts">
  import { onMount } from "svelte";

  import AppShell from "$lib/components/layout/AppShell.svelte";
  import { getSettings, updateSettings } from "$lib/api/commands";
  import { createDefaultSettings, type AppSettings } from "$lib/api/types";
  import { applyTheme } from "$lib/theme";

  let settings: AppSettings = createDefaultSettings();
  let isLoading = true;
  let isSaving = false;
  let errorText = "";
  let saveMessage = "";

  onMount(loadSettings);

  async function loadSettings() {
    isLoading = true;

    try {
      settings = await getSettings();
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

    <form class="settings-form" on:submit|preventDefault={handleSubmit}>
      <label>
        <span class="field-label">Theme</span>
        <select class="text-input" bind:value={settings.theme}>
          <option value="system">System</option>
          <option value="light">Light</option>
          <option value="dark">Dark</option>
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
        <input type="checkbox" bind:checked={settings.followRedirects} />
        <span>Follow redirects automatically</span>
      </label>

      <label class="settings-toggle">
        <input type="checkbox" bind:checked={settings.validateTls} />
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
