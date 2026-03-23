<script lang="ts">
  import { goto } from "$app/navigation";
  import { page } from "$app/stores";
  import { onMount } from "svelte";
  import { createKeyValueRow, type EnvironmentDetail, type EnvironmentSummary } from "$lib/api/types";
  import {
    createEnvironment,
    deleteEnvironment,
    getEnvironment,
    listEnvironments,
    setActiveEnvironment,
    updateEnvironment
  } from "$lib/api/commands";
  import AppShell from "$lib/components/layout/AppShell.svelte";

  let environments: EnvironmentSummary[] = [];
  let selectedEnvironmentId = "";
  let environmentDetail: EnvironmentDetail | null = null;
  let isLoading = true;
  let isDetailLoading = false;
  let isSaving = false;
  let isCreating = false;
  let pendingDeleteId = "";
  let pendingActivateId = "";
  let errorText = "";
  let requestedEnvironmentId = "";
  type EnvironmentVariable = NonNullable<EnvironmentDetail["variables"]>[number];

  $: requestedEnvironmentId = $page.url.searchParams.get("environmentId") ?? "";
  onMount(() => {
    void loadEnvironments(requestedEnvironmentId);
  });

  $: if (!isLoading) {
    void syncEnvironmentFromRoute(requestedEnvironmentId);
  }

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
</script>

<svelte:head>
  <title>PostNot Environments</title>
</svelte:head>

<AppShell title="PostNot" subtitle="Define reusable variables and choose which environment is active on send.">
  <div class="workspace-grid">
    <section class="panel collections-page-panel">
      <div class="editor-header">
        <h1>Environments</h1>
        <button class="ghost-button" type="button" on:click={handleCreateEnvironment} disabled={isCreating}>
          {isCreating ? "Creating..." : "New environment"}
        </button>
      </div>

      {#if errorText}
        <div class="response-error">{errorText}</div>
      {/if}

      {#if environments.length === 0 && !isLoading}
        <div class="empty-state">Create an environment to start resolving variables like <code>{'{{base_url}}'}</code> and <code>{'{{api_token}}'}</code>.</div>
      {:else}
        <div class="collections-list">
          {#each environments as environment (environment.id)}
            <article class:collection-item-active={selectedEnvironmentId === environment.id} class="collection-item">
              <button class="collection-select" type="button" on:click={() => loadEnvironmentDetail(environment.id)}>
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
                  on:click={() => handleActivate(environment.isActive ? null : environment.id)}
                  disabled={pendingActivateId === environment.id || pendingActivateId === "__none__"}
                >
                  {environment.isActive ? "Deactivate" : "Set active"}
                </button>
                <button
                  class="icon-button"
                  type="button"
                  on:click={() => handleDelete(environment.id)}
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
            <button class="ghost-button" type="button" on:click={addVariable}>Add variable</button>
          </div>

          <div class="row-list">
            {#each environmentDetail.variables as row, index (row.id)}
              <div class="kv-row">
                <input
                  class="row-toggle"
                  type="checkbox"
                  checked={row.enabled}
                  on:change={(event) => updateVariable(index, { enabled: event.currentTarget.checked })}
                />
                <input
                  class="text-input"
                  value={row.key}
                  placeholder="Variable name"
                  on:input={(event) => updateVariable(index, { key: event.currentTarget.value })}
                />
                <input
                  class="text-input"
                  value={row.value}
                  placeholder="Value"
                  on:input={(event) => updateVariable(index, { value: event.currentTarget.value })}
                />
                <button class="icon-button" type="button" on:click={() => removeVariable(row.id)}>Remove</button>
              </div>
            {/each}
          </div>

          <div class="detail-kv-item">
            <span class="field-label">Usage</span>
            <strong>Reference variables in requests with <code>{'{{name}}'}</code> syntax.</strong>
          </div>

          <div class="collections-page-actions">
            <button class="send-button" type="button" on:click={handleSave} disabled={isSaving}>
              {isSaving ? "Saving..." : "Save environment"}
            </button>
          </div>
        </div>
      {/if}
    </section>
  </div>
</AppShell>
