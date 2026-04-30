<script lang="ts">
  import type { RequestScriptExecution, ResponsePayload } from "$lib/api/types";
  import JsonViewer from "$lib/components/response/JsonViewer.svelte";

  let {
    response = null,
    scriptExecution = null
  }: {
    response?: ResponsePayload | null;
    scriptExecution?: RequestScriptExecution | null;
  } = $props();

  let responseErrorSummary = $derived.by(() => {
    if (!response?.errorText) {
      return "";
    }

    return response.statusText || "Request failed";
  });
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
      <div class="response-error">
        <strong>{responseErrorSummary}</strong>
        <details class="response-error-details">
          <summary>Details</summary>
          <pre>{response.errorText}</pre>
        </details>
      </div>
    {/if}

    {#if scriptExecution?.preRequestErrorText}
      <div class="response-error">{scriptExecution.preRequestErrorText}</div>
    {/if}

    {#if scriptExecution?.testScriptErrorText}
      <div class="response-error">{scriptExecution.testScriptErrorText}</div>
    {/if}

    {#if scriptExecution && scriptExecution.tests.length > 0}
      <div class="response-tests">
        <div class="editor-header">
          <h3>Tests</h3>
          <span class="history-meta">
            {scriptExecution.tests.filter((test) => test.status === "passed").length} passed
            ·
            {scriptExecution.tests.filter((test) => test.status === "failed").length} failed
          </span>
        </div>

        <div class="response-test-list">
          {#each scriptExecution.tests as test (test.id)}
            <article class={["response-test-card", test.status === "failed" && "response-test-card-failed"]}>
              <div class="response-test-header">
                <strong>{test.name}</strong>
                <span>{test.status === "passed" ? "Passed" : "Failed"}</span>
              </div>

              {#if test.errorText}
                <p>{test.errorText}</p>
              {/if}
            </article>
          {/each}
        </div>
      </div>
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
        <JsonViewer source={response.bodyText} />
      </div>
    </div>
  {:else}
    <div class="empty-state">
      Send a request to inspect the status, headers, and response body here.
    </div>
  {/if}
</section>
