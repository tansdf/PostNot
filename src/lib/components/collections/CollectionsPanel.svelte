<script lang="ts">
  import type { CollectionItemSummary, CollectionSummary } from "$lib/api/types";

  let {
    collection = null,
    collectionItems = [],
    isCollectionsLoading = false,
    isCollectionItemsLoading = false,
    isSavingCollection = false,
    isCreatingFolder = false,
    pendingDeleteCollectionId = "",
    pendingDeleteCollectionItemId = "",
    errorText = "",
    isImporting = false,
    isExporting = false,
    onOpenImport = () => {},
    onCreateRootFolder = () => {},
    onCreateChildFolder = () => {},
    onExportCollection = () => {},
    onSaveCollection = () => false,
    onDeleteCollection = () => {},
    onOpenSavedRequest = () => {},
    onDeleteCollectionItem = () => {},
  }: {
    collection?: CollectionSummary | null;
    collectionItems?: CollectionItemSummary[];
    isCollectionsLoading?: boolean;
    isCollectionItemsLoading?: boolean;
    isSavingCollection?: boolean;
    isCreatingFolder?: boolean;
    pendingDeleteCollectionId?: string;
    pendingDeleteCollectionItemId?: string;
    errorText?: string;
    isImporting?: boolean;
    isExporting?: boolean;
    onOpenImport?: () => Promise<void> | void;
    onCreateRootFolder?: () => Promise<void> | void;
    onCreateChildFolder?: (parentId: string) => Promise<void> | void;
    onExportCollection?: () => Promise<void> | void;
    onSaveCollection?: (name: string, description: string) => Promise<boolean> | boolean;
    onDeleteCollection?: (collectionId: string) => Promise<void> | void;
    onOpenSavedRequest?: (itemId: string) => Promise<void> | void;
    onDeleteCollectionItem?: (item: CollectionItemSummary) => Promise<void> | void;
  } = $props();

  let editableCollectionId = $state("");
  let draftName = $state("");
  let draftDescription = $state("");

  $effect(() => {
    const nextId = collection?.id ?? "";
    if (nextId !== editableCollectionId) {
      editableCollectionId = nextId;
      if (collection) {
        draftName = collection.name;
        draftDescription = collection.description;
      } else {
        draftName = "";
        draftDescription = "";
      }
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
    <div class="request-section-header">
      <div class="request-section-title">
        <h1>Collection View</h1>
        <button class="system-button" type="button" onclick={onOpenImport}>Import</button>
        <button class="system-button" type="button" onclick={onCreateRootFolder} disabled={!collection || isCreatingFolder}>
          {isCreatingFolder ? "Creating..." : "New folder"}
        </button>
        <button class="system-button" type="button" onclick={onExportCollection} disabled={!collection || isExporting}>
          {isExporting ? "Exporting..." : "Export"}
        </button>
      </div>

      {#if isExporting}
        <span class="history-meta">Exporting...</span>
      {:else if isImporting}
        <span class="history-meta">Importing...</span>
      {:else if isCollectionsLoading}
        <span class="history-meta">Loading...</span>
      {/if}
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
      <h2>Collection Items</h2>
      {#if collection}
        <span class="history-meta">
          {isCollectionItemsLoading
            ? "Refreshing..."
            : `${collection.requestCount} request${collection.requestCount === 1 ? "" : "s"}`}
        </span>
      {/if}
    </div>

    {#if !collection}
      <div class="empty-state">Select a collection to inspect its folders and saved requests.</div>
    {:else if collectionItems.length === 0 && !isCollectionItemsLoading}
      <div class="empty-state">No folders or saved requests yet. Use `New folder` or the request editor `Save` flow to start organizing.</div>
    {:else}
      {#snippet renderItems(items: CollectionItemSummary[], depth: number)}
        <div class="collection-item-tree">
          {#each items as item (item.id)}
            <article class={["collection-item", item.kind === "folder" && "collection-folder-item"]} style={`--tree-depth:${depth};`}>
              <div class="saved-request-meta">
                <strong class="collection-item-title">
                  {#if item.kind === "folder"}
                    <span class="collection-item-kind">Folder</span> {item.name}
                  {:else}
                    {#if item.name}
                      {item.name}
                    {:else}
                      <span class={`method-badge method-${item.method?.toLowerCase() ?? "get"}`}>{item.method ?? "GET"}</span> {item.url ?? ""}
                    {/if}
                  {/if}
                </strong>

                {#if item.kind === "folder"}
                  <span>{item.children.length} item{item.children.length === 1 ? "" : "s"}</span>
                {:else}
                  <span><span class={`method-badge method-${item.method?.toLowerCase() ?? "get"}`}>{item.method ?? "GET"}</span> {item.url ?? ""}</span>
                {/if}

                <span class="history-meta">Updated {formatUpdatedAt(item.updatedAt)}</span>
              </div>

              <div class="saved-request-actions">
                {#if item.kind === "folder"}
                  <button class="tab-button" type="button" onclick={() => onCreateChildFolder(item.id)}>
                    Add subfolder
                  </button>
                {:else}
                  <button class="tab-button" type="button" onclick={() => onOpenSavedRequest(item.id)}>
                    Open in Requests
                  </button>
                {/if}

                <button
                  class="icon-button"
                  type="button"
                  onclick={() => onDeleteCollectionItem(item)}
                  disabled={pendingDeleteCollectionItemId === item.id}
                >
                  {pendingDeleteCollectionItemId === item.id ? "Deleting..." : "Delete"}
                </button>
              </div>
            </article>

            {#if item.kind === "folder" && item.children.length > 0}
              {@render renderItems(item.children, depth + 1)}
            {/if}
          {/each}
        </div>
      {/snippet}

      {@render renderItems(collectionItems, 0)}
    {/if}
  </section>
</div>
