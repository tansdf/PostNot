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
    getSavedRequest,
    getSettings,
    importCurlRequestToDraft,
    importOpenApiRequestToDraft,
    listEnvironments,
    listHistory,
    setActiveEnvironment,
    sendRequest,
    updateSettings,
    updateEnvironment
  } from "$lib/api/commands";
  import type {
    AppSettings,
    CollectionItemSummary,
    EnvironmentDetail,
    EnvironmentSummary,
    HistoryEntryDetail,
    HistoryEntrySummary,
    RequestDraft,
    RequestWorkspaceTab,
    RequestScriptExecution
  } from "$lib/api/types";
  import { cloneRequestDraft, createDefaultSettings, createRequestDraft } from "$lib/api/types";
  import HistoryPanel from "$lib/components/history/HistoryPanel.svelte";
  import RequestEditor from "$lib/components/request/RequestEditor.svelte";
  import RequestTabs from "$lib/components/request/RequestTabs.svelte";
  import ResponseViewer from "$lib/components/response/ResponseViewer.svelte";
  import { createStaleGuard } from "$lib/async-stale-guard";
  import { modalFocusTrap } from "$lib/modal-focus-trap";
  import {
    createEmptyRequestScriptExecution,
    type InheritedRequestScripts,
    runPreRequestScript,
    runTestScript
  } from "$lib/request-scripts";
  import { collections } from "$lib/stores/collections.svelte";
  import { notifications } from "$lib/stores/notifications.svelte";
  import { requestWorkspace } from "$lib/stores/request-workspace.svelte";

  let request = $state(createRequestDraft());
  let settings: AppSettings = $state(createDefaultSettings());
  let history: HistoryEntrySummary[] = $state([]);
  let isHistoryLoading = $state(true);
  let isHistoryDetailLoading = $state(false);
  let isClearingHistory = $state(false);
  let restoringHistoryId = $state("");
  let historyErrorText = $state("");
  let historyDetailErrorText = $state("");
  let settingsErrorText = $state("");
  let environments: EnvironmentSummary[] = $state([]);
  let activeEnvironmentId = $state("");
  let activeEnvironmentDetail: EnvironmentDetail | null = $state(null);
  let isEnvironmentsLoading = $state(true);
  let isEnvironmentChanging = $state(false);
  let environmentsErrorText = $state("");
  let selectedHistoryId = $state("");
  let selectedHistoryDetail: HistoryEntryDetail | null = $state(null);
  let isSaveDialogOpen = $state(false);
  let saveDialogMode = $state<"replace-tab" | "copy-new-tab">("replace-tab");
  let saveDialogTabId = $state("");
  let isRequestImportDialogOpen = $state(false);
  let requestImportFormat = $state<"curl" | "openapi">("curl");
  let curlImportSource = $state("");
  let openApiImportSource = $state("");
  let isImportingRequest = $state(false);
  let isHistoryCollapseSaving = $state(false);
  let requestImportErrorText = $state("");
  let openApiImportFileInput: HTMLInputElement | null = $state(null);
  let saveTargetCollectionId = $state("");
  let saveTargetParentId: string | null = $state(null);
  let requestTabsScrollRequest = $state({ n: 0, tabId: "" });
  let requestedSavedRequestId = $derived(page.url.searchParams.get("savedRequestId") ?? "");
  let activeTab = $derived(
    requestWorkspace.tabs.find((tab) => tab.id === requestWorkspace.activeTabId) ?? null
  );
  let activeTabResponse = $derived(activeTab?.response ?? null);
  let activeTabScriptExecution = $derived(activeTab?.scriptExecution ?? null);
  let activeTabErrorText = $derived(activeTab?.errorText ?? "");
  let activeTabIsSending = $derived(activeTab?.id === requestWorkspace.inFlightTabId);
  let activeTabSendLocked = $derived(
    Boolean(requestWorkspace.inFlightTabId && requestWorkspace.inFlightTabId !== activeTab?.id)
  );
  let isSyncingRequestFromWorkspace = false;
  let requestOwnerTabId = "";
  let lastSyncedCollectionId = "";
  let lastHandledRequestedSavedRequestId = "";

  const savedRequestRoute = createStaleGuard();

  const openApiRequestImportPlaceholder = `openapi: 3.0.3
info:
  title: Example API
paths:
  /items:
    get:
      summary: List items`;

  onMount(() => {
    void initializePage();
  });

  $effect(() => {
    const nextActiveTab = activeTab;
    if (!nextActiveTab) {
      return;
    }

    if (nextActiveTab.id === requestOwnerTabId) {
      return;
    }

    if (requestOwnerTabId) {
      const previousTab = getTabById(requestOwnerTabId);
      if (previousTab && !requestEquals(previousTab.request, request)) {
        requestWorkspace.updateTabRequest(requestOwnerTabId, request);
      }
    }

    isSyncingRequestFromWorkspace = true;
    requestOwnerTabId = nextActiveTab.id;
    request = cloneRequestDraft(nextActiveTab.request);
  });

  $effect(() => {
    const nextActiveTab = activeTab;
    if (!nextActiveTab) {
      return;
    }

    if (isSyncingRequestFromWorkspace) {
      isSyncingRequestFromWorkspace = false;
      return;
    }

    if (nextActiveTab.id !== requestOwnerTabId) {
      return;
    }

    if (!requestEquals(nextActiveTab.request, request)) {
      requestWorkspace.updateTabRequest(requestOwnerTabId, request);
    }
  });

  $effect(() => {
    const requestedId = requestedSavedRequestId;
    if (!requestWorkspace.initialized) {
      return;
    }

    if (requestedId === lastHandledRequestedSavedRequestId) {
      return;
    }

    lastHandledRequestedSavedRequestId = requestedId;

    if (!requestedId) {
      return;
    }

    if (activeTab?.savedRequestId === requestedId) {
      if (activeTab) {
        bumpRequestTabsScrollIntoView(activeTab.id);
      }
      return;
    }

    void openSavedRequestFromRoute(requestedId);
  });

  $effect(() => {
    const collectionId = activeTab?.collectionId ?? "";
    if (!collectionId) {
      lastSyncedCollectionId = "";
      return;
    }

    if (collectionId === lastSyncedCollectionId) {
      return;
    }

    lastSyncedCollectionId = collectionId;
    void syncActiveCollection(collectionId);
  });

  async function initializePage() {
    await Promise.all([loadSettings(), loadHistory(), collections.ensureLoaded(), loadEnvironments(), requestWorkspace.ensureInitialized()]);

    if (requestedSavedRequestId) {
      await openSavedRequestFromRoute(requestedSavedRequestId);
      return;
    }

    await syncRouteToActiveTab();
  }

  function requestEquals(left: RequestDraft, right: RequestDraft) {
    return JSON.stringify(left) === JSON.stringify(right);
  }

  function getTabById(tabId: string) {
    return requestWorkspace.tabs.find((tab) => tab.id === tabId) ?? null;
  }

  async function syncRouteToActiveTab() {
    const savedRequestId = activeTab?.savedRequestId ?? "";
    const currentSavedRequestId = page.url.searchParams.get("savedRequestId") ?? "";

    if (savedRequestId === currentSavedRequestId) {
      return;
    }

    const gotoOpts = {
      replaceState: true,
      noScroll: true,
      keepFocus: true
    } as const;

    if (savedRequestId) {
      await goto(resolve(`/?savedRequestId=${encodeURIComponent(savedRequestId)}`), gotoOpts);
    } else {
      await goto(resolve("/"), gotoOpts);
    }
  }

  async function syncActiveCollection(collectionId: string) {
    await collections.ensureLoaded(collectionId);
    await collections.selectCollection(collectionId);
  }

  async function loadSettings() {
    try {
      settings = await getSettings();
      settingsErrorText = "";
    } catch (error) {
      settingsErrorText = error instanceof Error ? error.message : String(error);
    }
  }

  function isPrimarySaveShortcut(event: KeyboardEvent) {
    return (event.ctrlKey || event.metaKey) && !event.shiftKey && !event.altKey && event.key.toLowerCase() === "s";
  }

  async function handleHistoryCollapsedChange(isCollapsed: boolean) {
    if (isHistoryCollapseSaving || settings.isHistoryCollapsed === isCollapsed) {
      return;
    }

    const previousSettings = settings;
    settings = {
      ...settings,
      isHistoryCollapsed: isCollapsed
    };
    isHistoryCollapseSaving = true;

    try {
      settings = await updateSettings(settings);
      settingsErrorText = "";
    } catch (error) {
      settings = previousSettings;
      settingsErrorText = error instanceof Error ? error.message : String(error);
      notifications.error(settingsErrorText, "History preference not saved");
    } finally {
      isHistoryCollapseSaving = false;
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

  async function handleRestoreHistoryEntry(id: string) {
    if (!requestWorkspace.initialized || restoringHistoryId) {
      return;
    }

    restoringHistoryId = id;

    try {
      const detail =
        selectedHistoryDetail?.id === id ? selectedHistoryDetail : await getHistoryEntry(id);
      const openedTab = requestWorkspace.openHistoryRequest(detail.requestSnapshot);
      bumpRequestTabsScrollIntoView(openedTab.id);
      await syncRouteToActiveTab();

      const restoredLabel = detail.requestSnapshot.name.trim() || detail.url;
      notifications.success(`${restoredLabel} is now open in a new request tab.`, "Request restored");
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      notifications.error(message, "Restore failed");
    } finally {
      restoringHistoryId = "";
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

  function activeCollectionScripts(tab: RequestWorkspaceTab): InheritedRequestScripts | null {
    if (!tab.collectionId) {
      return null;
    }

    const collection = collections.collections.find((item) => item.id === tab.collectionId);
    if (!collection) {
      return null;
    }

    return {
      preRequestScript: collection.preRequestScript,
      testScript: collection.testScript,
      folderScripts: folderScriptPath(
        collections.collectionItemsByCollection[tab.collectionId] ?? [],
        tab.parentId ?? null
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

  function bumpRequestTabsScrollIntoView(tabId?: string) {
    const id = tabId ?? requestWorkspace.activeTabId;
    requestTabsScrollRequest = {
      n: requestTabsScrollRequest.n + 1,
      tabId: id
    };
  }

  async function openSavedRequestFromRoute(itemId: string) {
    const existingTab = requestWorkspace.findTabBySavedRequestId(itemId);
    if (existingTab) {
      if (existingTab.id !== requestWorkspace.activeTabId) {
        requestWorkspace.activateTab(existingTab.id);
      }
      bumpRequestTabsScrollIntoView(existingTab.id);
      return;
    }

    const seq = savedRequestRoute.next();

    try {
      const savedRequest = await getSavedRequest(itemId);
      if (savedRequestRoute.isStale(seq)) {
        return;
      }

      const openedTab = requestWorkspace.openSavedRequest(savedRequest);
      bumpRequestTabsScrollIntoView(openedTab.id);
    } catch (error) {
      if (savedRequestRoute.isStale(seq)) {
        return;
      }

      const message = error instanceof Error ? error.message : String(error);
      if (activeTab) {
        requestWorkspace.setTabError(activeTab.id, message);
      } else {
        notifications.error(message, "Request load failed");
      }
    }
  }

  async function handleSend() {
    if (!requestWorkspace.initialized) {
      return;
    }

    const tab = activeTab ?? getTabById(requestOwnerTabId);
    if (!tab || requestWorkspace.inFlightTabId) {
      return;
    }

    const tabId = tab.id;
    const requestToSend = cloneRequestDraft(request);
    requestWorkspace.clearTabError(tabId);
    requestWorkspace.markSendStarted(tabId);

    try {
      const inheritedScripts = activeCollectionScripts(tab);
      const preparedRequest = await runPreRequestScript(
        requestToSend,
        activeEnvironmentDetail?.variables ?? [],
        inheritedScripts,
        {
          activeEnvironment: activeEnvironmentDetail,
          persistActiveEnvironment: persistActiveEnvironmentFromScript
        }
      );

      if (preparedRequest.errorText) {
        const execution = {
          ...createEmptyRequestScriptExecution(),
          preRequestErrorText: preparedRequest.errorText
        };
        requestWorkspace.setTabResponse(tabId, {
          statusCode: null,
          statusText: "Pre-request script failed",
          durationMs: 0,
          sizeBytes: 0,
          headers: [],
          bodyText: "",
          errorText: "",
          executedAt: new Date().toISOString()
        }, execution);
        return;
      }

      const sendResult = await sendRequest(preparedRequest.request);
      const scriptExecution = await runTestScript(
        requestToSend,
        sendResult.response,
        activeEnvironmentDetail?.variables ?? [],
        inheritedScripts,
        {
          activeEnvironment: activeEnvironmentDetail,
          persistActiveEnvironment: persistActiveEnvironmentFromScript
        }
      );

      requestWorkspace.setTabResponse(tabId, sendResult.response, scriptExecution);

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

      requestWorkspace.setTabResponse(tabId, {
        statusCode: null,
        statusText: errorText === "Request canceled." ? "Request canceled" : "Request failed",
        durationMs: 0,
        sizeBytes: 0,
        headers: [],
        bodyText: "",
        errorText,
        executedAt: new Date().toISOString()
      });
    } finally {
      requestWorkspace.markSendFinished(tabId);
      await loadHistory();

      if (selectedHistoryId) {
        await inspectHistoryEntry(selectedHistoryId, true);
      }
    }
  }

  async function handleCancelRequest() {
    if (!requestWorkspace.inFlightTabId || requestWorkspace.isCanceling || requestWorkspace.inFlightTabId !== activeTab?.id) {
      return;
    }

    requestWorkspace.markCanceling();

    try {
      await cancelActiveRequest();
    } catch {
      requestWorkspace.isCanceling = false;
    }
  }

  async function handleSaveRequest() {
    if (!requestWorkspace.initialized) {
      return;
    }

    const tab = activeTab ?? getTabById(requestOwnerTabId);
    if (!tab) {
      return;
    }

    const requestToSave = cloneRequestDraft(request);
    requestWorkspace.clearTabError(tab.id);
    collections.resetError();

    const hasSavedRequest =
      !!tab.savedRequestId &&
      !!tab.collectionId &&
      collections.collections.some((collection) => collection.id === tab.collectionId);

    if (hasSavedRequest) {
      const savedRequest = await collections.updateExistingSavedRequest(tab.savedRequestId!, tab.collectionId!, requestToSave);

      if (!savedRequest) {
        requestWorkspace.setTabError(tab.id, collections.errorText);
        return;
      }

      requestWorkspace.setTabSaved(tab.id, savedRequest, requestToSave);
      await syncRouteToActiveTab();
      return;
    }

    if (collections.collections.length === 0) {
      requestWorkspace.setTabError(tab.id, "Create a collection first from the sidebar.");
      return;
    }

    saveDialogMode = "replace-tab";
    saveDialogTabId = tab.id;
    saveTargetCollectionId = collections.selectedCollectionId || tab.collectionId || collections.collections[0].id;
    saveTargetParentId = tab.parentId ?? null;
    isSaveDialogOpen = true;
  }

  async function handleSaveAsNewRequest() {
    if (!requestWorkspace.initialized) {
      return;
    }

    const tab = activeTab ?? getTabById(requestOwnerTabId);
    if (!tab) {
      return;
    }

    requestWorkspace.clearTabError(tab.id);
    collections.resetError();

    if (collections.collections.length === 0) {
      requestWorkspace.setTabError(tab.id, "Create a collection first from the sidebar.");
      return;
    }

    saveDialogMode = "copy-new-tab";
    saveDialogTabId = tab.id;
    saveTargetCollectionId = collections.selectedCollectionId || tab.collectionId || collections.collections[0].id;
    saveTargetParentId = tab.parentId ?? null;
    isSaveDialogOpen = true;
  }

  async function confirmSaveRequest() {
    if (!saveTargetCollectionId) {
      const saveTab = getTabById(saveDialogTabId);
      if (saveTab) {
        requestWorkspace.setTabError(saveTab.id, "Choose a collection first.");
      }
      return;
    }

    const saveTab = getTabById(saveDialogTabId);
    if (!saveTab) {
      closeSaveDialog();
      return;
    }

    const draftToSave =
      saveDialogTabId === requestOwnerTabId ? cloneRequestDraft(request) : cloneRequestDraft(saveTab.request);

    if (saveDialogMode === "copy-new-tab") {
      const savedSummary = await collections.saveNewRequest(saveTargetCollectionId, draftToSave, saveTargetParentId);

      if (!savedSummary) {
        requestWorkspace.setTabError(saveTab.id, collections.errorText);
        return;
      }

      try {
        const detail = await getSavedRequest(savedSummary.id);
        const openedTab = requestWorkspace.openSavedRequest(detail);
        bumpRequestTabsScrollIntoView(openedTab.id);
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        requestWorkspace.setTabError(saveTab.id, message);
        return;
      }

      isSaveDialogOpen = false;
      saveDialogMode = "replace-tab";
      saveDialogTabId = "";
      saveTargetParentId = null;
      await collections.selectCollection(savedSummary.collectionId);
      await syncRouteToActiveTab();
      return;
    }

    const savedRequest = await collections.saveNewRequest(saveTargetCollectionId, draftToSave, saveTargetParentId);

    if (!savedRequest) {
      requestWorkspace.setTabError(saveTab.id, collections.errorText);
      return;
    }

    requestWorkspace.setTabSaved(saveTab.id, savedRequest, draftToSave);
    isSaveDialogOpen = false;
    saveDialogMode = "replace-tab";
    saveDialogTabId = "";
    await collections.selectCollection(savedRequest.collectionId);
    await syncRouteToActiveTab();
  }

  function closeSaveDialog() {
    isSaveDialogOpen = false;
    saveDialogMode = "replace-tab";
    saveDialogTabId = "";
    saveTargetParentId = null;
  }

  async function handleNewRequest() {
    requestWorkspace.createBlankTab();
    await syncRouteToActiveTab();
  }

  async function handleActivateTab(tabId: string) {
    if (tabId === requestWorkspace.activeTabId) {
      return;
    }

    requestWorkspace.activateTab(tabId);
    await syncRouteToActiveTab();
  }

  async function handleCloseTab(tabId: string) {
    const tab = getTabById(tabId);
    if (!tab) {
      return;
    }

    if (requestWorkspace.inFlightTabId === tabId) {
      notifications.info("Cancel the in-flight request before closing this tab.", "Request still running");
      return;
    }

    if (tabId === requestWorkspace.activeTabId && tabId === requestOwnerTabId) {
      const stored = getTabById(tabId);
      if (stored && !requestEquals(stored.request, request)) {
        requestWorkspace.updateTabRequest(tabId, request);
      }
    }

    const tabForClose = getTabById(tabId) ?? tab;

    if (requestWorkspace.isDirty(tabForClose) && !window.confirm("Close this tab and discard unsaved changes?")) {
      return;
    }

    if (saveDialogTabId === tabId) {
      closeSaveDialog();
    }

    requestWorkspace.closeTab(tabId);
    await syncRouteToActiveTab();
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

      requestWorkspace.openImportedTab(imported.request);
      closeRequestImportDialog();
      await syncRouteToActiveTab();
      notifications.success(
        requestImportFormat === "curl"
          ? "The imported cURL command is now loaded into a new request tab."
          : "The imported OpenAPI request is now loaded into a new request tab.",
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

  function handleWindowKeydown(event: KeyboardEvent) {
    if (!isPrimarySaveShortcut(event)) {
      return;
    }

    if (isRequestImportDialogOpen) {
      event.preventDefault();
      return;
    }

    event.preventDefault();

    if (isSaveDialogOpen) {
      void confirmSaveRequest();
      return;
    }

    void handleSaveRequest();
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

  <RequestTabs
    tabs={requestWorkspace.tabs}
    activeTabId={requestWorkspace.activeTabId}
    inFlightTabId={requestWorkspace.inFlightTabId}
    scrollRequest={requestTabsScrollRequest}
    onIsDirty={(tab) => requestWorkspace.isDirty(tab)}
    onActivate={handleActivateTab}
    onClose={handleCloseTab}
    onCreate={handleNewRequest}
  />

  <RequestEditor
    bind:request
    environmentVariables={activeEnvironmentDetail?.variables ?? []}
    isSending={activeTabIsSending}
    isCanceling={requestWorkspace.isCanceling}
    isSaving={collections.isSavingRequest}
    saveLabel={activeTab?.savedRequestId ? "Update" : "Save"}
    saveDisabled={!requestWorkspace.initialized || activeTabIsSending}
    sendDisabled={!requestWorkspace.initialized || activeTabSendLocked}
    handleNewRequest={handleNewRequest}
    handleOpenImport={openRequestImportDialog}
    handleSendRequest={handleSend}
    handleCancelRequest={handleCancelRequest}
    handleSaveRequest={handleSaveRequest}
    showSaveMenu={requestWorkspace.initialized && collections.collections.length > 0}
    handleSaveAsRequest={handleSaveAsNewRequest}
  />

  {#if activeTabErrorText}
    <div class="response-error">{activeTabErrorText}</div>
  {/if}

  <ResponseViewer response={activeTabResponse} scriptExecution={activeTabScriptExecution} />

  <HistoryPanel
    items={history}
    isLoading={isHistoryLoading}
    errorText={historyErrorText}
    isCollapsed={settings.isHistoryCollapsed}
    selectedId={selectedHistoryId}
    detail={selectedHistoryDetail}
    detailErrorText={historyDetailErrorText}
    isDetailLoading={isHistoryDetailLoading}
    isClearing={isClearingHistory}
    restoringId={restoringHistoryId}
    onToggleCollapse={handleHistoryCollapsedChange}
    onInspect={inspectHistoryEntry}
    onRestore={handleRestoreHistoryEntry}
    onClear={handleClearHistory}
    onCloseDetail={closeHistoryDetail}
  />
</div>

<svelte:window onkeydown={handleWindowKeydown} />

{#if isSaveDialogOpen}
  <div
    class="modal-backdrop"
    role="button"
    tabindex="0"
    aria-label="Close save request dialog"
    use:modalFocusTrap={{ onEscape: closeSaveDialog }}
    onclick={(event) => {
      if (event.target === event.currentTarget) {
        closeSaveDialog();
      }
    }}
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
        <h2 id="save-request-title">{saveDialogMode === "copy-new-tab" ? "Save copy" : "Save request"}</h2>
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
            {collections.isSavingRequest ? "Saving..." : saveDialogMode === "copy-new-tab" ? "Save copy" : "Save request"}
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
    onclick={(event) => {
      if (event.target === event.currentTarget) {
        closeRequestImportDialog();
      }
    }}
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
          <p class="field-help">Load an OpenAPI 3 document from JSON or YAML. Single-operation files open directly in a new request tab.</p>

          <label>
            <span class="field-label">Paste source</span>
            <textarea
              class="text-input collections-import-source"
              bind:value={openApiImportSource}
              placeholder={openApiRequestImportPlaceholder}
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
