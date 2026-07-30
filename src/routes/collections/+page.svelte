<script lang="ts">
  import { goto } from "$app/navigation";
  import { resolve } from "$app/paths";
  import { page } from "$app/state";
  import { tick } from "svelte";

  import type { CollectionItemSummary } from "$lib/api/types";
  import { createStaleGuard } from "$lib/async-stale-guard";
  import { buildAccessibleMoveInput, type DraggedCollectionItem } from "$lib/collections/drag-and-drop";
  import CollectionsPanel from "$lib/components/collections/CollectionsPanel.svelte";
  import DialogShell from "$lib/components/layout/DialogShell.svelte";
  import { exportCollection, importRequests } from "$lib/api/commands";
  import { notifications } from "$lib/stores/notifications.svelte";
  import { collections } from "$lib/stores/collections.svelte";
  import { requestWorkspace } from "$lib/stores/request-workspace.svelte";
  import { realtimeWorkspace } from "$lib/stores/realtime-workspace.svelte";

  let isSavingCollection = $state(false);
  let importFormat = $state<"postman" | "openapi" | "postnot">("postman");
  let importSource = $state("");
  let isImporting = $state(false);
  let isExporting = $state(false);
  let isExportModalOpen = $state(false);
  let exportFormat = $state<"postman" | "postnot">("postnot");
  let importErrorText = $state("");
  let isImportModalOpen = $state(false);
  let importFileInput: HTMLInputElement | null = $state(null);
  let revealedItemId = $state("");
  let moveItem: CollectionItemSummary | null = $state(null);
  let moveTargetCollectionId = $state("");
  let moveTargetParentId = $state("");
  let moveAfterItemId = $state("");
  let moveErrorText = $state("");
  let isMoveTargetLoading = $state(false);
  let revealResetTimer: ReturnType<typeof setTimeout> | null = null;

  let requestedCollectionId = $derived(page.url.searchParams.get("collectionId") ?? "");
  let requestedItemId = $derived(page.url.searchParams.get("itemId") ?? "");

  const collectionRoute = createStaleGuard();
  const moveTargetLoad = createStaleGuard();

  $effect(() => {
    void syncCollectionFromRoute(requestedCollectionId, requestedItemId);
  });

  async function syncCollectionFromRoute(collectionId: string, itemId: string) {
    const seq = collectionRoute.next();
    await collections.ensureLoaded(collectionId);
    if (collectionRoute.isStale(seq)) {
      return;
    }

    if (collectionId && collections.selectedCollectionId !== collectionId) {
      await collections.selectCollection(collectionId);
      if (collectionRoute.isStale(seq)) {
        return;
      }
    }

    if (!collectionId) {
      resetReveal();
      const fallbackCollectionId = collections.selectedCollectionId;
      if (fallbackCollectionId) {
        await goto(resolve(`/collections?collectionId=${encodeURIComponent(fallbackCollectionId)}`), {
          replaceState: true,
          noScroll: true,
          keepFocus: true
        });
      }
      return;
    }

    if (!itemId) {
      resetReveal();
      return;
    }

    await revealCollectionItem(itemId, seq);
  }

  async function revealCollectionItem(itemId: string, seq: number) {
    revealedItemId = itemId;
    await tick();
    if (collectionRoute.isStale(seq)) {
      return;
    }

    const escapedId =
      typeof CSS !== "undefined" && typeof CSS.escape === "function"
        ? CSS.escape(itemId)
        : itemId.replace(/["\\]/g, "\\$&");

    const element = document.querySelector<HTMLElement>(
      `[data-collection-reveal-id="${escapedId}"]`
    );

    if (!element) {
      resetReveal();
      return;
    }

    element.scrollIntoView({
      block: "start",
      behavior: "smooth"
    });

    if (revealResetTimer) {
      clearTimeout(revealResetTimer);
    }

    revealResetTimer = setTimeout(() => {
      revealedItemId = "";
      revealResetTimer = null;
    }, 1800);
  }

  function resetReveal() {
    revealedItemId = "";
    if (revealResetTimer) {
      clearTimeout(revealResetTimer);
      revealResetTimer = null;
    }
  }

  async function handleCreateCollection() {
    const collection = await collections.createBlankCollection();
    if (!collection) {
      return;
    }

    await goto(resolve(`/collections?collectionId=${encodeURIComponent(collection.id)}`), {
      noScroll: true,
      keepFocus: true
    });
  }

  async function handleSaveCollection(
    name: string,
    description: string,
    preRequestScript: string,
    testScript: string
  ) {
    const collection = collections.selectedCollection;
    if (!collection) {
      return false;
    }

    isSavingCollection = true;

    try {
      const saved = await collections.saveDetails(collection.id, {
        name: name.trim(),
        description: description.trim(),
        preRequestScript,
        testScript
      });

      return Boolean(saved);
    } finally {
      isSavingCollection = false;
    }
  }

  async function handleCreateFolder(parentId?: string) {
    const collection = collections.selectedCollection;
    if (!collection) {
      return;
    }

    const name = window.prompt(parentId ? "Folder name for the new subfolder:" : "Folder name:", "New folder")?.trim();
    if (!name) {
      return;
    }

    await collections.createFolder(collection.id, name, parentId ?? null);
  }

  async function handleSaveFolder(
    itemId: string,
    name: string,
    preRequestScript: string,
    testScript: string
  ) {
    const collection = collections.selectedCollection;
    if (!collection) {
      return false;
    }

    const saved = await collections.saveFolderDetails(collection.id, itemId, {
      name: name.trim(),
      preRequestScript,
      testScript
    });

    return Boolean(saved);
  }

  async function handleDeleteCollection(collectionId: string) {
    if (!window.confirm("Delete this collection and all saved requests inside it?")) {
      return;
    }

    const deleted = await collections.removeCollection(collectionId);
    if (!deleted) {
      return;
    }

    requestWorkspace.unlinkSavedRequestsForCollection(collectionId);
    realtimeWorkspace.unlinkSavedRequestsForCollection(collectionId);

    const nextCollectionId = collections.selectedCollectionId;
    const navOpts = { replaceState: true, noScroll: true, keepFocus: true } as const;
    if (nextCollectionId) {
      await goto(resolve(`/collections?collectionId=${encodeURIComponent(nextCollectionId)}`), navOpts);
    } else {
      await goto(resolve("/collections"), navOpts);
    }
  }

  async function handleOpenSavedRequest(item: CollectionItemSummary) {
    if (item.requestType === "websocket" || item.requestType === "socketio") {
      await goto(resolve(`/websockets?savedRequestId=${encodeURIComponent(item.id)}`));
      return;
    }
    await goto(resolve(`/?savedRequestId=${encodeURIComponent(item.id)}`));
  }

  async function handleDeleteCollectionItem(item: CollectionItemSummary) {
    const collection = collections.selectedCollection;
    if (!collection) {
      return;
    }

    const message =
      item.kind === "folder"
        ? "Delete this folder and everything inside it?"
        : "Delete this saved request?";

    if (!window.confirm(message)) {
      return;
    }

    const deletedSavedRequestIds = collectSavedRequestIds(item);
    const deleted = await collections.removeCollectionItem(collection.id, item.id, item.name || "Collection item");
    if (!deleted) {
      return;
    }

    requestWorkspace.unlinkSavedRequests(deletedSavedRequestIds);
    realtimeWorkspace.unlinkSavedRequests(deletedSavedRequestIds);
    await collections.loadCollections(collection.id);
  }

  function collectSavedRequestIds(item: CollectionItemSummary): string[] {
    if (item.kind === "request") {
      return [item.id];
    }

    return item.children.flatMap(collectSavedRequestIds);
  }

  function findItemLocation(
    items: CollectionItemSummary[],
    itemId: string,
    parentId: string | null = null
  ): { item: CollectionItemSummary; parentId: string | null; siblings: CollectionItemSummary[]; index: number } | null {
    for (const [index, item] of items.entries()) {
      if (item.id === itemId) {
        return { item, parentId, siblings: items, index };
      }
      if (item.kind === "folder") {
        const nested = findItemLocation(item.children, itemId, item.id);
        if (nested) return nested;
      }
    }
    return null;
  }

  function containsItem(item: CollectionItemSummary, itemId: string): boolean {
    return item.id === itemId || item.children.some((child) => containsItem(child, itemId));
  }

  function moveFolderTargets() {
    const items = collections.collectionItemsByCollection[moveTargetCollectionId] ?? [];
    const targets: Array<{ id: string; name: string; depth: number }> = [
      { id: "", name: "Collection root", depth: 0 }
    ];

    function visit(children: CollectionItemSummary[], depth: number) {
      for (const child of children) {
        if (child.kind !== "folder" || (moveItem?.kind === "folder" && containsItem(moveItem, child.id))) {
          continue;
        }
        targets.push({ id: child.id, name: child.name || "Untitled folder", depth });
        visit(child.children, depth + 1);
      }
    }

    visit(items, 0);
    return targets;
  }

  function movePositionItems() {
    const items = collections.collectionItemsByCollection[moveTargetCollectionId] ?? [];
    const siblings = moveTargetParentId
      ? findItemLocation(items, moveTargetParentId)?.item.children ?? []
      : items;
    return siblings.filter((item) => item.id !== moveItem?.id);
  }

  function openMoveDialog(item: CollectionItemSummary) {
    moveTargetLoad.next();
    moveErrorText = "";
    isMoveTargetLoading = false;
    moveItem = item;
    moveTargetCollectionId = item.collectionId;

    const sourceItems = collections.collectionItemsByCollection[item.collectionId] ?? [];
    const location = findItemLocation(sourceItems, item.id);
    moveTargetParentId = location?.parentId ?? "";
    moveAfterItemId = location && location.index > 0 ? location.siblings[location.index - 1]?.id ?? "" : "";
  }

  async function handleMoveTargetCollectionChange(collectionId: string) {
    moveTargetCollectionId = collectionId;
    const seq = moveTargetLoad.next();
    moveTargetParentId = "";
    moveAfterItemId = "";
    moveErrorText = "";

    if (collectionId in collections.collectionItemsByCollection) {
      isMoveTargetLoading = false;
      return;
    }

    isMoveTargetLoading = true;
    await collections.loadCollectionItems(collectionId);
    if (moveTargetLoad.isStale(seq) || moveTargetCollectionId !== collectionId) {
      return;
    }

    isMoveTargetLoading = false;
    if (!(collectionId in collections.collectionItemsByCollection)) {
      moveErrorText = collections.errorText || "The destination collection could not be loaded.";
    }
  }

  function closeMoveDialog() {
    moveTargetLoad.next();
    moveItem = null;
    moveTargetCollectionId = "";
    moveTargetParentId = "";
    moveAfterItemId = "";
    moveErrorText = "";
    isMoveTargetLoading = false;
  }

  async function confirmMove() {
    if (!moveItem) return;

    const sourceItems = collections.collectionItemsByCollection[moveItem.collectionId] ?? [];
    const targetItems = collections.collectionItemsByCollection[moveTargetCollectionId] ?? [];
    const dragged: DraggedCollectionItem = {
      itemId: moveItem.id,
      collectionId: moveItem.collectionId,
      parentId: moveItem.parentId ?? null,
      name: moveItem.name,
      kind: moveItem.kind
    };
    const input = buildAccessibleMoveInput({
      dragged,
      sourceItems,
      targetItems,
      target: {
        targetCollectionId: moveTargetCollectionId,
        targetParentId: moveTargetParentId || null,
        afterItemId: moveAfterItemId || null
      }
    });

    if (!input) {
      moveErrorText = "Choose a different destination or position. A folder cannot be moved inside itself or one of its subfolders.";
      return;
    }

    moveErrorText = "";
    const moved = await collections.moveCollectionItem(moveItem.id, moveItem.collectionId, input);
    if (moved) closeMoveDialog();
    else moveErrorText = collections.errorText || "The collection item could not be moved.";
  }

  function openExportModal() {
    exportFormat = "postnot";
    isExportModalOpen = true;
  }

  async function handleExportCollection() {
    const collection = collections.selectedCollection;
    if (!collection) {
      return;
    }

    isExporting = true;

    try {
      collections.errorText = "";
      const result = await exportCollection(collection.id, exportFormat);
      if (result) {
        isExportModalOpen = false;
        const omitted = result.omittedRealtimeRequestCount ?? 0;
        notifications.success(
          omitted
            ? `${result.filePath}. ${omitted} realtime request${omitted === 1 ? " was" : "s were"} omitted.`
            : result.filePath,
          "Collection exported"
        );
      }
    } catch (error) {
      collections.errorText = error instanceof Error ? error.message : String(error);
    } finally {
      isExporting = false;
    }
  }

  async function handleImportRequests() {
    importErrorText = "";

    const source = importSource.trim();
    if (!source) {
      importErrorText = importFormat === "postman"
        ? "Open a Postman collection JSON file or paste its JSON payload to import."
        : importFormat === "postnot"
          ? "Open a PostNot collection JSON file or paste its JSON payload to import."
          : "Open an OpenAPI 3 JSON or YAML file, or paste its document payload to import.";
      return;
    }

    isImporting = true;

    try {
      collections.errorText = "";
      const result = await importRequests({
        format: importFormat,
        source,
        targetCollectionId: null
      });

      await collections.loadCollections(result.collectionId);

      await goto(resolve(`/collections?collectionId=${encodeURIComponent(result.collectionId)}`), {
        replaceState: true,
        noScroll: true,
        keepFocus: true
      });

      notifications.success(
        `${result.importedRequestCount} request${result.importedRequestCount === 1 ? "" : "s"} imported into ${result.collectionName}.`,
        importFormat === "postman" ? "Collection imported" : importFormat === "postnot" ? "PostNot collection imported" : "OpenAPI collection imported",
        result.details
          ? {
              details: {
                title: importFormat === "postman" ? "Postman import details" : importFormat === "postnot" ? "PostNot import details" : "OpenAPI import details",
                summary: result.details.summary,
                items: result.details.importedItems,
                warnings: result.details.warnings,
                errors: result.details.errors
              }
            }
          : undefined
      );
      importSource = "";
    } catch (error) {
      importErrorText = error instanceof Error ? error.message : String(error);
      notifications.error(importErrorText, "Import failed", {
        details: {
          title: "Import failure details",
          summary: "No collection changes were saved.",
          errors: [importErrorText]
        }
      });
    } finally {
      isImporting = false;
    }
  }

  function openImportModal() {
    importFormat = "postman";
    importErrorText = "";
    isImportModalOpen = true;
  }

  function closeImportModal() {
    isImportModalOpen = false;
    importFormat = "postman";
    importSource = "";
    importErrorText = "";
  }
</script>

<svelte:head>
  <title>PostNot Collections</title>
</svelte:head>

<CollectionsPanel
    collection={collections.selectedCollection}
    collectionItems={collections.selectedCollectionItems}
    isCollectionsLoading={collections.isCollectionsLoading}
    isCollectionItemsLoading={collections.isCollectionItemsLoading}
    {isSavingCollection}
    isCreatingFolder={collections.isCreatingFolder}
    pendingSaveFolderId={collections.pendingSaveFolderId}
    pendingDeleteCollectionId={collections.pendingDeleteCollectionId}
    pendingDeleteCollectionItemId={collections.pendingDeleteCollectionItemId}
    {revealedItemId}
    errorText={collections.errorText}
    {isImporting}
    {isExporting}
    onOpenImport={openImportModal}
    onCreateCollection={handleCreateCollection}
    onCreateRootFolder={() => handleCreateFolder()}
    onCreateChildFolder={(parentId: string) => handleCreateFolder(parentId)}
    onExportCollection={openExportModal}
    onSaveCollection={handleSaveCollection}
    onSaveFolder={handleSaveFolder}
    onDeleteCollection={handleDeleteCollection}
    onOpenSavedRequest={handleOpenSavedRequest}
    onMoveCollectionItem={openMoveDialog}
    onDeleteCollectionItem={handleDeleteCollectionItem}
  />

  {#if isExportModalOpen}
    <DialogShell ariaLabelledby="export-collection-title" onDismiss={() => (isExportModalOpen = false)} sizeClass="save-dialog">
      <div class="editor-header import-dialog-header">
        <div>
          <h2 id="export-collection-title">Export collection</h2>
          <span class="history-meta">{collections.selectedCollection?.name}</span>
        </div>
      </div>
      <div class="editor-block modal-scroll-body">
        <fieldset class="settings-theme-group">
          <legend class="field-label">Format</legend>
          <label class={["settings-theme-option", exportFormat === "postnot" && "settings-theme-option-active"]}>
            <input type="radio" name="collection-export-format" value="postnot" bind:group={exportFormat} />
            <span class="settings-theme-copy"><strong>PostNot JSON</strong><span>Lossless export for HTTP, WebSocket, Socket.IO, folders, and scripts.</span></span>
          </label>
          <label class={["settings-theme-option", exportFormat === "postman" && "settings-theme-option-active"]}>
            <input type="radio" name="collection-export-format" value="postman" bind:group={exportFormat} />
            <span class="settings-theme-copy"><strong>Postman Collection v2.1</strong><span>Exports HTTP requests only. Realtime definitions will be reported and omitted.</span></span>
          </label>
        </fieldset>
        {#if exportFormat === "postman"}
          <div class="feedback feedback-warning">WebSocket and Socket.IO definitions cannot be represented by the Postman export format.</div>
        {/if}
        <div class="collections-page-actions">
          <button class="button-secondary" type="button" onclick={() => (isExportModalOpen = false)}>Cancel</button>
          <button class="button-primary" type="button" onclick={handleExportCollection} disabled={isExporting}>{isExporting ? "Exporting…" : "Export collection"}</button>
        </div>
      </div>
    </DialogShell>
  {/if}

  {#if moveItem}
    <DialogShell ariaLabelledby="move-collection-item-title" onDismiss={closeMoveDialog}>
      <div class="editor-header import-dialog-header">
        <div>
          <h2 id="move-collection-item-title">Move {moveItem.kind === "folder" ? "folder" : "saved request"}</h2>
          <span class="history-meta">{moveItem.name || "Untitled item"}</span>
        </div>
      </div>

      <div class="editor-block modal-scroll-body move-item-dialog-body">
        <label>
          <span class="field-label">Collection</span>
          <select
            class="text-input"
            bind:value={moveTargetCollectionId}
            onchange={(event) => void handleMoveTargetCollectionChange(event.currentTarget.value)}
          >
            {#each collections.collections as targetCollection (targetCollection.id)}
              <option value={targetCollection.id}>{targetCollection.name}</option>
            {/each}
          </select>
        </label>

        <label>
          <span class="field-label">Folder</span>
          <select
            class="text-input"
            bind:value={moveTargetParentId}
            disabled={isMoveTargetLoading}
            onchange={() => {
              moveAfterItemId = "";
              moveErrorText = "";
            }}
          >
            {#each moveFolderTargets() as target (`${moveTargetCollectionId}-${target.id || "root"}`)}
              <option value={target.id}>{`${"— ".repeat(target.depth)}${target.name}`}</option>
            {/each}
          </select>
        </label>

        <label>
          <span class="field-label">Position</span>
          <select class="text-input" bind:value={moveAfterItemId} disabled={isMoveTargetLoading} onchange={() => (moveErrorText = "")}>
            <option value="">First</option>
            {#each movePositionItems() as sibling (sibling.id)}
              <option value={sibling.id}>After {sibling.name || (sibling.kind === "folder" ? "Untitled folder" : sibling.url || "Untitled request")}</option>
            {/each}
          </select>
        </label>

        {#if moveErrorText}
          <div class="feedback feedback-error" role="alert">{moveErrorText}</div>
        {/if}

        <div class="collections-page-actions">
          <button class="button-secondary" type="button" onclick={closeMoveDialog}>Cancel</button>
          <button
            class="button-primary"
            type="button"
            onclick={confirmMove}
            disabled={isMoveTargetLoading || collections.isMovingCollectionItem}
          >
            {isMoveTargetLoading ? "Loading…" : collections.isMovingCollectionItem ? "Moving…" : "Move"}
          </button>
        </div>
      </div>
    </DialogShell>
  {/if}

  {#if isImportModalOpen}
    <DialogShell ariaLabelledby="import-collection-title" onDismiss={closeImportModal}>
        <div class="editor-header import-dialog-header">
          <h2 id="import-collection-title">Import</h2>
          <span class="history-meta">
            {importFormat === "postman" ? "Postman Collection v2.1 JSON" : importFormat === "postnot" ? "PostNot collection JSON" : "OpenAPI 3 JSON or YAML"}
          </span>
        </div>

        <div class="editor-block modal-scroll-body">
          <div class="import-format-toggle" role="tablist" aria-label="Choose collection import format">
            <button
              class={["button-secondary", "button-compact", importFormat === "postman" && "toggle-active"]}
              type="button"
              role="tab"
              aria-selected={importFormat === "postman"}
              onclick={() => {
                importFormat = "postman";
                importErrorText = "";
              }}
            >
              Postman
            </button>
            <button
              class={["button-secondary", "button-compact", importFormat === "openapi" && "toggle-active"]}
              type="button"
              role="tab"
              aria-selected={importFormat === "openapi"}
              onclick={() => {
                importFormat = "openapi";
                importErrorText = "";
              }}
            >
              OpenAPI 3
            </button>
            <button
              class={["button-secondary", "button-compact", importFormat === "postnot" && "toggle-active"]}
              type="button"
              role="tab"
              aria-selected={importFormat === "postnot"}
              onclick={() => {
                importFormat = "postnot";
                importErrorText = "";
              }}
            >
              PostNot
            </button>
          </div>

          <p class="field-help">
            {importFormat === "postman"
              ? "Import a Postman collection by opening a JSON file or pasting the collection payload directly."
              : importFormat === "postnot"
                ? "Import a lossless PostNot collection, including WebSocket and Socket.IO definitions."
                : "Import an OpenAPI 3 document by opening a JSON or YAML file or pasting the document payload directly."}
          </p>

          <label>
            <span class="field-label">Paste source</span>
            <textarea
              class="text-input collections-import-source"
              bind:value={importSource}
              placeholder={importFormat === "postman"
                ? '{ "info": { "name": "My collection" }, "item": [...] }'
                : importFormat === "postnot"
                  ? '{ "schema": "https://post-not.com/schemas/collection.json", "version": 1, ... }'
                  : 'openapi: 3.0.3\ninfo:\n  title: Example API\npaths:\n  /items:\n    get:\n      summary: List items'}
            ></textarea>
          </label>

          <input
            bind:this={importFileInput}
            class="sr-only"
            type="file"
            aria-label="Open collection import file"
            accept={importFormat === "postman" || importFormat === "postnot"
              ? ".json,application/json"
              : ".json,.yaml,.yml,application/json,application/yaml,text/yaml,text/x-yaml"}
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
              {importFormat === "postman" || importFormat === "postnot" ? "Open JSON file" : "Open file"}
            </button>
            <button
              class="button-primary"
              type="button"
              onclick={async () => {
                await handleImportRequests();
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
