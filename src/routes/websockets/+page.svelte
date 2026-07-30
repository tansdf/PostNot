<script lang="ts">
  import { goto } from "$app/navigation";
  import { resolve } from "$app/paths";
  import { page } from "$app/state";
  import { onMount, untrack } from "svelte";

  import {
    exportRealtimeTranscript,
    getSavedRealtimeRequest,
    readRealtimePayload,
    saveRealtimePayload
  } from "$lib/api/realtime";
  import {
    getEnvironment,
    getSettings,
    listEnvironments,
    setActiveEnvironment
  } from "$lib/api/commands";
  import {
    cloneRealtimeRequestDraft,
    createDefaultSettings,
    createRealtimeRequestDraft,
    type AppSettings,
    type EnvironmentDetail,
    type EnvironmentSummary,
    type RealtimeRequestDraft,
    type RealtimeWorkspaceTab
  } from "$lib/api/types";
  import CollectionSaveDialog from "$lib/components/collections/CollectionSaveDialog.svelte";
  import DialogShell from "$lib/components/layout/DialogShell.svelte";
  import RealtimeEditor from "$lib/components/realtime/RealtimeEditor.svelte";
  import RealtimeTabs from "$lib/components/realtime/RealtimeTabs.svelte";
  import RealtimeTranscript from "$lib/components/realtime/RealtimeTranscript.svelte";
  import { collections } from "$lib/stores/collections.svelte";
  import { notifications } from "$lib/stores/notifications.svelte";
  import { realtimeWorkspace } from "$lib/stores/realtime-workspace.svelte";
  import { readCachedJson, UI_CACHE_KEYS, writeCachedJson } from "$lib/ui-cache";

  function cachedSettings() {
    return {
      ...createDefaultSettings(),
      ...(readCachedJson<Partial<AppSettings>>(UI_CACHE_KEYS.settings) ?? {})
    };
  }

  let draft = $state(createRealtimeRequestDraft());
  let draftOwnerTabId = $state("");
  let settings = $state(cachedSettings());
  let environments = $state<EnvironmentSummary[]>(readCachedJson<EnvironmentSummary[]>(UI_CACHE_KEYS.environmentsList) ?? []);
  let activeEnvironmentId = $state(readCachedJson<string>(UI_CACHE_KEYS.environmentsActiveId) ?? "");
  let activeEnvironment: EnvironmentDetail | null = $state(null);
  let isEnvironmentChanging = $state(false);
  let environmentErrorText = $state("");
  let isSaveDialogOpen = $state(false);
  let saveDialogMode: "replace-tab" | "save-as" = $state("replace-tab");
  let saveDialogTabId = $state("");
  let saveTargetCollectionId = $state("");
  let saveTargetParentId: string | null = $state(null);
  let lastHandledRouteId = $state("");
  let pendingCloseTabId = $state("");
  let editorValid = $state(true);

  let activeTab = $derived(realtimeWorkspace.activeTab);
  let requestedSavedRequestId = $derived(page.url.searchParams.get("savedRequestId") ?? "");
  let requestedTabId = $derived(page.url.searchParams.get("tabId") ?? "");

  onMount(() => {
    void initializePage();
    const handleAgentActivity = (event: Event) => {
      const entries = (event as CustomEvent<Array<{ targetKind: string; targetId: string | null; operation: string }>>).detail ?? [];
      realtimeWorkspace.markExternallyChanged(
        entries
          .filter((entry) => ["realtime_request", "request"].includes(entry.targetKind) && entry.targetId && entry.operation.includes("update"))
          .map((entry) => entry.targetId!)
      );
    };
    window.addEventListener("postnot-agent-activity", handleAgentActivity);
    return () => window.removeEventListener("postnot-agent-activity", handleAgentActivity);
  });

  $effect(() => {
    const tab = activeTab;
    if (!tab || tab.id === draftOwnerTabId) return;
    draftOwnerTabId = tab.id;
    draft = cloneRealtimeRequestDraft(tab.draft);
  });

  $effect(() => {
    const tabId = requestedTabId;
    if (realtimeWorkspace.initialized && tabId && realtimeWorkspace.tabs.some((tab) => tab.id === tabId)) {
      realtimeWorkspace.activateTab(tabId);
    }
  });

  $effect(() => {
    const tabId = draftOwnerTabId;
    const nextDraft = cloneRealtimeRequestDraft(draft);
    const nextFingerprint = JSON.stringify(nextDraft);
    if (!tabId) return;
    untrack(() => {
      const tab = realtimeWorkspace.tabs.find((item) => item.id === tabId);
      if (tab && JSON.stringify(tab.draft) !== nextFingerprint) {
        realtimeWorkspace.updateDraft(tab.id, nextDraft);
      }
    });
  });

  $effect(() => {
    const id = requestedSavedRequestId;
    if (!realtimeWorkspace.initialized || !id || id === lastHandledRouteId) return;
    lastHandledRouteId = id;
    if (activeTab?.savedRequestId === id) return;
    void openSavedRequest(id);
  });

  async function initializePage() {
    try {
      await Promise.all([realtimeWorkspace.ensureInitialized(), collections.ensureLoaded(), loadProfile()]);
      if (requestedSavedRequestId) {
        lastHandledRouteId = requestedSavedRequestId;
        await openSavedRequest(requestedSavedRequestId);
      } else if (requestedTabId && realtimeWorkspace.tabs.some((tab) => tab.id === requestedTabId)) {
        realtimeWorkspace.activateTab(requestedTabId);
        await syncRoute();
      } else {
        await syncRoute();
      }
    } catch (error) {
      notifications.error(error instanceof Error ? error.message : String(error), "WebSocket workspace failed");
    }
  }

  async function loadProfile() {
    try {
      [settings, environments] = await Promise.all([getSettings(), listEnvironments()]);
      writeCachedJson(UI_CACHE_KEYS.settings, settings);
      writeCachedJson(UI_CACHE_KEYS.environmentsList, environments);
      const active = environments.find((environment) => environment.isActive);
      activeEnvironmentId = active?.id ?? "";
      writeCachedJson(UI_CACHE_KEYS.environmentsActiveId, activeEnvironmentId);
      activeEnvironment = activeEnvironmentId ? await getEnvironment(activeEnvironmentId) : null;
      environmentErrorText = "";
    } catch (error) {
      environmentErrorText = error instanceof Error ? error.message : String(error);
    }
  }

  async function changeEnvironment(environmentId: string) {
    isEnvironmentChanging = true;
    try {
      await setActiveEnvironment(environmentId || null);
      activeEnvironmentId = environmentId;
      activeEnvironment = environmentId ? await getEnvironment(environmentId) : null;
      environments = environments.map((environment) => ({ ...environment, isActive: environment.id === environmentId }));
      writeCachedJson(UI_CACHE_KEYS.environmentsActiveId, environmentId);
      realtimeWorkspace.markLiveTabsReconnectRequired();
    } catch (error) {
      environmentErrorText = error instanceof Error ? error.message : String(error);
    } finally {
      isEnvironmentChanging = false;
    }
  }

  async function openSavedRequest(itemId: string) {
    const existing = realtimeWorkspace.findTabBySavedRequestId(itemId);
    if (existing) {
      realtimeWorkspace.activateTab(existing.id);
      return;
    }
    try {
      realtimeWorkspace.openSavedRequest(await getSavedRealtimeRequest(itemId));
    } catch (error) {
      notifications.error(error instanceof Error ? error.message : String(error), "Connection load failed");
    }
  }

  async function activateTab(tabId: string) {
    realtimeWorkspace.activateTab(tabId);
    await syncRoute();
  }

  async function createTab() {
    realtimeWorkspace.createBlankTab();
    await syncRoute();
  }

  async function closeTab(tabId: string) {
    const tab = realtimeWorkspace.tabs.find((item) => item.id === tabId);
    if (!tab) return;
    const needsConfirmation =
      !["disconnected", "failed"].includes(tab.status) || realtimeWorkspace.isDirty(tab);
    if (needsConfirmation) {
      pendingCloseTabId = tabId;
      return;
    }
    await realtimeWorkspace.closeTab(tabId);
    await syncRoute();
  }

  async function confirmCloseTab() {
    const tabId = pendingCloseTabId;
    pendingCloseTabId = "";
    if (!tabId) return;
    await realtimeWorkspace.closeTab(tabId);
    await syncRoute();
  }

  async function syncRoute() {
    const savedRequestId = realtimeWorkspace.activeTab?.savedRequestId ?? "";
    const current = page.url.searchParams.get("savedRequestId") ?? "";
    if (savedRequestId === current) return;
    const target = savedRequestId
      ? resolve(`/websockets?savedRequestId=${encodeURIComponent(savedRequestId)}`)
      : resolve("/websockets");
    await goto(target, { replaceState: true, noScroll: true, keepFocus: true });
  }

  async function connect() {
    if (!activeTab || !editorValid) return;
    try {
      await realtimeWorkspace.connect(activeTab.id);
    } catch (error) {
      notifications.error(error instanceof Error ? error.message : String(error), "Connection failed");
    }
  }

  async function disconnect() {
    if (!activeTab) return;
    try {
      await realtimeWorkspace.disconnect(activeTab.id);
    } catch (error) {
      notifications.error(error instanceof Error ? error.message : String(error), "Disconnect failed");
    }
  }

  async function send() {
    if (!activeTab || !editorValid) return;
    try {
      const request = cloneRealtimeRequestDraft(draft);
      await realtimeWorkspace.send(activeTab.id, { requestType: request.requestType, composer: request.composer } as Parameters<typeof realtimeWorkspace.send>[1]);
    } catch (error) {
      notifications.error(error instanceof Error ? error.message : String(error), "Send failed");
    }
  }

  async function saveRequest(saveAs = false) {
    const tab = activeTab;
    if (!tab || !editorValid) return;
    collections.resetError();
    if (!saveAs && tab.savedRequestId && tab.collectionId) {
      const saved = await collections.updateExistingRealtimeRequest(
        tab.savedRequestId,
        tab.collectionId,
        cloneRealtimeRequestDraft(draft),
        tab.sourceUpdatedAt
      );
      if (saved) {
        realtimeWorkspace.setSaved(tab.id, saved, draft);
        await syncRoute();
      } else {
        realtimeWorkspace.setError(tab.id, collections.errorText);
      }
      return;
    }
    if (!collections.collections.length) {
      realtimeWorkspace.setError(tab.id, "Create a collection first from the sidebar.");
      return;
    }
    saveDialogMode = saveAs ? "save-as" : "replace-tab";
    saveDialogTabId = tab.id;
    saveTargetCollectionId = tab.collectionId || collections.selectedCollectionId || collections.collections[0].id;
    saveTargetParentId = tab.parentId ?? null;
    await collections.loadCollectionItems(saveTargetCollectionId);
    isSaveDialogOpen = true;
  }

  async function confirmSave() {
    const tab = realtimeWorkspace.tabs.find((item) => item.id === saveDialogTabId);
    if (!tab || !saveTargetCollectionId) return;
    const saved = await collections.saveNewRealtimeRequest(saveTargetCollectionId, cloneRealtimeRequestDraft(tab.draft), saveTargetParentId);
    if (!saved) {
      realtimeWorkspace.setError(tab.id, collections.errorText);
      return;
    }
    if (saveDialogMode === "replace-tab") realtimeWorkspace.setSaved(tab.id, saved, tab.draft);
    isSaveDialogOpen = false;
    if (saveDialogMode === "replace-tab") await syncRoute();
  }

  async function reloadExternal() {
    if (!activeTab?.savedRequestId) return;
    try {
      const saved = await getSavedRealtimeRequest(activeTab.savedRequestId);
      realtimeWorkspace.replaceSavedTab(activeTab.id, saved);
      draft = cloneRealtimeRequestDraft(saved.request);
      notifications.success(saved.name, "External change loaded");
    } catch (error) {
      realtimeWorkspace.setError(activeTab.id, error instanceof Error ? error.message : String(error));
    }
  }

  async function exportTranscript() {
    if (!activeTab) return;
    const result = await exportRealtimeTranscript(activeTab.id);
    if (result) notifications.success(result.filePath, "Transcript exported");
  }

  async function savePayload(handleId: string, label: string) {
    const path = await saveRealtimePayload(handleId, label);
    if (path) notifications.success(path, "Payload saved");
  }

  function handleWindowKeydown(event: KeyboardEvent) {
    if (!(event.ctrlKey || event.metaKey)) return;
    if (event.key.toLowerCase() === "s") {
      event.preventDefault();
      void saveRequest();
    } else if (event.key === "Enter") {
      event.preventDefault();
      void send();
    }
  }
</script>

<svelte:head><title>PostNot WebSockets</title></svelte:head>

<div class="workspace-grid realtime-workspace">
  <div class="profile-bar">
    <div class="profile-facts">
      <span class="profile-fact">Connect timeout <strong>{settings.realtimeConnectTimeoutMs / 1000}s</strong></span>
      <span class="profile-fact">TLS <strong>{settings.validateTls ? "Validated" : "Relaxed"}</strong></span>
      <span class="profile-fact">Sessions <strong>{settings.realtimeMaxConcurrentSessions}</strong></span>
    </div>
    <span class="profile-divider"></span>
    <div class="profile-env-section">
      <label>
        <span class="sr-only">Active environment</span>
        <select class="text-input profile-env-select" value={activeEnvironmentId} onchange={(event) => changeEnvironment(event.currentTarget.value)} disabled={isEnvironmentChanging}>
          <option value="">No environment</option>
          {#each environments as environment (environment.id)}<option value={environment.id}>{environment.name}</option>{/each}
        </select>
      </label>
      {#if activeEnvironment}<span class="profile-env-hint">{activeEnvironment.variables.length} vars</span>{/if}
    </div>
    {#if environmentErrorText}<span class="profile-env-hint realtime-error-text">{environmentErrorText}</span>{/if}
  </div>

  <RealtimeTabs
    tabs={realtimeWorkspace.tabs}
    activeTabId={realtimeWorkspace.activeTabId}
    onIsDirty={(tab) => realtimeWorkspace.isDirty(tab)}
    onActivate={activateTab}
    onClose={closeTab}
    onCreate={createTab}
  />

  {#if activeTab}
    <div
      id="realtime-connection-panel"
      class="realtime-active-panel"
      role="tabpanel"
      aria-labelledby={`realtime-tab-${activeTab.id}`}
      tabindex="0"
    >
      <RealtimeEditor
        bind:draft
        variables={activeEnvironment?.variables ?? []}
        status={activeTab.status}
        statusMessage={activeTab.statusMessage}
        reconnectRequired={activeTab.reconnectRequired}
        isSaving={collections.isSavingRequest}
        onConnect={connect}
        onDisconnect={disconnect}
        onPing={() => realtimeWorkspace.ping(activeTab.id)}
        onClose={(code, reason) => realtimeWorkspace.closeGracefully(activeTab.id, code, reason)}
        onSend={send}
        onSave={() => saveRequest()}
        onSaveAs={() => saveRequest(true)}
        onValidityChange={(valid) => (editorValid = valid)}
      />
      {#if activeTab.errorText}<div class="feedback feedback-error" role="alert">{activeTab.errorText}</div>{/if}
      {#if activeTab.externallyChanged}
        <div class="feedback feedback-warning">
          This saved connection changed through MCP. Your current draft was kept.
          <button class="button-secondary button-compact" type="button" onclick={reloadExternal}>Reload saved version</button>
        </div>
      {/if}
      <RealtimeTranscript
        entries={activeTab.transcript}
        sizeBytes={activeTab.transcriptSizeBytes}
        onClear={() => realtimeWorkspace.clearTranscript(activeTab.id)}
        onExport={exportTranscript}
        onReadPayload={readRealtimePayload}
        onSavePayload={savePayload}
      />
    </div>
  {/if}
</div>

<svelte:window onkeydown={handleWindowKeydown} />

{#if isSaveDialogOpen}
  <CollectionSaveDialog
    title={saveDialogMode === "save-as" ? "Save connection as" : "Save connection"}
    titleId="save-realtime-title"
    confirmLabel="Save connection"
    collections={collections.collections}
    folders={collections.folderTargets(saveTargetCollectionId)}
    selectedCollectionId={saveTargetCollectionId}
    selectedParentId={saveTargetParentId}
    isSaving={collections.isSavingRequest}
    onSelectCollection={async (collectionId) => {
      saveTargetCollectionId = collectionId;
      saveTargetParentId = null;
      await collections.loadCollectionItems(collectionId);
    }}
    onSelectFolder={(parentId) => (saveTargetParentId = parentId)}
    onConfirm={confirmSave}
    onDismiss={() => (isSaveDialogOpen = false)}
  />
{/if}

{#if pendingCloseTabId}
  {@const closingTab = realtimeWorkspace.tabs.find((tab) => tab.id === pendingCloseTabId)}
  <DialogShell ariaLabelledby="close-realtime-tab-title" onDismiss={() => (pendingCloseTabId = "")} sizeClass="save-dialog">
    <div class="editor-header"><h2 id="close-realtime-tab-title">Close connection tab?</h2></div>
    <div class="editor-block">
      <p>
        {#if closingTab && !["disconnected", "failed"].includes(closingTab.status)}
          Closing <strong>{closingTab.draft.name}</strong> will disconnect its active session.
        {:else}
          Close <strong>{closingTab?.draft.name ?? "this connection"}</strong>?
        {/if}
      </p>
      {#if closingTab && realtimeWorkspace.isDirty(closingTab)}
        <p class="feedback feedback-warning">Unsaved changes in this draft will be discarded.</p>
      {/if}
      <div class="collections-page-actions">
        <button class="button-danger" type="button" onclick={confirmCloseTab}>Close tab</button>
        <button class="button-secondary" type="button" onclick={() => (pendingCloseTabId = "")}>Cancel</button>
      </div>
    </div>
  </DialogShell>
{/if}
