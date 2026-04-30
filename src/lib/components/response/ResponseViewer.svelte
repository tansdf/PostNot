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

  let decodedBinaryText = $state("");
  let decodeErrorText = $state("");
  let decodedResponseKey = $state("");

  let responseKey = $derived(
    response ? `${response.executedAt}-${response.sizeBytes}-${response.bodyEncoding}` : ""
  );
  let visibleBodyText = $derived(decodedBinaryText || response?.bodyText || "");
  let canDecodeBinaryPreview = $derived(Boolean(response?.bodyIsBinary && response.bodyBase64));

  $effect(() => {
    if (responseKey === decodedResponseKey) {
      return;
    }

    decodedBinaryText = "";
    decodeErrorText = "";
    decodedResponseKey = responseKey;
  });

  function decodeBase64ToBytes(value: string) {
    const binary = atob(value);
    const bytes = new Uint8Array(binary.length);
    for (let index = 0; index < binary.length; index += 1) {
      bytes[index] = binary.charCodeAt(index);
    }
    return bytes;
  }

  function decodeBinaryPreview() {
    if (!response?.bodyBase64) {
      return;
    }

    try {
      decodedBinaryText = new TextDecoder("utf-8", { fatal: false }).decode(
        decodeBase64ToBytes(response.bodyBase64)
      );
      decodeErrorText = "";
    } catch (error) {
      decodeErrorText = error instanceof Error ? error.message : String(error);
    }
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
        {#if response.bodyContentType}
          <span>{response.bodyContentType}</span>
        {/if}
      </div>
    {/if}
  </div>

  {#if response}
    {#if response.errorText}
      <div class="response-error">{response.errorText}</div>
    {/if}

    {#if scriptExecution?.preRequestErrorText}
      <div class="response-error">{scriptExecution.preRequestErrorText}</div>
    {/if}

    {#if scriptExecution?.testScriptErrorText}
      <div class="response-error">{scriptExecution.testScriptErrorText}</div>
    {/if}

    {#if response.bodyIsBinary}
      <div class="settings-update-feedback">
        <strong>Binary response</strong>
        <p>
          {#if response.bodyText}
            This binary-looking body was decoded as {response.bodyEncoding}.
          {:else}
            Body text was not decoded automatically.
          {/if}
          {#if response.bodyContentType}
            Content type: {response.bodyContentType}.
          {/if}
        </p>

        {#if canDecodeBinaryPreview && !decodedBinaryText && !response.bodyText}
          <button class="system-button" type="button" onclick={decodeBinaryPreview}>
            Decode preview as text
          </button>
        {/if}

        {#if decodeErrorText}
          <p>{decodeErrorText}</p>
        {/if}
      </div>
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
        <JsonViewer source={visibleBodyText} />
      </div>
    </div>
  {:else}
    <div class="empty-state">
      Send a request to inspect the status, headers, and response body here.
    </div>
  {/if}
</section>
