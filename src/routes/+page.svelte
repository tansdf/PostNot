<script lang="ts">
  import { goto } from "$app/navigation";
  import { resolve } from "$app/paths";
  import { page } from "$app/state";
  import { onMount, tick } from "svelte";

  import {
    cancelActiveRequest,
    clearHistory,
    getEnvironment,
    getHistoryEntry,
    importCurlRequestToDraft,
    importOpenApiRequestToDraft,
    getSavedRequest,
    getSettings,
    listEnvironments,
    listHistory,
    setActiveEnvironment,
    sendRequest,
    updateEnvironment
  } from "$lib/api/commands";
  import type {
    AppSettings,
    EnvironmentDetail,
    EnvironmentSummary,
    HistoryEntryDetail,
    HistoryEntrySummary,
    CollectionItemSummary,
    RequestScriptExecution,
    ResponsePayload
  } from "$lib/api/types";
  import { createDefaultSettings, createRequestDraft } from "$lib/api/types";
  import HistoryPanel from "$lib/components/history/HistoryPanel.svelte";
  import RequestEditor from "$lib/components/request/RequestEditor.svelte";
  import ResponseViewer from "$lib/components/response/ResponseViewer.svelte";
  import {
    createEmptyRequestScriptExecution,
    type InheritedRequestScripts,
    runPreRequestScript,
    runTestScript
  } from "$lib/request-scripts";
  import { createStaleGuard } from "$lib/async-stale-guard";
  import { modalFocusTrap } from "$lib/modal-focus-trap";
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
  let scriptExecution: RequestScriptExecution = $state(createEmptyRequestScriptExecution());
  let activeSavedRequestId = $state("");
  let activeSavedRequestCollectionId = $state("");
  let activeSavedRequestParentId: string | null = $state(null);
  let isSaveDialogOpen = $state(false);
  let isRequestImportDialogOpen = $state(false);
  let requestImportFormat = $state<"curl" | "openapi">("curl");
  let curlImportSource = $state("");
  let openApiImportSource = $state("");
  let isImportingRequest = $state(false);
  let requestImportErrorText = $state("");
  let openApiImportFileInput: HTMLInputElement | null = $state(null);
  let saveTargetCollectionId = $state("");
  let saveTargetParentId: string | null = $state(null);
  let lastLoadedSavedRequestId = $state("");
  let isLoadingSavedRequest = $state(false);
  const savedRequestRoute = createStaleGuard();

  let requestedSavedRequestId = $derived(page.url.searchParams.get("savedRequestId") ?? "");

  onMount(async () => {
    await Promise.all([loadSettings(), loadHistory(), collections.ensureLoaded(), loadEnvironments()]);
  });

  $effect(() => {
    const id = requestedSavedRequestId;
    if (!id) {
      lastLoadedSavedRequestId = "";
      return;
    }
    if (id !== lastLoadedSavedRequestId) {
      void loadSavedRequestFromRoute(id);
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

  async function persistActiveEnvironmentFromScript(nextEnvironment: EnvironmentDetail): Promise<EnvironmentDetail> {
    const updated = await updateEnvironment(nextEnvironment.id, {
      name: nextEnvironment.name.trim(),
      variables: nextEnvironment.variables
    });

    activeEnvironmentDetail = updated;
    activeEnvironmentId = updated.id;
    environments = environments.map((environment) =>
      environment.id === updated.id
        ? {
            ...environment,
            name: updated.name,
            isActive: updated.isActive,
            variableCount: updated.variables.length,
            updatedAt: updated.updatedAt
          }
        : environment
    );

    return updated;
  }

  async function handleSend() {
    isSending = true;
    isCancelingRequest = false;
    scriptExecution = createEmptyRequestScriptExecution();
    const inheritedScripts = activeCollectionScripts();

    try {
      const preparedRequest = await runPreRequestScript(
        request,
        activeEnvironmentDetail?.variables ?? [],
        inheritedScripts,
        {
          activeEnvironment: activeEnvironmentDetail,
          persistActiveEnvironment: persistActiveEnvironmentFromScript
        }
      );
      if (preparedRequest.errorText) {
        scriptExecution = {
          ...createEmptyRequestScriptExecution(),
          preRequestErrorText: preparedRequest.errorText
        };
        response = {
          statusCode: null,
          statusText: "Pre-request script failed",
          durationMs: 0,
          sizeBytes: 0,
          headers: [],
          bodyText: "",
          errorText: "",
          executedAt: new Date().toISOString()
        };
        return;
      }

      const sendResult = await sendRequest(preparedRequest.request);
      response = sendResult.response;
      scriptExecution = await runTestScript(
        request,
        sendResult.response,
        activeEnvironmentDetail?.variables ?? [],
        inheritedScripts,
        {
          activeEnvironment: activeEnvironmentDetail,
          persistActiveEnvironment: persistActiveEnvironmentFromScript
        }
      );

      if (scriptExecution.testScriptErrorText) {
        notifications.warning(
          `The response was received, but the test script stopped early: ${scriptExecution.testScriptErrorText}`,
          "Test script error"
        );
      } else if (scriptExecution.tests.some((test) => test.status === "failed")) {
        const failedCount = scriptExecution.tests.filter((test) => test.status === "failed").length;
        notifications.warning(
          `${failedCount} scripted test${failedCount === 1 ? "" : "s"} failed for this response.`,
          "Tests failed"
        );
      }

      if (sendResult.historyPersistenceError) {
        notifications.warning(
          `The response is shown, but this run was not saved to history: ${sendResult.historyPersistenceError}`,
          "History not saved"
        );
      }
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

  function activeCollectionScripts(): InheritedRequestScripts | null {
    if (!activeSavedRequestCollectionId) {
      return null;
    }

    const collection = collections.collections.find((item) => item.id === activeSavedRequestCollectionId);
    if (!collection) {
      return null;
    }

    return {
      preRequestScript: collection.preRequestScript,
      testScript: collection.testScript,
      folderScripts: folderScriptPath(
        collections.collectionItemsByCollection[activeSavedRequestCollectionId] ?? [],
        activeSavedRequestParentId
      )
    };
  }

  function folderScriptPath(
    items: CollectionItemSummary[],
    targetFolderId: string | null
  ): InheritedRequestScripts["folderScripts"] {
    if (!targetFolderId) {
      return [];
    }

    const path = findFolderPath(items, targetFolderId);
    return path.map((folder) => ({
      name: folder.name,
      preRequestScript: folder.preRequestScript,
      testScript: folder.testScript
    }));
  }

  function findFolderPath(items: CollectionItemSummary[], targetFolderId: string): CollectionItemSummary[] {
    for (const item of items) {
      if (item.kind !== "folder") {
        continue;
      }

      if (item.id === targetFolderId) {
        return [item];
      }

      const childPath = findFolderPath(item.children, targetFolderId);
      if (childPath.length > 0) {
        return [item, ...childPath];
      }
    }

    return [];
  }

  async function loadSavedRequestFromRoute(itemId: string) {
    const seq = savedRequestRoute.next();
    isLoadingSavedRequest = true;

    try {
      const savedRequest = await getSavedRequest(itemId);
      if (savedRequestRoute.isStale(seq)) {
        return;
      }

      await collections.ensureLoaded(savedRequest.collectionId);
      if (savedRequestRoute.isStale(seq)) {
        return;
      }

      request = structuredClone(savedRequest.request);
      response = null;
      scriptExecution = createEmptyRequestScriptExecution();
      activeSavedRequestId = savedRequest.id;
      activeSavedRequestCollectionId = savedRequest.collectionId;
      activeSavedRequestParentId = savedRequest.parentId ?? null;
      lastLoadedSavedRequestId = savedRequest.id;
      requestSaveErrorText = "";
      collections.resetError();
      await collections.selectCollection(savedRequest.collectionId);
    } catch (error) {
      if (!savedRequestRoute.isStale(seq)) {
        requestSaveErrorText = error instanceof Error ? error.message : String(error);
      }
    } finally {
      if (!savedRequestRoute.isStale(seq)) {
        isLoadingSavedRequest = false;
      }
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

      await goto(resolve(`/?savedRequestId=${encodeURIComponent(savedRequest.id)}`), {
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
    saveTargetParentId = activeSavedRequestParentId ?? null;
    isSaveDialogOpen = true;
  }

  async function confirmSaveRequest() {
    if (!saveTargetCollectionId) {
      requestSaveErrorText = "Choose a collection first.";
      return;
    }

    const savedRequest = await collections.saveNewRequest(saveTargetCollectionId, request, saveTargetParentId);

    if (!savedRequest) {
      requestSaveErrorText = collections.errorText;
      return;
    }

    activeSavedRequestId = savedRequest.id;
    activeSavedRequestCollectionId = savedRequest.collectionId;
    activeSavedRequestParentId = savedRequest.parentId ?? null;
    lastLoadedSavedRequestId = savedRequest.id;
    requestSaveErrorText = "";
    isSaveDialogOpen = false;
    await collections.selectCollection(savedRequest.collectionId);
    await goto(resolve(`/?savedRequestId=${encodeURIComponent(savedRequest.id)}`), {
      replaceState: true,
      noScroll: true,
      keepFocus: true
    });
  }

  function closeSaveDialog() {
    isSaveDialogOpen = false;
    requestSaveErrorText = "";
    saveTargetParentId = null;
  }

  async function handleNewRequest() {
    request = createRequestDraft();
    response = null;
    scriptExecution = createEmptyRequestScriptExecution();
    requestSaveErrorText = "";
    activeSavedRequestId = "";
    activeSavedRequestCollectionId = "";
    activeSavedRequestParentId = null;
    lastLoadedSavedRequestId = "";
    await goto(resolve("/"), {
      replaceState: true,
      noScroll: true,
      keepFocus: true
    });
  }

  function openRequestImportDialog() {
    requestImportFormat = "curl";
    curlImportSource = "";
    openApiImportSource = "";
    requestImportErrorText = "";
    isRequestImportDialogOpen = true;
  }

  function closeRequestImportDialog() {
    isRequestImportDialogOpen = false;
    curlImportSource = "";
    openApiImportSource = "";
    requestImportErrorText = "";
  }

  async function handleImportRequest() {
    requestImportErrorText = "";
    const source = requestImportFormat === "curl" ? curlImportSource.trim() : openApiImportSource.trim();
    if (!source) {
      requestImportErrorText =
        requestImportFormat === "curl"
          ? "Paste a complete cURL command to import."
          : "Open an OpenAPI 3 JSON or YAML file, or paste the document payload to import.";
      return;
    }

    isImportingRequest = true;

    try {
      const imported =
        requestImportFormat === "curl"
          ? await importCurlRequestToDraft({ source })
          : await importOpenApiRequestToDraft({ source });
      request = structuredClone(imported.request);
      response = null;
      scriptExecution = createEmptyRequestScriptExecution();
      requestSaveErrorText = "";
      activeSavedRequestId = "";
      activeSavedRequestCollectionId = "";
      activeSavedRequestParentId = null;
      lastLoadedSavedRequestId = "";
      closeRequestImportDialog();
      await goto(resolve("/"), {
        replaceState: true,
        noScroll: true,
        keepFocus: true
      });
      notifications.success(
        requestImportFormat === "curl"
          ? "The imported cURL command is now loaded into the request editor."
          : "The imported OpenAPI request is now loaded into the request editor.",
        requestImportFormat === "curl" ? "cURL imported" : "OpenAPI request imported"
      );
    } catch (error) {
      requestImportErrorText = error instanceof Error ? error.message : String(error);
    } finally {
      isImportingRequest = false;
    }
  }

  function handleSaveDialogBackdropKeydown(event: KeyboardEvent) {
    if (event.key === "Enter" || event.key === " ") {
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
      onOpenImport={openRequestImportDialog}
      onSend={handleSend}
      onCancel={handleCancelRequest}
      onSave={handleSaveRequest}
    />

    {#if requestSaveErrorText}
      <div class="response-error">{requestSaveErrorText}</div>
    {/if}

    <ResponseViewer {response} {scriptExecution} />
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
      use:modalFocusTrap={{ onEscape: closeSaveDialog }}
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
                  onclick={async () => {
                    saveTargetCollectionId = collection.id;
                    saveTargetParentId = null;
                    await collections.loadCollectionItems(collection.id);
                  }}
                >
                  <strong>{collection.name}</strong>
                  <span>{collection.requestCount} request{collection.requestCount === 1 ? "" : "s"}</span>
                </button>
              {/each}
            </div>
          </div>

          {#if saveTargetCollectionId}
            <div>
              <span class="field-label">Choose a folder</span>
              <div class="save-target-list" role="listbox" aria-label="Choose a folder">
                {#each collections.folderTargets(saveTargetCollectionId) as folderTarget (`${saveTargetCollectionId}-${folderTarget.id ?? "root"}`)}
                  <button
                    class={[
                      "save-target-button",
                      folderTarget.id ? "save-target-folder" : "save-target-root",
                      saveTargetParentId === folderTarget.id && "save-target-active"
                    ]}
                    type="button"
                    role="option"
                    aria-selected={saveTargetParentId === folderTarget.id}
                    onclick={() => (saveTargetParentId = folderTarget.id)}
                    style={`--tree-depth:${folderTarget.depth};`}
                  >
                    <strong>{folderTarget.name}</strong>
                    <span>{folderTarget.id ? "Folder" : "Collection root"}</span>
                  </button>
                {/each}
              </div>
            </div>
          {/if}

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

  {#if isRequestImportDialogOpen}
    <div
      class="modal-backdrop"
      role="button"
      tabindex="0"
      aria-label="Close request import dialog"
      use:modalFocusTrap={{ onEscape: closeRequestImportDialog }}
      onclick={(e) => { if (e.target === e.currentTarget) closeRequestImportDialog(); }}
      onkeydown={(event) => {
        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault();
          closeRequestImportDialog();
        }
      }}
    >
      <div class="panel save-dialog" role="dialog" tabindex="-1" aria-modal="true" aria-labelledby="request-import-title">
        <div class="editor-header import-dialog-header">
          <h2 id="request-import-title">Import Request</h2>
          <span class="history-meta">
            {requestImportFormat === "curl" ? "cURL command" : "OpenAPI 3 JSON or YAML"}
          </span>
        </div>

        <div class="editor-block">
          <div class="import-format-toggle" role="tablist" aria-label="Choose request import format">
            <button
              class={["system-button", requestImportFormat === "curl" && "toggle-active"]}
              type="button"
              role="tab"
              aria-selected={requestImportFormat === "curl"}
              onclick={() => {
                requestImportFormat = "curl";
                requestImportErrorText = "";
              }}
            >
              cURL
            </button>
            <button
              class={["system-button", requestImportFormat === "openapi" && "toggle-active"]}
              type="button"
              role="tab"
              aria-selected={requestImportFormat === "openapi"}
              onclick={() => {
                requestImportFormat = "openapi";
                requestImportErrorText = "";
              }}
            >
              OpenAPI 3
            </button>
          </div>

          {#if requestImportFormat === "curl"}
            <label>
              <span class="field-label">Paste cURL command</span>
              <textarea
                class="text-input collections-import-source"
                bind:value={curlImportSource}
                placeholder='curl --request GET https://api.example.com/items -H "Authorization: Bearer token"'
              ></textarea>
            </label>
          {:else}
            <p class="field-help">Load an OpenAPI 3 document from JSON or YAML. Single-operation files open directly in the request editor.</p>

            <label>
              <span class="field-label">Paste source</span>
              <textarea
                class="text-input collections-import-source"
                bind:value={openApiImportSource}
                placeholder={'openapi: 3.0.3\ninfo:\n  title: Example API\npaths:\n  /items:\n    get:\n      summary: List items'}
              ></textarea>
            </label>

            <input
              bind:this={openApiImportFileInput}
              class="sr-only"
              type="file"
              accept=".json,.yaml,.yml,application/json,application/yaml,text/yaml,text/x-yaml"
              onchange={async (event: Event & { currentTarget: HTMLInputElement }) => {
                const file = event.currentTarget.files?.[0];
                if (!file) {
                  return;
                }

                openApiImportSource = await file.text();
                requestImportErrorText = "";
                event.currentTarget.value = "";
              }}
            />
          {/if}

          {#if requestImportErrorText}
            <div class="response-error">{requestImportErrorText}</div>
          {/if}

          <div class="collections-page-actions">
            {#if requestImportFormat === "openapi"}
              <button class="ghost-button" type="button" onclick={() => openApiImportFileInput?.click()}>
                Open file
              </button>
            {/if}
            <button class="send-button" type="button" onclick={handleImportRequest} disabled={isImportingRequest}>
              {isImportingRequest ? "Importing..." : "Import request"}
            </button>
            <button class="ghost-button" type="button" onclick={closeRequestImportDialog}>
              Cancel
            </button>
          </div>
        </div>
      </div>
    </div>
  {/if}
