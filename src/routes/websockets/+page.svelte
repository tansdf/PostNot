<script lang="ts">
  import { goto } from "$app/navigation";
  import { resolve } from "$app/paths";
  import { page } from "$app/state";
  import { onMount, tick, untrack } from "svelte";
  import {
    createRealtimeConnectionProfile, deleteRealtimeConnectionProfile, exportRealtimeConnectionProfiles, exportRealtimeTranscript,
    getRealtimeConnectionProfile, getSavedRealtimeMessage, listRealtimeConnectionProfiles,
    importRealtimeConnectionProfiles, readRealtimePayload, saveRealtimePayload, updateRealtimeConnectionProfile
  } from "$lib/api/realtime";
  import { getEnvironment, getSettings, listEnvironments, setActiveEnvironment } from "$lib/api/commands";
  import {
    cloneRealtimeConnectionDraft, cloneRealtimeMessageDraft, createDefaultSettings,
    type AppSettings, type EnvironmentDetail, type EnvironmentSummary, type RealtimeConnectionDraft,
    type RealtimeConnectionProfileSummary, type RealtimeMessageDraft
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

  const cachedSettings = () => ({ ...createDefaultSettings(), ...(readCachedJson<Partial<AppSettings>>(UI_CACHE_KEYS.settings) ?? {}) });
  let connection = $state<RealtimeConnectionDraft>(cloneRealtimeConnectionDraft(realtimeWorkspace.activeTab?.connectionDraft!));
  let message = $state<RealtimeMessageDraft>(cloneRealtimeMessageDraft(realtimeWorkspace.activeTab?.messageDraft!));
  let draftOwnerTabId = $state("");
  let profiles = $state<RealtimeConnectionProfileSummary[]>([]);
  let settings = $state(cachedSettings());
  let environments = $state<EnvironmentSummary[]>(readCachedJson(UI_CACHE_KEYS.environmentsList) ?? []);
  let activeEnvironmentId = $state(readCachedJson<string>(UI_CACHE_KEYS.environmentsActiveId) ?? "");
  let activeEnvironment: EnvironmentDetail | null = $state(null);
  let environmentErrorText = $state("");
  let isEnvironmentChanging = $state(false);
  let isSaveDialogOpen = $state(false);
  let saveAsMessage = $state(false);
  let saveDialogTabId = $state("");
  let saveTargetCollectionId = $state("");
  let saveTargetParentId: string | null = $state(null);
  let pendingCloseTabId = $state("");
  let pendingMessageId = $state("");
  let isNewMessagePending = $state(false);
  let editorValid = $state(true);
  let lastHandledMessageId = $state("");
  let routeReady = $state(false);
  let activeTab = $derived(realtimeWorkspace.activeTab);
  let requestedMessageId = $derived(page.url.searchParams.get("messageId") ?? page.url.searchParams.get("savedRequestId") ?? "");
  let requestedProfileId = $derived(page.url.searchParams.get("profileId") ?? "");
  let requestedTabId = $derived(page.url.searchParams.get("tabId") ?? "");

  onMount(() => {
    void initialize();
    const activity = (event: Event) => {
      const entries = (event as CustomEvent<Array<{ targetKind: string; targetId: string | null; operation: string }>>).detail ?? [];
      realtimeWorkspace.markExternallyChanged(entries.filter((e) => e.targetKind === "realtime_message" && e.targetId).map((e) => e.targetId!));
      realtimeWorkspace.markProfilesExternallyChanged(entries.filter((e) => e.targetKind === "realtime_connection" && e.targetId).map((e) => e.targetId!));
      if (entries.some((entry) => entry.targetKind === "realtime_connection")) void refreshProfiles();
    };
    window.addEventListener("postnot-agent-activity", activity);
    return () => window.removeEventListener("postnot-agent-activity", activity);
  });

  $effect(() => {
    const tab = activeTab;
    if (!tab || tab.id === draftOwnerTabId) return;
    draftOwnerTabId = tab.id;
    connection = cloneRealtimeConnectionDraft(tab.connectionDraft);
    message = cloneRealtimeMessageDraft(tab.messageDraft);
  });
  $effect(() => {
    const tabId = draftOwnerTabId;
    const nextConnection = cloneRealtimeConnectionDraft(connection);
    const nextMessage = cloneRealtimeMessageDraft(message);
    if (!tabId) return;
    untrack(() => {
      const tab = realtimeWorkspace.tabs.find((item) => item.id === tabId);
      if (!tab) return;
      if (JSON.stringify(tab.connectionDraft) !== JSON.stringify(nextConnection)) realtimeWorkspace.updateConnectionDraft(tabId, nextConnection);
      if (JSON.stringify(tab.messageDraft) !== JSON.stringify(nextMessage)) realtimeWorkspace.updateMessageDraft(tabId, nextMessage);
    });
  });
  $effect(() => { if (realtimeWorkspace.initialized && requestedTabId && realtimeWorkspace.tabs.some((tab) => tab.id === requestedTabId)) realtimeWorkspace.activateTab(requestedTabId); });
  $effect(() => { if (routeReady && requestedMessageId && requestedMessageId !== lastHandledMessageId) void handleRequestedMessage(requestedMessageId); });

  async function initialize() {
    try {
      await Promise.all([realtimeWorkspace.ensureInitialized(), collections.ensureLoaded(), loadEnvironment(), refreshProfiles()]);
      if (requestedTabId) realtimeWorkspace.activateTab(requestedTabId);
      if (requestedProfileId && ["disconnected", "failed"].includes(realtimeWorkspace.activeTab?.status ?? "")) await selectProfile(requestedProfileId, false);
      if (requestedMessageId) await handleRequestedMessage(requestedMessageId);
      routeReady = true;
      if (!pendingMessageId) await syncRoute();
    } catch (error) { notifications.error(String(error), "Realtime workspace failed"); }
  }
  async function loadEnvironment() {
    [settings, environments] = await Promise.all([getSettings(), listEnvironments()]);
    writeCachedJson(UI_CACHE_KEYS.settings, settings); writeCachedJson(UI_CACHE_KEYS.environmentsList, environments);
    activeEnvironmentId = environments.find((item) => item.isActive)?.id ?? "";
    activeEnvironment = activeEnvironmentId ? await getEnvironment(activeEnvironmentId) : null;
  }
  async function refreshProfiles() { profiles = await listRealtimeConnectionProfiles(); }
  async function changeEnvironment(id: string) { isEnvironmentChanging = true; try { await setActiveEnvironment(id || null); activeEnvironmentId = id; activeEnvironment = id ? await getEnvironment(id) : null; environments = environments.map((item) => ({ ...item, isActive: item.id === id })); realtimeWorkspace.markLiveTabsReconnectRequired(); } catch (error) { environmentErrorText = String(error); } finally { isEnvironmentChanging = false; } }

  async function waitForWorkspacePaint() {
    await tick();
    await new Promise<void>((resolvePaint) => requestAnimationFrame(() => requestAnimationFrame(() => resolvePaint())));
  }
  async function handleRequestedMessage(id: string) {
    if (!id || id === lastHandledMessageId) return;
    lastHandledMessageId = id;
    const tab = realtimeWorkspace.activeTab;
    if (tab?.selectedMessageId === id) return;
    await waitForWorkspacePaint();
    if (tab && realtimeWorkspace.isMessageDirty(tab)) {
      pendingMessageId = id;
      return;
    }
    await loadMessage(id);
  }
  async function loadMessage(id: string) {
    try {
      const saved = await getSavedRealtimeMessage(id);
      const loaded = realtimeWorkspace.openSavedMessage(saved);
      draftOwnerTabId = loaded.id;
      message = cloneRealtimeMessageDraft(saved.message);
      await syncRoute();
    } catch (error) {
      notifications.error(String(error), "Message load failed");
      await syncRoute();
    }
  }
  async function confirmPendingMessage() {
    const id = pendingMessageId;
    pendingMessageId = "";
    if (id) await loadMessage(id);
  }
  async function cancelPendingMessage() {
    pendingMessageId = "";
    await syncRoute();
  }
  async function selectProfile(id: string, shouldSyncRoute = true) {
    if (!id || !activeTab || !["disconnected", "failed"].includes(activeTab.status)) return;
    if (realtimeWorkspace.isConnectionDirty(activeTab) && !window.confirm("Discard unsaved connection edits and select this profile?")) return;
    const profile = await getRealtimeConnectionProfile(id); realtimeWorkspace.selectProfile(activeTab.id, profile); connection = cloneRealtimeConnectionDraft(profile.connection); if (shouldSyncRoute) await syncRoute();
  }
  function newProfile() { if (!activeTab || !["disconnected", "failed"].includes(activeTab.status)) return; if (realtimeWorkspace.isConnectionDirty(activeTab) && !window.confirm("Discard unsaved connection edits?")) return; realtimeWorkspace.newConnection(activeTab.id, connection.protocol); connection = cloneRealtimeConnectionDraft(realtimeWorkspace.activeTab!.connectionDraft); }
  async function saveProfile(saveAs = false) {
    if (!activeTab) return;
    try {
      const saved = !saveAs && activeTab.selectedProfileId
        ? await updateRealtimeConnectionProfile(activeTab.selectedProfileId, cloneRealtimeConnectionDraft(connection), activeTab.profileUpdatedAt)
        : await createRealtimeConnectionProfile(cloneRealtimeConnectionDraft(connection));
      realtimeWorkspace.setProfileSaved(activeTab.id, saved, connection); await refreshProfiles(); await syncRoute(); notifications.success(saved.name, "Connection profile saved");
    } catch (error) { realtimeWorkspace.setError(activeTab.id, String(error)); }
  }
  async function deleteProfile() { if (!activeTab?.selectedProfileId || !window.confirm("Delete this connection profile? Messages will not be affected.")) return; const id = activeTab.selectedProfileId; await deleteRealtimeConnectionProfile(id, activeTab.profileUpdatedAt); realtimeWorkspace.unlinkProfile(id); await refreshProfiles(); await syncRoute(); }
  async function importProfiles() { const imported = await importRealtimeConnectionProfiles(); await refreshProfiles(); if (imported.length) notifications.success(`${imported.length} profile${imported.length === 1 ? "" : "s"} imported`, "Connections imported"); }
  async function exportProfile() { if (!activeTab?.selectedProfileId) return; const includeSensitive = window.confirm("Include literal credentials in this export? Choose Cancel for a redacted export."); const result = await exportRealtimeConnectionProfiles([activeTab.selectedProfileId], includeSensitive); if (result) notifications.success(result.filePath, "Connection profile exported"); }

  async function syncRoute() {
    const tab = realtimeWorkspace.activeTab; if (!tab) return;
    const params = new URLSearchParams(); params.set("tabId", tab.id); if (tab.selectedMessageId) params.set("messageId", tab.selectedMessageId); if (tab.selectedProfileId) params.set("profileId", tab.selectedProfileId);
    const target = resolve(`/websockets?${params}`); if (`${page.url.pathname}${page.url.search}` !== target) await goto(target, { replaceState: true, noScroll: true, keepFocus: true });
  }
  async function activateTab(id: string) { realtimeWorkspace.activateTab(id); await syncRoute(); }
  async function createTab() { realtimeWorkspace.createBlankTab(); await syncRoute(); }
  async function closeTab(id: string) { const tab = realtimeWorkspace.tabs.find((item) => item.id === id); if (!tab) return; if (!["disconnected", "failed"].includes(tab.status) || realtimeWorkspace.isDirty(tab)) { pendingCloseTabId = id; return; } await realtimeWorkspace.closeTab(id); await syncRoute(); }
  async function confirmCloseTab() { const id = pendingCloseTabId; pendingCloseTabId = ""; if (id) { await realtimeWorkspace.closeTab(id); await syncRoute(); } }
  async function connect() { if (activeTab && editorValid) try { await realtimeWorkspace.connect(activeTab.id); } catch (error) { notifications.error(String(error), "Connection failed"); } }
  async function disconnect() { if (activeTab) await realtimeWorkspace.disconnect(activeTab.id); }
  async function send() { if (activeTab && editorValid) try { await realtimeWorkspace.send(activeTab.id, cloneRealtimeMessageDraft(message)); } catch (error) { notifications.error(String(error), "Send failed"); } }
  async function saveMessage(saveAs = false) {
    if (!activeTab || !editorValid) return;
    if (!saveAs && activeTab.selectedMessageId && activeTab.collectionId) {
      const saved = await collections.updateExistingRealtimeMessage(activeTab.selectedMessageId, activeTab.collectionId, cloneRealtimeMessageDraft(message), activeTab.sourceUpdatedAt);
      if (saved) { realtimeWorkspace.setMessageSaved(activeTab.id, saved, message); await syncRoute(); } return;
    }
    if (!collections.collections.length) { realtimeWorkspace.setError(activeTab.id, "Create a collection first."); return; }
    saveAsMessage = saveAs; saveDialogTabId = activeTab.id; saveTargetCollectionId = activeTab.collectionId || collections.selectedCollectionId || collections.collections[0].id; saveTargetParentId = activeTab.parentId ?? null; await collections.loadCollectionItems(saveTargetCollectionId); isSaveDialogOpen = true;
  }
  async function newMessage() {
    if (!activeTab) return;
    if (realtimeWorkspace.isMessageDirty(activeTab)) {
      isNewMessagePending = true;
      return;
    }
    await replaceWithNewMessage();
  }
  async function replaceWithNewMessage() {
    const tab = realtimeWorkspace.activeTab;
    isNewMessagePending = false;
    if (!tab) return;
    const protocol = message.protocol;
    realtimeWorkspace.newMessage(tab.id, protocol);
    draftOwnerTabId = tab.id;
    message = cloneRealtimeMessageDraft(realtimeWorkspace.activeTab!.messageDraft);
    await syncRoute();
    lastHandledMessageId = "";
  }
  async function confirmSave() { const tab = realtimeWorkspace.tabs.find((item) => item.id === saveDialogTabId); if (!tab) return; const saved = await collections.saveNewRealtimeMessage(saveTargetCollectionId, cloneRealtimeMessageDraft(tab.messageDraft), saveTargetParentId); if (!saved) return; if (!saveAsMessage) realtimeWorkspace.setMessageSaved(tab.id, saved, tab.messageDraft); isSaveDialogOpen = false; await syncRoute(); }
  async function reloadMessage() { if (!activeTab?.selectedMessageId) return; const saved = await getSavedRealtimeMessage(activeTab.selectedMessageId); realtimeWorkspace.replaceSavedMessage(activeTab.id, saved); message = cloneRealtimeMessageDraft(saved.message); }
  async function reloadProfile() { if (activeTab?.selectedProfileId && ["disconnected", "failed"].includes(activeTab.status)) await selectProfile(activeTab.selectedProfileId); }
  async function exportTranscript() { if (!activeTab) return; const result = await exportRealtimeTranscript(activeTab.id); if (result) notifications.success(result.filePath, "Transcript exported"); }
  async function savePayload(handleId: string, label: string) { const path = await saveRealtimePayload(handleId, label); if (path) notifications.success(path, "Payload saved"); }
  function handleWindowKeydown(event: KeyboardEvent) { if (!(event.ctrlKey || event.metaKey)) return; if (event.key.toLowerCase() === "s") { event.preventDefault(); void saveMessage(); } else if (event.key === "Enter") { event.preventDefault(); void send(); } }
</script>

<svelte:head><title>PostNot WebSockets</title></svelte:head>
<div class="workspace-grid realtime-workspace">
  <div class="profile-bar"><div class="profile-facts"><span class="profile-fact">Connect timeout <strong>{settings.realtimeConnectTimeoutMs / 1000}s</strong></span><span class="profile-fact">TLS <strong>{settings.validateTls ? "Validated" : "Relaxed"}</strong></span><span class="profile-fact">Sessions <strong>{settings.realtimeMaxConcurrentSessions}</strong></span></div><span class="profile-divider"></span><div class="profile-env-section"><label><span class="sr-only">Active environment</span><select class="text-input profile-env-select" value={activeEnvironmentId} onchange={(event) => changeEnvironment(event.currentTarget.value)} disabled={isEnvironmentChanging}><option value="">No environment</option>{#each environments as environment (environment.id)}<option value={environment.id}>{environment.name}</option>{/each}</select></label>{#if activeEnvironment}<span class="profile-env-hint">{activeEnvironment.variables.length} vars</span>{/if}</div>{#if environmentErrorText}<span class="profile-env-hint realtime-error-text">{environmentErrorText}</span>{/if}</div>
  <RealtimeTabs tabs={realtimeWorkspace.tabs} activeTabId={realtimeWorkspace.activeTabId} onIsDirty={(tab) => realtimeWorkspace.isDirty(tab)} onActivate={activateTab} onClose={closeTab} onCreate={createTab} />
  {#if activeTab}
    <div id="realtime-connection-panel" class="realtime-active-panel" role="tabpanel" aria-labelledby={`realtime-tab-${activeTab.id}`} tabindex="0">
      <RealtimeEditor
        bind:connection
        bind:message
        {profiles}
        selectedProfileId={activeTab.selectedProfileId}
        selectedMessageId={activeTab.selectedMessageId}
        connectionDirty={realtimeWorkspace.isConnectionDirty(activeTab)}
        messageDirty={realtimeWorkspace.isMessageDirty(activeTab)}
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
        onNewMessage={newMessage}
        onSave={() => saveMessage()}
        onSaveAs={() => saveMessage(true)}
        onSelectProfile={selectProfile}
        onNewProfile={newProfile}
        onSaveProfile={() => saveProfile()}
        onSaveProfileAs={() => saveProfile(true)}
        onDeleteProfile={deleteProfile}
        onImportProfiles={importProfiles}
        onExportProfile={exportProfile}
        onValidityChange={(valid) => (editorValid = valid)}
      />
      {#if activeTab.errorText}<div class="feedback feedback-error" role="alert">{activeTab.errorText}</div>{/if}
      {#if activeTab.messageExternallyChanged}<div class="feedback feedback-warning">This saved message changed externally. Your draft was kept. <button class="button-secondary button-compact" type="button" onclick={reloadMessage}>Reload message</button></div>{/if}
      {#if activeTab.connectionExternallyChanged}<div class="feedback feedback-warning">This profile changed externally. The live connection snapshot was kept. {#if ["disconnected", "failed"].includes(activeTab.status)}<button class="button-secondary button-compact" type="button" onclick={reloadProfile}>Reload profile</button>{/if}</div>{/if}
      <RealtimeTranscript entries={activeTab.transcript} sizeBytes={activeTab.transcriptSizeBytes} onClear={() => realtimeWorkspace.clearTranscript(activeTab.id)} onExport={exportTranscript} onReadPayload={readRealtimePayload} onSavePayload={savePayload} />
    </div>
  {/if}
</div>
<svelte:window onkeydown={handleWindowKeydown} />
{#if isSaveDialogOpen}<CollectionSaveDialog title="Save realtime message" titleId="save-realtime-message-title" confirmLabel="Save message" collections={collections.collections} folders={collections.folderTargets(saveTargetCollectionId)} selectedCollectionId={saveTargetCollectionId} selectedParentId={saveTargetParentId} isSaving={collections.isSavingRequest} onSelectCollection={async (id) => { saveTargetCollectionId = id; saveTargetParentId = null; await collections.loadCollectionItems(id); }} onSelectFolder={(id) => (saveTargetParentId = id)} onConfirm={confirmSave} onDismiss={() => (isSaveDialogOpen = false)} />{/if}
{#if isNewMessagePending}<DialogShell ariaLabelledby="new-realtime-message-title" onDismiss={() => (isNewMessagePending = false)} sizeClass="save-dialog"><div class="editor-header"><h2 id="new-realtime-message-title">Start a new message?</h2></div><div class="editor-block"><p>The active message has unsaved changes. Discard them and start with an empty {message.protocol === "socketio" ? "Socket.IO" : "WebSocket"} message?</p><p class="feedback feedback-warning">The connection and session transcript will be preserved.</p><div class="collections-page-actions"><button class="button-danger" type="button" onclick={replaceWithNewMessage}>Discard and create</button><button class="button-secondary" type="button" onclick={() => (isNewMessagePending = false)}>Cancel</button></div></div></DialogShell>{/if}
{#if pendingMessageId}<DialogShell ariaLabelledby="replace-realtime-message-title" onDismiss={cancelPendingMessage} sizeClass="save-dialog"><div class="editor-header"><h2 id="replace-realtime-message-title">Replace message draft?</h2></div><div class="editor-block"><p>The active tab has unsaved message changes. Discard them and open the selected collection message?</p><p class="feedback feedback-warning">The connection and session transcript will be preserved.</p><div class="collections-page-actions"><button class="button-danger" type="button" onclick={confirmPendingMessage}>Discard and open</button><button class="button-secondary" type="button" onclick={cancelPendingMessage}>Cancel</button></div></div></DialogShell>{/if}
{#if pendingCloseTabId}{@const closingTab = realtimeWorkspace.tabs.find((tab) => tab.id === pendingCloseTabId)}<DialogShell ariaLabelledby="close-realtime-tab-title" onDismiss={() => (pendingCloseTabId = "")} sizeClass="save-dialog"><div class="editor-header"><h2 id="close-realtime-tab-title">Close connection tab?</h2></div><div class="editor-block"><p>Close <strong>{closingTab?.connectionDraft.name ?? "this connection"}</strong>? Active sessions will be disconnected.</p>{#if closingTab && realtimeWorkspace.isDirty(closingTab)}<p class="feedback feedback-warning">Unsaved connection or message changes will be discarded.</p>{/if}<div class="collections-page-actions"><button class="button-danger" type="button" onclick={confirmCloseTab}>Close tab</button><button class="button-secondary" type="button" onclick={() => (pendingCloseTabId = "")}>Cancel</button></div></div></DialogShell>{/if}
