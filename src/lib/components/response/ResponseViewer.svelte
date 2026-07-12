<script lang="ts">
  import type { RequestScriptExecution, ResponsePayload } from "$lib/api/types";
  import JsonViewer from "$lib/components/response/JsonViewer.svelte";
  import VirtualResponseDocument from "$lib/components/response/VirtualResponseDocument.svelte";

  type RequestResponseProgress = {
    downloadedBytes: number;
    contentLength: number | null;
    finished: boolean;
  };

  let {
    response = null,
    scriptExecution = null,
    isSending = false,
    progress = null,
    elapsedMs = 0,
    areTestsRunning = false
  }: {
    response?: ResponsePayload | null;
    scriptExecution?: RequestScriptExecution | null;
    isSending?: boolean;
    progress?: RequestResponseProgress | null;
    elapsedMs?: number;
    areTestsRunning?: boolean;
  } = $props();

  let responseErrorSummary = $derived.by(() => {
    if (!response?.errorText) {
      return "";
    }

    return response.statusText || "Request failed";
  });

  let progressPercent = $derived(
    progress?.contentLength
      ? Math.min(100, Math.max(0, (progress.downloadedBytes / progress.contentLength) * 100))
      : null
  );
  let receiveLabel = $derived.by(() => {
    if (!isSending) {
      return "";
    }

    const elapsed = formatDuration(elapsedMs);
    if (!progress) {
      return `Connecting... ${elapsed}`;
    }

    const downloaded = formatBytes(progress.downloadedBytes);
    if (progress.contentLength) {
      return `Receiving ${downloaded} of ${formatBytes(progress.contentLength)} · ${elapsed}`;
    }

    return `Receiving ${downloaded} · ${elapsed}`;
  });

  function formatBytes(bytes: number) {
    if (!Number.isFinite(bytes) || bytes <= 0) {
      return "0 B";
    }

    const units = ["B", "KB", "MB", "GB"];
    let value = bytes;
    let unitIndex = 0;

    while (value >= 1024 && unitIndex < units.length - 1) {
      value /= 1024;
      unitIndex += 1;
    }

    const precision = value >= 10 || unitIndex === 0 ? 0 : 1;
    return `${value.toFixed(precision)} ${units[unitIndex]}`;
  }

  function formatDuration(ms: number) {
    if (!Number.isFinite(ms) || ms <= 0) {
      return "0.0 s";
    }

    return `${(ms / 1000).toFixed(1)} s`;
  }
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

  {#if isSending}
    <div class="response-receive-status">
      <div class="response-receive-label">
        <span class="response-live-dot" aria-hidden="true"></span>
        <span>{receiveLabel}</span>
      </div>
      <div class="response-receive-track" aria-hidden="true">
        {#if progressPercent !== null}
          <span class="response-receive-bar" style:width={`${progressPercent}%`}></span>
        {:else}
          <span class="response-receive-bar response-receive-bar-indeterminate"></span>
        {/if}
      </div>
    </div>
  {/if}

  {#if response}
    {#if response.errorText}
      <div class="feedback feedback-error">
        <strong>{responseErrorSummary}</strong>
        <details class="response-error-details">
          <summary>Details</summary>
          <pre>{response.errorText}</pre>
        </details>
      </div>
    {/if}

    {#if scriptExecution?.preRequestErrorText}
      <div class="feedback feedback-error">{scriptExecution.preRequestErrorText}</div>
    {/if}

    {#if scriptExecution?.testScriptErrorText}
      <div class="feedback feedback-error">{scriptExecution.testScriptErrorText}</div>
    {/if}

    {#if areTestsRunning}
      <div class="feedback feedback-info" role="status">Running response tests…</div>
    {:else if scriptExecution && scriptExecution.tests.length > 0}
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
        {#if response.body.mode === "file"}
          <VirtualResponseDocument body={response.body} />
        {:else}
          <JsonViewer source={response.body.text} />
        {/if}
      </div>
    </div>
  {:else}
    <div class="empty-state">
      Send a request to inspect the status, headers, and response body here.
    </div>
  {/if}
</section>
