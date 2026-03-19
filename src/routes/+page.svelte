<script lang="ts">
  import AppShell from "$lib/components/layout/AppShell.svelte";
  import RequestEditor from "$lib/components/request/RequestEditor.svelte";
  import ResponseViewer from "$lib/components/response/ResponseViewer.svelte";
  import { sendRequest } from "$lib/api/commands";
  import { createRequestDraft, type ResponsePayload } from "$lib/api/types";

  let request = createRequestDraft();
  let response: ResponsePayload | null = null;
  let isSending = false;

  async function handleSend() {
    isSending = true;

    try {
      response = await sendRequest(request);
    } catch (error) {
      response = {
        statusCode: null,
        statusText: "Request failed",
        durationMs: 0,
        sizeBytes: 0,
        headers: [],
        bodyText: "",
        errorText: error instanceof Error ? error.message : String(error),
        executedAt: new Date().toISOString()
      };
    } finally {
      isSending = false;
    }
  }
</script>

<svelte:head>
  <title>PostNot</title>
</svelte:head>

<AppShell>
  <div class="workspace-grid">
    <RequestEditor bind:request {isSending} onSend={handleSend} />
    <ResponseViewer {response} />
  </div>
</AppShell>
