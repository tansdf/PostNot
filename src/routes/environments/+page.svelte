<script lang="ts">
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import { onMount } from "svelte";
  import { createKeyValueRow, type EnvironmentDetail, type EnvironmentSummary } from "$lib/api/types";
  import {
    createEnvironment,
    deleteEnvironment,
    getEnvironment,
    importPostmanEnvironment,
    listEnvironments,
    setActiveEnvironment,
    updateEnvironment
  } from "$lib/api/commands";
  let environments: EnvironmentSummary[] = $state([]);
  let selectedEnvironmentId = $state("");
  let environmentDetail: EnvironmentDetail | null = $state(null);
  let isLoading = $state(true);
  let isDetailLoading = $state(false);
  let isSaving = $state(false);
  let isCreating = $state(false);
  let isImporting = $state(false);
  let pendingDeleteId = $state("");
  let pendingActivateId = $state("");
  let errorText = $state("");
  let importSource = $state("");
  let importErrorText = $state("");
  let importSuccessText = $state("");
  let importSetActive = $state(false);
  let isImportModalOpen = $state(false);
  let importFileInput: HTMLInputElement | null = $state(null);
  type EnvironmentVariable = NonNullable<EnvironmentDetail["variables"]>[number];

  let requestedEnvironmentId = $derived(page.url.searchParams.get("environmentId") ?? "");

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

    if (environmentId && environmentId !== selectedEnvironmentId) {
      await loadEnvironmentDetail(environmentId);
      return;
    }

    if (!environmentId && environments.length > 0 && !selectedEnvironmentId) {
      await goto(`/environments?environmentId=${encodeURIComponent(environments[0].id)}`, {
        replaceState: true,
        noScroll: true,
        keepFocus: true
      });
    }
  }

  async function loadEnvironments(preferredEnvironmentId = selectedEnvironmentId) {
    isLoading = true;

    try {
      environments = await listEnvironments();
      errorText = "";

      const nextEnvironmentId =
        preferredEnvironmentId && environments.some((environment) => environment.id === preferredEnvironmentId)
          ? preferredEnvironmentId
          : environments[0]?.id ?? "";

      selectedEnvironmentId = nextEnvironmentId;

      if (requestedEnvironmentId !== nextEnvironmentId) {
        await goto(nextEnvironmentId ? `/environments?environmentId=${encodeURIComponent(nextEnvironmentId)}` : "/environments", {
          replaceState: true,
          noScroll: true,
          keepFocus: true
        });
      }

      if (nextEnvironmentId) {
        await loadEnvironmentDetail(nextEnvironmentId);
      } else {
        environmentDetail = null;
      }
    } catch (error) {
      errorText = error instanceof Error ? error.message : String(error);
    } finally {
      isLoading = false;
    }
  }

  async function loadEnvironmentDetail(environmentId: string) {
    selectedEnvironmentId = environmentId;
    isDetailLoading = true;

    try {
      environmentDetail = await getEnvironment(environmentId);
      errorText = "";
    } catch (error) {
      errorText = error instanceof Error ? error.message : String(error);
      environmentDetail = null;
    } finally {
      isDetailLoading = false;
    }
  }

  async function handleCreateEnvironment() {
    importSuccessText = "";
    isCreating = true;

    try {
      const created = await createEnvironment();
      await loadEnvironments(created.id);
      await goto(`/environments?environmentId=${encodeURIComponent(created.id)}`, {
        replaceState: true,
        noScroll: true,
        keepFocus: true
      });
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

    try {
      await deleteEnvironment(environmentId);
      environmentDetail = null;
      await loadEnvironments(selectedEnvironmentId === environmentId ? "" : selectedEnvironmentId);
    } catch (error) {
      errorText = error instanceof Error ? error.message : String(error);
    } finally {
      pendingDeleteId = "";
    }
  }

  async function handleActivate(environmentId: string | null) {
    importSuccessText = "";
    pendingActivateId = environmentId ?? "__none__";

    try {
      await setActiveEnvironment(environmentId);
      await loadEnvironments(environmentId ?? selectedEnvironmentId);
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
      variables: [...environmentDetail.variables, createKeyValueRow()]
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
          ? [createKeyValueRow()]
          : environmentDetail.variables.filter((row) => row.id !== id)
    };
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

  async function handleImportEnvironment() {
    importErrorText = "";
    importSuccessText = "";

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

      importSuccessText = `${result.importedVariableCount} variable${
        result.importedVariableCount === 1 ? "" : "s"
      } imported into ${result.environmentName}.${result.activated ? " Environment is now active." : ""}`;
      importSource = "";
      await loadEnvironments(result.environmentId);
      await goto(`/environments?environmentId=${encodeURIComponent(result.environmentId)}`, {
        replaceState: true,
        noScroll: true,
        keepFocus: true
      });
    } catch (error) {
      importErrorText = error instanceof Error ? error.message : String(error);
    } finally {
      isImporting = false;
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

      {#if importSuccessText}
        <div class="collections-import-success">{importSuccessText}</div>
      {/if}

      {#if environments.length === 0 && !isLoading}
        <div class="empty-state">Create an environment to start resolving variables like <code>{'{{base_url}}'}</code> and <code>{'{{api_token}}'}</code>.</div>
      {:else}
        <div class="collections-list">
          {#each environments as environment (environment.id)}
            <article class={["collection-item", selectedEnvironmentId === environment.id && "collection-item-active"]}>
              <button class="collection-select" type="button" onclick={() => loadEnvironmentDetail(environment.id)}>
                <strong>{environment.name}</strong>
                <span>{environment.variableCount} variable{environment.variableCount === 1 ? "" : "s"}</span>
                <span class="history-meta">Updated {formatUpdatedAt(environment.updatedAt)}</span>
                {#if environment.isActive}
                  <span class="history-status">Active</span>
                {/if}
              </button>

              <div class="saved-request-actions">
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
              <div class="kv-row">
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
                <input
                  class="text-input"
                  value={row.value}
                  placeholder="Value"
                  oninput={(event) => updateVariable(index, { value: event.currentTarget.value })}
                />
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
      onclick={(e) => { if (e.target === e.currentTarget) closeImportModal(); }}
      onkeydown={(event) => {
        if (event.key === "Escape" || event.key === "Enter" || event.key === " ") {
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
