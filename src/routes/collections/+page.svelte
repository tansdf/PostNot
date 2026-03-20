<script lang="ts">
  import { goto } from "$app/navigation";
  import { page } from "$app/stores";
  import { get } from "svelte/store";

  import CollectionsPanel from "$lib/components/collections/CollectionsPanel.svelte";
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
    onSaveCollection={handleSaveCollection}
    onDeleteCollection={handleDeleteCollection}
    onOpenSavedRequest={handleOpenSavedRequest}
    onDeleteSavedRequest={handleDeleteSavedRequest}
  />
</AppShell>
