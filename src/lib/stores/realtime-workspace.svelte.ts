import { browser } from "$app/environment";
import {
  clearRealtimeTranscript, closeRealtimeConnection, connectRealtimeConnection, disconnectRealtimeConnection,
  getRealtimeSessionSnapshot, getRealtimeWorkspaceState, pingRealtimeConnection, releaseRealtimeConnection,
  saveRealtimeWorkspaceState, sendRealtimeMessage, type RealtimeEventSubscription
} from "$lib/api/realtime";
import {
  cloneRealtimeConnectionDraft, cloneRealtimeMessageDraft, createRealtimeConnectionDraft, createRealtimeMessageDraft,
  type RealtimeConnectionDraft, type RealtimeConnectionProfileDetail, type RealtimeConnectionProfileSummary,
  type RealtimeMessageDraft, type RealtimeRuntimeEvent, type RealtimeSessionSnapshot, type RealtimeWorkspaceState,
  type RealtimeWorkspaceTab, type SavedRealtimeMessageDetail, type SavedRealtimeMessageSummary
} from "$lib/api/types";
import {
  createRealtimeWorkspaceTab, normalizeRealtimeWorkspaceState, realtimeConnectionEquals, realtimeMessageEquals,
  serializeRealtimeWorkspaceState, trimRealtimeTranscript
} from "$lib/realtime-workspace";
import { readCachedJson, UI_CACHE_KEYS, writeCachedJson } from "$lib/ui-cache";
import { notifications } from "$lib/stores/notifications.svelte";

const PERSIST_DEBOUNCE_MS = 300;
function seedFromCache() {
  const tabs = readCachedJson<RealtimeWorkspaceTab[]>(UI_CACHE_KEYS.realtimeWorkspaceTabs);
  const activeTabId = readCachedJson<string>(UI_CACHE_KEYS.realtimeWorkspaceActiveTabId) ?? "";
  return normalizeRealtimeWorkspaceState(tabs?.length ? { tabs, activeTabId } : null);
}
const INITIAL_STATE = seedFromCache();

class RealtimeWorkspaceStore {
  initialized = $state(false);
  isInitializing = $state(false);
  tabs = $state.raw<RealtimeWorkspaceTab[]>(INITIAL_STATE.tabs);
  activeTabId = $state(INITIAL_STATE.activeTabId);
  persistTimer: ReturnType<typeof setTimeout> | null = null;
  private subscriptions = new Map<string, RealtimeEventSubscription>();
  private notifiedFailures = new Set<string>();

  get activeTab() { return this.tabs.find((tab) => tab.id === this.activeTabId) ?? null; }
  findTabBySavedMessageId(id: string) { return this.tabs.find((tab) => tab.selectedMessageId === id) ?? null; }
  findTabBySavedRequestId(id: string) { return this.findTabBySavedMessageId(id); }
  isConnectionDirty(tab: RealtimeWorkspaceTab | null | undefined) { return Boolean(tab && !realtimeConnectionEquals(tab.connectionDraft, tab.baselineConnectionDraft)); }
  isMessageDirty(tab: RealtimeWorkspaceTab | null | undefined) { return Boolean(tab && !realtimeMessageEquals(tab.messageDraft, tab.baselineMessageDraft)); }
  isDirty(tab: RealtimeWorkspaceTab | null | undefined) { return this.isConnectionDirty(tab) || this.isMessageDirty(tab); }

  async ensureInitialized() {
    if (!browser || this.initialized || this.isInitializing) return;
    this.isInitializing = true;
    try {
      const restored = await getRealtimeWorkspaceState();
      const state = normalizeRealtimeWorkspaceState(restored);
      this.tabs = state.tabs; this.activeTabId = state.activeTabId; this.initialized = true; this.writeCache();
      if (!restored) await this.persistNow();
    } finally { this.isInitializing = false; }
  }
  activateTab(tabId: string) { if (this.tabs.some((tab) => tab.id === tabId)) { this.activeTabId = tabId; void this.persistNow(); } }
  createBlankTab(protocol: "websocket" | "socketio" = "websocket") {
    const tab = createRealtimeWorkspaceTab(createRealtimeConnectionDraft(protocol), createRealtimeMessageDraft(protocol));
    this.insertAfterActive(tab); this.activeTabId = tab.id; void this.persistNow(); return tab;
  }
  updateConnectionDraft(tabId: string, draft: RealtimeConnectionDraft) { this.updateTab(tabId, (tab) => { tab.connectionDraft = cloneRealtimeConnectionDraft(draft); tab.errorText = ""; }); this.schedulePersist(); }
  updateMessageDraft(tabId: string, draft: RealtimeMessageDraft) { this.updateTab(tabId, (tab) => { tab.messageDraft = cloneRealtimeMessageDraft(draft); tab.errorText = ""; }); this.schedulePersist(); }

  selectProfile(tabId: string, profile: RealtimeConnectionProfileDetail) {
    this.updateTab(tabId, (tab) => {
      tab.selectedProfileId = profile.id; tab.profileUpdatedAt = profile.updatedAt; tab.connectionExternallyChanged = false;
      tab.connectionDraft = cloneRealtimeConnectionDraft(profile.connection); tab.baselineConnectionDraft = cloneRealtimeConnectionDraft(profile.connection); tab.errorText = "";
    }); void this.persistNow();
  }
  setProfileSaved(tabId: string, profile: RealtimeConnectionProfileSummary | RealtimeConnectionProfileDetail, connection: RealtimeConnectionDraft) {
    this.updateTab(tabId, (tab) => { tab.selectedProfileId = profile.id; tab.profileUpdatedAt = profile.updatedAt; tab.connectionExternallyChanged = false; tab.connectionDraft = cloneRealtimeConnectionDraft(connection); tab.baselineConnectionDraft = cloneRealtimeConnectionDraft(connection); }); void this.persistNow();
  }
  newConnection(tabId: string, protocol: "websocket" | "socketio" = "websocket") {
    const draft = createRealtimeConnectionDraft(protocol); this.updateTab(tabId, (tab) => { tab.selectedProfileId = null; tab.profileUpdatedAt = null; tab.connectionExternallyChanged = false; tab.connectionDraft = draft; tab.baselineConnectionDraft = cloneRealtimeConnectionDraft(draft); }); void this.persistNow();
  }
  newMessage(tabId: string, protocol: "websocket" | "socketio" = "websocket") {
    const draft = createRealtimeMessageDraft(protocol);
    this.updateTab(tabId, (tab) => {
      tab.selectedMessageId = null;
      tab.collectionId = null;
      tab.parentId = null;
      tab.sourceUpdatedAt = null;
      tab.messageExternallyChanged = false;
      tab.messageDraft = draft;
      tab.baselineMessageDraft = cloneRealtimeMessageDraft(draft);
      tab.errorText = "";
    });
    void this.persistNow();
  }
  unlinkProfile(profileId: string) { this.tabs = this.tabs.map((tab) => tab.selectedProfileId === profileId ? { ...tab, selectedProfileId: null, profileUpdatedAt: null, baselineConnectionDraft: null, connectionExternallyChanged: false } : tab); void this.persistNow(); }

  openSavedMessage(saved: SavedRealtimeMessageDetail) {
    const tab = this.activeTab ?? this.createBlankTab(saved.message.protocol);
    this.updateTab(tab.id, (item) => { item.selectedMessageId = saved.id; item.collectionId = saved.collectionId; item.parentId = saved.parentId ?? null; item.sourceUpdatedAt = saved.updatedAt; item.messageExternallyChanged = false; item.messageDraft = cloneRealtimeMessageDraft(saved.message); item.baselineMessageDraft = cloneRealtimeMessageDraft(saved.message); item.errorText = ""; });
    void this.persistNow(); return this.tabs.find((item) => item.id === tab.id)!;
  }
  openSavedRequest(saved: SavedRealtimeMessageDetail) { return this.openSavedMessage(saved); }
  setMessageSaved(tabId: string, saved: SavedRealtimeMessageSummary, message: RealtimeMessageDraft) {
    this.updateTab(tabId, (tab) => { tab.selectedMessageId = saved.id; tab.collectionId = saved.collectionId; tab.parentId = saved.parentId ?? null; tab.sourceUpdatedAt = saved.updatedAt; tab.messageExternallyChanged = false; tab.messageDraft = cloneRealtimeMessageDraft(message); tab.baselineMessageDraft = cloneRealtimeMessageDraft(message); tab.errorText = ""; }); void this.persistNow();
  }
  setSaved(tabId: string, saved: SavedRealtimeMessageSummary, message: RealtimeMessageDraft) { this.setMessageSaved(tabId, saved, message); }
  replaceSavedMessage(tabId: string, saved: SavedRealtimeMessageDetail) { this.openSavedMessageInto(tabId, saved); }
  replaceSavedTab(tabId: string, saved: SavedRealtimeMessageDetail) { this.openSavedMessageInto(tabId, saved); }
  private openSavedMessageInto(tabId: string, saved: SavedRealtimeMessageDetail) { this.updateTab(tabId, (tab) => { tab.selectedMessageId = saved.id; tab.collectionId = saved.collectionId; tab.parentId = saved.parentId ?? null; tab.sourceUpdatedAt = saved.updatedAt; tab.messageDraft = cloneRealtimeMessageDraft(saved.message); tab.baselineMessageDraft = cloneRealtimeMessageDraft(saved.message); tab.messageExternallyChanged = false; }); void this.persistNow(); }

  markExternallyChanged(ids: string[]) { const set = new Set(ids); this.tabs = this.tabs.map((tab) => tab.selectedMessageId && set.has(tab.selectedMessageId) ? { ...tab, messageExternallyChanged: true } : tab); void this.persistNow(); }
  markProfilesExternallyChanged(ids: string[]) { const set = new Set(ids); this.tabs = this.tabs.map((tab) => tab.selectedProfileId && set.has(tab.selectedProfileId) ? { ...tab, connectionExternallyChanged: true } : tab); void this.persistNow(); }
  markLiveTabsReconnectRequired() { this.tabs = this.tabs.map((tab) => ["connected", "reconnecting"].includes(tab.status) ? { ...tab, reconnectRequired: true } : tab); }
  unlinkSavedRequests(ids: string[]) { const set = new Set(ids); this.tabs = this.tabs.map((tab) => tab.selectedMessageId && set.has(tab.selectedMessageId) ? { ...tab, selectedMessageId: null, collectionId: null, parentId: null, sourceUpdatedAt: null, messageExternallyChanged: false, baselineMessageDraft: null } : tab); void this.persistNow(); }
  unlinkSavedRequestsForCollection(collectionId: string) { this.unlinkSavedRequests(this.tabs.filter((tab) => tab.collectionId === collectionId && tab.selectedMessageId).map((tab) => tab.selectedMessageId!)); }
  setError(tabId: string, errorText: string) { this.updateTab(tabId, (tab) => { tab.errorText = errorText; }); }

  async connect(tabId: string) {
    const tab = this.tabs.find((item) => item.id === tabId); if (!tab || ["connecting", "disconnecting"].includes(tab.status)) return;
    this.updateTab(tabId, (item) => { item.status = "connecting"; item.statusMessage = "Connecting"; item.errorText = ""; });
    try { this.subscriptions.get(tabId)?.close(); const { result, subscription } = await connectRealtimeConnection({ sessionId: tabId, connection: cloneRealtimeConnectionDraft(tab.connectionDraft) }, (event) => this.applyRuntimeEvent(event)); this.subscriptions.set(tabId, subscription); this.applySnapshot(tabId, result); }
    catch (error) { this.updateTab(tabId, (item) => { item.status = "failed"; item.statusMessage = "Connection failed"; item.errorText = error instanceof Error ? error.message : String(error); }); throw error; }
  }
  async disconnect(tabId: string) { this.updateTab(tabId, (tab) => { tab.status = "disconnecting"; tab.statusMessage = "Disconnecting"; }); try { await disconnectRealtimeConnection(tabId); this.updateTab(tabId, (tab) => { tab.status = "disconnected"; tab.statusMessage = "Disconnected"; tab.reconnectRequired = false; }); } catch (error) { this.setError(tabId, String(error)); throw error; } }
  async send(tabId: string, message: RealtimeMessageDraft) {
    const tab = this.tabs.find((item) => item.id === tabId);
    if (tab?.reconnectRequired) throw new Error("Reconnect this session before sending after the environment changed.");
    await sendRealtimeMessage(tabId, message);
  }
  async ping(tabId: string, payload?: string) { await pingRealtimeConnection(tabId, payload); }
  async closeGracefully(tabId: string, code = 1000, reason = "") { await closeRealtimeConnection(tabId, code, reason); }
  async refreshSnapshot(tabId: string) { this.applySnapshot(tabId, await getRealtimeSessionSnapshot(tabId)); }
  async clearTranscript(tabId: string) { await clearRealtimeTranscript(tabId); this.updateTab(tabId, (tab) => { tab.transcript = []; tab.transcriptSizeBytes = 0; tab.lastSequence = 0; }); }
  async closeTab(tabId: string) { const tab = this.tabs.find((item) => item.id === tabId); if (!tab) return; if (!["disconnected", "failed"].includes(tab.status)) await disconnectRealtimeConnection(tabId); this.subscriptions.get(tabId)?.close(); this.subscriptions.delete(tabId); await releaseRealtimeConnection(tabId); const index = this.tabs.findIndex((item) => item.id === tabId); const next = this.tabs.filter((item) => item.id !== tabId); if (!next.length) next.push(createRealtimeWorkspaceTab()); this.tabs = next; if (this.activeTabId === tabId) this.activeTabId = next[Math.min(index, next.length - 1)].id; await this.persistNow(); }

  private applyRuntimeEvent(event: RealtimeRuntimeEvent) {
    const tab = this.tabs.find((item) => item.id === event.sessionId); if (!tab || event.generation < tab.generation) return;
    if (event.generation === tab.generation && event.sequence > tab.lastSequence + 1 && tab.lastSequence > 0) { void this.refreshSnapshot(event.sessionId); return; }
    this.updateTab(event.sessionId, (item) => { item.generation = event.generation; item.lastSequence = Math.max(item.lastSequence, event.sequence); if (event.type === "status") { item.status = event.status; item.statusMessage = event.message || statusLabel(event.status); if (event.status === "connected") item.reconnectRequired = false; } else if (event.type === "transcript-reset") { const trimmed = trimRealtimeTranscript(event.entries, Number.MAX_SAFE_INTEGER, Number.MAX_SAFE_INTEGER); item.transcript = trimmed.entries; item.transcriptSizeBytes = trimmed.sizeBytes; } else { const trimmed = trimRealtimeTranscript([...item.transcript, event.entry], Number.MAX_SAFE_INTEGER, Number.MAX_SAFE_INTEGER); item.transcript = trimmed.entries; item.transcriptSizeBytes = trimmed.sizeBytes; } });
    if (event.type === "status" && event.status === "failed") { const key = `${event.sessionId}:${event.generation}`; if (this.activeTabId !== event.sessionId && !this.notifiedFailures.has(key)) { this.notifiedFailures.add(key); const name = this.tabs.find((item) => item.id === event.sessionId)?.connectionDraft.name || "Realtime connection"; notifications.error(event.message || "The connection failed.", name, { action: { label: "Open connection", kind: "navigate", href: `/websockets?tabId=${encodeURIComponent(event.sessionId)}` } }); } }
  }
  private applySnapshot(tabId: string, snapshot: RealtimeSessionSnapshot) { this.updateTab(tabId, (tab) => { const trimmed = trimRealtimeTranscript(snapshot.transcript, Number.MAX_SAFE_INTEGER, Number.MAX_SAFE_INTEGER); tab.generation = snapshot.generation; tab.lastSequence = snapshot.lastSequence; tab.status = snapshot.status; tab.statusMessage = snapshot.statusMessage || statusLabel(snapshot.status); tab.transcript = trimmed.entries; tab.transcriptSizeBytes = snapshot.transcriptSizeBytes || trimmed.sizeBytes; if (snapshot.status === "connected") tab.reconnectRequired = false; }); }
  private insertAfterActive(tab: RealtimeWorkspaceTab) { const index = this.tabs.findIndex((item) => item.id === this.activeTabId); this.tabs = index < 0 ? [...this.tabs, tab] : [...this.tabs.slice(0, index + 1), tab, ...this.tabs.slice(index + 1)]; }
  private updateTab(tabId: string, mutate: (tab: RealtimeWorkspaceTab) => void) { this.tabs = this.tabs.map((tab) => { if (tab.id !== tabId) return tab; const next = { ...tab }; mutate(next); return next; }); }
  private schedulePersist() { this.writeCache(); if (this.persistTimer) clearTimeout(this.persistTimer); this.persistTimer = setTimeout(() => { this.persistTimer = null; void this.persistNow(); }, PERSIST_DEBOUNCE_MS); }
  async persistNow() { if (this.persistTimer) { clearTimeout(this.persistTimer); this.persistTimer = null; } const state = serializeRealtimeWorkspaceState({ tabs: this.tabs, activeTabId: this.activeTabId }); this.writeCache(state); await saveRealtimeWorkspaceState(state); }
  private writeCache(state: RealtimeWorkspaceState = serializeRealtimeWorkspaceState({ tabs: this.tabs, activeTabId: this.activeTabId })) { writeCachedJson(UI_CACHE_KEYS.realtimeWorkspaceTabs, state.tabs); writeCachedJson(UI_CACHE_KEYS.realtimeWorkspaceActiveTabId, state.activeTabId); }
}
function statusLabel(status: RealtimeWorkspaceTab["status"]) { return status.charAt(0).toUpperCase() + status.slice(1); }
export const realtimeWorkspace = new RealtimeWorkspaceStore();
