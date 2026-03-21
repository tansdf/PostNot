<script lang="ts">
  import { goto } from "$app/navigation";
  import { page } from "$app/stores";
  import { get } from "svelte/store";
  import { onMount, tick } from "svelte";

  import {
    cancelActiveRequest,
    clearHistory,
    getEnvironment,
    getHistoryEntry,
    getSavedRequest,
    getSettings,
    listEnvironments,
    listHistory,
    setActiveEnvironment,
    sendRequest
  } from "$lib/api/commands";
  import type {
    AppSettings,
    EnvironmentDetail,
    EnvironmentSummary,
    HistoryEntryDetail,
    HistoryEntrySummary,
    ResponsePayload
  } from "$lib/api/types";
  import { createDefaultSettings, createRequestDraft } from "$lib/api/types";
  import HistoryPanel from "$lib/components/history/HistoryPanel.svelte";
  import AppShell from "$lib/components/layout/AppShell.svelte";
  import RequestEditor from "$lib/components/request/RequestEditor.svelte";
  import ResponseViewer from "$lib/components/response/ResponseViewer.svelte";
  import {
    collectionsState,
    ensureCollectionsLoaded,
    resetCollectionsError,
    saveNewRequest,
    selectCollection,
    updateExistingSavedRequest
  } from "$lib/stores/collections";

  let request = createRequestDraft();
  let response: ResponsePayload | null = null;
  let settings: AppSettings = createDefaultSettings();
  let history: HistoryEntrySummary[] = [];
  let isSending = false;
  let isCancelingRequest = false;
  let isHistoryLoading = true;
  let isHistoryDetailLoading = false;
  let isClearingHistory = false;
  let historyErrorText = "";
  let historyDetailErrorText = "";
  let settingsErrorText = "";
  let requestSaveErrorText = "";
  let environments: EnvironmentSummary[] = [];
  let activeEnvironmentId = "";
  let activeEnvironmentDetail: EnvironmentDetail | null = null;
  let isEnvironmentsLoading = true;
  let isEnvironmentChanging = false;
  let environmentsErrorText = "";
  let selectedHistoryId = "";
  let selectedHistoryDetail: HistoryEntryDetail | null = null;
  let activeSavedRequestId = "";
  let activeSavedRequestCollectionId = "";
  let isSaveDialogOpen = false;
  let saveTargetCollectionId = "";
  let lastLoadedSavedRequestId = "";
  let isLoadingSavedRequest = false;
  let requestedSavedRequestId = "";

  onMount(async () => {
    await Promise.all([loadSettings(), loadHistory(), ensureCollectionsLoaded(), loadEnvironments()]);
  });

  $: requestedSavedRequestId = $page.url.searchParams.get("savedRequestId") ?? "";

  $: if (
    requestedSavedRequestId &&
    requestedSavedRequestId !== lastLoadedSavedRequestId &&
    !isLoadingSavedRequest
  ) {
    void loadSavedRequestFromRoute(requestedSavedRequestId);
  }

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

  async function loadEnvironments(preferredEnvironmentId = activeEnvironmentId) {
    isEnvironmentsLoading = true;

    try {
      environments = await listEnvironments();
      const activeEnvironment = environments.find((environment) => environment.isActive) ?? null;
      activeEnvironmentId = activeEnvironment?.id ?? "";
      environmentsErrorText = "";

      const detailEnvironmentId =
        preferredEnvironmentId && environments.some((environment) => environment.id === preferredEnvironmentId)
          ? preferredEnvironmentId
          : activeEnvironment?.id ?? "";

      if (detailEnvironmentId) {
        activeEnvironmentDetail = await getEnvironment(detailEnvironmentId);
      } else {
        activeEnvironmentDetail = null;
      }
    } catch (error) {
      environmentsErrorText = error instanceof Error ? error.message : String(error);
      activeEnvironmentDetail = null;
    } finally {
      isEnvironmentsLoading = false;
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

  async function handleEnvironmentChange(environmentId: string) {
    isEnvironmentChanging = true;

    try {
      await setActiveEnvironment(environmentId || null);
      await loadEnvironments(environmentId);
    } catch (error) {
      environmentsErrorText = error instanceof Error ? error.message : String(error);
    } finally {
      isEnvironmentChanging = false;
    }
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
    isCancelingRequest = false;

    try {
      response = await sendRequest(request);
    } catch (error) {
      const errorText = error instanceof Error ? error.message : String(error);

      response = {
        statusCode: null,
        statusText: errorText === "Request canceled." ? "Request canceled" : "Request failed",
        durationMs: 0,
        sizeBytes: 0,
        headers: [],
        bodyText: "",
        errorText,
        executedAt: new Date().toISOString()
      };
    } finally {
      isSending = false;
      isCancelingRequest = false;
      await loadHistory();

      if (selectedHistoryId) {
        await inspectHistoryEntry(selectedHistoryId, true);
      }
    }
  }

  async function handleCancelRequest() {
    if (!isSending || isCancelingRequest) {
      return;
    }

    isCancelingRequest = true;

    try {
      await cancelActiveRequest();
    } catch {
      isCancelingRequest = false;
    }
  }

  async function loadSavedRequestFromRoute(itemId: string) {
    isLoadingSavedRequest = true;

    try {
      const savedRequest = await getSavedRequest(itemId);
      await ensureCollectionsLoaded(savedRequest.collectionId);
      request = structuredClone(savedRequest.request);
      response = null;
      activeSavedRequestId = savedRequest.id;
      activeSavedRequestCollectionId = savedRequest.collectionId;
      lastLoadedSavedRequestId = savedRequest.id;
      requestSaveErrorText = "";
      resetCollectionsError();
      await selectCollection(savedRequest.collectionId);
    } catch (error) {
      requestSaveErrorText = error instanceof Error ? error.message : String(error);
    } finally {
      isLoadingSavedRequest = false;
    }
  }

  async function handleSaveRequest() {
    requestSaveErrorText = "";
    resetCollectionsError();

    const collectionState = get(collectionsState);
    const hasActiveSavedRequest =
      activeSavedRequestId &&
      activeSavedRequestCollectionId &&
      collectionState.collections.some((collection) => collection.id === activeSavedRequestCollectionId);

    if (hasActiveSavedRequest) {
      const savedRequest = await updateExistingSavedRequest(activeSavedRequestId, activeSavedRequestCollectionId, request);

      if (!savedRequest) {
        requestSaveErrorText = get(collectionsState).errorText;
        return;
      }

      await goto(`/?savedRequestId=${encodeURIComponent(savedRequest.id)}`, {
        replaceState: true,
        noScroll: true,
        keepFocus: true
      });

      return;
    }

    if (collectionState.collections.length === 0) {
      requestSaveErrorText = "Create a collection first from the sidebar.";
      return;
    }

    saveTargetCollectionId =
      collectionState.selectedCollectionId || activeSavedRequestCollectionId || collectionState.collections[0].id;
    isSaveDialogOpen = true;
  }

  async function confirmSaveRequest() {
    if (!saveTargetCollectionId) {
      requestSaveErrorText = "Choose a collection first.";
      return;
    }

    const savedRequest = await saveNewRequest(saveTargetCollectionId, request);

    if (!savedRequest) {
      requestSaveErrorText = get(collectionsState).errorText;
      return;
    }

    activeSavedRequestId = savedRequest.id;
    activeSavedRequestCollectionId = savedRequest.collectionId;
    lastLoadedSavedRequestId = savedRequest.id;
    requestSaveErrorText = "";
    isSaveDialogOpen = false;
    await selectCollection(savedRequest.collectionId);
    await goto(`/?savedRequestId=${encodeURIComponent(savedRequest.id)}`, {
      replaceState: true,
      noScroll: true,
      keepFocus: true
    });
  }

  function closeSaveDialog() {
    isSaveDialogOpen = false;
    requestSaveErrorText = "";
  }

  function handleSaveDialogBackdropKeydown(event: KeyboardEvent) {
    if (event.key === "Escape" || event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      closeSaveDialog();
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
        <div class="status-item">
          <span class="status-label">Environment</span>
          <strong>{activeEnvironmentDetail?.name ?? "None"}</strong>
        </div>
      </div>

      <div class="environment-toolbar">
        <label class="environment-select-field">
          <span class="field-label">Active environment</span>
          <select
            class="text-input"
            value={activeEnvironmentId}
            on:change={(event) => handleEnvironmentChange(event.currentTarget.value)}
            disabled={isEnvironmentChanging}
          >
            <option value="">No environment</option>
            {#each environments as environment (environment.id)}
              <option value={environment.id}>{environment.name}</option>
            {/each}
          </select>
        </label>

        <div class="environment-summary">
          {#if isEnvironmentsLoading}
            <span class="history-meta">Loading environments...</span>
          {:else if activeEnvironmentDetail}
            <span class="history-meta">
              {activeEnvironmentDetail.variables.filter((item) => item.enabled && item.key.trim()).length}
              active variable{activeEnvironmentDetail.variables.filter((item) => item.enabled && item.key.trim()).length === 1 ? "" : "s"}
            </span>
          {:else}
            <span class="history-meta">Requests will be sent without variable substitution.</span>
          {/if}
        </div>
      </div>

      {#if settingsErrorText}
        <div class="response-error">{settingsErrorText}</div>
      {/if}

      {#if environmentsErrorText}
        <div class="response-error">{environmentsErrorText}</div>
      {/if}
    </section>

    <RequestEditor
      bind:request
      environmentVariables={activeEnvironmentDetail?.variables ?? []}
      {isSending}
      isCanceling={isCancelingRequest}
      isSaving={$collectionsState.isSavingRequest}
      saveLabel={activeSavedRequestId ? "Update" : "Save"}
      saveDisabled={isSending}
      onSend={handleSend}
      onCancel={handleCancelRequest}
      onSave={handleSaveRequest}
    />

    {#if requestSaveErrorText}
      <div class="response-error">{requestSaveErrorText}</div>
    {/if}

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

  {#if isSaveDialogOpen}
    <div
      class="modal-backdrop"
      role="button"
      tabindex="0"
      aria-label="Close save request dialog"
      on:click|self={closeSaveDialog}
      on:keydown={handleSaveDialogBackdropKeydown}
    >
      <div
        class="panel save-dialog"
        role="dialog"
        tabindex="-1"
        aria-modal="true"
        aria-labelledby="save-request-title"
      >
        <div class="editor-header">
          <h2 id="save-request-title">Save Request</h2>
        </div>

        <div class="editor-block">
          <div>
            <span class="field-label">Choose a collection</span>
            <div class="save-target-list" role="listbox" aria-label="Choose a collection">
              {#each $collectionsState.collections as collection (collection.id)}
                <button
                  class:save-target-active={saveTargetCollectionId === collection.id}
                  class="save-target-button"
                  type="button"
                  role="option"
                  aria-selected={saveTargetCollectionId === collection.id}
                  on:click={() => (saveTargetCollectionId = collection.id)}
                >
                  <strong>{collection.name}</strong>
                  <span>{collection.requestCount} request{collection.requestCount === 1 ? "" : "s"}</span>
                </button>
              {/each}
            </div>
          </div>

          <div class="collections-page-actions">
            <button class="send-button" type="button" on:click={confirmSaveRequest} disabled={$collectionsState.isSavingRequest}>
              {$collectionsState.isSavingRequest ? "Saving..." : "Save request"}
            </button>
            <button class="ghost-button" type="button" on:click={closeSaveDialog}>
              Cancel
            </button>
          </div>
        </div>
      </div>
    </div>
  {/if}
</AppShell>
