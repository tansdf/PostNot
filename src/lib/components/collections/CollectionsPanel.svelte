<script lang="ts">
  import type { CollectionItemSummary, CollectionSummary } from "$lib/api/types";

  import {
    type DraggedCollectionItem
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
    revealedItemId = "",
    errorText = "",
    isImporting = false,
    isExporting = false,
    onOpenImport = () => {},
    onCreateCollection = () => {},
    onCreateRootFolder = () => {},
    onCreateChildFolder = () => {},
    onExportCollection = () => {},
    onSaveCollection = () => false,
    onDeleteCollection = () => {},
    onSaveFolder = () => false,
    onOpenSavedRequest = () => {},
    onMoveCollectionItem = () => {},
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
    revealedItemId?: string;
    errorText?: string;
    isImporting?: boolean;
    isExporting?: boolean;
    onOpenImport?: () => Promise<void> | void;
    onCreateCollection?: () => Promise<void> | void;
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
    onOpenSavedRequest?: (item: CollectionItemSummary) => Promise<void> | void;
    onMoveCollectionItem?: (item: CollectionItemSummary) => Promise<void> | void;
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

  function createDraggedItem(item: CollectionItemSummary): DraggedCollectionItem {
    return {
      itemId: item.id,
      collectionId: item.collectionId,
      parentId: item.parentId ?? null,
      name: item.name,
      kind: item.kind
    };
  }

  function handleItemPointerDown(event: PointerEvent, item: CollectionItemSummary) {
    if (event.button !== 0 || collections.isMovingCollectionItem) {
      return;
    }

    collectionDnd.beginPotentialDrag(createDraggedItem(item), event.pointerId, {
      x: event.clientX,
      y: event.clientY
    });
  }

  function handleOpenSavedRequestClick(item: CollectionItemSummary) {
    if (collectionDnd.shouldSuppressClick()) {
      return;
    }

    return onOpenSavedRequest(item);
  }

</script>

<div class="workspace-grid">
  <section class="panel panel-inset collections-page-panel">
    <div class="request-section-header">
      <div class="request-section-title">
        <div class="panel-heading collections-title-copy">
          <h1 class="panel-title">{collection?.name ?? "Collections"}</h1>
          {#if !collection}
            <p>Save and organize reusable requests.</p>
          {/if}
        </div>
        {#if collection}
          <button class="button-primary button-compact" type="button" onclick={onCreateCollection}>New collection</button>
          <button class="button-secondary button-compact" type="button" onclick={onOpenImport}>Import</button>
          <button class="button-secondary button-compact" type="button" onclick={onCreateRootFolder} disabled={isCreatingFolder}>
            {isCreatingFolder ? "Creating..." : "New folder"}
          </button>
          <button class="button-secondary button-compact" type="button" onclick={onExportCollection} disabled={isExporting}>
            {isExporting ? "Exporting..." : "Export"}
          </button>
        {/if}
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
      <div class="feedback feedback-error collections-page-feedback">{errorText}</div>
    {/if}

    {#if collection}
      <div class="collections-subsection-heading">
        <h2>Collection settings</h2>
      </div>

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
      <div class="collection-empty-state collections-first-run-empty">
        <strong>No collections yet</strong>
        <span>Start from scratch or import an existing API workspace.</span>
        <div class="collections-page-actions">
          <button class="button-primary" type="button" onclick={onCreateCollection}>New collection</button>
          <button class="button-secondary" type="button" onclick={onOpenImport}>Import</button>
        </div>
      </div>
    {/if}
  </section>

  {#if collection}
  <section class="panel panel-inset collections-page-panel">
    <div class="collections-column-header">
      <h2>Saved requests</h2>
      <span class="history-meta">
        {isCollectionItemsLoading
          ? "Refreshing..."
          : `${collection.requestCount} request${collection.requestCount === 1 ? "" : "s"}`}
      </span>
    </div>

    {#if collectionDnd.isDragging}
      <button
        class={[
          "collection-root-drop",
          collectionDnd.matchesDropIndicator(collection.id, null, "root") && "collection-root-drop-active"
        ]}
        type="button"
        aria-label="Move item to the top level of this collection"
        data-collection-drop="root"
        data-collection-id={collection.id}
      >
        <strong>Top level</strong>
        <span>Drop here to place the item outside folders.</span>
      </button>
    {/if}

    {#if collectionItems.length === 0 && !isCollectionItemsLoading}
      <div class="empty-state collection-empty-state">
        <strong>No folders or saved requests yet</strong>
        <span>Use New folder here, or save the active request into this collection from the Requests screen.</span>
      </div>
    {:else}
      {#snippet renderItems(items: CollectionItemSummary[], depth: number)}
        <div class={["collection-item-tree", depth > 0 && "collection-item-tree-nested"]}>
          {#each items as item (item.id)}
            <article
              class={[
                "collection-item",
                item.kind === "folder" && "collection-folder-item",
                item.id === revealedItemId && "collection-item-revealed",
                collectionDnd.isDraggingItem(item.id) && "collection-item-dragging",
                collection?.id && collectionDnd.matchesDropIndicator(collection.id, item.id, "before") && "collection-drop-target-before",
                collection?.id && collectionDnd.matchesDropIndicator(collection.id, item.id, "after") && "collection-drop-target-after",
                item.kind === "folder" && collection?.id && collectionDnd.matchesDropIndicator(collection.id, item.id, "inside") && "collection-drop-target-inside"
              ]}
              style={`--tree-depth:${depth};`}
              onpointerdown={item.kind === "request" ? (event) => handleItemPointerDown(event, item) : undefined}
              data-collection-drop={collection ? "item" : undefined}
              data-collection-id={collection?.id}
              data-item-id={item.id}
              data-item-kind={item.kind}
              data-collection-reveal-id={item.id}
            >
              <div class="saved-request-meta">
                {#if item.kind === "folder"}
                  <!-- svelte-ignore a11y_no_static_element_interactions -->
                  <div class="collection-folder-heading" onpointerdown={(event) => handleItemPointerDown(event, item)}>
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
                      {#if item.requestType === "websocket" || item.requestType === "socketio"}
                        <span class="protocol-badge">{item.requestType === "socketio" ? "S.IO" : "WS"}</span>
                      {:else}
                        <span class={`method-badge method-${item.method?.toLowerCase() ?? "get"}`}>{item.method ?? "GET"}</span>
                      {/if}
                      <span class="collection-request-url-inline">{item.url ?? ""}</span>
                    {/if}
                  </strong>

                  {#if item.name}
                    <div class="collection-request-subline">
                      {#if item.requestType === "websocket" || item.requestType === "socketio"}
                        <span class="protocol-badge">{item.requestType === "socketio" ? "S.IO" : "WS"}</span>
                      {:else}
                        <span class={`method-badge method-${item.method?.toLowerCase() ?? "get"}`}>{item.method ?? "GET"}</span>
                      {/if}
                      <span class="collection-request-url-line">{item.url ?? ""}</span>
                    </div>
                  {/if}

                  <span class="history-meta">Updated {formatUpdatedAt(item.updatedAt)}</span>
                {/if}
              </div>

              <div class="saved-request-actions">
                {#if item.kind === "folder"}
                  <button class="tab-button button-compact" type="button" onclick={() => onCreateChildFolder(item.id)}>
                    Add subfolder
                  </button>
                {:else}
                  <button class="tab-button button-compact" type="button" onclick={() => handleOpenSavedRequestClick(item)}>
                    Open in {item.requestType === "websocket" || item.requestType === "socketio" ? "WebSockets" : "Requests"}
                  </button>
                {/if}

                <button
                  class="button-secondary button-compact"
                  type="button"
                  onclick={() => onMoveCollectionItem(item)}
                  disabled={collections.isMovingCollectionItem}
                >
                  Move…
                </button>

                <button
                  class="icon-button row-action-button row-action-danger button-compact"
                  type="button"
                  title={`Delete ${item.name}`}
                  aria-label={`Delete ${item.name}`}
                  onclick={() => onDeleteCollectionItem(item)}
                  disabled={pendingDeleteCollectionItemId === item.id}
                >
                  {#if pendingDeleteCollectionItemId === item.id}
                    <span class="sr-only">Deleting {item.name}</span>
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
  </section>
  {/if}
</div>
