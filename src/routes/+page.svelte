<script lang="ts">
  import { onMount, tick } from "svelte";

  import { clearHistory, getHistoryEntry, getSettings, listHistory, sendRequest } from "$lib/api/commands";
  import type { AppSettings, HistoryEntryDetail, HistoryEntrySummary, ResponsePayload } from "$lib/api/types";
  import { createDefaultSettings, createRequestDraft } from "$lib/api/types";
  import HistoryPanel from "$lib/components/history/HistoryPanel.svelte";
  import AppShell from "$lib/components/layout/AppShell.svelte";
  import RequestEditor from "$lib/components/request/RequestEditor.svelte";
  import ResponseViewer from "$lib/components/response/ResponseViewer.svelte";

  let request = createRequestDraft();
  let response: ResponsePayload | null = null;
  let settings: AppSettings = createDefaultSettings();
  let history: HistoryEntrySummary[] = [];
  let isSending = false;
  let isHistoryLoading = true;
  let isHistoryDetailLoading = false;
  let isClearingHistory = false;
  let historyErrorText = "";
  let historyDetailErrorText = "";
  let settingsErrorText = "";
  let selectedHistoryId = "";
  let selectedHistoryDetail: HistoryEntryDetail | null = null;

  onMount(async () => {
    await Promise.all([loadSettings(), loadHistory()]);
  });

  async function loadSettings() {
    try {
      settings = await getSettings();
      settingsErrorText = "";
    } catch (error) {
      settingsErrorText = error instanceof Error ? error.message : String(error);
    }
  }

  async function loadHistory() {
    isHistoryLoading = true;

    try {
      history = await listHistory(12);
      historyErrorText = "";

      if (selectedHistoryId && !history.some((entry) => entry.id === selectedHistoryId)) {
        closeHistoryDetail();
      }
    } catch (error) {
      historyErrorText = error instanceof Error ? error.message : String(error);
    } finally {
      isHistoryLoading = false;
    }
  }

  async function inspectHistoryEntry(id: string, shouldKeepExistingDetail = false) {
    const scrollY = window.scrollY;
    selectedHistoryId = id;
    isHistoryDetailLoading = true;
    historyDetailErrorText = "";

    if (!shouldKeepExistingDetail) {
      selectedHistoryDetail = null;
    }

    try {
      selectedHistoryDetail = await getHistoryEntry(id);
    } catch (error) {
      selectedHistoryDetail = null;
      historyDetailErrorText = error instanceof Error ? error.message : String(error);
    } finally {
      isHistoryDetailLoading = false;
      await tick();
      window.scrollTo({ top: scrollY });
    }
  }

  function closeHistoryDetail() {
    selectedHistoryId = "";
    selectedHistoryDetail = null;
    historyDetailErrorText = "";
    isHistoryDetailLoading = false;
  }

  async function handleClearHistory() {
    if (!window.confirm("Clear all stored request history? This cannot be undone.")) {
      return;
    }

    isClearingHistory = true;

    try {
      await clearHistory();
      closeHistoryDetail();
      await loadHistory();
      historyErrorText = "";
    } catch (error) {
      historyErrorText = error instanceof Error ? error.message : String(error);
    } finally {
      isClearingHistory = false;
    }
  }

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
      await loadHistory();

      if (selectedHistoryId) {
        await inspectHistoryEntry(selectedHistoryId, true);
      }
    }
  }
</script>

<svelte:head>
  <title>PostNot</title>
</svelte:head>

<AppShell>
  <div class="workspace-grid">
    <section class="panel status-panel">
      <div class="editor-header">
        <h2>Request Profile</h2>
      </div>

      <div class="status-grid">
        <div class="status-item">
          <span class="status-label">Timeout</span>
          <strong>{settings.requestTimeoutMs} ms</strong>
        </div>
        <div class="status-item">
          <span class="status-label">Redirects</span>
          <strong>{settings.followRedirects ? "Follow" : "Disabled"}</strong>
        </div>
        <div class="status-item">
          <span class="status-label">TLS</span>
          <strong>{settings.validateTls ? "Validated" : "Relaxed"}</strong>
        </div>
        <div class="status-item">
          <span class="status-label">History limit</span>
          <strong>{settings.historyLimit}</strong>
        </div>
      </div>

      {#if settingsErrorText}
        <div class="response-error">{settingsErrorText}</div>
      {/if}
    </section>

    <RequestEditor bind:request {isSending} onSend={handleSend} />
    <ResponseViewer {response} />
    <HistoryPanel
      items={history}
      isLoading={isHistoryLoading}
      errorText={historyErrorText}
      selectedId={selectedHistoryId}
      detail={selectedHistoryDetail}
      detailErrorText={historyDetailErrorText}
      isDetailLoading={isHistoryDetailLoading}
      isClearing={isClearingHistory}
      onInspect={inspectHistoryEntry}
      onClear={handleClearHistory}
      onCloseDetail={closeHistoryDetail}
    />
  </div>
</AppShell>
