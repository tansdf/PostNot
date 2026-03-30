<script lang="ts">
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import { onMount, tick } from "svelte";

  import {
    cancelActiveRequest,
    clearHistory,
    getEnvironment,
    getHistoryEntry,
    importCurlRequestToDraft,
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
  import RequestEditor from "$lib/components/request/RequestEditor.svelte";
  import ResponseViewer from "$lib/components/response/ResponseViewer.svelte";
  import { collections } from "$lib/stores/collections.svelte";
  import { notifications } from "$lib/stores/notifications.svelte";

  let request = $state(createRequestDraft());
  let response: ResponsePayload | null = $state(null);
  let settings: AppSettings = $state(createDefaultSettings());
  let history: HistoryEntrySummary[] = $state([]);
  let isSending = $state(false);
  let isCancelingRequest = $state(false);
  let isHistoryLoading = $state(true);
  let isHistoryDetailLoading = $state(false);
  let isClearingHistory = $state(false);
  let historyErrorText = $state("");
  let historyDetailErrorText = $state("");
  let settingsErrorText = $state("");
  let requestSaveErrorText = $state("");
  let environments: EnvironmentSummary[] = $state([]);
  let activeEnvironmentId = $state("");
  let activeEnvironmentDetail: EnvironmentDetail | null = $state(null);
  let isEnvironmentsLoading = $state(true);
  let isEnvironmentChanging = $state(false);
  let environmentsErrorText = $state("");
  let selectedHistoryId = $state("");
  let selectedHistoryDetail: HistoryEntryDetail | null = $state(null);
  let activeSavedRequestId = $state("");
  let activeSavedRequestCollectionId = $state("");
  let isSaveDialogOpen = $state(false);
  let isCurlImportDialogOpen = $state(false);
  let curlImportSource = $state("");
  let isImportingCurl = $state(false);
  let curlImportErrorText = $state("");
  let saveTargetCollectionId = $state("");
  let lastLoadedSavedRequestId = $state("");
  let isLoadingSavedRequest = $state(false);

  let requestedSavedRequestId = $derived(page.url.searchParams.get("savedRequestId") ?? "");

  onMount(async () => {
    await Promise.all([loadSettings(), loadHistory(), collections.ensureLoaded(), loadEnvironments()]);
  });

  $effect(() => {
    if (requestedSavedRequestId && requestedSavedRequestId !== lastLoadedSavedRequestId && !isLoadingSavedRequest) {
      void loadSavedRequestFromRoute(requestedSavedRequestId);
    }
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
      if (environmentId) {
        const environmentName = environments.find((environment) => environment.id === environmentId)?.name ?? "Environment";
        notifications.info(environmentName, "Active environment changed");
      } else {
        notifications.info("Requests will now run without an active environment.", "Environment cleared");
      }
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
      notifications.success("Stored request history was cleared.", "History cleared");
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
      await collections.ensureLoaded(savedRequest.collectionId);
      request = structuredClone(savedRequest.request);
      response = null;
      activeSavedRequestId = savedRequest.id;
      activeSavedRequestCollectionId = savedRequest.collectionId;
      lastLoadedSavedRequestId = savedRequest.id;
      requestSaveErrorText = "";
      collections.resetError();
      await collections.selectCollection(savedRequest.collectionId);
    } catch (error) {
      requestSaveErrorText = error instanceof Error ? error.message : String(error);
    } finally {
      isLoadingSavedRequest = false;
    }
  }

  async function handleSaveRequest() {
    requestSaveErrorText = "";
    collections.resetError();

    const hasActiveSavedRequest =
      activeSavedRequestId &&
      activeSavedRequestCollectionId &&
      collections.collections.some((collection) => collection.id === activeSavedRequestCollectionId);

    if (hasActiveSavedRequest) {
      const savedRequest = await collections.updateExistingSavedRequest(activeSavedRequestId, activeSavedRequestCollectionId, request);

      if (!savedRequest) {
        requestSaveErrorText = collections.errorText;
        return;
      }

      await goto(`/?savedRequestId=${encodeURIComponent(savedRequest.id)}`, {
        replaceState: true,
        noScroll: true,
        keepFocus: true
      });

      return;
    }

    if (collections.collections.length === 0) {
      requestSaveErrorText = "Create a collection first from the sidebar.";
      return;
    }

    saveTargetCollectionId =
      collections.selectedCollectionId || activeSavedRequestCollectionId || collections.collections[0].id;
    isSaveDialogOpen = true;
  }

  async function confirmSaveRequest() {
    if (!saveTargetCollectionId) {
      requestSaveErrorText = "Choose a collection first.";
      return;
    }

    const savedRequest = await collections.saveNewRequest(saveTargetCollectionId, request);

    if (!savedRequest) {
      requestSaveErrorText = collections.errorText;
      return;
    }

    activeSavedRequestId = savedRequest.id;
    activeSavedRequestCollectionId = savedRequest.collectionId;
    lastLoadedSavedRequestId = savedRequest.id;
    requestSaveErrorText = "";
    isSaveDialogOpen = false;
    await collections.selectCollection(savedRequest.collectionId);
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

  async function handleNewRequest() {
    request = createRequestDraft();
    response = null;
    requestSaveErrorText = "";
    activeSavedRequestId = "";
    activeSavedRequestCollectionId = "";
    lastLoadedSavedRequestId = "";
    await goto("/", {
      replaceState: true,
      noScroll: true,
      keepFocus: true
    });
  }

  function openCurlImportDialog() {
    curlImportSource = "";
    curlImportErrorText = "";
    isCurlImportDialogOpen = true;
  }

  function closeCurlImportDialog() {
    isCurlImportDialogOpen = false;
    curlImportSource = "";
    curlImportErrorText = "";
  }

  async function handleImportCurl() {
    curlImportErrorText = "";
    const source = curlImportSource.trim();
    if (!source) {
      curlImportErrorText = "Paste a complete cURL command to import.";
      return;
    }

    isImportingCurl = true;

    try {
      const imported = await importCurlRequestToDraft({ source });
      request = structuredClone(imported.request);
      response = null;
      requestSaveErrorText = "";
      activeSavedRequestId = "";
      activeSavedRequestCollectionId = "";
      lastLoadedSavedRequestId = "";
      closeCurlImportDialog();
      await goto("/", {
        replaceState: true,
        noScroll: true,
        keepFocus: true
      });
      notifications.success("The imported cURL command is now loaded into the request editor.", "cURL imported");
    } catch (error) {
      curlImportErrorText = error instanceof Error ? error.message : String(error);
    } finally {
      isImportingCurl = false;
    }
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

<div class="workspace-grid">
    <div class="profile-bar">
      <div class="profile-facts">
        <span class="profile-fact">Timeout <strong>{settings.requestTimeoutMs} ms</strong></span>
        <span class="profile-fact">Redirects <strong>{settings.followRedirects ? "Follow" : "Off"}</strong></span>
        <span class="profile-fact">TLS <strong>{settings.validateTls ? "Validated" : "Relaxed"}</strong></span>
        <span class="profile-fact">History <strong>{settings.historyLimit}</strong></span>
      </div>

      <span class="profile-divider"></span>

      <div class="profile-env-section">
        <label>
          <span class="sr-only">Active environment</span>
          <select
            class="text-input profile-env-select"
            value={activeEnvironmentId}
            onchange={(event) => handleEnvironmentChange(event.currentTarget.value)}
            disabled={isEnvironmentChanging}
          >
            <option value="">No environment</option>
            {#each environments as environment (environment.id)}
              <option value={environment.id}>{environment.name}</option>
            {/each}
          </select>
        </label>

        {#if isEnvironmentsLoading}
          <span class="profile-env-hint">Loading...</span>
        {:else if activeEnvironmentDetail}
          <span class="profile-env-hint">
            {activeEnvironmentDetail.variables.filter((item) => item.enabled && item.key.trim()).length} var{activeEnvironmentDetail.variables.filter((item) => item.enabled && item.key.trim()).length === 1 ? "" : "s"}
          </span>
        {/if}
      </div>

      {#if settingsErrorText}
        <span class="profile-env-hint" style="color: var(--danger)">{settingsErrorText}</span>
      {/if}

      {#if environmentsErrorText}
        <span class="profile-env-hint" style="color: var(--danger)">{environmentsErrorText}</span>
      {/if}
    </div>

    <RequestEditor
      bind:request
      environmentVariables={activeEnvironmentDetail?.variables ?? []}
      {isSending}
      isCanceling={isCancelingRequest}
      isSaving={collections.isSavingRequest}
      saveLabel={activeSavedRequestId ? "Update" : "Save"}
      saveDisabled={isSending}
      onNewRequest={handleNewRequest}
      onOpenCurlImport={openCurlImportDialog}
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
      onclick={(e) => { if (e.target === e.currentTarget) closeSaveDialog(); }}
      onkeydown={handleSaveDialogBackdropKeydown}
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
              {#each collections.collections as collection (collection.id)}
                <button
                  class={["save-target-button", saveTargetCollectionId === collection.id && "save-target-active"]}
                  type="button"
                  role="option"
                  aria-selected={saveTargetCollectionId === collection.id}
                  onclick={() => (saveTargetCollectionId = collection.id)}
                >
                  <strong>{collection.name}</strong>
                  <span>{collection.requestCount} request{collection.requestCount === 1 ? "" : "s"}</span>
                </button>
              {/each}
            </div>
          </div>

          <div class="collections-page-actions">
            <button class="send-button" type="button" onclick={confirmSaveRequest} disabled={collections.isSavingRequest}>
              {collections.isSavingRequest ? "Saving..." : "Save request"}
            </button>
            <button class="ghost-button" type="button" onclick={closeSaveDialog}>
              Cancel
            </button>
          </div>
        </div>
      </div>
    </div>
  {/if}

  {#if isCurlImportDialogOpen}
    <div
      class="modal-backdrop"
      role="button"
      tabindex="0"
      aria-label="Close cURL import dialog"
      onclick={(e) => { if (e.target === e.currentTarget) closeCurlImportDialog(); }}
      onkeydown={(event) => {
        if (event.key === "Escape" || event.key === "Enter" || event.key === " ") {
          event.preventDefault();
          closeCurlImportDialog();
        }
      }}
    >
      <div class="panel save-dialog" role="dialog" tabindex="-1" aria-modal="true" aria-labelledby="curl-import-title">
        <div class="editor-header">
          <h2 id="curl-import-title">Import cURL</h2>
        </div>

        <div class="editor-block">
          <label>
            <span class="field-label">Paste cURL command</span>
            <textarea
              class="text-input collections-import-source"
              bind:value={curlImportSource}
              placeholder='curl --request GET https://api.example.com/items -H "Authorization: Bearer token"'
            ></textarea>
          </label>

          {#if curlImportErrorText}
            <div class="response-error">{curlImportErrorText}</div>
          {/if}

          <div class="collections-page-actions">
            <button class="send-button" type="button" onclick={handleImportCurl} disabled={isImportingCurl}>
              {isImportingCurl ? "Importing..." : "Import request"}
            </button>
            <button class="ghost-button" type="button" onclick={closeCurlImportDialog}>
              Cancel
            </button>
          </div>
        </div>
      </div>
    </div>
  {/if}
