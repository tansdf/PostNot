<script lang="ts">
  import type { ResponsePayload } from "$lib/api/types";

  export let response: ResponsePayload | null = null;

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

  $: prettyBody = response ? formatBody(response.bodyText) : "";
</script>

<section class="panel response-panel">
  <div class="editor-header">
    <h2>Response</h2>

    {#if response}
      <div class="response-metrics">
        <span>Status: {response.statusCode ?? "-"} {response.statusText}</span>
        <span>Time: {response.durationMs} ms</span>
        <span>Size: {response.sizeBytes} bytes</span>
      </div>
    {/if}
  </div>

  {#if response}
    {#if response.errorText}
      <div class="response-error">{response.errorText}</div>
    {/if}

    <div class="response-columns">
      <div class="response-column">
        <h3>Headers</h3>
        <div class="header-list">
          {#each response.headers as header (header.id)}
            <div class="header-item">
              <strong>{header.key}</strong>
              <span>{header.value}</span>
            </div>
          {/each}
        </div>
      </div>

      <div class="response-column response-body-column">
        <h3>Body</h3>
        <pre class="response-body">{prettyBody}</pre>
      </div>
    </div>
  {:else}
    <div class="empty-state">
      Send a request to inspect the status, headers, and response body here.
    </div>
  {/if}
</section>
