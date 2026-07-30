import { browser } from "$app/environment";

import {
  clearRealtimeTranscript,
  closeRealtimeConnection,
  connectRealtimeConnection,
  disconnectRealtimeConnection,
  getRealtimeSessionSnapshot,
  getRealtimeWorkspaceState,
  pingRealtimeConnection,
  releaseRealtimeConnection,
  saveRealtimeWorkspaceState,
  sendRealtimeMessage,
  type RealtimeEventSubscription,
  type RealtimeSendMessage
} from "$lib/api/realtime";
import {
  cloneRealtimeRequestDraft,
  createRealtimeRequestDraft,
  type RealtimeRequestDraft,
  type RealtimeRuntimeEvent,
  type RealtimeSessionSnapshot,
  type RealtimeWorkspaceState,
  type RealtimeWorkspaceTab,
  type SavedRealtimeRequestDetail,
  type SavedRealtimeRequestSummary
} from "$lib/api/types";
import {
  createRealtimeWorkspaceTab,
  normalizeRealtimeWorkspaceState,
  realtimeDraftEquals,
  serializeRealtimeWorkspaceState,
  trimRealtimeTranscript
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

  get activeTab() {
    return this.tabs.find((tab) => tab.id === this.activeTabId) ?? null;
  }

  findTabBySavedRequestId(savedRequestId: string) {
    return this.tabs.find((tab) => tab.savedRequestId === savedRequestId) ?? null;
  }

  isDirty(tab: RealtimeWorkspaceTab | null | undefined) {
    if (!tab) return false;
    return !realtimeDraftEquals(tab.draft, tab.baselineDraft);
  }

  async ensureInitialized() {
    if (!browser || this.initialized || this.isInitializing) return;
    this.isInitializing = true;
    try {
      const restored = await getRealtimeWorkspaceState();
      const state = normalizeRealtimeWorkspaceState(restored);
      this.tabs = state.tabs;
      this.activeTabId = state.activeTabId;
      this.initialized = true;
      this.writeCache();
      if (!restored) await this.persistNow();
    } finally {
      this.isInitializing = false;
    }
  }

  activateTab(tabId: string) {
    if (!this.tabs.some((tab) => tab.id === tabId)) return;
    this.activeTabId = tabId;
    void this.persistNow();
  }

  createBlankTab(protocol: "websocket" | "socketio" = "websocket") {
    const tab = createRealtimeWorkspaceTab(createRealtimeRequestDraft(protocol));
    this.insertAfterActive(tab);
    this.activeTabId = tab.id;
    void this.persistNow();
    return tab;
  }

  openSavedRequest(saved: SavedRealtimeRequestDetail) {
    const existing = this.findTabBySavedRequestId(saved.id);
    if (existing) {
      this.activateTab(existing.id);
      return existing;
    }
    const tab = createRealtimeWorkspaceTab(saved.request, {
      source: "saved",
      savedRequestId: saved.id,
      collectionId: saved.collectionId,
      parentId: saved.parentId ?? null,
      sourceUpdatedAt: saved.updatedAt,
      baselineDraft: saved.request
    });
    this.insertAfterActive(tab);
    this.activeTabId = tab.id;
    void this.persistNow();
    return tab;
  }

  updateDraft(tabId: string, draft: RealtimeRequestDraft) {
    this.updateTab(tabId, (tab) => {
      if (tab.status === "connected" || tab.status === "reconnecting") {
        tab.reconnectRequired = !realtimeDraftEquals(tab.draft, draft) || tab.reconnectRequired;
      }
      tab.draft = cloneRealtimeRequestDraft(draft);
      tab.errorText = "";
    });
    this.schedulePersist();
  }

  setSaved(tabId: string, saved: SavedRealtimeRequestSummary, draft: RealtimeRequestDraft) {
    this.updateTab(tabId, (tab) => {
      tab.source = "saved";
      tab.savedRequestId = saved.id;
      tab.collectionId = saved.collectionId;
      tab.parentId = saved.parentId ?? null;
      tab.sourceUpdatedAt = saved.updatedAt;
      tab.externallyChanged = false;
      tab.draft = cloneRealtimeRequestDraft(draft);
      tab.baselineDraft = cloneRealtimeRequestDraft(draft);
      tab.errorText = "";
    });
    void this.persistNow();
  }

  replaceSavedTab(tabId: string, saved: SavedRealtimeRequestDetail) {
    this.updateTab(tabId, (tab) => {
      tab.collectionId = saved.collectionId;
      tab.parentId = saved.parentId ?? null;
      tab.sourceUpdatedAt = saved.updatedAt;
      tab.draft = cloneRealtimeRequestDraft(saved.request);
      tab.baselineDraft = cloneRealtimeRequestDraft(saved.request);
      tab.externallyChanged = false;
      tab.reconnectRequired = tab.status === "connected" || tab.status === "reconnecting";
      tab.errorText = "";
    });
    void this.persistNow();
  }

  markExternallyChanged(savedRequestIds: string[]) {
    const ids = new Set(savedRequestIds);
    if (!ids.size) return;
    this.tabs = this.tabs.map((tab) =>
      tab.savedRequestId && ids.has(tab.savedRequestId) ? { ...tab, externallyChanged: true } : tab
    );
    void this.persistNow();
  }

  markLiveTabsReconnectRequired() {
    this.tabs = this.tabs.map((tab) =>
      tab.status === "connected" || tab.status === "reconnecting"
        ? { ...tab, reconnectRequired: true }
        : tab
    );
  }

  unlinkSavedRequests(savedRequestIds: string[]) {
    const ids = new Set(savedRequestIds);
    if (!ids.size) return;
    this.tabs = this.tabs.map((tab) =>
      tab.savedRequestId && ids.has(tab.savedRequestId)
        ? {
            ...tab,
            source: "blank",
            savedRequestId: null,
            collectionId: null,
            parentId: null,
            sourceUpdatedAt: null,
            externallyChanged: false,
            baselineDraft: null
          }
        : tab
    );
    void this.persistNow();
  }

  unlinkSavedRequestsForCollection(collectionId: string) {
    this.unlinkSavedRequests(
      this.tabs.filter((tab) => tab.collectionId === collectionId && tab.savedRequestId).map((tab) => tab.savedRequestId!)
    );
  }

  setError(tabId: string, errorText: string) {
    this.updateTab(tabId, (tab) => {
      tab.errorText = errorText;
    });
  }

  async connect(tabId: string) {
    const tab = this.tabs.find((item) => item.id === tabId);
    if (!tab || ["connecting", "disconnecting"].includes(tab.status)) return;
    this.updateTab(tabId, (item) => {
      item.status = "connecting";
      item.statusMessage = "Connecting";
      item.errorText = "";
    });
    try {
      this.subscriptions.get(tabId)?.close();
      const { result, subscription } = await connectRealtimeConnection(
        { connectionId: tabId, request: cloneRealtimeRequestDraft(tab.draft) },
        (event) => this.applyRuntimeEvent(event)
      );
      this.subscriptions.set(tabId, subscription);
      this.applySnapshot(tabId, result);
    } catch (error) {
      this.updateTab(tabId, (item) => {
        item.status = "failed";
        item.statusMessage = "Connection failed";
        item.errorText = error instanceof Error ? error.message : String(error);
      });
      throw error;
    }
  }

  async disconnect(tabId: string) {
    this.updateTab(tabId, (tab) => {
      tab.status = "disconnecting";
      tab.statusMessage = "Disconnecting";
    });
    try {
      await disconnectRealtimeConnection(tabId);
      this.updateTab(tabId, (tab) => {
        tab.status = "disconnected";
        tab.statusMessage = "Disconnected";
        tab.reconnectRequired = false;
      });
    } catch (error) {
      this.setError(tabId, error instanceof Error ? error.message : String(error));
      throw error;
    }
  }

  async send(tabId: string, message: RealtimeSendMessage) {
    await sendRealtimeMessage(tabId, message);
  }

  async ping(tabId: string, payload?: string) {
    await pingRealtimeConnection(tabId, payload);
  }

  async closeGracefully(tabId: string, code = 1000, reason = "") {
    await closeRealtimeConnection(tabId, code, reason);
  }

  async refreshSnapshot(tabId: string) {
    const snapshot = await getRealtimeSessionSnapshot(tabId);
    this.applySnapshot(tabId, snapshot);
  }

  async clearTranscript(tabId: string) {
    await clearRealtimeTranscript(tabId);
    this.updateTab(tabId, (tab) => {
      tab.transcript = [];
      tab.transcriptSizeBytes = 0;
      tab.lastSequence = 0;
    });
  }

  async closeTab(tabId: string) {
    const tab = this.tabs.find((item) => item.id === tabId);
    if (!tab) return;
    if (tab.status !== "disconnected" && tab.status !== "failed") {
      await disconnectRealtimeConnection(tabId);
    }
    this.subscriptions.get(tabId)?.close();
    this.subscriptions.delete(tabId);
    await releaseRealtimeConnection(tabId);
    const index = this.tabs.findIndex((item) => item.id === tabId);
    const next = this.tabs.filter((item) => item.id !== tabId);
    if (!next.length) next.push(createRealtimeWorkspaceTab());
    this.tabs = next;
    if (this.activeTabId === tabId) {
      this.activeTabId = next[Math.min(index, next.length - 1)].id;
    }
    await this.persistNow();
  }

  private applyRuntimeEvent(event: RealtimeRuntimeEvent) {
    const tab = this.tabs.find((item) => item.id === event.connectionId);
    if (!tab || event.generation < tab.generation) return;
    if (event.generation === tab.generation && event.sequence > tab.lastSequence + 1 && tab.lastSequence > 0) {
      void this.refreshSnapshot(event.connectionId);
      return;
    }
    this.updateTab(event.connectionId, (item) => {
      item.generation = event.generation;
      item.lastSequence = Math.max(item.lastSequence, event.sequence);
      if (event.type === "status") {
        item.status = event.status;
        item.statusMessage = event.message || statusLabel(event.status);
        if (event.status === "connected") item.reconnectRequired = false;
      } else if (event.type === "transcript-reset") {
        const trimmed = trimRealtimeTranscript(event.entries, Number.MAX_SAFE_INTEGER, Number.MAX_SAFE_INTEGER);
        item.transcript = trimmed.entries;
        item.transcriptSizeBytes = trimmed.sizeBytes;
      } else {
        const trimmed = trimRealtimeTranscript(
          [...item.transcript, event.entry],
          Number.MAX_SAFE_INTEGER,
          Number.MAX_SAFE_INTEGER
        );
        item.transcript = trimmed.entries;
        item.transcriptSizeBytes = trimmed.sizeBytes;
      }
    });
    if (event.type === "status" && event.status === "failed") {
      const failureKey = `${event.connectionId}:${event.generation}`;
      const isBackground =
        this.activeTabId !== event.connectionId ||
        (typeof window !== "undefined" && !window.location.pathname.startsWith("/websockets"));
      if (isBackground && !this.notifiedFailures.has(failureKey)) {
        this.notifiedFailures.add(failureKey);
        const name = this.tabs.find((item) => item.id === event.connectionId)?.draft.name || "Realtime connection";
        notifications.error(event.message || "The connection failed.", name, {
          action: {
            label: "Open connection",
            kind: "navigate",
            href: `/websockets?tabId=${encodeURIComponent(event.connectionId)}`
          }
        });
      }
    }
  }

  private applySnapshot(tabId: string, snapshot: RealtimeSessionSnapshot) {
    this.updateTab(tabId, (tab) => {
      const trimmed = trimRealtimeTranscript(
        snapshot.transcript,
        Number.MAX_SAFE_INTEGER,
        Number.MAX_SAFE_INTEGER
      );
      tab.generation = snapshot.generation;
      tab.lastSequence = snapshot.lastSequence;
      tab.status = snapshot.status;
      tab.statusMessage = snapshot.statusMessage || statusLabel(snapshot.status);
      tab.transcript = trimmed.entries;
      tab.transcriptSizeBytes = snapshot.transcriptSizeBytes || trimmed.sizeBytes;
      if (snapshot.status === "connected") tab.reconnectRequired = false;
    });
  }

  private insertAfterActive(tab: RealtimeWorkspaceTab) {
    const index = this.tabs.findIndex((item) => item.id === this.activeTabId);
    this.tabs = index < 0
      ? [...this.tabs, tab]
      : [...this.tabs.slice(0, index + 1), tab, ...this.tabs.slice(index + 1)];
  }

  private updateTab(tabId: string, mutate: (tab: RealtimeWorkspaceTab) => void) {
    this.tabs = this.tabs.map((tab) => {
      if (tab.id !== tabId) return tab;
      const next = { ...tab };
      mutate(next);
      return next;
    });
  }

  private schedulePersist() {
    this.writeCache();
    if (this.persistTimer) clearTimeout(this.persistTimer);
    this.persistTimer = setTimeout(() => {
      this.persistTimer = null;
      void this.persistNow();
    }, PERSIST_DEBOUNCE_MS);
  }

  async persistNow() {
    if (this.persistTimer) {
      clearTimeout(this.persistTimer);
      this.persistTimer = null;
    }
    const state = serializeRealtimeWorkspaceState({ tabs: this.tabs, activeTabId: this.activeTabId });
    this.writeCache(state);
    await saveRealtimeWorkspaceState(state);
  }

  private writeCache(state: RealtimeWorkspaceState = serializeRealtimeWorkspaceState({
    tabs: this.tabs,
    activeTabId: this.activeTabId
  })) {
    writeCachedJson(UI_CACHE_KEYS.realtimeWorkspaceTabs, state.tabs);
    writeCachedJson(UI_CACHE_KEYS.realtimeWorkspaceActiveTabId, state.activeTabId);
  }
}

function statusLabel(status: RealtimeWorkspaceTab["status"]) {
  return status.charAt(0).toUpperCase() + status.slice(1);
}

export const realtimeWorkspace = new RealtimeWorkspaceStore();
