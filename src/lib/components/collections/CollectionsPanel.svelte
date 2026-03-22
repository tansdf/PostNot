<script lang="ts">
  import type { CollectionSummary, SavedRequestSummary } from "$lib/api/types";

  export let collection: CollectionSummary | null = null;
  export let savedRequests: SavedRequestSummary[] = [];
  export let isCollectionsLoading = false;
  export let isSavedRequestsLoading = false;
  export let isSavingCollection = false;
  export let pendingDeleteCollectionId = "";
  export let pendingDeleteSavedRequestId = "";
  export let errorText = "";
  export let importSource = "";
  export let isImporting = false;
  export let importErrorText = "";
  export let importSuccessText = "";
  export let onSaveCollection: (name: string, description: string) => Promise<boolean> | boolean = () => false;
  export let onDeleteCollection: (collectionId: string) => Promise<void> | void = () => {};
  export let onOpenSavedRequest: (itemId: string) => Promise<void> | void = () => {};
  export let onDeleteSavedRequest: (itemId: string) => Promise<void> | void = () => {};
  export let onImportRequests: () => Promise<void> | void = () => {};

  let importFileInput: HTMLInputElement | null = null;

  let editableCollectionId = "";
  let draftName = "";
  let draftDescription = "";

  $: if (collection && collection.id !== editableCollectionId) {
    editableCollectionId = collection.id;
    draftName = collection.name;
    draftDescription = collection.description;
  }

  $: if (!collection) {
    editableCollectionId = "";
    draftName = "";
    draftDescription = "";
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

  async function handleSubmit() {
    if (!collection) {
      return;
    }

    const saved = await onSaveCollection(draftName, draftDescription);
    if (saved && collection.id === editableCollectionId) {
      editableCollectionId = collection.id;
    }
  }
</script>

<section class="panel collections-import-panel">
  <div class="collections-column-header">
    <h2>Import</h2>
    <span class="history-meta">Postman Collection v2.1 JSON</span>
  </div>

  <p class="field-help">Import a Postman collection by opening a JSON file or pasting the collection payload directly.</p>

  <label>
    <span class="field-label">Paste source</span>
    <textarea
      class="text-input collections-import-source"
      bind:value={importSource}
      placeholder={'{ "info": { "name": "My collection" }, "item": [...] }'}
    ></textarea>
  </label>

  <input
    bind:this={importFileInput}
    class="sr-only"
    type="file"
    accept=".json,application/json"
    on:change={async (event) => {
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

  {#if importSuccessText}
    <div class="response-placeholder collections-import-success">{importSuccessText}</div>
  {/if}

  <div class="collections-page-actions">
    <button class="ghost-button" type="button" on:click={() => importFileInput?.click()}>
      Open JSON file
    </button>
    <button class="send-button" type="button" on:click={onImportRequests} disabled={isImporting}>
      {isImporting ? "Importing..." : "Import"}
    </button>
  </div>
</section>

<div class="workspace-grid">
  <section class="panel collections-page-panel">
    <div class="editor-header">
      <h1>Collection View</h1>
      {#if isCollectionsLoading}
        <span class="history-meta">Loading...</span>
      {/if}
    </div>

    {#if errorText}
      <div class="response-error">{errorText}</div>
    {/if}

    {#if collection}
      <form class="collections-detail-form" on:submit|preventDefault={handleSubmit}>
        <div class="detail-facts">
          <div class="detail-kv-item">
            <span class="field-label">Saved requests</span>
            <strong>{collection.requestCount}</strong>
          </div>
          <div class="detail-kv-item detail-wide">
            <span class="field-label">Last updated</span>
            <strong>{formatUpdatedAt(collection.updatedAt)}</strong>
          </div>
        </div>

        <label>
          <span class="field-label">Collection name</span>
          <input class="text-input" bind:value={draftName} placeholder="Untitled collection" required />
        </label>

        <label>
          <span class="field-label">Description</span>
          <textarea
            class="text-input collection-description-input"
            bind:value={draftDescription}
            placeholder="Describe what this collection is for"
          ></textarea>
        </label>

        <div class="collections-page-actions">
          <button class="send-button" type="submit" disabled={isSavingCollection}>
            {isSavingCollection ? "Saving..." : "Save collection"}
          </button>
          <button
            class="icon-button"
            type="button"
            on:click={() => collection && onDeleteCollection(collection.id)}
            disabled={pendingDeleteCollectionId === collection.id}
          >
            {pendingDeleteCollectionId === collection.id ? "Deleting..." : "Delete collection"}
          </button>
        </div>
      </form>
    {:else}
      <div class="empty-state">Pick a collection from the sidebar or create one with the `+` button.</div>
    {/if}
  </section>

  <section class="panel collections-page-panel">
    <div class="collections-column-header">
      <h2>Saved Requests</h2>
      {#if collection}
        <span class="history-meta">{isSavedRequestsLoading ? "Refreshing..." : `${savedRequests.length} item${savedRequests.length === 1 ? "" : "s"}`}</span>
      {/if}
    </div>

    {#if !collection}
      <div class="empty-state">Select a collection to inspect its saved requests.</div>
    {:else if savedRequests.length === 0 && !isSavedRequestsLoading}
      <div class="empty-state">No saved requests yet. Use the `Save` button in the request editor to add one here.</div>
    {:else}
      <div class="collections-list">
        {#each savedRequests as item (item.id)}
          <article class="collection-item">
            <div class="saved-request-meta">
              <strong>{item.name || `${item.method} ${item.url}`}</strong>
              <span>{item.method} {item.url}</span>
              <span class="history-meta">Updated {formatUpdatedAt(item.updatedAt)}</span>
            </div>

            <div class="saved-request-actions">
              <button class="tab-button" type="button" on:click={() => onOpenSavedRequest(item.id)}>
                Open in Requests
              </button>
              <button
                class="icon-button"
                type="button"
                on:click={() => onDeleteSavedRequest(item.id)}
                disabled={pendingDeleteSavedRequestId === item.id}
              >
                {pendingDeleteSavedRequestId === item.id ? "Deleting..." : "Delete"}
              </button>
            </div>
          </article>
        {/each}
      </div>
    {/if}
  </section>
</div>
