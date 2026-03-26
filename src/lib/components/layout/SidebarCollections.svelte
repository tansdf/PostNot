<script lang="ts">
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import { onMount } from "svelte";
  import { SvelteSet } from "svelte/reactivity";

  import { collections } from "$lib/stores/collections.svelte";

  let expandedCollectionIds = new SvelteSet<string>();

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
    void collections.ensureLoaded();
  });

  async function handleCreateCollection() {
    const collection = await collections.createBlankCollection();
    if (!collection) {
      return;
    }

    await goto(`/collections?collectionId=${encodeURIComponent(collection.id)}`);
  }

  async function openCollection(collectionId: string) {
    await collections.selectCollection(collectionId);
    await goto(`/collections?collectionId=${encodeURIComponent(collectionId)}`);
  }

  async function toggleCollection(collectionId: string) {
    if (expandedCollectionIds.has(collectionId)) {
      expandedCollectionIds.delete(collectionId);
      return;
    }

    expandedCollectionIds.add(collectionId);

    if (!(collections.savedRequestsByCollection[collectionId]?.length)) {
      await collections.loadSavedRequests(collectionId);
    }
  }

  async function openSavedRequest(collectionId: string, itemId: string) {
    await collections.selectCollection(collectionId);
    await goto(`/?savedRequestId=${encodeURIComponent(itemId)}`);
  }
</script>

<section class="sidebar-section">
  <div class="sidebar-section-header">
    <h2>Collections</h2>
    <button
      class="sidebar-plus-button"
      type="button"
      onclick={handleCreateCollection}
      disabled={collections.isCreatingCollection}
      aria-label="Create collection"
      title="Create collection"
    >
      {collections.isCreatingCollection ? "..." : "+"}
    </button>
  </div>

  <div class="sidebar-section-scroll">
    {#if collections.errorText}
      <div class="sidebar-inline-error">{collections.errorText}</div>
    {/if}

    {#if collections.collections.length === 0 && !collections.isCollectionsLoading}
      <div class="sidebar-empty-state">Create a collection to keep saved requests close at hand.</div>
    {:else}
      <div class="sidebar-collection-stack">
        {#each collections.collections as collection (collection.id)}
          <article class={["sidebar-collection-card", collections.selectedCollectionId === collection.id && "sidebar-collection-active"]}>
            <div class="sidebar-collection-row">
              <button class="sidebar-collection-button" type="button" onclick={() => openCollection(collection.id)}>
                <strong>{collection.name}</strong>
                <span>{collection.requestCount} request{collection.requestCount === 1 ? "" : "s"}</span>
                <span class="sidebar-collection-meta">Updated {formatUpdatedAt(collection.updatedAt)}</span>
              </button>

              <button
                class="sidebar-toggle-button"
                type="button"
                onclick={() => toggleCollection(collection.id)}
                aria-expanded={expandedCollectionIds.has(collection.id)}
                aria-label={expandedCollectionIds.has(collection.id) ? "Collapse collection" : "Expand collection"}
                title={expandedCollectionIds.has(collection.id) ? "Collapse" : "Expand"}
              >
                <span
                  class={["sidebar-toggle-icon", expandedCollectionIds.has(collection.id) && "sidebar-toggle-icon-expanded"]}
                  aria-hidden="true"
                >
                  &gt;
                </span>
              </button>
            </div>

            {#if expandedCollectionIds.has(collection.id)}
              <div class="sidebar-request-stack">
                {#if collections.isSavedRequestsLoading && !(collections.savedRequestsByCollection[collection.id]?.length)}
                  <span class="sidebar-collection-meta">Loading requests...</span>
                {:else if (collections.savedRequestsByCollection[collection.id] ?? []).length === 0}
                  <span class="sidebar-collection-meta">No saved requests yet.</span>
                {:else}
                  {#each collections.savedRequestsByCollection[collection.id] ?? [] as item (item.id)}
                    <button
                      class={["sidebar-request-link", page.url.searchParams.get("savedRequestId") === item.id && "sidebar-request-active"]}
                      type="button"
                      onclick={() => openSavedRequest(collection.id, item.id)}
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
  </div>
</section>
