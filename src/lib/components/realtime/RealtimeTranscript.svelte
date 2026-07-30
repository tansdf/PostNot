<script lang="ts">
  import { tick } from "svelte";

  import type { RealtimeTranscriptEntry } from "$lib/api/types";
  import JsonViewer from "$lib/components/response/JsonViewer.svelte";

  let {
    entries = [],
    sizeBytes = 0,
    onClear = () => {},
    onExport = () => {},
    onReadPayload = async () => "",
    onSavePayload = () => {}
  }: {
    entries?: RealtimeTranscriptEntry[];
    sizeBytes?: number;
    onClear?: () => Promise<void> | void;
    onExport?: () => Promise<void> | void;
    onReadPayload?: (handleId: string) => Promise<string>;
    onSavePayload?: (handleId: string, label: string) => Promise<void> | void;
  } = $props();

  let filter: "all" | "sent" | "received" | "events" = $state("all");
  let query = $state("");
  let transcriptNode: HTMLDivElement | null = $state(null);
  let follow = $state(true);
  let expandedPayloads: Record<string, string> = $state({});
  let payloadErrors: Record<string, string> = $state({});
  const filterOptions = ["all", "sent", "received", "events"] as const;

  let filteredEntries = $derived(entries.filter((entry) => {
    if (filter === "sent" && entry.direction !== "sent") return false;
    if (filter === "received" && entry.direction !== "received") return false;
    if (filter === "events" && !["lifecycle", "event", "ack", "error", "trimmed"].includes(entry.kind)) return false;
    const search = query.trim().toLowerCase();
    if (!search) return true;
    const payload = entry.payload?.mode === "inline" ? entry.payload.text : entry.payload?.previewText ?? "";
    return [entry.label, entry.eventName ?? "", payload].join(" ").toLowerCase().includes(search);
  }));

  $effect(() => {
    entries.length;
    if (!follow) return;
    void tick().then(() => transcriptNode?.scrollTo({ top: transcriptNode.scrollHeight }));
  });

  function syncFollow() {
    if (!transcriptNode) return;
    follow = transcriptNode.scrollHeight - transcriptNode.scrollTop - transcriptNode.clientHeight < 24;
  }

  function payloadText(entry: RealtimeTranscriptEntry) {
    if (expandedPayloads[entry.id] !== undefined) return expandedPayloads[entry.id];
    return entry.payload?.mode === "inline" ? entry.payload.text : entry.payload?.previewText ?? "";
  }

  async function expand(entry: RealtimeTranscriptEntry) {
    if (entry.payload?.mode !== "file") return;
    try {
      expandedPayloads = { ...expandedPayloads, [entry.id]: await onReadPayload(entry.payload.handleId) };
      const { [entry.id]: _, ...rest } = payloadErrors;
      payloadErrors = rest;
    } catch (error) {
      payloadErrors = { ...payloadErrors, [entry.id]: error instanceof Error ? error.message : String(error) };
    }
  }

  async function copy(entry: RealtimeTranscriptEntry) {
    await navigator.clipboard.writeText(payloadText(entry));
  }

  function saveFilePayload(entry: RealtimeTranscriptEntry) {
    const payload = entry.payload;
    if (payload?.mode !== "file") return;
    return onSavePayload(payload.handleId, entry.eventName || entry.label);
  }

  function formatBytes(value: number) {
    if (value < 1024) return `${value} B`;
    if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KiB`;
    return `${(value / (1024 * 1024)).toFixed(1)} MiB`;
  }

  function formatTime(value: string) {
    const parsed = new Date(value);
    return Number.isNaN(parsed.getTime()) ? value : parsed.toLocaleTimeString();
  }

  function isJson(entry: RealtimeTranscriptEntry, text: string) {
    if (entry.kind === "json") return true;
    if (!text.trim() || !["{", "["].includes(text.trim()[0])) return false;
    try { JSON.parse(text); return true; } catch { return false; }
  }

  function filterDomId(option: (typeof filterOptions)[number]) {
    return `realtime-transcript-filter-${option}`;
  }

  function handleFilterKeydown(event: KeyboardEvent, filterIndex: number) {
    let nextIndex = filterIndex;
    if (event.key === "ArrowRight") nextIndex = (filterIndex + 1) % filterOptions.length;
    else if (event.key === "ArrowLeft") nextIndex = (filterIndex - 1 + filterOptions.length) % filterOptions.length;
    else if (event.key === "Home") nextIndex = 0;
    else if (event.key === "End") nextIndex = filterOptions.length - 1;
    else return;
    event.preventDefault();
    filter = filterOptions[nextIndex];
    document.getElementById(filterDomId(filter))?.focus();
  }
</script>

<section class="panel panel-inset realtime-transcript-panel" aria-labelledby="realtime-transcript-title">
  <div class="request-section-header">
    <div class="panel-heading">
      <h2 id="realtime-transcript-title">Session transcript</h2>
      <p class="field-help">{entries.length} entries · {formatBytes(sizeBytes)} · cleared when the app closes</p>
    </div>
    <div class="request-actions">
      <button class="button-secondary button-compact" type="button" onclick={onExport} disabled={!entries.length}>Export transcript</button>
      <button class="button-ghost button-compact" type="button" onclick={onClear} disabled={!entries.length}>Clear</button>
    </div>
  </div>

  <div class="realtime-transcript-tools">
    <div class="panel-tabs" role="tablist" aria-label="Transcript filters">
      {#each filterOptions as option, index}
        <button
          id={filterDomId(option)}
          class:active={filter === option}
          class="tab-button button-compact"
          type="button"
          role="tab"
          aria-selected={filter === option}
          aria-controls="realtime-transcript-log"
          tabindex={filter === option ? 0 : -1}
          onclick={() => (filter = option)}
          onkeydown={(event) => handleFilterKeydown(event, index)}
        >
          {option.charAt(0).toUpperCase() + option.slice(1)}
        </button>
      {/each}
    </div>
    <label class="realtime-transcript-search">
      <span class="sr-only">Search transcript</span>
      <input class="text-input" type="search" bind:value={query} placeholder="Search messages and events" />
    </label>
  </div>

  <div
    id="realtime-transcript-log"
    class="realtime-transcript"
    bind:this={transcriptNode}
    onscroll={syncFollow}
    role="log"
    aria-live="off"
    aria-label="WebSocket session messages"
  >
    {#if !filteredEntries.length}
      <div class="empty-state">
        <strong>{entries.length ? "No matching messages" : "No session messages yet"}</strong>
        <span>{entries.length ? "Adjust the filter or search." : "Connect and send a message to begin the transcript."}</span>
      </div>
    {:else}
      {#each filteredEntries as entry (entry.id)}
        {@const text = payloadText(entry)}
        <article class={["realtime-transcript-entry", `realtime-direction-${entry.direction}`, `realtime-kind-${entry.kind}`]}>
          <header>
            <span class="realtime-direction-label">{entry.direction === "sent" ? "Sent" : entry.direction === "received" ? "Received" : "Event"}</span>
            <strong>{entry.eventName || entry.label}</strong>
            <time datetime={entry.occurredAt}>{formatTime(entry.occurredAt)}</time>
            {#if entry.payload}<span>{formatBytes(entry.payload.sizeBytes)}</span>{/if}
          </header>
          {#if entry.payload}
            <div class="realtime-payload-actions">
              {#if !isJson(entry, text)}
                <button class="button-ghost button-compact" type="button" onclick={() => copy(entry)}>Copy</button>
              {/if}
              {#if entry.payload.mode === "file" && expandedPayloads[entry.id] === undefined}
                <button class="button-ghost button-compact" type="button" onclick={() => expand(entry)}>Read full payload</button>
              {/if}
              {#if entry.payload.mode === "file"}
                <button class="button-ghost button-compact" type="button" onclick={() => saveFilePayload(entry)}>Save as…</button>
              {/if}
            </div>
            {#if entry.kind === "binary"}
              <pre class="realtime-payload-preview">{text || "Binary payload"}</pre>
            {:else if isJson(entry, text)}
              <JsonViewer source={text} maxHeight="20rem" />
            {:else}
              <pre class="realtime-payload-preview">{text}</pre>
            {/if}
            {#if entry.payload.truncated}<p class="field-help">Preview truncated; read or save the full payload.</p>{/if}
            {#if payloadErrors[entry.id]}<p class="feedback feedback-error" role="alert">{payloadErrors[entry.id]}</p>{/if}
          {/if}
        </article>
      {/each}
    {/if}
  </div>

  {#if entries.length && !follow}
    <button class="button-secondary realtime-follow-button" type="button" onclick={() => { follow = true; transcriptNode?.scrollTo({ top: transcriptNode.scrollHeight, behavior: "smooth" }); }}>
      Follow latest
    </button>
  {/if}
</section>
