<script lang="ts">
  import { goto } from "$app/navigation";
  import { resolve } from "$app/paths";
  import { page } from "$app/state";
  import { onMount } from "svelte";
  import { SvelteSet } from "svelte/reactivity";

  import { getCollectionSidebarState, saveCollectionSidebarState } from "$lib/api/commands";
  import type { CollectionItemSummary } from "$lib/api/types";
  import FolderGlyph from "$lib/components/icons/FolderGlyph.svelte";
  import { collections } from "$lib/stores/collections.svelte";

  let expandedCollectionIds = new SvelteSet<string>();
  let expandedFolderIds = new SvelteSet<string>();
  let hasLoadedSidebarState = $state(false);
  let isSavingSidebarState = false;

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
    void initializeSidebarState();
  });

  async function initializeSidebarState() {
    await collections.ensureLoaded();

    try {
      const sidebarState = await getCollectionSidebarState();
      expandedCollectionIds.clear();
      for (const id of sidebarState.expandedCollectionIds) {
        expandedCollectionIds.add(id);
      }
      expandedFolderIds.clear();
      for (const id of sidebarState.expandedFolderIds) {
        expandedFolderIds.add(id);
      }

      const validExpandedCollectionIds = sidebarState.expandedCollectionIds.filter((collectionId) =>
        collections.collections.some((collection) => collection.id === collectionId)
      );

      if (validExpandedCollectionIds.length > 0) {
        await Promise.all(
          validExpandedCollectionIds.map((collectionId) => collections.loadCollectionItems(collectionId))
        );
      }

      await pruneAndPersistExpandedState();
    } finally {
      hasLoadedSidebarState = true;
    }
  }

  async function persistSidebarState() {
    if (!hasLoadedSidebarState || isSavingSidebarState) {
      return;
    }

    isSavingSidebarState = true;

    try {
      await saveCollectionSidebarState({
        expandedCollectionIds: Array.from(expandedCollectionIds),
        expandedFolderIds: Array.from(expandedFolderIds)
      });
    } finally {
      isSavingSidebarState = false;
    }
  }

  async function pruneAndPersistExpandedState() {
    const validCollectionIds = new Set(collections.collections.map((collection) => collection.id));
    const validFolderIds = new Set<string>();

    for (const items of Object.values(collections.collectionItemsByCollection)) {
      collectFolderIds(items, validFolderIds);
    }

    let didChange = false;

    for (const collectionId of Array.from(expandedCollectionIds)) {
      if (!validCollectionIds.has(collectionId)) {
        expandedCollectionIds.delete(collectionId);
        didChange = true;
      }
    }

    for (const folderId of Array.from(expandedFolderIds)) {
      if (!validFolderIds.has(folderId)) {
        expandedFolderIds.delete(folderId);
        didChange = true;
      }
    }

    if (didChange) {
      await persistSidebarState();
    }
  }

  async function handleCreateCollection() {
    const collection = await collections.createBlankCollection();
    if (!collection) {
      return;
    }

    await goto(resolve(`/collections?collectionId=${encodeURIComponent(collection.id)}`));
  }

  async function openCollection(collectionId: string) {
    await collections.selectCollection(collectionId);
    await goto(resolve(`/collections?collectionId=${encodeURIComponent(collectionId)}`));
  }

  async function toggleCollection(collectionId: string) {
    if (expandedCollectionIds.has(collectionId)) {
      expandedCollectionIds.delete(collectionId);
      await persistSidebarState();
      return;
    }

    expandedCollectionIds.add(collectionId);

    if (!(collections.collectionItemsByCollection[collectionId]?.length)) {
      await collections.loadCollectionItems(collectionId);
    }

    await persistSidebarState();
  }

  async function toggleFolder(folderId: string) {
    if (expandedFolderIds.has(folderId)) {
      expandedFolderIds.delete(folderId);
      await persistSidebarState();
      return;
    }

    expandedFolderIds.add(folderId);
    await persistSidebarState();
  }

  async function openSavedRequest(collectionId: string, itemId: string) {
    await collections.selectCollection(collectionId);
    await goto(resolve(`/?savedRequestId=${encodeURIComponent(itemId)}`));
  }

  function collectFolderIds(items: CollectionItemSummary[], target: Set<string>) {
    for (const item of items) {
      if (item.kind !== "folder") {
        continue;
      }

      target.add(item.id);
      collectFolderIds(item.children, target);
    }
  }

  $effect(() => {
    void collections.collections;
    void collections.collectionItemsByCollection;

    if (hasLoadedSidebarState) {
      void pruneAndPersistExpandedState();
    }
  });
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
                {#if collections.isCollectionItemsLoading && !(collections.collectionItemsByCollection[collection.id]?.length)}
                  <span class="sidebar-collection-meta">Loading items...</span>
                {:else if (collections.collectionItemsByCollection[collection.id] ?? []).length === 0}
                  <span class="sidebar-collection-meta">No saved requests yet.</span>
                {:else}
                  {#snippet renderSidebarItems(items: CollectionItemSummary[], depth: number)}
                    <div class={["sidebar-item-tree", depth > 0 && "sidebar-item-tree-nested"]}>
                      {#each items as item (item.id)}
                        {#if item.kind === "folder"}
                          <div class={["sidebar-folder-group", depth > 0 && "sidebar-folder-group-nested"]}>
                            <button
                              class={[
                                "sidebar-folder-button",
                                expandedFolderIds.has(item.id) && "sidebar-folder-open"
                              ]}
                              type="button"
                              onclick={() => toggleFolder(item.id)}
                              aria-expanded={expandedFolderIds.has(item.id)}
                              style={`--tree-depth:${depth};`}
                            >
                              <span class="sidebar-folder-icon" aria-hidden="true">
                                <FolderGlyph
                                  variant={expandedFolderIds.has(item.id) ? "sidebar-open" : "sidebar-closed"}
                                />
                              </span>
                              <span class="sidebar-folder-text">
                                <strong class="sidebar-folder-name">{item.name}</strong>
                                <span class="sidebar-collection-meta sidebar-folder-count">
                                  {item.children.length === 0
                                    ? "Empty"
                                    : `${item.children.length} item${item.children.length === 1 ? "" : "s"}`}
                                </span>
                              </span>
                            </button>

                            {#if expandedFolderIds.has(item.id)}
                              {@render renderSidebarItems(item.children, depth + 1)}
                            {/if}
                          </div>
                        {:else}
                          <button
                            class={["sidebar-request-link", page.url.searchParams.get("savedRequestId") === item.id && "sidebar-request-active"]}
                            type="button"
                            onclick={() => openSavedRequest(collection.id, item.id)}
                            style={`--tree-depth:${depth};`}
                          >
                            <strong class="sidebar-request-name">
                              {#if item.name}
                                {item.name}
                              {:else}
                                <span class={`method-badge method-${item.method?.toLowerCase() ?? "get"}`}>{item.method ?? "GET"}</span> {item.url ?? ""}
                              {/if}
                            </strong>
                            <span class="sidebar-request-url">
                              <span class={`method-badge method-${item.method?.toLowerCase() ?? "get"}`}>{item.method ?? "GET"}</span>
                              {item.url ?? ""}
                            </span>
                          </button>
                        {/if}
                      {/each}
                    </div>
                  {/snippet}

                  {@render renderSidebarItems(collections.collectionItemsByCollection[collection.id] ?? [], 0)}
                {/if}
              </div>
            {/if}
          </article>
        {/each}
      </div>
    {/if}
  </div>
</section>
