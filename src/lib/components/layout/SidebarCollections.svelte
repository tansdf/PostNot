<script lang="ts">
  import { goto } from "$app/navigation";
  import { page } from "$app/stores";
  import { onMount } from "svelte";

  import {
    collectionsState,
    createBlankCollection,
    ensureCollectionsLoaded,
    loadSavedRequests,
    selectCollection
  } from "$lib/stores/collections";

  let expandedCollectionIds = new Set<string>();

  function formatUpdatedAt(value: string) {
    try {
      return new Intl.DateTimeFormat(undefined, {
        month: "short",
        day: "numeric"
      }).format(new Date(value));
    } catch {
      return value;
    }
  }

  onMount(() => {
    void ensureCollectionsLoaded();
  });

  async function handleCreateCollection() {
    const collection = await createBlankCollection();
    if (!collection) {
      return;
    }

    await goto(`/collections?collectionId=${encodeURIComponent(collection.id)}`);
  }

  async function openCollection(collectionId: string) {
    await selectCollection(collectionId);
    await goto(`/collections?collectionId=${encodeURIComponent(collectionId)}`);
  }

  async function toggleCollection(collectionId: string) {
    const nextExpandedCollectionIds = new Set(expandedCollectionIds);

    if (nextExpandedCollectionIds.has(collectionId)) {
      nextExpandedCollectionIds.delete(collectionId);
      expandedCollectionIds = nextExpandedCollectionIds;
      return;
    }

    nextExpandedCollectionIds.add(collectionId);
    expandedCollectionIds = nextExpandedCollectionIds;

    if (!($collectionsState.savedRequestsByCollection[collectionId]?.length)) {
      await loadSavedRequests(collectionId);
    }
  }

  async function openSavedRequest(collectionId: string, itemId: string) {
    await selectCollection(collectionId);
    await goto(`/?savedRequestId=${encodeURIComponent(itemId)}`);
  }
</script>

<section class="sidebar-section">
  <div class="sidebar-section-header">
    <h2>Collections</h2>
    <button
      class="sidebar-plus-button"
      type="button"
      on:click={handleCreateCollection}
      disabled={$collectionsState.isCreatingCollection}
      aria-label="Create collection"
      title="Create collection"
    >
      {$collectionsState.isCreatingCollection ? "..." : "+"}
    </button>
  </div>

  {#if $collectionsState.errorText}
    <div class="sidebar-inline-error">{$collectionsState.errorText}</div>
  {/if}

  {#if $collectionsState.collections.length === 0 && !$collectionsState.isCollectionsLoading}
    <div class="sidebar-empty-state">Create a collection to keep saved requests close at hand.</div>
  {:else}
    <div class="sidebar-collection-stack">
      {#each $collectionsState.collections as collection (collection.id)}
        <article
          class:sidebar-collection-active={$collectionsState.selectedCollectionId === collection.id}
          class="sidebar-collection-card"
        >
          <div class="sidebar-collection-row">
            <button class="sidebar-collection-button" type="button" on:click={() => openCollection(collection.id)}>
              <strong>{collection.name}</strong>
              <span>{collection.requestCount} request{collection.requestCount === 1 ? "" : "s"}</span>
              <span class="sidebar-collection-meta">Updated {formatUpdatedAt(collection.updatedAt)}</span>
            </button>

            <button
              class="sidebar-toggle-button"
              type="button"
              on:click={() => toggleCollection(collection.id)}
              aria-expanded={expandedCollectionIds.has(collection.id)}
              aria-label={expandedCollectionIds.has(collection.id) ? "Collapse collection" : "Expand collection"}
              title={expandedCollectionIds.has(collection.id) ? "Collapse" : "Expand"}
            >
              <span
                class:sidebar-toggle-icon-expanded={expandedCollectionIds.has(collection.id)}
                class="sidebar-toggle-icon"
                aria-hidden="true"
              >
                &gt;
              </span>
            </button>
          </div>

          {#if expandedCollectionIds.has(collection.id)}
            <div class="sidebar-request-stack">
              {#if $collectionsState.isSavedRequestsLoading && !($collectionsState.savedRequestsByCollection[collection.id]?.length)}
                <span class="sidebar-collection-meta">Loading requests...</span>
              {:else if ($collectionsState.savedRequestsByCollection[collection.id] ?? []).length === 0}
                <span class="sidebar-collection-meta">No saved requests yet.</span>
              {:else}
                {#each $collectionsState.savedRequestsByCollection[collection.id] ?? [] as item (item.id)}
                  <button
                    class:sidebar-request-active={$page.url.searchParams.get("savedRequestId") === item.id}
                    class="sidebar-request-link"
                    type="button"
                    on:click={() => openSavedRequest(collection.id, item.id)}
                  >
                    <strong class="sidebar-request-name">{item.name || `${item.method} ${item.url}`}</strong>
                    <span class="sidebar-request-url">{item.method} {item.url}</span>
                  </button>
                {/each}
              {/if}
            </div>
          {/if}
        </article>
      {/each}
    </div>
  {/if}
</section>
