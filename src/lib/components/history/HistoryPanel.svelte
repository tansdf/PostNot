<script lang="ts">
  import type { HistoryEntryDetail, HistoryEntrySummary } from "$lib/api/types";
  import HistoryDetail from "$lib/components/history/HistoryDetail.svelte";

  export let items: HistoryEntrySummary[] = [];
  export let isLoading = false;
  export let errorText = "";
  export let selectedId = "";
  export let detail: HistoryEntryDetail | null = null;
  export let detailErrorText = "";
  export let isDetailLoading = false;
  export let isClearing = false;
  export let onInspect: (id: string) => Promise<void> | void = () => {};
  export let onClear: () => Promise<void> | void = () => {};
  export let onCloseDetail: () => void = () => {};

  function handleInspect(event: MouseEvent, id: string) {
    (event.currentTarget as HTMLButtonElement | null)?.blur();
    void onInspect(id);
  }

  function formatExecutedAt(value: string) {
    try {
      return new Intl.DateTimeFormat(undefined, {
        dateStyle: "medium",
        timeStyle: "short"
      }).format(new Date(value));
    } catch {
      return value;
    }
  }
</script>

<section class="panel history-panel">
  <div class="editor-header">
    <h2>History</h2>
    <div class="history-toolbar">
      {#if isLoading}
        <span class="history-meta">Refreshing...</span>
      {:else}
        <span class="history-meta">{items.length} entries</span>
      {/if}

      <button class="ghost-button" type="button" disabled={isClearing || items.length === 0} on:click={() => onClear()}>
        {isClearing ? "Clearing..." : "Clear history"}
      </button>
    </div>
  </div>

  {#if errorText}
    <div class="response-error">{errorText}</div>
  {:else if items.length === 0 && !isLoading}
    <div class="empty-state">Request history will appear here after the first send.</div>
  {:else}
    <div class="history-content">
      <div class="history-list-column">
        <div class="history-list">
          {#each items as item (item.id)}
            <article class:history-item-active={selectedId === item.id} class="history-item">
              <div class="history-item-top">
                <div>
                  <strong>{item.requestName || item.url}</strong>
                  <div class="history-url">{item.method} {item.url}</div>
                </div>
                <div class:history-status-error={item.statusCode === null || !!item.errorText} class="history-status">
                  {#if item.statusCode !== null}
                    {item.statusCode}
                  {:else}
                    Error
                  {/if}
                </div>
              </div>

              <div class="history-item-meta">
                <span>{item.durationMs} ms</span>
                <span>{formatExecutedAt(item.executedAt)}</span>
              </div>

              <div class="history-row-actions">
                <button
                  class:active={selectedId === item.id}
                  class="tab-button"
                  type="button"
                  on:click={(event) => handleInspect(event, item.id)}
                >
                  {selectedId === item.id ? "Inspecting" : "Inspect"}
                </button>
              </div>

              {#if item.errorText}
                <p class="history-preview history-preview-error">{item.errorText}</p>
              {:else if item.responseBodyPreview}
                <pre class="history-preview">{item.responseBodyPreview}</pre>
              {/if}
            </article>
          {/each}
        </div>
      </div>

      <div class="history-detail-column">
        <HistoryDetail
          {detail}
          errorText={detailErrorText}
          isLoading={isDetailLoading}
          onClose={onCloseDetail}
        />
      </div>
    </div>
  {/if}
</section>
