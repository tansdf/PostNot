<script lang="ts">
  import type { HistoryEntrySummary } from "$lib/api/types";

  export let items: HistoryEntrySummary[] = [];
  export let isLoading = false;
  export let errorText = "";

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
    {#if isLoading}
      <span class="history-meta">Refreshing...</span>
    {:else}
      <span class="history-meta">{items.length} entries</span>
    {/if}
  </div>

  {#if errorText}
    <div class="response-error">{errorText}</div>
  {:else if items.length === 0 && !isLoading}
    <div class="empty-state">Request history will appear here after the first send.</div>
  {:else}
    <div class="history-list">
      {#each items as item (item.id)}
        <article class="history-item">
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

          {#if item.errorText}
            <p class="history-preview history-preview-error">{item.errorText}</p>
          {:else if item.responseBodyPreview}
            <pre class="history-preview">{item.responseBodyPreview}</pre>
          {/if}
        </article>
      {/each}
    </div>
  {/if}
</section>
