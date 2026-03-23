<script lang="ts">
  import type { HistoryEntryDetail, KeyValueRow } from "$lib/api/types";

  export let detail: HistoryEntryDetail | null = null;
  export let isLoading = false;
  export let errorText = "";
  export let onClose: () => void = () => {};

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

  function filterEnabled(rows: KeyValueRow[]) {
    return rows.filter((row) => row.enabled && (row.key.trim() || row.value.trim()));
  }

  function formatBody(bodyText: string) {
    if (!bodyText) {
      return "";
    }

    try {
      return JSON.stringify(JSON.parse(bodyText), null, 2);
    } catch {
      return bodyText;
    }
  }

  $: queryParams = detail ? filterEnabled(detail.requestSnapshot.queryParams) : [];
  $: requestHeaders = detail ? filterEnabled(detail.requestSnapshot.headers) : [];
  $: responseHeaders = detail ? filterEnabled(detail.responseHeaders) : [];
  $: requestBody = detail ? formatBody(detail.requestSnapshot.body.raw) : "";
  $: responseBody = detail ? formatBody(detail.responseBodyText) : "";
</script>

{#if detail || isLoading || errorText}
  <section class="history-detail history-detail-filled">
    <div class="editor-header">
      <h3>History Detail</h3>
      <button class="ghost-button" type="button" on:click={() => onClose()} disabled={!detail && !errorText}>
        Close
      </button>
    </div>

    {#if isLoading}
      <div class="empty-state">Loading stored request details...</div>
    {:else if errorText}
      <div class="response-error">{errorText}</div>
    {:else if detail}
      <div class="detail-grid">
        <section class="detail-card detail-card-span">
          <h4 class="detail-section-title">Overview</h4>
          <div class="detail-facts">
            <div class="status-item">
              <span class="status-label">Request</span>
              <strong>{detail.requestName || detail.url}</strong>
            </div>
            <div class="status-item">
              <span class="status-label">Method</span>
              <strong>{detail.method}</strong>
            </div>
            <div class="status-item">
              <span class="status-label">Status</span>
              <strong>{detail.statusCode ?? "Error"}</strong>
            </div>
            <div class="status-item">
              <span class="status-label">Executed</span>
              <strong>{formatExecutedAt(detail.executedAt)}</strong>
            </div>
            <div class="status-item">
              <span class="status-label">Duration</span>
              <strong>{detail.durationMs} ms</strong>
            </div>
            <div class="status-item">
              <span class="status-label">Auth</span>
              <strong>{detail.requestSnapshot.auth.type}</strong>
            </div>
            <div class="status-item">
              <span class="status-label">Body mode</span>
              <strong>{detail.requestSnapshot.body.mode}</strong>
            </div>
            <div class="status-item detail-wide">
              <span class="status-label">URL</span>
              <strong class="detail-url-value" title={detail.url}>{detail.url}</strong>
            </div>
          </div>
        </section>

        <section class="detail-card detail-card-span">
          <h4 class="detail-section-title">Request Snapshot</h4>
          <div class="detail-response-columns">
            <div class="detail-response-column">
              <h5 class="detail-subtitle">Headers</h5>
              {#if queryParams.length || requestHeaders.length}
                <div class="detail-stack">
                  {#if queryParams.length}
                    <div class="detail-block">
                      <h6 class="detail-micro-title">Query Parameters</h6>
                      <div class="detail-kv-list">
                        {#each queryParams as row (row.id)}
                          <div class="detail-kv-item">
                            <strong>{row.key || "(empty key)"}</strong>
                            <span>{row.value || "(empty value)"}</span>
                          </div>
                        {/each}
                      </div>
                    </div>
                  {/if}

                  {#if requestHeaders.length}
                    <div class="detail-block">
                      <h6 class="detail-micro-title">Request Headers</h6>
                      <div class="detail-kv-list">
                        {#each requestHeaders as row (row.id)}
                          <div class="detail-kv-item">
                            <strong>{row.key || "(empty key)"}</strong>
                            <span>{row.value || "(empty value)"}</span>
                          </div>
                        {/each}
                      </div>
                    </div>
                  {/if}
                </div>
              {:else}
                <div class="empty-state">No query parameters or request headers were stored.</div>
              {/if}
            </div>

            <div class="detail-response-column">
              <h5 class="detail-subtitle">Body</h5>
              {#if requestBody}
                <pre class="detail-code-block">{requestBody}</pre>
              {:else}
                <div class="empty-state">No request body was stored for this entry.</div>
              {/if}
            </div>
          </div>
        </section>

        <section class="detail-card detail-card-span">
          <h4 class="detail-section-title">Stored Response</h4>

          {#if detail.errorText}
            <div class="response-error">{detail.errorText}</div>
          {/if}

          <div class="detail-response-columns">
            <div class="detail-response-column">
              <h5 class="detail-subtitle">Headers</h5>
              {#if responseHeaders.length}
                <div class="detail-kv-list detail-kv-list-compact">
                  {#each responseHeaders as row (row.id)}
                    <div class="detail-kv-item">
                      <strong>{row.key}</strong>
                      <span>{row.value}</span>
                    </div>
                  {/each}
                </div>
              {:else}
                <div class="empty-state">No response headers were stored for this entry.</div>
              {/if}
            </div>

            <div class="detail-response-column">
              <h5 class="detail-subtitle">Body</h5>
              {#if responseBody}
                <pre class="detail-code-block">{responseBody}</pre>
              {:else if !detail.errorText}
                <div class="empty-state">No response preview was stored for this history entry.</div>
              {/if}
            </div>
          </div>
        </section>
      </div>
    {/if}
  </section>
{:else}
  <section class="history-detail history-detail-empty">
    <div class="editor-header">
      <h3>History Detail</h3>
    </div>

    <div class="history-detail-empty-card">
      <strong class="history-detail-empty-title">No history entry selected</strong>
      <p class="history-detail-empty-text">Select a history entry to inspect its stored request and response details.</p>
    </div>
  </section>
{/if}
