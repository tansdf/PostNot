<script lang="ts">
  import { goto } from "$app/navigation";
  import { page } from "$app/stores";
  import { get } from "svelte/store";

  import CollectionsPanel from "$lib/components/collections/CollectionsPanel.svelte";
  import { importRequests } from "$lib/api/commands";
  import AppShell from "$lib/components/layout/AppShell.svelte";
  import {
    collectionsState,
    ensureCollectionsLoaded,
    loadCollections,
    removeCollection,
    removeSavedRequestItem,
    saveCollectionDetails,
    selectCollection,
    selectedCollection,
    selectedSavedRequests
  } from "$lib/stores/collections";
  let isSavingCollection = false;
  let requestedCollectionId = "";
  let importSource = "";
  let isImporting = false;
  let importErrorText = "";
  let importSuccessText = "";
  let isImportModalOpen = false;
  let importFileInput: HTMLInputElement | null = null;

  $: requestedCollectionId = $page.url.searchParams.get("collectionId") ?? "";

  $: void syncCollectionFromRoute(requestedCollectionId);

  async function syncCollectionFromRoute(collectionId: string) {
    await ensureCollectionsLoaded(collectionId);

    if (collectionId && get(collectionsState).selectedCollectionId !== collectionId) {
      await selectCollection(collectionId);
      return;
    }

    if (!collectionId) {
      const fallbackCollectionId = get(collectionsState).selectedCollectionId;
      if (fallbackCollectionId) {
        await goto(`/collections?collectionId=${encodeURIComponent(fallbackCollectionId)}`, {
          replaceState: true,
          noScroll: true,
          keepFocus: true
        });
      }
    }
  }

  async function handleSaveCollection(name: string, description: string) {
    const collection = get(selectedCollection);
    if (!collection) {
      return false;
    }

    isSavingCollection = true;

    try {
      const saved = await saveCollectionDetails(collection.id, {
        name: name.trim(),
        description: description.trim()
      });

      return Boolean(saved);
    } finally {
      isSavingCollection = false;
    }
  }

  async function handleDeleteCollection(collectionId: string) {
    if (!window.confirm("Delete this collection and all saved requests inside it?")) {
      return;
    }

    await removeCollection(collectionId);

    const nextCollectionId = get(collectionsState).selectedCollectionId;
    await goto(nextCollectionId ? `/collections?collectionId=${encodeURIComponent(nextCollectionId)}` : "/collections", {
      replaceState: true,
      noScroll: true,
      keepFocus: true
    });
  }

  async function handleOpenSavedRequest(itemId: string) {
    await goto(`/?savedRequestId=${encodeURIComponent(itemId)}`);
  }

  async function handleDeleteSavedRequest(itemId: string) {
    const collection = get(selectedCollection);
    if (!collection) {
      return;
    }

    if (!window.confirm("Delete this saved request?")) {
      return;
    }

    await removeSavedRequestItem(collection.id, itemId);
    await loadCollections(collection.id);
  }

  async function handleImportRequests() {
    importErrorText = "";
    importSuccessText = "";

    const source = importSource.trim();
    if (!source) {
      importErrorText = "Open a Postman collection JSON file or paste its JSON payload to import.";
      return;
    }

    isImporting = true;

    try {
      const result = await importRequests({
        format: "postman",
        source,
        targetCollectionId: null
      });

      await loadCollections(result.collectionId);

      await goto(`/collections?collectionId=${encodeURIComponent(result.collectionId)}`, {
        replaceState: true,
        noScroll: true,
        keepFocus: true
      });

      importSuccessText = `${result.importedRequestCount} request${
        result.importedRequestCount === 1 ? "" : "s"
      } imported into ${result.collectionName}.`;
      importSource = "";
    } catch (error) {
      importErrorText = error instanceof Error ? error.message : String(error);
    } finally {
      isImporting = false;
    }
  }

  function openImportModal() {
    importErrorText = "";
    isImportModalOpen = true;
  }

  function closeImportModal() {
    isImportModalOpen = false;
    importErrorText = "";
  }
</script>

<svelte:head>
  <title>PostNot Collections</title>
</svelte:head>

<AppShell title="PostNot" subtitle="Organize saved requests and keep them ready for reuse.">
  <CollectionsPanel
    collection={$selectedCollection}
    savedRequests={$selectedSavedRequests}
    isCollectionsLoading={$collectionsState.isCollectionsLoading}
    isSavedRequestsLoading={$collectionsState.isSavedRequestsLoading}
    {isSavingCollection}
    pendingDeleteCollectionId={$collectionsState.pendingDeleteCollectionId}
    pendingDeleteSavedRequestId={$collectionsState.pendingDeleteSavedRequestId}
    errorText={$collectionsState.errorText}
    {isImporting}
    {importSuccessText}
    onOpenImport={openImportModal}
    onSaveCollection={handleSaveCollection}
    onDeleteCollection={handleDeleteCollection}
    onOpenSavedRequest={handleOpenSavedRequest}
    onDeleteSavedRequest={handleDeleteSavedRequest}
  />

  {#if isImportModalOpen}
    <div
      class="modal-backdrop"
      role="button"
      tabindex="0"
      aria-label="Close import dialog"
      on:click|self={closeImportModal}
      on:keydown={(event) => {
        if (event.key === "Escape" || event.key === "Enter" || event.key === " ") {
          event.preventDefault();
          closeImportModal();
        }
      }}
    >
      <div class="panel save-dialog" role="dialog" tabindex="-1" aria-modal="true" aria-labelledby="import-collection-title">
        <div class="editor-header import-dialog-header">
          <h2 id="import-collection-title">Import</h2>
          <span class="history-meta">Postman Collection v2.1 JSON</span>
        </div>

        <div class="editor-block">
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

          <div class="collections-page-actions">
            <button class="ghost-button" type="button" on:click={() => importFileInput?.click()}>
              Open JSON file
            </button>
            <button
              class="send-button"
              type="button"
              on:click={async () => {
                await handleImportRequests();
                if (!importErrorText) {
                  closeImportModal();
                }
              }}
              disabled={isImporting}
            >
              {isImporting ? "Importing..." : "Import"}
            </button>
            <button class="ghost-button" type="button" on:click={closeImportModal}>
              Cancel
            </button>
          </div>
        </div>
      </div>
    </div>
  {/if}
</AppShell>
