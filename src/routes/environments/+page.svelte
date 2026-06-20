<script lang="ts">
  import { beforeNavigate, goto } from "$app/navigation";
  import { resolve } from "$app/paths";
  import { page } from "$app/state";
  import { onDestroy, onMount } from "svelte";
  import {
    createDefaultSettings,
    createEnvironmentVariable,
    type AppSettings,
    type EnvironmentDetail,
    type EnvironmentInput,
    type EnvironmentSummary
  } from "$lib/api/types";
  import {
    createEnvironment,
    deleteEnvironment,
    exportEnvironment,
    getEnvironment,
    getSettings,
    importPostmanEnvironment,
    listEnvironments,
    setActiveEnvironment,
    updateEnvironment
  } from "$lib/api/commands";
  import { createStaleGuard } from "$lib/async-stale-guard";
  import DialogShell from "$lib/components/layout/DialogShell.svelte";
  import { notifications } from "$lib/stores/notifications.svelte";
  import { readCachedJson, writeCachedJson, UI_CACHE_KEYS } from "$lib/ui-cache";

  const ENVIRONMENT_AUTOSAVE_DELAY_MS = 400;

  function mergeCachedSettings(): AppSettings {
    const defaults = createDefaultSettings();
    const cached = readCachedJson<Partial<AppSettings>>(UI_CACHE_KEYS.settings);
    if (!cached || typeof cached !== "object") {
      return defaults;
    }
    return { ...defaults, ...cached };
  }

  let environments: EnvironmentSummary[] = $state(
    readCachedJson<EnvironmentSummary[]>(UI_CACHE_KEYS.environmentsList) ?? []
  );
  let settings: AppSettings = $state(mergeCachedSettings());
  let selectedEnvironmentId = $state("");
  let environmentDetail: EnvironmentDetail | null = $state(null);
  let isLoading = $state(true);
  let isDetailLoading = $state(false);
  let isSaving = $state(false);
  let isCreating = $state(false);
  let isImporting = $state(false);
  let isExporting = $state(false);
  let pendingDeleteId = $state("");
  let pendingActivateId = $state("");
  let errorText = $state("");
  let importSource = $state("");
  let importErrorText = $state("");
  let importSetActive = $state(false);
  let isImportModalOpen = $state(false);
  let importFileInput: HTMLInputElement | null = $state(null);
  let lastSavedEnvironmentFingerprint = $state("");
  let lastFailedAutosaveFingerprint = $state("");
  let environmentEditVersion = $state(0);
  type EnvironmentVariable = NonNullable<EnvironmentDetail["variables"]>[number];
  let revealedSecretRowIds = $state<string[]>([]);
  let environmentAutosaveTimer: ReturnType<typeof setTimeout> | null = null;
  let suppressUnsavedEnvironmentNavigationPrompt = false;

  let requestedEnvironmentId = $derived(page.url.searchParams.get("environmentId") ?? "");
  let isEnvironmentDirty = $derived(
    Boolean(environmentDetail && environmentFingerprint(environmentDetail) !== lastSavedEnvironmentFingerprint)
  );
  let environmentSaveStatus = $derived.by(() => {
    if (!environmentDetail) {
      return "";
    }

    if (isDetailLoading) {
      return "Loading...";
    }

    if (isSaving) {
      return settings.environmentAutosave ? "Autosaving..." : "Saving...";
    }

    if (settings.environmentAutosave) {
      return isEnvironmentDirty ? "Unsaved changes" : "Autosave on";
    }

    return isEnvironmentDirty ? "Unsaved changes" : "Saved";
  });

  const envAsync = createStaleGuard();

  beforeNavigate((navigation) => {
    if (!shouldConfirmDiscardUnsavedEnvironment() || suppressUnsavedEnvironmentNavigationPrompt) {
      return;
    }

    const nextPath = navigation.to?.url.pathname ?? null;
    const nextEnvironmentId = navigation.to?.url.searchParams.get("environmentId") ?? "";
    const currentPath = page.url.pathname;
    const currentEnvironmentId = requestedEnvironmentId;

    if (nextPath === currentPath) {
      if (nextEnvironmentId === currentEnvironmentId) {
        return;
      }

      if (!window.confirm("Switch environments and discard unsaved changes?")) {
        navigation.cancel();
      }

      return;
    }

    if (!window.confirm("Leave this page and discard unsaved environment changes?")) {
      navigation.cancel();
    }
  });

  onMount(() => {
    void Promise.all([loadSettings(), loadEnvironments(requestedEnvironmentId)]);
  });

  onDestroy(() => {
    clearEnvironmentAutosaveTimer();
  });

  $effect(() => {
    if (!isLoading) {
      void syncEnvironmentFromRoute(requestedEnvironmentId);
    }
  });

  $effect(() => {
    const detail = environmentDetail;
    const fingerprint = detail ? environmentFingerprint(detail) : "";

    clearEnvironmentAutosaveTimer();

    if (
      !detail ||
      !settings.environmentAutosave ||
      isDetailLoading ||
      isSaving ||
      !fingerprint ||
      fingerprint === lastSavedEnvironmentFingerprint ||
      fingerprint === lastFailedAutosaveFingerprint ||
      isImportModalOpen
    ) {
      return;
    }

    const scheduledEditVersion = environmentEditVersion;
    environmentAutosaveTimer = setTimeout(() => {
      environmentAutosaveTimer = null;
      void persistEnvironmentDetail({
        expectedEditVersion: scheduledEditVersion
      });
    }, ENVIRONMENT_AUTOSAVE_DELAY_MS);
  });

  async function loadSettings() {
    try {
      settings = await getSettings();
      writeCachedJson(UI_CACHE_KEYS.settings, settings);
    } catch (error) {
      errorText = error instanceof Error ? error.message : String(error);
    }
  }

  async function syncEnvironmentFromRoute(environmentId: string) {
    if (isLoading) {
      return;
    }

    if (environmentId && environmentId !== selectedEnvironmentId) {
      const seq = envAsync.next();
      await loadEnvironmentDetail(environmentId, seq);
      return;
    }

    if (!environmentId && environments.length > 0 && !selectedEnvironmentId) {
      await goto(resolve(`/environments?environmentId=${encodeURIComponent(environments[0].id)}`), {
        replaceState: true,
        noScroll: true,
        keepFocus: true
      });
    }
  }

  async function loadEnvironments(preferredEnvironmentId = selectedEnvironmentId) {
    const seq = envAsync.next();
    isLoading = true;

    try {
      environments = await listEnvironments();
      if (envAsync.isStale(seq)) {
        return;
      }

      errorText = "";
      writeCachedJson(UI_CACHE_KEYS.environmentsList, environments);

      const nextEnvironmentId =
        preferredEnvironmentId && environments.some((environment) => environment.id === preferredEnvironmentId)
          ? preferredEnvironmentId
          : environments[0]?.id ?? "";

      selectedEnvironmentId = nextEnvironmentId;

      if (requestedEnvironmentId !== nextEnvironmentId) {
        const navOpts = { replaceState: true, noScroll: true, keepFocus: true } as const;
        if (nextEnvironmentId) {
          await goto(resolve(`/environments?environmentId=${encodeURIComponent(nextEnvironmentId)}`), navOpts);
        } else {
          await goto(resolve("/environments"), navOpts);
        }
        if (envAsync.isStale(seq)) {
          return;
        }
      }

      if (nextEnvironmentId) {
        await loadEnvironmentDetail(nextEnvironmentId, seq);
      } else {
        if (envAsync.isStale(seq)) {
          return;
        }
        environmentDetail = null;
        lastSavedEnvironmentFingerprint = "";
        lastFailedAutosaveFingerprint = "";
        revealedSecretRowIds = [];
      }
    } catch (error) {
      if (!envAsync.isStale(seq)) {
        errorText = error instanceof Error ? error.message : String(error);
        revealedSecretRowIds = [];
      }
    } finally {
      if (!envAsync.isStale(seq)) {
        isLoading = false;
      }
    }
  }

  async function loadEnvironmentDetail(environmentId: string, reuseSeq?: number) {
    const seq = reuseSeq ?? envAsync.next();
    selectedEnvironmentId = environmentId;
    isDetailLoading = true;
    revealedSecretRowIds = [];

    try {
      const loadedEnvironment = await getEnvironment(environmentId);
      if (envAsync.isStale(seq)) {
        return;
      }
      environmentDetail = loadedEnvironment;
      lastSavedEnvironmentFingerprint = environmentFingerprint(loadedEnvironment);
      lastFailedAutosaveFingerprint = "";
      errorText = "";
    } catch (error) {
      if (!envAsync.isStale(seq)) {
        errorText = error instanceof Error ? error.message : String(error);
        environmentDetail = null;
        lastSavedEnvironmentFingerprint = "";
        lastFailedAutosaveFingerprint = "";
      }
    } finally {
      if (!envAsync.isStale(seq)) {
        isDetailLoading = false;
      }
    }
  }

  async function openEnvironmentDetail(environmentId: string) {
    if (requestedEnvironmentId === environmentId) {
      if (!confirmDiscardUnsavedEnvironment("Reload this environment and discard unsaved changes?")) {
        return;
      }

      await loadEnvironmentDetail(environmentId);
      return;
    }

    if (!confirmDiscardUnsavedEnvironment("Switch environments and discard unsaved changes?")) {
      return;
    }

    await withSuppressedUnsavedEnvironmentNavigationPrompt(() =>
      goto(resolve(`/environments?environmentId=${encodeURIComponent(environmentId)}`), {
        noScroll: true,
        keepFocus: true
      })
    );
  }

  async function handleCreateEnvironment() {
    if (!confirmDiscardUnsavedEnvironment("Create a new environment and discard unsaved changes?")) {
      return;
    }

    isCreating = true;

    try {
      const created = await createEnvironment();
      await withSuppressedUnsavedEnvironmentNavigationPrompt(async () => {
        await loadEnvironments(created.id);
        await goto(resolve(`/environments?environmentId=${encodeURIComponent(created.id)}`), {
          replaceState: true,
          noScroll: true,
          keepFocus: true
        });
      });
      notifications.success(created.name, "Environment created");
    } catch (error) {
      errorText = error instanceof Error ? error.message : String(error);
    } finally {
      isCreating = false;
    }
  }

  async function handleSave() {
    await persistEnvironmentDetail({ showNotification: true });
  }

  async function handleDelete(environmentId: string) {
    if (!window.confirm("Delete this environment?")) {
      return;
    }

    if (!confirmDiscardUnsavedEnvironment("Discard unsaved changes before continuing?")) {
      return;
    }

    pendingDeleteId = environmentId;
    const environmentName = environments.find((environment) => environment.id === environmentId)?.name ?? "Environment";

    try {
      await deleteEnvironment(environmentId);
      environmentDetail = null;
      lastSavedEnvironmentFingerprint = "";
      lastFailedAutosaveFingerprint = "";
      await withSuppressedUnsavedEnvironmentNavigationPrompt(() =>
        loadEnvironments(selectedEnvironmentId === environmentId ? "" : selectedEnvironmentId)
      );
      notifications.success(environmentName, "Environment deleted");
    } catch (error) {
      errorText = error instanceof Error ? error.message : String(error);
    } finally {
      pendingDeleteId = "";
    }
  }

  async function handleActivate(environmentId: string | null) {
    if (!confirmDiscardUnsavedEnvironment("Change environments and discard unsaved changes?")) {
      return;
    }

    pendingActivateId = environmentId ?? "__none__";

    try {
      await setActiveEnvironment(environmentId);
      await withSuppressedUnsavedEnvironmentNavigationPrompt(() =>
        loadEnvironments(environmentId ?? selectedEnvironmentId)
      );
      if (environmentId) {
        const environmentName = environments.find((environment) => environment.id === environmentId)?.name ?? "Environment";
        notifications.success(environmentName, "Environment activated");
      } else {
        notifications.info("No environment is active now.", "Environment deactivated");
      }
    } catch (error) {
      errorText = error instanceof Error ? error.message : String(error);
    } finally {
      pendingActivateId = "";
    }
  }

  function updateVariable(index: number, patch: Partial<EnvironmentVariable>) {
    if (!environmentDetail) {
      return;
    }

    setEnvironmentDetail({
      ...environmentDetail,
      variables: environmentDetail.variables.map((row, rowIndex) => (rowIndex === index ? { ...row, ...patch } : row))
    });
  }

  function addVariable() {
    if (!environmentDetail) {
      return;
    }

    setEnvironmentDetail({
      ...environmentDetail,
      variables: [...environmentDetail.variables, createEnvironmentVariable()]
    });
  }

  function removeVariable(id: string) {
    if (!environmentDetail) {
      return;
    }

    setEnvironmentDetail({
      ...environmentDetail,
      variables:
        environmentDetail.variables.length === 1
          ? [createEnvironmentVariable()]
          : environmentDetail.variables.filter((row) => row.id !== id)
    });

    revealedSecretRowIds = revealedSecretRowIds.filter((rowId) => rowId !== id);
  }

  function formatUpdatedAt(value: string) {
    try {
      return new Intl.DateTimeFormat(undefined, {
        dateStyle: "medium",
        timeStyle: "short"
      }).format(new Date(value));
    } catch {
      return value;
    }
  }

  function toggleSecretVisibility(id: string) {
    revealedSecretRowIds = revealedSecretRowIds.includes(id)
      ? revealedSecretRowIds.filter((rowId) => rowId !== id)
      : [...revealedSecretRowIds, id];
  }

  function isSecretVisible(id: string) {
    return revealedSecretRowIds.includes(id);
  }

  function variableName(row: EnvironmentVariable) {
    return row.key.trim() || "environment variable";
  }

  async function copyVariableValue(row: EnvironmentVariable) {
    try {
      await navigator.clipboard.writeText(row.value);
      notifications.success(row.key || "Environment variable value", "Value copied");
    } catch (error) {
      errorText = error instanceof Error ? error.message : String(error);
    }
  }

  async function handleImportEnvironment() {
    importErrorText = "";

    const source = importSource.trim();
    if (!source) {
      importErrorText = "Open a Postman environment JSON file or paste its JSON payload to import.";
      return;
    }

    if (!confirmDiscardUnsavedEnvironment("Import an environment and discard unsaved changes?")) {
      return;
    }

    isImporting = true;

    try {
      const result = await importPostmanEnvironment({
        source,
        setActive: importSetActive
      });

      importSource = "";
      await withSuppressedUnsavedEnvironmentNavigationPrompt(async () => {
        await loadEnvironments(result.environmentId);
        await goto(resolve(`/environments?environmentId=${encodeURIComponent(result.environmentId)}`), {
          replaceState: true,
          noScroll: true,
          keepFocus: true
        });
      });
      notifications.success(
        `${result.importedVariableCount} variable${result.importedVariableCount === 1 ? "" : "s"} imported into ${result.environmentName}.${result.activated ? " Environment is now active." : ""}`,
        "Environment imported"
      );
    } catch (error) {
      importErrorText = error instanceof Error ? error.message : String(error);
    } finally {
      isImporting = false;
    }
  }

  async function handleExportEnvironment() {
    if (!environmentDetail) {
      return;
    }

    isExporting = true;

    try {
      const result = await exportEnvironment(environmentDetail.id);
      if (result) {
        notifications.success(result.filePath, "Environment exported");
      }
    } catch (error) {
      errorText = error instanceof Error ? error.message : String(error);
    } finally {
      isExporting = false;
    }
  }

  function openImportModal() {
    importErrorText = "";
    importSetActive = false;
    isImportModalOpen = true;
  }

  function closeImportModal() {
    isImportModalOpen = false;
    importErrorText = "";
  }

  function clearEnvironmentAutosaveTimer() {
    if (environmentAutosaveTimer !== null) {
      clearTimeout(environmentAutosaveTimer);
      environmentAutosaveTimer = null;
    }
  }

  function buildEnvironmentInput(detail: EnvironmentDetail): EnvironmentInput {
    return {
      name: detail.name.trim(),
      variables: detail.variables
    };
  }

  function environmentFingerprint(detail: EnvironmentDetail) {
    return JSON.stringify(buildEnvironmentInput(detail));
  }

  function shouldConfirmDiscardUnsavedEnvironment() {
    return !settings.environmentAutosave && isEnvironmentDirty && Boolean(environmentDetail) && !isSaving;
  }

  function confirmDiscardUnsavedEnvironment(message: string) {
    return !shouldConfirmDiscardUnsavedEnvironment() || window.confirm(message);
  }

  async function withSuppressedUnsavedEnvironmentNavigationPrompt<T>(callback: () => Promise<T>) {
    suppressUnsavedEnvironmentNavigationPrompt = true;

    try {
      return await callback();
    } finally {
      suppressUnsavedEnvironmentNavigationPrompt = false;
    }
  }

  function setEnvironmentDetail(nextDetail: EnvironmentDetail) {
    environmentDetail = nextDetail;
    environmentEditVersion += 1;
    lastFailedAutosaveFingerprint = "";
    errorText = "";
  }

  function setEnvironmentName(name: string) {
    if (!environmentDetail) {
      return;
    }

    setEnvironmentDetail({
      ...environmentDetail,
      name
    });
  }

  function syncEnvironmentSummary(updated: EnvironmentDetail) {
    environments = environments.map((environment) =>
      environment.id === updated.id
        ? {
            ...environment,
            name: updated.name,
            isActive: updated.isActive,
            variableCount: updated.variables.length,
            updatedAt: updated.updatedAt
          }
        : environment
    );
    writeCachedJson(UI_CACHE_KEYS.environmentsList, environments);
  }

  async function persistEnvironmentDetail(
    options: {
      showNotification?: boolean;
      expectedEditVersion?: number;
    } = {}
  ) {
    if (!environmentDetail) {
      return false;
    }

    const { showNotification = false, expectedEditVersion = environmentEditVersion } = options;
    const detailToSave = environmentDetail;
    const fingerprint = environmentFingerprint(detailToSave);

    if (fingerprint === lastSavedEnvironmentFingerprint) {
      if (showNotification) {
        notifications.info(`${detailToSave.name || "Environment"} is already up to date.`, "Already saved");
      }
      return true;
    }

    isSaving = true;
    clearEnvironmentAutosaveTimer();

    try {
      const updated = await updateEnvironment(detailToSave.id, buildEnvironmentInput(detailToSave));
      lastSavedEnvironmentFingerprint = fingerprint;
      lastFailedAutosaveFingerprint = "";
      syncEnvironmentSummary(updated);

      if (environmentDetail?.id === updated.id && expectedEditVersion === environmentEditVersion) {
        environmentDetail = updated;
      }

      errorText = "";

      if (showNotification) {
        notifications.success(updated.name, "Environment saved");
      }

      return true;
    } catch (error) {
      errorText = error instanceof Error ? error.message : String(error);
      lastFailedAutosaveFingerprint = fingerprint;

      if (!showNotification) {
        notifications.error(errorText, "Environment autosave failed");
      }

      return false;
    } finally {
      isSaving = false;
    }
  }

  function isPrimarySaveShortcut(event: KeyboardEvent) {
    return (event.ctrlKey || event.metaKey) && !event.shiftKey && !event.altKey && event.key.toLowerCase() === "s";
  }

  function handleWindowKeydown(event: KeyboardEvent) {
    if (!isPrimarySaveShortcut(event)) {
      return;
    }

    if (isImportModalOpen || !environmentDetail) {
      event.preventDefault();
      return;
    }

    event.preventDefault();
    void handleSave();
  }

  function handleBeforeUnload(event: BeforeUnloadEvent) {
    if (!shouldConfirmDiscardUnsavedEnvironment()) {
      return;
    }

    event.preventDefault();
    event.returnValue = "";
  }
</script>

<svelte:head>
  <title>PostNot Environments</title>
</svelte:head>

<div class="workspace-grid">
    <section class="panel collections-page-panel">
      <div class="request-section-header">
        <div class="request-section-title">
          <h1>Environments</h1>
          <button class="button-secondary button-compact" type="button" onclick={handleExportEnvironment} disabled={!environmentDetail || isExporting}>
            {isExporting ? "Exporting..." : "Export"}
          </button>
          <button class="button-secondary button-compact" type="button" onclick={openImportModal}>
            Import
          </button>
          <button class="button-secondary button-compact" type="button" onclick={handleCreateEnvironment} disabled={isCreating}>
            {isCreating ? "Creating..." : "New"}
          </button>
        </div>
      </div>

      {#if errorText}
        <div class="feedback feedback-error">{errorText}</div>
      {/if}

      {#if environments.length === 0 && !isLoading}
        <div class="empty-state">Create an environment to start resolving variables like <code>{'{{base_url}}'}</code> and <code>{'{{api_token}}'}</code>.</div>
      {:else}
        <div class="environment-list">
          {#each environments as environment (environment.id)}
            <article class={["environment-card", selectedEnvironmentId === environment.id && "environment-card-active"]}>
              <button class="environment-card-select" type="button" onclick={() => openEnvironmentDetail(environment.id)}>
                <div class="environment-card-topline">
                  <strong>{environment.name}</strong>
                  {#if environment.isActive}
                    <span class="history-status">Active</span>
                  {/if}
                </div>
                <div class="environment-card-meta">
                  <span>{environment.variableCount} variable{environment.variableCount === 1 ? "" : "s"}</span>
                  <span class="history-meta">Updated {formatUpdatedAt(environment.updatedAt)}</span>
                </div>
              </button>

              <div class="environment-card-actions">
                <button
                  class="tab-button"
                  type="button"
                  onclick={() => handleActivate(environment.isActive ? null : environment.id)}
                  disabled={pendingActivateId === environment.id || pendingActivateId === "__none__"}
                >
                  {environment.isActive ? "Deactivate" : "Set active"}
                </button>
                <button
                  class="icon-button row-action-button row-action-danger"
                  type="button"
                  title={`Delete ${environment.name}`}
                  aria-label={`Delete ${environment.name}`}
                  onclick={() => handleDelete(environment.id)}
                  disabled={pendingDeleteId === environment.id}
                >
                  {#if pendingDeleteId === environment.id}
                    <span class="sr-only">Deleting {environment.name}</span>
                    <span aria-hidden="true">...</span>
                  {:else}
                    <svg viewBox="0 0 20 20" aria-hidden="true">
                      <path d="M3 5h14" />
                      <path d="M8 5V3h4v2" />
                      <path d="M6 8v8" />
                      <path d="M10 8v8" />
                      <path d="M14 8v8" />
                      <path d="M5 5l1 12h8l1-12" />
                    </svg>
                  {/if}
                </button>
              </div>
            </article>
          {/each}
        </div>
      {/if}
    </section>

    <section class="panel collections-page-panel">
      <div class="editor-header">
        <h2>Environment Detail</h2>
        {#if environmentSaveStatus}
          <span class="history-meta">{environmentSaveStatus}</span>
        {/if}
      </div>

      {#if !environmentDetail}
        {#if !isLoading && !isDetailLoading}
          <div class="empty-state">Select an environment to edit its variables.</div>
        {/if}
      {:else}
        <div class="editor-block">
          <label>
            <span class="field-label">Environment name</span>
            <input
              class="text-input"
              value={environmentDetail.name}
              placeholder="Untitled environment"
              oninput={(event) => setEnvironmentName(event.currentTarget.value)}
            />
          </label>

          <div class="editor-header">
            <h2>Variables</h2>
            <button class="button-secondary" type="button" onclick={addVariable}>Add variable</button>
          </div>

          <div class="row-list">
            {#each environmentDetail.variables as row, index (row.id)}
              <div class="kv-row environment-kv-row">
                <input
                  class="row-toggle"
                  type="checkbox"
                  checked={row.enabled}
                  onchange={(event) => updateVariable(index, { enabled: event.currentTarget.checked })}
                />
                <input
                  class="text-input"
                  value={row.key}
                  placeholder="Variable name"
                  oninput={(event) => updateVariable(index, { key: event.currentTarget.value })}
                />
                <div class="environment-variable-value">
                  <div class="environment-variable-input-shell">
                    <input
                      class="text-input environment-variable-input"
                      type={row.isSecret && !isSecretVisible(row.id) ? "password" : "text"}
                      value={row.value}
                      placeholder={row.isSecret ? "Secret value" : "Value"}
                      oninput={(event) => updateVariable(index, { value: event.currentTarget.value })}
                    />
                    <div class="environment-variable-icon-actions">
                      {#if row.isSecret}
                        <button
                          class={["environment-variable-icon-button", isSecretVisible(row.id) && "environment-variable-icon-button-active"]}
                          type="button"
                          title={isSecretVisible(row.id) ? "Hide secret value" : "Show secret value"}
                          aria-label={isSecretVisible(row.id) ? "Hide secret value" : "Show secret value"}
                          onclick={() => toggleSecretVisibility(row.id)}
                        >
                          {#if isSecretVisible(row.id)}
                            <svg viewBox="0 0 20 20" aria-hidden="true">
                              <path d="M1.5 10s3.2-5 8.5-5 8.5 5 8.5 5-3.2 5-8.5 5-8.5-5-8.5-5Z" />
                              <circle cx="10" cy="10" r="2.5" />
                            </svg>
                          {:else}
                            <svg viewBox="0 0 20 20" aria-hidden="true">
                              <path d="M1.5 10s3.2-5 8.5-5 8.5 5 8.5 5-3.2 5-8.5 5-8.5-5-8.5-5Z" />
                              <circle cx="10" cy="10" r="2.5" />
                              <path d="M3 3 17 17" />
                            </svg>
                          {/if}
                        </button>
                      {/if}
                    <button
                      class="environment-variable-icon-button"
                      type="button"
                      title={`Copy ${variableName(row)} value`}
                      aria-label={`Copy ${variableName(row)} value`}
                      onclick={() => copyVariableValue(row)}
                    >
                        <svg viewBox="0 0 20 20" aria-hidden="true">
                          <rect x="7" y="3" width="9" height="11" rx="2" />
                          <rect x="4" y="6" width="9" height="11" rx="2" />
                        </svg>
                      </button>
                    </div>
                  </div>
                </div>
                <button
                  class={["environment-variable-icon-button", row.isSecret && "environment-variable-icon-button-active"]}
                  type="button"
                  title={row.isSecret ? "Secret variable" : "Plain variable"}
                  aria-label={row.isSecret ? "Secret variable" : "Plain variable"}
                  onclick={() => {
                    updateVariable(index, { isSecret: !row.isSecret });
                    revealedSecretRowIds = revealedSecretRowIds.filter((rowId) => rowId !== row.id);
                  }}
                >
                  <svg viewBox="0 0 20 20" aria-hidden="true">
                    <circle cx="6.5" cy="10" r="3.25" />
                    <path d="M9.75 10H17" />
                    <path d="M13.4 10V7.9" />
                    <path d="M15.9 10V8.8" />
                  </svg>
                </button>
                <button
                  class="icon-button row-action-button row-action-danger"
                  type="button"
                  title={`Remove ${variableName(row)}`}
                  aria-label={`Remove ${variableName(row)}`}
                  onclick={() => removeVariable(row.id)}
                >
                  <svg viewBox="0 0 20 20" aria-hidden="true">
                    <path d="M3 5h14" />
                    <path d="M8 5V3h4v2" />
                    <path d="M6 8v8" />
                    <path d="M10 8v8" />
                    <path d="M14 8v8" />
                    <path d="M5 5l1 12h8l1-12" />
                  </svg>
                </button>
              </div>
            {/each}
          </div>

          <div class="detail-kv-item">
            <span class="field-label">Usage</span>
            <strong>Reference variables in requests with <code>{'{{name}}'}</code> syntax.</strong>
          </div>

          <div class="collections-page-actions">
            <button class="button-primary" type="button" onclick={handleSave} disabled={isSaving}>
              {isSaving ? "Saving..." : settings.environmentAutosave ? "Save now" : "Save environment"}
            </button>
          </div>
        </div>
      {/if}
    </section>
  </div>

  <svelte:window onkeydown={handleWindowKeydown} onbeforeunload={handleBeforeUnload} />

  {#if isImportModalOpen}
    <DialogShell ariaLabelledby="import-environment-title" onDismiss={closeImportModal}>
        <div class="editor-header import-dialog-header">
          <h2 id="import-environment-title">Import</h2>
          <span class="history-meta">Postman Environment JSON</span>
        </div>

        <div class="editor-block modal-scroll-body">
          <p class="field-help">Import a Postman environment by opening a JSON file or pasting the environment payload directly.</p>

          <label>
            <span class="field-label">Paste source</span>
            <textarea
              class="text-input collections-import-source"
              bind:value={importSource}
              placeholder={'{ "name": "Local", "values": [{ "key": "base_url", "value": "https://api.example.com" }] }'}
            ></textarea>
          </label>

          <label class="checkbox-label">
            <input class="row-toggle" type="checkbox" bind:checked={importSetActive} />
            <span>Set imported environment as active</span>
          </label>

          <input
            bind:this={importFileInput}
            class="sr-only"
            type="file"
            accept=".json,application/json"
            onchange={async (event: Event & { currentTarget: HTMLInputElement }) => {
              const file = event.currentTarget.files?.[0];
              if (!file) {
                return;
              }

              importSource = await file.text();
              event.currentTarget.value = "";
            }}
          />

          {#if importErrorText}
            <div class="feedback feedback-error">{importErrorText}</div>
          {/if}

          <div class="collections-page-actions">
            <button class="button-secondary" type="button" onclick={() => importFileInput?.click()}>
              Open JSON file
            </button>
            <button
              class="button-primary"
              type="button"
              onclick={async () => {
                await handleImportEnvironment();
                if (!importErrorText) {
                  closeImportModal();
                }
              }}
              disabled={isImporting}
            >
              {isImporting ? "Importing..." : "Import"}
            </button>
            <button class="button-secondary" type="button" onclick={closeImportModal}>
              Cancel
            </button>
          </div>
        </div>
    </DialogShell>
  {/if}
