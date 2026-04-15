<script lang="ts">
  import type { CollectionItemSummary, CollectionSummary } from "$lib/api/types";

  import {
    type DraggedCollectionRequest
  } from "$lib/collections/drag-and-drop";
  import FolderGlyph from "$lib/components/icons/FolderGlyph.svelte";
  import { collectionDnd } from "$lib/stores/collection-dnd.svelte";
  import { collections } from "$lib/stores/collections.svelte";
  import CollectionDetailForm from "./CollectionDetailForm.svelte";
  import FolderScriptForm from "./FolderScriptForm.svelte";

  let {
    collection = null,
    collectionItems = [],
    isCollectionsLoading = false,
    isCollectionItemsLoading = false,
    isSavingCollection = false,
    isCreatingFolder = false,
    pendingSaveFolderId = "",
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
    onSaveFolder = () => false,
    onOpenSavedRequest = () => {},
    onDeleteCollectionItem = () => {},
  }: {
    collection?: CollectionSummary | null;
    collectionItems?: CollectionItemSummary[];
    isCollectionsLoading?: boolean;
    isCollectionItemsLoading?: boolean;
    isSavingCollection?: boolean;
    isCreatingFolder?: boolean;
    pendingSaveFolderId?: string;
    pendingDeleteCollectionId?: string;
    pendingDeleteCollectionItemId?: string;
    errorText?: string;
    isImporting?: boolean;
    isExporting?: boolean;
    onOpenImport?: () => Promise<void> | void;
    onCreateRootFolder?: () => Promise<void> | void;
    onCreateChildFolder?: (parentId: string) => Promise<void> | void;
    onExportCollection?: () => Promise<void> | void;
    onSaveCollection?: (
      name: string,
      description: string,
      preRequestScript: string,
      testScript: string
    ) => Promise<boolean> | boolean;
    onDeleteCollection?: (collectionId: string) => Promise<void> | void;
    onSaveFolder?: (
      itemId: string,
      name: string,
      preRequestScript: string,
      testScript: string
    ) => Promise<boolean> | boolean;
    onOpenSavedRequest?: (itemId: string) => Promise<void> | void;
    onDeleteCollectionItem?: (item: CollectionItemSummary) => Promise<void> | void;
  } = $props();

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

  function folderContentsLabel(childCount: number) {
    if (childCount === 0) {
      return "Empty folder";
    }

    return `${childCount} item${childCount === 1 ? "" : "s"}`;
  }

  function createDraggedRequest(item: CollectionItemSummary): DraggedCollectionRequest {
    return {
      itemId: item.id,
      collectionId: item.collectionId,
      parentId: item.parentId ?? null,
      name: item.name
    };
  }

  function handleRequestPointerDown(event: PointerEvent, item: CollectionItemSummary) {
    if (event.button !== 0 || collections.isMovingCollectionItem) {
      return;
    }

    collectionDnd.beginPotentialDrag(createDraggedRequest(item), event.pointerId, {
      x: event.clientX,
      y: event.clientY
    });
  }

  function handleOpenSavedRequestClick(itemId: string) {
    if (collectionDnd.shouldSuppressClick()) {
      return;
    }

    return onOpenSavedRequest(itemId);
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
      {#key collection.id}
        <CollectionDetailForm
          {collection}
          {isSavingCollection}
          {pendingDeleteCollectionId}
          {onSaveCollection}
          {onDeleteCollection}
        />
      {/key}
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
    {:else}
      <button
        class={[
          "collection-root-drop",
          collectionDnd.matchesDropIndicator(collection.id, null, "root") && "collection-root-drop-active"
        ]}
        type="button"
        aria-label="Move a saved request to the collection root"
        data-collection-drop="root"
        data-collection-id={collection.id}
      >
        <strong>Collection root</strong>
        <span>Drop a saved request here to move it to the top level.</span>
      </button>

      {#if collectionItems.length === 0 && !isCollectionItemsLoading}
        <div class="empty-state">No folders or saved requests yet. Use `New folder` or the request editor `Save` flow to start organizing.</div>
      {:else}
      {#snippet renderItems(items: CollectionItemSummary[], depth: number)}
        <div class={["collection-item-tree", depth > 0 && "collection-item-tree-nested"]}>
          {#each items as item (item.id)}
            <article
              class={[
                "collection-item",
                item.kind === "folder" && "collection-folder-item",
                item.kind === "request" && collectionDnd.isDraggingRequest(item.id) && "collection-item-dragging",
                collection?.id && collectionDnd.matchesDropIndicator(collection.id, item.id, "before") && "collection-drop-target-before",
                collection?.id && collectionDnd.matchesDropIndicator(collection.id, item.id, "after") && "collection-drop-target-after",
                item.kind === "folder" && collection?.id && collectionDnd.matchesDropIndicator(collection.id, item.id, "inside") && "collection-drop-target-inside"
              ]}
              style={`--tree-depth:${depth};`}
              onpointerdown={item.kind === "request" ? (event) => handleRequestPointerDown(event, item) : undefined}
              data-collection-drop={collection ? "item" : undefined}
              data-collection-id={collection?.id}
              data-item-id={item.id}
              data-item-kind={item.kind}
            >
              <div class="saved-request-meta">
                {#if item.kind === "folder"}
                  <div class="collection-folder-heading">
                    <span class="collection-folder-icon" aria-hidden="true">
                      <FolderGlyph variant="panel-closed" />
                    </span>
                    <div class="collection-folder-heading-text">
                      <span class="collection-folder-eyebrow">Folder</span>
                      <strong class="collection-folder-name">{item.name}</strong>
                    </div>
                  </div>
                  <div class="collection-folder-meta">
                    <span class="collection-folder-count-badge">{folderContentsLabel(item.children.length)}</span>
                    <span class="history-meta">Updated {formatUpdatedAt(item.updatedAt)}</span>
                  </div>
                {:else}
                  <strong class="collection-item-title">
                    {#if item.name}
                      {item.name}
                    {:else}
                      <span class={`method-badge method-${item.method?.toLowerCase() ?? "get"}`}>{item.method ?? "GET"}</span>
                      <span class="collection-request-url-inline">{item.url ?? ""}</span>
                    {/if}
                  </strong>

                  {#if item.name}
                    <div class="collection-request-subline">
                      <span class={`method-badge method-${item.method?.toLowerCase() ?? "get"}`}>{item.method ?? "GET"}</span>
                      <span class="collection-request-url-line">{item.url ?? ""}</span>
                    </div>
                  {/if}

                  <span class="history-meta">Updated {formatUpdatedAt(item.updatedAt)}</span>
                {/if}
              </div>

              <div class="saved-request-actions">
                {#if item.kind === "folder"}
                  <button class="tab-button" type="button" onclick={() => onCreateChildFolder(item.id)}>
                    Add subfolder
                  </button>
                {:else}
                  <button class="tab-button" type="button" onclick={() => handleOpenSavedRequestClick(item.id)}>
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

              {#if item.kind === "folder"}
                {#key item.id}
                  <FolderScriptForm
                    {item}
                    isSaving={pendingSaveFolderId === item.id}
                    {onSaveFolder}
                  />
                {/key}
              {/if}
            </article>

            {#if item.kind === "folder" && item.children.length > 0}
              {@render renderItems(item.children, depth + 1)}
            {/if}
          {/each}
        </div>
      {/snippet}

      {@render renderItems(collectionItems, 0)}
      {/if}
    {/if}
  </section>
</div>
