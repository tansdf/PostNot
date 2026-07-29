<script lang="ts">
  import {
    cancelResponseSearch,
    cancelResponseBodyJob,
    getResponseBodyPath,
    readResponseBodyText,
    readResponseBodyWindow,
    formatResponseBody,
    findResponseMatch,
    releaseResponseBody,
    retainResponseBody,
    saveResponseBody,
    searchResponseBody,
    type ResponseBodyRow,
    type ResponseSearchMatch,
    type ResponseSearchResult
  } from "$lib/api/commands";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { onMount, tick } from "svelte";
  import type { ResponseBody } from "$lib/api/types";
  import { computeVirtualWindowStart, moveWrappedMatchIndex, prepareRepresentationSwitch } from "$lib/response-window";

  const WINDOW_ROWS = 180;
  const OVERSCAN_ROWS = 40;
  const COPY_WARNING_BYTES = 32 * 1024 * 1024;

  let {
    body,
    maxHeight = "clamp(16rem, 62vh, 44rem)"
  }: {
    body: Extract<ResponseBody, { mode: "file" }>;
    maxHeight?: string;
  } = $props();

  let viewport: HTMLDivElement | null = $state(null);
  let formattedBody: Extract<ResponseBody, { mode: "file" }> | null = $state(null);
  let viewMode = $state<"raw" | "formatted">("raw");
  let rows: ResponseBodyRow[] = $state([]);
  let totalRows = $state(0);
  let startRow = $state(0);
  let averageRowHeight = $state(23);
  let wrap = $state(true);
  let loading = $state(true);
  let errorText = $state("");
  let operationText = $state("");
  let imageSource = $state("");
  let findOpen = $state(false);
  let query = $state("");
  let caseSensitive = $state(false);
  let searching = $state(false);
  let searchResult: ResponseSearchResult | null = $state(null);
  let activeMatch = $state(-1);
  let overflowMatch: ResponseSearchMatch | null = $state(null);
  let activeMatchOrdinal = $state(0);
  let loadSequence = 0;
  let searchSequence = 0;
  const windowCache = new Map<string, Awaited<ReturnType<typeof readResponseBodyWindow>>>();
  let activeSearchId = "";
  let formatJobId = $state("");
  let progressiveMatches = $state(0);
  let searchTimer: number | null = null;

  let topSpacer = $derived(startRow * averageRowHeight);
  let bottomSpacer = $derived(Math.max(0, totalRows - startRow - rows.length) * averageRowHeight);
  let searchStatus = $derived.by(() => {
    if (searching) return progressiveMatches ? `Searching… ${progressiveMatches.toLocaleString()} found` : "Searching…";
    if (!query) return "";
    if (!searchResult?.totalMatches) return "No matches";
    const total = searchResult.capped ? "100,000+" : searchResult.totalMatches.toLocaleString();
    return `${Math.max(1, activeMatchOrdinal)} of ${total}`;
  });
  let representation = $derived<"raw" | "formatted" | "hex">(
    body.presentation === "binary" ? "hex" : viewMode
  );
  let activeBody = $derived(viewMode === "formatted" && formattedBody ? formattedBody : body);

  onMount(() => {
    let disposed = false;
    let unlisten: (() => void) | null = null;
    let unlistenJob: (() => void) | null = null;
    void listen<{
      searchId: string;
      totalMatches: number;
      firstMatch: { rowIndex: number } | null;
    }>("response-search-progress", (event) => {
      if (event.payload.searchId !== activeSearchId) return;
      progressiveMatches = event.payload.totalMatches;
      if (event.payload.firstMatch && activeMatch < 0) {
        const rowIndex = event.payload.firstMatch.rowIndex;
        void loadWindow(Math.max(0, rowIndex - 4)).then(() => {
          if (viewport) viewport.scrollTop = rowIndex * averageRowHeight;
        });
      }
    }).then((dispose) => {
      if (disposed) dispose(); else unlisten = dispose;
    });
    void listen<{ jobId: string; processedBytes: number; totalBytes: number }>("response-body-job-progress", (event) => {
      if (event.payload.jobId !== formatJobId) return;
      const percent = event.payload.totalBytes > 0
        ? Math.min(100, Math.round((event.payload.processedBytes / event.payload.totalBytes) * 100))
        : 0;
      operationText = `Formatting… ${percent}%`;
    }).then((dispose) => {
      if (disposed) dispose(); else unlistenJob = dispose;
    });
    return () => {
      disposed = true;
      unlisten?.();
      unlistenJob?.();
    };
  });

  $effect(() => {
    const handleId = body.handleId;
    void retainResponseBody(handleId);
    if (body.presentation === "image") {
      void getResponseBodyPath(handleId).then((path) => (imageSource = convertFileSrc(path)));
    } else {
      void loadWindow(0);
    }
    return () => {
      if (activeSearchId) void cancelResponseSearch(activeSearchId);
      if (formatJobId) void cancelResponseBodyJob(formatJobId);
      if (searchTimer) window.clearTimeout(searchTimer);
      void releaseResponseBody(handleId);
      if (formattedBody) void releaseResponseBody(formattedBody.handleId);
    };
  });

  $effect(() => {
    if (!viewport || typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(() => measureRows());
    observer.observe(viewport);
    return () => observer.disconnect();
  });

  async function loadWindow(nextStart: number) {
    const sequence = ++loadSequence;
    const boundedStart = Math.max(0, Math.floor(nextStart));
    const cacheKey = `${activeBody.handleId}:${representation}:${boundedStart}`;
    const cached = windowCache.get(cacheKey);
    if (cached) {
      loading = false;
      errorText = "";
      rows = cached.rows;
      startRow = cached.startRow;
      totalRows = cached.totalRows;
      await tick();
      measureRows();
      return;
    }
    loading = rows.length === 0;
    errorText = "";
    try {
      const window = await readResponseBodyWindow({
        handleId: activeBody.handleId,
        startRow: boundedStart,
        rowCount: WINDOW_ROWS,
        maxBytes: 2 * 1024 * 1024,
        representation
      });
      if (sequence !== loadSequence) return;
      windowCache.set(cacheKey, window);
      while (windowCache.size > 8) {
        const oldest = windowCache.keys().next().value;
        if (oldest === undefined) break;
        windowCache.delete(oldest);
      }
      rows = window.rows;
      startRow = window.startRow;
      totalRows = window.totalRows;
      await tick();
      measureRows();
    } catch (error) {
      if (sequence === loadSequence) {
        errorText = error instanceof Error ? error.message : String(error);
      }
    } finally {
      if (sequence === loadSequence) loading = false;
    }
  }

  function measureRows() {
    if (!viewport) return;
    const elements = Array.from(viewport.querySelectorAll<HTMLElement>(".virtual-response-row"));
    if (!elements.length) return;
    const measured = elements.reduce((sum, element) => sum + element.getBoundingClientRect().height, 0) / elements.length;
    if (!Number.isFinite(measured) || measured <= 0 || Math.abs(measured - averageRowHeight) < 0.5) return;
    const anchorRow = viewport.scrollTop / Math.max(1, averageRowHeight);
    averageRowHeight = measured;
    viewport.scrollTop = anchorRow * averageRowHeight;
  }

  function handleScroll() {
    if (!viewport || loading) return;
    const desiredStart = computeVirtualWindowStart(viewport.scrollTop, averageRowHeight, OVERSCAN_ROWS);
    if (desiredStart < startRow + 15 || desiredStart > startRow + Math.max(30, rows.length - 70)) {
      void loadWindow(desiredStart);
    }
  }

  function handleKeydown(event: KeyboardEvent) {
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "f") {
      event.preventDefault();
      openFind();
      return;
    }
    if (event.key === "Escape" && findOpen) {
      closeFind();
    }
  }

  function openFind() {
    findOpen = true;
    queueMicrotask(() => document.getElementById(`response-find-${body.handleId}`)?.focus());
  }

  function closeFind() {
    findOpen = false;
    if (activeSearchId) void cancelResponseSearch(activeSearchId);
  }

  function scheduleSearch() {
    if (searchTimer) window.clearTimeout(searchTimer);
    searchTimer = window.setTimeout(() => {
      searchTimer = null;
      void runSearch();
    }, 180);
  }

  async function runSearch() {
    if (activeSearchId) void cancelResponseSearch(activeSearchId);
    const sequence = ++searchSequence;
    const searchId = `${body.handleId}:${sequence}:${Date.now()}`;
    activeSearchId = searchId;
    activeMatch = -1;
    activeMatchOrdinal = 0;
    overflowMatch = null;
    progressiveMatches = 0;
    searchResult = null;
    if (!query) {
      activeSearchId = "";
      searching = false;
      return;
    }
    searching = true;
    try {
      const result = await searchResponseBody({ handleId: activeBody.handleId, query, caseSensitive, searchId, representation });
      if (sequence !== searchSequence) return;
      searchResult = result;
      if (result.matches.length > 0) {
        activeMatch = 0;
        activeMatchOrdinal = 1;
        await revealMatch(0);
      }
    } catch (error) {
      if (sequence === searchSequence) errorText = error instanceof Error ? error.message : String(error);
    } finally {
      if (sequence === searchSequence) {
        searching = false;
        activeSearchId = "";
      }
    }
  }

  async function moveMatch(delta: number) {
    const matches = searchResult?.matches ?? [];
    if (matches.length === 0) return;
    const wrappingBackward = Boolean(searchResult?.capped && delta < 0 && activeMatch === 0 && !overflowMatch);
    if (overflowMatch || (searchResult?.capped && ((delta > 0 && activeMatch === matches.length - 1) || wrappingBackward))) {
      const current = overflowMatch ?? matches[activeMatch];
      const found = await findResponseMatch({
        handleId: activeBody.handleId,
        query,
        caseSensitive,
        fromOffset: current.byteOffset,
        direction: delta > 0 ? "next" : "previous",
        wrap: true,
        representation
      });
      if (!found) return;
      if (found.byteOffset <= matches[matches.length - 1].byteOffset) {
        overflowMatch = null;
        activeMatch = matches.findLastIndex((match) => match.byteOffset <= found.byteOffset);
        activeMatchOrdinal = activeMatch + 1;
        await revealSearchMatch(matches[activeMatch]);
      } else {
        overflowMatch = found;
        activeMatchOrdinal = wrappingBackward
          ? (searchResult?.totalMatches ?? matches.length)
          : delta > 0 ? activeMatchOrdinal + 1 : Math.max(1, activeMatchOrdinal - 1);
        await revealSearchMatch(found);
      }
      return;
    }
    activeMatch = moveWrappedMatchIndex(activeMatch, delta, matches.length);
    activeMatchOrdinal = activeMatch + 1;
    await revealMatch(activeMatch);
  }

  async function revealMatch(index: number) {
    const match = searchResult?.matches[index];
    if (!match) return;
    await revealSearchMatch(match);
  }

  async function revealSearchMatch(match: ResponseSearchMatch) {
    const nextStart = Math.max(0, match.rowIndex - 4);
    await loadWindow(nextStart);
    if (viewport) viewport.scrollTop = match.rowIndex * averageRowHeight;
  }

  async function handleCopy() {
    if (body.sizeBytes > COPY_WARNING_BYTES) {
      const confirmed = window.confirm(
        "This response is large and copying it may temporarily use substantial memory. Save as a file instead when possible. Continue copying?"
      );
      if (!confirmed) return;
    }
    operationText = "Preparing copy…";
    try {
      const text = await readResponseBodyText(body.handleId);
      await navigator.clipboard.writeText(text);
      operationText = "Copied";
    } catch (error) {
      errorText = error instanceof Error ? error.message : String(error);
      operationText = "";
    }
  }

  async function handleSave() {
    operationText = "Saving…";
    try {
      const path = await saveResponseBody(body.handleId, suggestedFileName());
      operationText = path ? "Saved" : "";
    } catch (error) {
      errorText = error instanceof Error ? error.message : String(error);
      operationText = "";
    }
  }

  function suggestedFileName() {
    if (body.presentation === "json") return "response-body.json";
    if (body.presentation === "image") {
      const subtype = body.contentType?.split("/")[1]?.split(";")[0]?.replace("jpeg", "jpg") || "img";
      return `response-body.${subtype}`;
    }
    return body.presentation === "binary" ? "response-body.bin" : "response-body.txt";
  }

  async function handleFormat() {
    if (formatJobId) {
      void cancelResponseBodyJob(formatJobId);
      return;
    }
    operationText = "Formatting…";
    const scrollRatio = currentScrollRatio();
    formatJobId = `${body.handleId}:format:${Date.now()}`;
    try {
      if (!formattedBody) {
        const next = await formatResponseBody(body.handleId, formatJobId);
        if (next.mode !== "file") throw new Error("Formatted response was not file-backed.");
        formattedBody = next;
      }
      invalidateActiveSearch();
      viewMode = "formatted";
      rows = [];
      await loadWindow(0);
      restoreScrollRatio(scrollRatio);
      operationText = "Formatted";
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      if (!message.toLowerCase().includes("cancel")) errorText = message;
      operationText = "";
    } finally {
      formatJobId = "";
    }
  }

  async function switchRepresentation(next: "raw" | "formatted") {
    invalidateActiveSearch();
    const scrollRatio = currentScrollRatio();
    viewMode = next;
    rows = [];
    searchResult = null;
    activeMatch = -1;
    await loadWindow(0);
    restoreScrollRatio(scrollRatio);
  }

  function invalidateActiveSearch() {
    const searchSwitch = prepareRepresentationSwitch(activeSearchId, searchSequence);
    searchSequence = searchSwitch.nextSearchSequence;
    if (searchSwitch.searchIdToCancel) void cancelResponseSearch(searchSwitch.searchIdToCancel);
    activeSearchId = "";
    searching = false;
  }

  function currentScrollRatio() {
    if (!viewport || totalRows <= 1) return 0;
    return viewport.scrollTop / Math.max(1, totalRows * averageRowHeight);
  }

  function restoreScrollRatio(ratio: number) {
    if (!viewport || ratio <= 0) return;
    const targetRow = Math.floor(totalRows * Math.min(1, ratio));
    viewport.scrollTop = targetRow * averageRowHeight;
    void loadWindow(Math.max(0, targetRow - OVERSCAN_ROWS));
  }

  function highlightedParts(text: string) {
    if (!query) return [{ text, match: false }];
    const source = caseSensitive ? text : text.toLocaleLowerCase();
    const needle = caseSensitive ? query : query.toLocaleLowerCase();
    if (!needle) return [{ text, match: false }];
    const result: { text: string; match: boolean }[] = [];
    let cursor = 0;
    while (cursor < text.length) {
      const index = source.indexOf(needle, cursor);
      if (index < 0) {
        result.push({ text: text.slice(cursor), match: false });
        break;
      }
      if (index > cursor) result.push({ text: text.slice(cursor, index), match: false });
      result.push({ text: text.slice(index, index + query.length), match: true });
      cursor = index + query.length;
    }
    return result.length ? result : [{ text, match: false }];
  }

  function syntaxParts(text: string) {
    if (activeBody.presentation !== "json") return [{ text, className: "" }];
    const parts: { text: string; className: string }[] = [];
    const pattern = /"(?:\\.|[^"\\])*"(?=\s*:)|"(?:\\.|[^"\\])*"|-?\d+(?:\.\d+)?(?:[eE][+-]?\d+)?|\b(?:true|false)\b|\bnull\b/g;
    let cursor = 0;
    for (const match of text.matchAll(pattern)) {
      const index = match.index ?? 0;
      if (index > cursor) parts.push({ text: text.slice(cursor, index), className: "" });
      const value = match[0];
      const after = text.slice(index + value.length);
      const className = value.startsWith('"')
        ? (/^\s*:/.test(after) ? "jt-key" : "jt-string")
        : value === "true" || value === "false" ? "jt-bool"
        : value === "null" ? "jt-null"
        : "jt-number";
      parts.push({ text: value, className });
      cursor = index + value.length;
    }
    if (cursor < text.length) parts.push({ text: text.slice(cursor), className: "" });
    return parts.length ? parts : [{ text, className: "" }];
  }
</script>

<div class="virtual-response" role="region" aria-label="File-backed response">
  <div class="virtual-response-toolbar">
    <span>{body.sizeBytes.toLocaleString()} bytes · file-backed</span>
    <div class="virtual-response-actions">
      {#if operationText}<span class="history-meta" aria-live="polite">{operationText}</span>{/if}
      <button class="button-secondary button-compact" type="button" disabled={body.presentation === "image"} onclick={() => findOpen ? closeFind() : openFind()}>Find</button>
      <button class="button-secondary button-compact" type="button" aria-pressed={wrap} onclick={async () => { wrap = !wrap; await tick(); measureRows(); }}>
        {wrap ? "Wrap on" : "Wrap off"}
      </button>
      {#if body.presentation === "json"}
        {#if viewMode === "formatted"}
          <button class="button-secondary button-compact" type="button" onclick={() => void switchRepresentation("raw")}>Raw</button>
        {:else}
          <button class="button-secondary button-compact" type="button" onclick={handleFormat}>{formatJobId ? "Cancel format" : "Format"}</button>
        {/if}
      {/if}
      <button class="button-secondary button-compact" type="button" onclick={handleCopy}>Copy</button>
      <button class="button-secondary button-compact" type="button" onclick={handleSave}>Save as…</button>
    </div>
  </div>

  {#if findOpen}
    <div class="response-find-bar">
      <input
        id={`response-find-${body.handleId}`}
        class="text-input"
        type="search"
        placeholder="Find in response"
        bind:value={query}
        oninput={scheduleSearch}
        onkeydown={(event) => {
          if (event.key === "Enter") {
            event.preventDefault();
            if (searchResult) void moveMatch(event.shiftKey ? -1 : 1);
            else void runSearch();
          }
        }}
      />
      <label class="response-find-case"><input type="checkbox" bind:checked={caseSensitive} onchange={scheduleSearch} /> Match case</label>
      <span class="response-find-status" aria-live="polite">{searchStatus}</span>
      <button class="icon-button button-compact" type="button" aria-label="Previous match" title="Previous match" onclick={() => void moveMatch(-1)}>↑</button>
      <button class="icon-button button-compact" type="button" aria-label="Next match" title="Next match" onclick={() => void moveMatch(1)}>↓</button>
      <button class="icon-button button-compact" type="button" aria-label="Close find" title="Close find" onclick={closeFind}>×</button>
    </div>
  {/if}

  {#if errorText}<div class="feedback feedback-error">{errorText}</div>{/if}
  {#if body.presentation === "image"}
    <div class="virtual-response-image-wrap">
      {#if imageSource}<img src={imageSource} alt="Response body preview" />{:else}<div class="virtual-response-loading">Preparing image preview…</div>{/if}
    </div>
  {:else}<div
    class={["virtual-response-viewport", wrap && "virtual-response-wrap"]}
    style:max-height={maxHeight}
    bind:this={viewport}
    onscroll={handleScroll}
    onkeydown={handleKeydown}
    tabindex="0"
    role="textbox"
    aria-readonly="true"
    aria-label="Response body"
  >
    <div style:height={`${topSpacer}px`}></div>
    {#if loading && rows.length === 0}
      <div class="virtual-response-loading">Preparing response view…</div>
    {:else}
      {#each rows as row (row.key)}
        <pre class="virtual-response-row" data-row={row.rowIndex}>{#each syntaxParts(row.text) as syntax}<span class={syntax.className}>{#each highlightedParts(syntax.text) as part}{#if part.match}<mark>{part.text}</mark>{:else}{part.text}{/if}{/each}</span>{/each}</pre>
      {/each}
    {/if}
    <div style:height={`${bottomSpacer}px`}></div>
  </div>{/if}
</div>
