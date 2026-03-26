<script lang="ts">
  import type { CollectionSummary, SavedRequestSummary } from "$lib/api/types";

  let {
    collection = null,
    savedRequests = [],
    isCollectionsLoading = false,
    isSavedRequestsLoading = false,
    isSavingCollection = false,
    pendingDeleteCollectionId = "",
    pendingDeleteSavedRequestId = "",
    errorText = "",
    isImporting = false,
    importSuccessText = "",
    onOpenImport = () => {},
    onSaveCollection = () => false,
    onDeleteCollection = () => {},
    onOpenSavedRequest = () => {},
    onDeleteSavedRequest = () => {},
  }: {
    collection?: CollectionSummary | null;
    savedRequests?: SavedRequestSummary[];
    isCollectionsLoading?: boolean;
    isSavedRequestsLoading?: boolean;
    isSavingCollection?: boolean;
    pendingDeleteCollectionId?: string;
    pendingDeleteSavedRequestId?: string;
    errorText?: string;
    isImporting?: boolean;
    importSuccessText?: string;
    onOpenImport?: () => Promise<void> | void;
    onSaveCollection?: (name: string, description: string) => Promise<boolean> | boolean;
    onDeleteCollection?: (collectionId: string) => Promise<void> | void;
    onOpenSavedRequest?: (itemId: string) => Promise<void> | void;
    onDeleteSavedRequest?: (itemId: string) => Promise<void> | void;
  } = $props();

  let editableCollectionId = $state("");
  let draftName = $state("");
  let draftDescription = $state("");

  $effect(() => {
    if (collection && collection.id !== editableCollectionId) {
      editableCollectionId = collection.id;
      draftName = collection.name;
      draftDescription = collection.description;
    }
  });

  $effect(() => {
    if (!collection) {
      editableCollectionId = "";
      draftName = "";
      draftDescription = "";
    }
  });

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

<div class="workspace-grid">
  <section class="panel collections-page-panel">
    <div class="editor-header">
      <h1>Collection View</h1>
      <div class="collections-page-actions">
        {#if isImporting}
          <span class="history-meta">Importing...</span>
        {:else if importSuccessText}
          <span class="history-meta">{importSuccessText}</span>
        {:else if isCollectionsLoading}
          <span class="history-meta">Loading...</span>
        {/if}
        <button class="ghost-button" type="button" onclick={onOpenImport}>Import</button>
      </div>
    </div>

    {#if errorText}
      <div class="response-error">{errorText}</div>
    {/if}

    {#if collection}
      <form class="collections-detail-form" onsubmit={(e) => { e.preventDefault(); handleSubmit(); }}>
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
            onclick={() => collection && onDeleteCollection(collection.id)}
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
              <span><span class={`method-badge method-${item.method.toLowerCase()}`}>{item.method}</span> {item.url}</span>
              <span class="history-meta">Updated {formatUpdatedAt(item.updatedAt)}</span>
            </div>

            <div class="saved-request-actions">
              <button class="tab-button" type="button" onclick={() => onOpenSavedRequest(item.id)}>
                Open in Requests
              </button>
              <button
                class="icon-button"
                type="button"
                onclick={() => onDeleteSavedRequest(item.id)}
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
