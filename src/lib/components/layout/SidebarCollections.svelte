<script lang="ts">
  import { goto } from "$app/navigation";
  import { resolve } from "$app/paths";
  import { page } from "$app/state";
  import { onMount, tick, untrack } from "svelte";
  import { SvelteSet } from "svelte/reactivity";

  import { getCollectionSidebarState, saveCollectionSidebarState } from "$lib/api/commands";
  import type { CollectionItemSummary, CollectionSearchResult } from "$lib/api/types";
  import { type DraggedCollectionItem } from "$lib/collections/drag-and-drop";
  import FolderGlyph from "$lib/components/icons/FolderGlyph.svelte";
  import { collectionDnd } from "$lib/stores/collection-dnd.svelte";
  import { collectionSearch } from "$lib/stores/collection-search.svelte";
  import { collections } from "$lib/stores/collections.svelte";
  import { readCachedJson, writeCachedJson, UI_CACHE_KEYS } from "$lib/ui-cache";

  type CachedSidebarState = {
    expandedCollectionIds: string[];
    expandedFolderIds: string[];
  };

  type HighlightSegment = {
    text: string;
    matched: boolean;
  };

  const cachedSidebarState = readCachedJson<CachedSidebarState>(UI_CACHE_KEYS.sidebarExpanded);

  let expandedCollectionIds = new SvelteSet<string>(cachedSidebarState?.expandedCollectionIds ?? []);
  let expandedFolderIds = new SvelteSet<string>(cachedSidebarState?.expandedFolderIds ?? []);
  let hasLoadedSidebarState = $state(cachedSidebarState !== null);
  let isSavingSidebarState = false;
  let searchInput: HTMLInputElement | null = $state(null);
  let revealedSidebarCollectionId = $state("");
  let revealedSidebarItemId = $state("");
  let sidebarRevealResetTimer: ReturnType<typeof setTimeout> | null = null;

  function writeSidebarStateCache() {
    writeCachedJson(UI_CACHE_KEYS.sidebarExpanded, {
      expandedCollectionIds: Array.from(expandedCollectionIds),
      expandedFolderIds: Array.from(expandedFolderIds)
    });
  }

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

  function tokenizeQuery(query: string) {
    return query
      .trim()
      .toLowerCase()
      .split(/\s+/)
      .filter(Boolean);
  }

  function buildHighlightSegments(text: string, query: string): HighlightSegment[] {
    const tokens = tokenizeQuery(query);
    if (!text || tokens.length === 0) {
      return [{ text, matched: false }];
    }

    const normalizedText = text.toLowerCase();
    const ranges: Array<{ start: number; end: number }> = [];

    for (const token of tokens) {
      let startIndex = 0;

      while (startIndex < normalizedText.length) {
        const foundIndex = normalizedText.indexOf(token, startIndex);
        if (foundIndex === -1) {
          break;
        }

        ranges.push({
          start: foundIndex,
          end: foundIndex + token.length
        });
        startIndex = foundIndex + token.length;
      }
    }

    if (ranges.length === 0) {
      return [{ text, matched: false }];
    }

    ranges.sort((left, right) => left.start - right.start);

    const mergedRanges: Array<{ start: number; end: number }> = [];
    for (const range of ranges) {
      const previous = mergedRanges[mergedRanges.length - 1];
      if (!previous || range.start > previous.end) {
        mergedRanges.push({ ...range });
        continue;
      }

      previous.end = Math.max(previous.end, range.end);
    }

    const segments: HighlightSegment[] = [];
    let cursor = 0;

    for (const range of mergedRanges) {
      if (range.start > cursor) {
        segments.push({
          text: text.slice(cursor, range.start),
          matched: false
        });
      }

      segments.push({
        text: text.slice(range.start, range.end),
        matched: true
      });
      cursor = range.end;
    }

    if (cursor < text.length) {
      segments.push({
        text: text.slice(cursor),
        matched: false
      });
    }

    return segments;
  }

  function resultBreadcrumb(result: CollectionSearchResult) {
    return [result.collectionName, ...result.ancestorNames].join(" / ");
  }

  function isTypingTarget(target: EventTarget | null) {
    const element = target instanceof HTMLElement ? target : null;
    if (!element) {
      return false;
    }

    if (element.isContentEditable) {
      return true;
    }

    return Boolean(element.closest("input, textarea, select, [contenteditable='true']"));
  }

  function clearSearch() {
    collectionSearch.clear();
  }

  function escapeCssAttribute(value: string) {
    if (typeof CSS !== "undefined" && typeof CSS.escape === "function") {
      return CSS.escape(value);
    }

    return value.replace(/["\\]/g, "\\$&");
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
      writeSidebarStateCache();

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
    writeSidebarStateCache();

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

  async function openCollection(collectionId: string, options: { preserveScroll?: boolean } = {}) {
    if (collectionDnd.shouldSuppressClick()) {
      return;
    }

    await collections.selectCollection(collectionId);
    await goto(
      resolve(`/collections?collectionId=${encodeURIComponent(collectionId)}`),
      options.preserveScroll
        ? {
            noScroll: true,
            keepFocus: true
          }
        : undefined
    );
  }

  async function openCollectionItem(collectionId: string, itemId: string) {
    await collections.selectCollection(collectionId);
    revealSearchResultPath({
      collectionId,
      ancestorIds: [],
      itemId
    });
    await goto(
      resolve(
        `/collections?collectionId=${encodeURIComponent(collectionId)}&itemId=${encodeURIComponent(itemId)}`
      ),
      {
        noScroll: true,
        keepFocus: true
      }
    );
  }

  async function toggleCollection(collectionId: string) {
    if (collectionDnd.shouldSuppressClick()) {
      return;
    }

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
    if (collectionDnd.shouldSuppressClick()) {
      return;
    }

    if (expandedFolderIds.has(folderId)) {
      expandedFolderIds.delete(folderId);
      await persistSidebarState();
      return;
    }

    expandedFolderIds.add(folderId);
    await persistSidebarState();
  }

  async function openSavedRequest(
    collectionId: string,
    itemId: string,
    requestType: CollectionItemSummary["requestType"] = "http"
  ) {
    if (collectionDnd.shouldSuppressClick()) {
      return;
    }

    await collections.selectCollection(collectionId);
    const target = requestType === "websocket" || requestType === "socketio"
      ? resolve(`/websockets?messageId=${encodeURIComponent(itemId)}`)
      : resolve(`/?savedRequestId=${encodeURIComponent(itemId)}`);
    await goto(target);
  }

  function revealSearchResultPath(result: {
    collectionId: string;
    ancestorIds?: string[];
    itemId?: string;
    kind?: CollectionSearchResult["kind"];
  }) {
    expandedCollectionIds.add(result.collectionId);

    for (const ancestorId of result.ancestorIds ?? []) {
      expandedFolderIds.add(ancestorId);
    }

    if (result.kind === "folder" && result.itemId) {
      expandedFolderIds.add(result.itemId);
    }

    void persistSidebarState();
  }

  async function clearSearchAndScrollSidebarToResult(result: CollectionSearchResult) {
    clearSearch();
    await tick();

    const escapedId = escapeCssAttribute(result.id);
    const selector =
      result.kind === "collection"
        ? `[data-sidebar-collection-card-id="${escapedId}"]`
        : `[data-sidebar-item-id="${escapedId}"]`;

    const element = document.querySelector<HTMLElement>(selector);
    element?.scrollIntoView({
      block: "start",
      behavior: "smooth"
    });

    if (result.kind === "collection") {
      revealedSidebarCollectionId = result.id;
      revealedSidebarItemId = "";
    } else {
      revealedSidebarCollectionId = "";
      revealedSidebarItemId = result.id;
    }

    if (sidebarRevealResetTimer) {
      clearTimeout(sidebarRevealResetTimer);
    }

    sidebarRevealResetTimer = setTimeout(() => {
      revealedSidebarCollectionId = "";
      revealedSidebarItemId = "";
      sidebarRevealResetTimer = null;
    }, 1600);
  }

  async function openSearchResult(result: CollectionSearchResult) {
    revealSearchResultPath({
      collectionId: result.collectionId,
      ancestorIds: result.ancestorIds,
      itemId: result.id,
      kind: result.kind
    });

    if (result.kind === "request") {
      await openSavedRequest(result.collectionId, result.id, result.requestType);
      await clearSearchAndScrollSidebarToResult(result);
      return;
    }

    if (result.kind === "folder") {
      await openCollectionItem(result.collectionId, result.id);
      await clearSearchAndScrollSidebarToResult(result);
      return;
    }

    await openCollection(result.collectionId, { preserveScroll: true });
    await clearSearchAndScrollSidebarToResult(result);
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
    if (event.button !== 0 || collections.isMovingCollectionItem || collectionSearch.isActive) {
      return;
    }

    collectionDnd.beginPotentialDrag(createDraggedItem(item), event.pointerId, {
      x: event.clientX,
      y: event.clientY
    });
  }

  function handleSearchInput(event: Event) {
    const target = event.currentTarget;
    if (!(target instanceof HTMLInputElement)) {
      return;
    }

    collectionSearch.setQuery(target.value);
  }

  async function handleSearchKeydown(event: KeyboardEvent) {
    if (event.key === "ArrowDown") {
      event.preventDefault();
      collectionSearch.moveSelection(1);
      return;
    }

    if (event.key === "ArrowUp") {
      event.preventDefault();
      collectionSearch.moveSelection(-1);
      return;
    }

    if (event.key === "Enter") {
      const result = collectionSearch.activeResult ?? collectionSearch.results[0];
      if (!result) {
        return;
      }

      event.preventDefault();
      await openSearchResult(result);
      return;
    }

    if (event.key === "Escape") {
      if (!collectionSearch.isActive) {
        searchInput?.blur();
        return;
      }

      event.preventDefault();
      clearSearch();
      searchInput?.focus();
    }
  }

  function handleTreeKeydown(
    event: KeyboardEvent,
    options?: { expanded: boolean; toggle: () => Promise<void> | void }
  ) {
    const current = event.currentTarget;
    if (!(current instanceof HTMLButtonElement)) {
      return;
    }

    if (event.key === "ArrowRight" && options && !options.expanded) {
      event.preventDefault();
      void options.toggle();
      return;
    }

    if (event.key === "ArrowLeft" && options && options.expanded) {
      event.preventDefault();
      void options.toggle();
      return;
    }

    if (!["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)) {
      return;
    }

    const rows = Array.from(
      current.closest(".sidebar-section")?.querySelectorAll<HTMLButtonElement>("[data-sidebar-tree-row='true']") ?? []
    ).filter((row) => row.offsetParent !== null && !row.disabled);
    const currentIndex = rows.indexOf(current);
    if (currentIndex === -1) {
      return;
    }

    event.preventDefault();
    const nextIndex =
      event.key === "Home"
        ? 0
        : event.key === "End"
          ? rows.length - 1
          : Math.max(0, Math.min(rows.length - 1, currentIndex + (event.key === "ArrowDown" ? 1 : -1)));
    rows[nextIndex]?.focus();
  }

  function handleWindowKeydown(event: KeyboardEvent) {
    const key = event.key.toLowerCase();
    const isSearchShortcut =
      (event.ctrlKey || event.metaKey) && !event.shiftKey && !event.altKey && key === "k";

    if (isSearchShortcut) {
      if (isTypingTarget(event.target) && event.target !== searchInput) {
        return;
      }

      event.preventDefault();
      searchInput?.focus();
      searchInput?.select();
    }
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

  $effect(() => {
    void collections.collections;
    void collections.collectionItemsByCollection;

    if (untrack(() => collectionSearch.isActive)) {
      untrack(() => {
        void collectionSearch.refresh();
      });
    }
  });

  $effect(() => {
    void collectionSearch.activeIndex;
    void collectionSearch.results;

    const activeId = collectionSearch.activeResult?.id;
    if (!activeId) {
      return;
    }

    void tick().then(() => {
      const activeElement = document.querySelector<HTMLElement>("[data-sidebar-search-active='true']");
      activeElement?.scrollIntoView({ block: "nearest" });
    });
  });
</script>

<svelte:window onkeydown={handleWindowKeydown} />

<section class="sidebar-section">
  <div class="sidebar-section-header">
    <h2>
      <a
        class={["sidebar-section-link", page.url.pathname.startsWith("/collections") && "sidebar-section-link-active"]}
        href={resolve("/collections")}
        aria-current={page.url.pathname.startsWith("/collections") ? "page" : undefined}
        title="Open Collections workspace"
      >Collections</a>
    </h2>
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

  <div class="sidebar-search-shell">
    <input
      bind:this={searchInput}
      class="text-input sidebar-search-input"
      type="text"
      value={collectionSearch.query}
      placeholder="Search collections"
      aria-label="Search collections, folders, and saved requests"
      aria-controls={collectionSearch.isActive && collectionSearch.results.length > 0 ? "sidebar-collection-search-results" : undefined}
      aria-activedescendant={collectionSearch.activeResult ? `sidebar-search-result-${collectionSearch.activeResult.id}` : undefined}
      autocapitalize="off"
      autocomplete="off"
      autocorrect="off"
      spellcheck="false"
      oninput={handleSearchInput}
      onkeydown={handleSearchKeydown}
    />

    {#if collectionSearch.query}
      <button
        class="sidebar-search-clear"
        type="button"
        onclick={() => {
          clearSearch();
          searchInput?.focus();
        }}
        aria-label="Clear collection search"
        title="Clear"
      >
        ×
      </button>
    {/if}
  </div>

  <div class="sidebar-section-scroll scrollbar-invisible">
    {#if collections.errorText}
      <div class="sidebar-inline-error" role="alert">{collections.errorText}</div>
    {/if}

    {#if collectionSearch.errorText}
      <div class="sidebar-inline-error" role="alert">{collectionSearch.errorText}</div>
    {/if}

    {#if collectionSearch.isActive}
      <div class="sidebar-search-status" role="status" aria-live="polite">
        {#if collectionSearch.isSearching}
          <span>Searching...</span>
        {:else}
          <span>
            {collectionSearch.results.length} result{collectionSearch.results.length === 1 ? "" : "s"}
          </span>
          <span class="sidebar-search-hint">Enter to open</span>
        {/if}
      </div>

      {#if collectionSearch.results.length === 0 && !collectionSearch.isSearching}
        <div class="sidebar-empty-state">
          No collections, folders, or requests match "{collectionSearch.query.trim()}".
        </div>
      {:else}
        <div class="sidebar-search-results" id="sidebar-collection-search-results" role="listbox">
          {#each collectionSearch.results as result, index (`${result.kind}-${result.id}`)}
            <button
              id={`sidebar-search-result-${result.id}`}
              class={[
                "sidebar-search-result",
                collectionSearch.activeIndex === index && "sidebar-search-result-active"
              ]}
              type="button"
              role="option"
              aria-selected={collectionSearch.activeIndex === index}
              data-sidebar-search-active={collectionSearch.activeIndex === index}
              onclick={() => openSearchResult(result)}
              onmouseenter={() => collectionSearch.setActiveIndex(index)}
              onfocus={() => collectionSearch.setActiveIndex(index)}
            >
              <div class="sidebar-search-result-topline">
                <span class={["sidebar-search-kind", `sidebar-search-kind-${result.kind}`]}>
                  {result.kind}
                </span>

                {#if result.kind === "collection" && result.requestCount !== null && result.requestCount !== undefined}
                  <span class="sidebar-search-count">
                    {result.requestCount} request{result.requestCount === 1 ? "" : "s"}
                  </span>
                {/if}
              </div>

              <div class="sidebar-search-result-body">
                {#if result.kind === "folder"}
                  <span class="sidebar-search-folder-icon" aria-hidden="true">
                    <FolderGlyph variant="sidebar-closed" />
                  </span>
                {/if}

                <div class="sidebar-search-copy">
                  <strong class="sidebar-search-title">
                    {#if result.kind === "request" && !result.name}
                      {#if result.requestType === "websocket" || result.requestType === "socketio"}
                        <span class="protocol-badge">{result.requestType === "socketio" ? "S.IO" : "WS"}</span>
                      {:else}
                        <span class={`method-badge method-${result.method?.toLowerCase() ?? "get"}`}>{result.method ?? "GET"}</span>
                      {/if}
                      <span class="sidebar-search-title-text">
                        {#each buildHighlightSegments(result.url ?? "", collectionSearch.query) as segment}
                          {#if segment.matched}
                            <mark>{segment.text}</mark>
                          {:else}
                            {segment.text}
                          {/if}
                        {/each}
                      </span>
                    {:else}
                      {#each buildHighlightSegments(result.name, collectionSearch.query) as segment}
                        {#if segment.matched}
                          <mark>{segment.text}</mark>
                        {:else}
                          {segment.text}
                        {/if}
                      {/each}
                    {/if}
                  </strong>

                  {#if result.kind === "request" && result.name}
                    <span class="sidebar-search-request-meta">
                      {#if result.requestType === "websocket" || result.requestType === "socketio"}
                        <span class="protocol-badge">{result.requestType === "socketio" ? "S.IO" : "WS"}</span>
                      {:else}
                        <span class={`method-badge method-${result.method?.toLowerCase() ?? "get"}`}>{result.method ?? "GET"}</span>
                      {/if}
                      <span class="sidebar-search-url-text">
                        {#each buildHighlightSegments(result.url ?? "", collectionSearch.query) as segment}
                          {#if segment.matched}
                            <mark>{segment.text}</mark>
                          {:else}
                            {segment.text}
                          {/if}
                        {/each}
                      </span>
                    </span>
                  {/if}

                  {#if result.kind !== "collection"}
                    <span class="sidebar-search-breadcrumb">
                      {#each buildHighlightSegments(resultBreadcrumb(result), collectionSearch.query) as segment}
                        {#if segment.matched}
                          <mark>{segment.text}</mark>
                        {:else}
                          {segment.text}
                        {/if}
                      {/each}
                    </span>
                  {/if}

                  <span class="sidebar-search-updated">Updated {formatUpdatedAt(result.updatedAt)}</span>
                </div>
              </div>
            </button>
          {/each}
        </div>
      {/if}
    {:else if collections.collections.length === 0 && !collections.isCollectionsLoading}
      <div class="sidebar-empty-state">No collections yet.</div>
    {:else}
      <div class="sidebar-collection-stack" aria-label="Collection tree">
        {#each collections.collections as collection (collection.id)}
          <article
            class={[
              "sidebar-collection-card",
              collections.selectedCollectionId === collection.id && "sidebar-collection-active",
              revealedSidebarCollectionId === collection.id && "sidebar-search-revealed",
              collectionDnd.matchesDropIndicator(collection.id, null, "root") && "sidebar-drop-target-root"
            ]}
            data-sidebar-collection-card-id={collection.id}
          >
            <div class="sidebar-collection-row">
              <button
                class="sidebar-collection-button"
                type="button"
                onclick={() => openCollection(collection.id)}
                onkeydown={(event) =>
                  handleTreeKeydown(event, {
                    expanded: expandedCollectionIds.has(collection.id),
                    toggle: () => toggleCollection(collection.id)
                  })}
                aria-expanded={expandedCollectionIds.has(collection.id)}
                aria-current={collections.selectedCollectionId === collection.id ? "page" : undefined}
                data-sidebar-tree-row="true"
                data-collection-drop="root"
                data-collection-id={collection.id}
              >
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
                <svg
                  class={["sidebar-toggle-icon", expandedCollectionIds.has(collection.id) && "sidebar-toggle-icon-expanded"]}
                  viewBox="0 0 24 24"
                  aria-hidden="true"
                >
                  <path d="M6 9l6 6 6-6" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round" />
                </svg>
              </button>
            </div>

            {#if expandedCollectionIds.has(collection.id)}
              <div class="sidebar-request-stack">
                {#if collections.isCollectionItemsLoading && !(collections.collectionItemsByCollection[collection.id]?.length)}
                  <span class="sidebar-collection-meta" role="status" aria-live="polite">Loading collection items...</span>
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
                                expandedFolderIds.has(item.id) && "sidebar-folder-open",
                                revealedSidebarItemId === item.id && "sidebar-search-revealed",
                                collectionDnd.isDraggingItem(item.id) && "sidebar-request-dragging",
                                collectionDnd.matchesDropIndicator(collection.id, item.id, "before") && "sidebar-drop-target-before",
                                collectionDnd.matchesDropIndicator(collection.id, item.id, "after") && "sidebar-drop-target-after",
                                collectionDnd.matchesDropIndicator(collection.id, item.id, "inside") && "sidebar-drop-target-inside"
                              ]}
                              type="button"
                              onclick={() => toggleFolder(item.id)}
                              onkeydown={(event) =>
                                handleTreeKeydown(event, {
                                  expanded: expandedFolderIds.has(item.id),
                                  toggle: () => toggleFolder(item.id)
                                })}
                              onpointerdown={(event) => handleItemPointerDown(event, item)}
                              aria-expanded={expandedFolderIds.has(item.id)}
                              aria-label={`${item.name}, ${item.children.length === 0 ? "empty" : `${item.children.length} item${item.children.length === 1 ? "" : "s"}`}`}
                              data-sidebar-tree-row="true"
                              style={`--tree-depth:${depth};`}
                              data-collection-drop="item"
                              data-collection-id={collection.id}
                              data-item-id={item.id}
                              data-item-kind={item.kind}
                              data-sidebar-item-id={item.id}
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
                            class={[
                              "sidebar-request-link",
                              page.url.searchParams.get(item.requestType === "websocket" || item.requestType === "socketio" ? "messageId" : "savedRequestId") === item.id && "sidebar-request-active",
                              revealedSidebarItemId === item.id && "sidebar-search-revealed",
                              collectionDnd.isDraggingRequest(item.id) && "sidebar-request-dragging",
                              collectionDnd.matchesDropIndicator(collection.id, item.id, "before") && "sidebar-drop-target-before",
                              collectionDnd.matchesDropIndicator(collection.id, item.id, "after") && "sidebar-drop-target-after"
                            ]}
                            type="button"
                            onclick={() => openSavedRequest(collection.id, item.id, item.requestType)}
                            onkeydown={handleTreeKeydown}
                            aria-current={page.url.searchParams.get(item.requestType === "websocket" || item.requestType === "socketio" ? "messageId" : "savedRequestId") === item.id ? "page" : undefined}
                            data-sidebar-tree-row="true"
                            style={`--tree-depth:${depth};`}
                            onpointerdown={(event) => handleItemPointerDown(event, item)}
                            data-collection-drop="item"
                            data-collection-id={collection.id}
                            data-item-id={item.id}
                            data-item-kind={item.kind}
                            data-sidebar-item-id={item.id}
                          >
                            <strong class="sidebar-request-name">
                              {#if item.name}
                                {item.name}
                              {:else}
                                {#if item.requestType === "websocket" || item.requestType === "socketio"}
                                  <span class="protocol-badge">{item.requestType === "socketio" ? "S.IO" : "WS"}</span> {item.url ?? ""}
                                {:else}
                                  <span class={`method-badge method-${item.method?.toLowerCase() ?? "get"}`}>{item.method ?? "GET"}</span> {item.url ?? ""}
                                {/if}
                              {/if}
                            </strong>
                            <span class="sidebar-request-url">
                              {#if item.requestType === "websocket" || item.requestType === "socketio"}
                                <span class="protocol-badge">{item.requestType === "socketio" ? "S.IO" : "WS"}</span>
                              {:else}
                                <span class={`method-badge method-${item.method?.toLowerCase() ?? "get"}`}>{item.method ?? "GET"}</span>
                              {/if}
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
