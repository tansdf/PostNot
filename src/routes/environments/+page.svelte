<script lang="ts">
  import { goto } from "$app/navigation";
  import { resolve } from "$app/paths";
  import { page } from "$app/state";
  import { onMount } from "svelte";
  import { createEnvironmentVariable, type EnvironmentDetail, type EnvironmentSummary } from "$lib/api/types";
  import {
    createEnvironment,
    deleteEnvironment,
    exportEnvironment,
    getEnvironment,
    importPostmanEnvironment,
    listEnvironments,
    setActiveEnvironment,
    updateEnvironment
  } from "$lib/api/commands";
  import { createStaleGuard } from "$lib/async-stale-guard";
  import { modalFocusTrap } from "$lib/modal-focus-trap";
  import { notifications } from "$lib/stores/notifications.svelte";
  let environments: EnvironmentSummary[] = $state([]);
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
  type EnvironmentVariable = NonNullable<EnvironmentDetail["variables"]>[number];
  let revealedSecretRowIds = $state<string[]>([]);

  let requestedEnvironmentId = $derived(page.url.searchParams.get("environmentId") ?? "");

  const envAsync = createStaleGuard();

  onMount(() => {
    void loadEnvironments(requestedEnvironmentId);
  });

  $effect(() => {
    if (!isLoading) {
      void syncEnvironmentFromRoute(requestedEnvironmentId);
    }
  });

  async function syncEnvironmentFromRoute(environmentId: string) {
    if (isLoading) {
      return;
    }

    const seq = envAsync.next();

    if (environmentId && environmentId !== selectedEnvironmentId) {
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
      environmentDetail = await getEnvironment(environmentId);
      if (envAsync.isStale(seq)) {
        return;
      }
      errorText = "";
    } catch (error) {
      if (!envAsync.isStale(seq)) {
        errorText = error instanceof Error ? error.message : String(error);
        environmentDetail = null;
      }
    } finally {
      if (!envAsync.isStale(seq)) {
        isDetailLoading = false;
      }
    }
  }

  async function openEnvironmentDetail(environmentId: string) {
    if (requestedEnvironmentId === environmentId) {
      await loadEnvironmentDetail(environmentId);
      return;
    }

    await goto(resolve(`/environments?environmentId=${encodeURIComponent(environmentId)}`), {
      noScroll: true,
      keepFocus: true
    });
  }

  async function handleCreateEnvironment() {
    isCreating = true;

    try {
      const created = await createEnvironment();
      await loadEnvironments(created.id);
      await goto(resolve(`/environments?environmentId=${encodeURIComponent(created.id)}`), {
        replaceState: true,
        noScroll: true,
        keepFocus: true
      });
      notifications.success(created.name, "Environment created");
    } catch (error) {
      errorText = error instanceof Error ? error.message : String(error);
    } finally {
      isCreating = false;
    }
  }

  async function handleSave() {
    if (!environmentDetail) {
      return;
    }

    isSaving = true;

    try {
      environmentDetail = await updateEnvironment(environmentDetail.id, {
        name: environmentDetail.name.trim(),
        variables: environmentDetail.variables
      });
      await loadEnvironments(environmentDetail.id);
      notifications.success(environmentDetail.name, "Environment saved");
    } catch (error) {
      errorText = error instanceof Error ? error.message : String(error);
    } finally {
      isSaving = false;
    }
  }

  async function handleDelete(environmentId: string) {
    if (!window.confirm("Delete this environment?")) {
      return;
    }

    pendingDeleteId = environmentId;
    const environmentName = environments.find((environment) => environment.id === environmentId)?.name ?? "Environment";

    try {
      await deleteEnvironment(environmentId);
      environmentDetail = null;
      await loadEnvironments(selectedEnvironmentId === environmentId ? "" : selectedEnvironmentId);
      notifications.success(environmentName, "Environment deleted");
    } catch (error) {
      errorText = error instanceof Error ? error.message : String(error);
    } finally {
      pendingDeleteId = "";
    }
  }

  async function handleActivate(environmentId: string | null) {
    pendingActivateId = environmentId ?? "__none__";

    try {
      await setActiveEnvironment(environmentId);
      await loadEnvironments(environmentId ?? selectedEnvironmentId);
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

    environmentDetail = {
      ...environmentDetail,
      variables: environmentDetail.variables.map((row, rowIndex) => (rowIndex === index ? { ...row, ...patch } : row))
    };
  }

  function addVariable() {
    if (!environmentDetail) {
      return;
    }

    environmentDetail = {
      ...environmentDetail,
      variables: [...environmentDetail.variables, createEnvironmentVariable()]
    };
  }

  function removeVariable(id: string) {
    if (!environmentDetail) {
      return;
    }

    environmentDetail = {
      ...environmentDetail,
      variables:
        environmentDetail.variables.length === 1
          ? [createEnvironmentVariable()]
          : environmentDetail.variables.filter((row) => row.id !== id)
    };

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

    isImporting = true;

    try {
      const result = await importPostmanEnvironment({
        source,
        setActive: importSetActive
      });

      importSource = "";
      await loadEnvironments(result.environmentId);
      await goto(resolve(`/environments?environmentId=${encodeURIComponent(result.environmentId)}`), {
        replaceState: true,
        noScroll: true,
        keepFocus: true
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
</script>

<svelte:head>
  <title>PostNot Environments</title>
</svelte:head>

<div class="workspace-grid">
    <section class="panel collections-page-panel">
      <div class="request-section-header">
        <div class="request-section-title">
          <h1>Environments</h1>
          <button class="system-button" type="button" onclick={handleExportEnvironment} disabled={!environmentDetail || isExporting}>
            {isExporting ? "Exporting..." : "Export"}
          </button>
          <button class="system-button" type="button" onclick={openImportModal}>
            Import
          </button>
          <button class="system-button" type="button" onclick={handleCreateEnvironment} disabled={isCreating}>
            {isCreating ? "Creating..." : "New"}
          </button>
        </div>
      </div>

      {#if errorText}
        <div class="response-error">{errorText}</div>
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
                  class="icon-button"
                  type="button"
                  onclick={() => handleDelete(environment.id)}
                  disabled={pendingDeleteId === environment.id}
                >
                  {pendingDeleteId === environment.id ? "Deleting..." : "Delete"}
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
        {#if isDetailLoading}
          <span class="history-meta">Loading...</span>
        {/if}
      </div>

      {#if !environmentDetail}
        <div class="empty-state">Select an environment to edit its variables.</div>
      {:else}
        <div class="editor-block">
          <label>
            <span class="field-label">Environment name</span>
            <input class="text-input" bind:value={environmentDetail.name} placeholder="Untitled environment" />
          </label>

          <div class="editor-header">
            <h2>Variables</h2>
            <button class="ghost-button" type="button" onclick={addVariable}>Add variable</button>
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
                        title="Copy value"
                        aria-label="Copy value"
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
                <button class="icon-button" type="button" onclick={() => removeVariable(row.id)}>Remove</button>
              </div>
            {/each}
          </div>

          <div class="detail-kv-item">
            <span class="field-label">Usage</span>
            <strong>Reference variables in requests with <code>{'{{name}}'}</code> syntax.</strong>
          </div>

          <div class="collections-page-actions">
            <button class="send-button" type="button" onclick={handleSave} disabled={isSaving}>
              {isSaving ? "Saving..." : "Save environment"}
            </button>
          </div>
        </div>
      {/if}
    </section>
  </div>

  {#if isImportModalOpen}
    <div
      class="modal-backdrop"
      role="button"
      tabindex="0"
      aria-label="Close import dialog"
      use:modalFocusTrap={{ onEscape: closeImportModal }}
      onclick={(e) => { if (e.target === e.currentTarget) closeImportModal(); }}
      onkeydown={(event) => {
        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault();
          closeImportModal();
        }
      }}
    >
      <div class="panel save-dialog" role="dialog" tabindex="-1" aria-modal="true" aria-labelledby="import-environment-title">
        <div class="editor-header import-dialog-header">
          <h2 id="import-environment-title">Import</h2>
          <span class="history-meta">Postman Environment JSON</span>
        </div>

        <div class="editor-block">
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
            <div class="response-error">{importErrorText}</div>
          {/if}

          <div class="collections-page-actions">
            <button class="ghost-button" type="button" onclick={() => importFileInput?.click()}>
              Open JSON file
            </button>
            <button
              class="send-button"
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
            <button class="ghost-button" type="button" onclick={closeImportModal}>
              Cancel
            </button>
          </div>
        </div>
      </div>
    </div>
  {/if}
