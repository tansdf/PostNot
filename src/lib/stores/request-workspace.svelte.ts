import { browser } from "$app/environment";

import { getRequestWorkspaceState, releaseResponseBody, saveRequestWorkspaceState } from "$lib/api/commands";
import type {
  RequestDraft,
  RequestScriptExecution,
  RequestWorkspaceState,
  RequestWorkspaceTab,
  RequestWorkspaceTabSource,
  ResponsePayload,
  SavedRequestDetail
} from "$lib/api/types";
import {
  cloneRequestDraft,
  cloneRequestWorkspaceState,
  cloneResponsePayload,
  createRequestDraft
} from "$lib/api/types";
import { createEmptyRequestScriptExecution } from "$lib/request-scripts";
import { readCachedJson, writeCachedJson, UI_CACHE_KEYS } from "$lib/ui-cache";

const REQUEST_PERSIST_DEBOUNCE_MS = 300;
const VALID_TAB_SOURCES = new Set(["blank", "saved", "imported", "history"]);

function seedWorkspaceFromCache(): RequestWorkspaceState {
  const cachedTabs = readCachedJson<RequestWorkspaceTab[]>(UI_CACHE_KEYS.workspaceTabs);
  const cachedActiveTabId = readCachedJson<string>(UI_CACHE_KEYS.workspaceActiveTabId) ?? "";

  if (!cachedTabs || cachedTabs.length === 0) {
    const fallbackTab = createBlankWorkspaceTab();
    return { tabs: [fallbackTab], activeTabId: fallbackTab.id };
  }

  const normalizedTabs = cachedTabs.map(normalizePersistedWorkspaceTab);
  const activeTabId = normalizedTabs.some((tab) => tab.id === cachedActiveTabId)
    ? cachedActiveTabId
    : normalizedTabs[0].id;

  return { tabs: normalizedTabs, activeTabId };
}

function createId() {
  if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
    return crypto.randomUUID();
  }

  return `workspace-tab-${Date.now()}-${Math.random().toString(36).slice(2, 10)}`;
}

function createWorkspaceTab(
  source: RequestWorkspaceTabSource,
  request: RequestDraft,
  options: {
    savedRequestId?: string | null;
    collectionId?: string | null;
    parentId?: string | null;
    sourceUpdatedAt?: string | null;
    baselineRequest?: RequestDraft | null;
  } = {}
): RequestWorkspaceTab {
  return {
    id: createId(),
    source,
    savedRequestId: options.savedRequestId ?? null,
    collectionId: options.collectionId ?? null,
    parentId: options.parentId ?? null,
    sourceUpdatedAt: options.sourceUpdatedAt ?? null,
    externallyChanged: false,
    request: cloneRequestDraft(request),
    baselineRequest: options.baselineRequest ? cloneRequestDraft(options.baselineRequest) : null,
    response: null,
    scriptExecution: createEmptyRequestScriptExecution(),
    errorText: ""
  };
}

function createBlankWorkspaceTab() {
  const request = createRequestDraft();
  return createWorkspaceTab("blank", request, {
    baselineRequest: request
  });
}

function requestEquals(left: RequestDraft | null, right: RequestDraft | null) {
  if (left === right) {
    return true;
  }

  if (!left || !right) {
    return false;
  }

  return JSON.stringify(left) === JSON.stringify(right);
}

function normalizeScriptExecution(
  execution: RequestScriptExecution | null | undefined
): RequestScriptExecution {
  if (!execution) {
    return createEmptyRequestScriptExecution();
  }

  return {
    preRequestErrorText: execution.preRequestErrorText ?? "",
    testScriptErrorText: execution.testScriptErrorText ?? "",
    tests: Array.isArray(execution.tests)
      ? execution.tests.map((test) => ({
          id: test.id,
          name: test.name,
          status: test.status,
          errorText: test.errorText ?? ""
        }))
      : []
  };
}

function normalizeWorkspaceTab(tab: RequestWorkspaceTab): RequestWorkspaceTab {
  const source = VALID_TAB_SOURCES.has(tab.source) ? tab.source : "blank";

  return {
    id: tab.id || createId(),
    source,
    savedRequestId: tab.savedRequestId ?? null,
    collectionId: tab.collectionId ?? null,
    parentId: tab.parentId ?? null,
    sourceUpdatedAt: tab.sourceUpdatedAt ?? null,
    externallyChanged: tab.externallyChanged ?? false,
    request: cloneRequestDraft(tab.request ?? createRequestDraft()),
    baselineRequest: tab.baselineRequest ? cloneRequestDraft(tab.baselineRequest) : null,
    response: tab.response ? cloneResponsePayload(tab.response) : null,
    scriptExecution: normalizeScriptExecution(tab.scriptExecution),
    errorText: tab.errorText ?? ""
  };
}

function normalizePersistedWorkspaceTab(tab: RequestWorkspaceTab): RequestWorkspaceTab {
  const normalized = normalizeWorkspaceTab(tab);
  return {
    ...normalized,
    response: null,
    scriptExecution: createEmptyRequestScriptExecution(),
    errorText: ""
  };
}

function normalizeWorkspaceState(state: RequestWorkspaceState | null): RequestWorkspaceState {
  const tabs = state?.tabs?.length ? state.tabs.map(normalizePersistedWorkspaceTab) : [createBlankWorkspaceTab()];
  const activeTabId = tabs.some((tab) => tab.id === state?.activeTabId) ? state?.activeTabId ?? tabs[0].id : tabs[0].id;

  return {
    tabs,
    activeTabId
  };
}

const INITIAL_WORKSPACE_STATE = seedWorkspaceFromCache();

class RequestWorkspaceStore {
  initialized = $state(false);
  isInitializing = $state(false);
  tabs = $state.raw<RequestWorkspaceTab[]>(INITIAL_WORKSPACE_STATE.tabs);
  activeTabId = $state(INITIAL_WORKSPACE_STATE.activeTabId);
  inFlightTabId = $state("");
  isCanceling = $state(false);
  persistTimer: ReturnType<typeof setTimeout> | null = null;

  get activeTab(): RequestWorkspaceTab | null {
    return this.tabs.find((tab) => tab.id === this.activeTabId) ?? null;
  }

  findTabBySavedRequestId(savedRequestId: string) {
    return this.tabs.find((tab) => tab.savedRequestId === savedRequestId) ?? null;
  }

  isDirty(tab: RequestWorkspaceTab | null | undefined) {
    if (!tab) {
      return false;
    }

    if (tab.source === "imported" || tab.source === "history") {
      return true;
    }

    return !requestEquals(tab.request, tab.baselineRequest);
  }

  async ensureInitialized() {
    if (!browser || this.initialized || this.isInitializing) {
      return;
    }

    this.isInitializing = true;

    try {
      const restored = await getRequestWorkspaceState();
      const nextState = normalizeWorkspaceState(restored);
      this.tabs = nextState.tabs;
      this.activeTabId = nextState.activeTabId;
      this.initialized = true;
      this.writeCache();

      if (!restored) {
        await this.persistNow();
      }
    } finally {
      this.isInitializing = false;
    }
  }

  private writeCache() {
    writeCachedJson(UI_CACHE_KEYS.workspaceTabs, this.serializeForPersistence().tabs);
    writeCachedJson(UI_CACHE_KEYS.workspaceActiveTabId, this.activeTabId);
  }

  activateTab(tabId: string) {
    if (!this.tabs.some((tab) => tab.id === tabId)) {
      return;
    }

    this.activeTabId = tabId;
    void this.persistNow();
  }

  createBlankTab() {
    const nextTab = createBlankWorkspaceTab();
    this.insertAfterActive(nextTab);
    this.activeTabId = nextTab.id;
    void this.persistNow();
    return cloneRequestDraft(nextTab.request);
  }

  openImportedTab(request: RequestDraft) {
    const nextTab = createWorkspaceTab("imported", request);
    this.insertAfterActive(nextTab);
    this.activeTabId = nextTab.id;
    void this.persistNow();
    return nextTab;
  }

  openHistoryRequest(request: RequestDraft) {
    const nextTab = createWorkspaceTab("history", request);
    this.insertAfterActive(nextTab);
    this.activeTabId = nextTab.id;
    void this.persistNow();
    return nextTab;
  }

  openSavedRequest(savedRequest: SavedRequestDetail) {
    const existingTab = this.findTabBySavedRequestId(savedRequest.id);
    if (existingTab) {
      this.activeTabId = existingTab.id;
      void this.persistNow();
      return existingTab;
    }

    const nextTab = createWorkspaceTab("saved", savedRequest.request, {
      savedRequestId: savedRequest.id,
      collectionId: savedRequest.collectionId,
      parentId: savedRequest.parentId ?? null,
      sourceUpdatedAt: savedRequest.updatedAt,
      baselineRequest: savedRequest.request
    });
    this.insertAfterActive(nextTab);
    this.activeTabId = nextTab.id;
    void this.persistNow();
    return nextTab;
  }

  updateTabRequest(tabId: string, request: RequestDraft) {
    if (!tabId) {
      return;
    }

    this.updateTab(tabId, (tab) => {
      tab.request = cloneRequestDraft(request);
    });
    this.schedulePersist();
  }

  clearTabError(tabId: string) {
    this.updateTab(tabId, (tab) => {
      tab.errorText = "";
    });
    this.schedulePersist();
  }

  setTabError(tabId: string, errorText: string) {
    this.updateTab(tabId, (tab) => {
      tab.errorText = errorText;
    });
    this.schedulePersist();
  }

  setTabResponse(
    tabId: string,
    response: ResponsePayload | null,
    scriptExecution: RequestScriptExecution = createEmptyRequestScriptExecution()
  ) {
    const previous = this.tabs.find((tab) => tab.id === tabId)?.response;
    if (previous?.body.mode === "file" && previous.body.handleId !== (response?.body.mode === "file" ? response.body.handleId : "")) {
      void releaseResponseBody(previous.body.handleId);
    }
    this.updateTab(tabId, (tab) => {
      tab.response = response ? cloneResponsePayload(response) : null;
      tab.scriptExecution = normalizeScriptExecution(scriptExecution);
    });
    void this.persistNow();
  }

  setTabSaved(
    tabId: string,
    savedRequest: {
      id: string;
      collectionId: string;
      parentId?: string | null;
      updatedAt?: string;
    },
    request: RequestDraft
  ) {
    this.updateTab(tabId, (tab) => {
      tab.source = "saved";
      tab.savedRequestId = savedRequest.id;
      tab.collectionId = savedRequest.collectionId;
      tab.parentId = savedRequest.parentId ?? null;
      tab.sourceUpdatedAt = savedRequest.updatedAt ?? tab.sourceUpdatedAt;
      tab.externallyChanged = false;
      tab.request = cloneRequestDraft(request);
      tab.baselineRequest = cloneRequestDraft(request);
      tab.errorText = "";
    });
    void this.persistNow();
  }

  markExternallyChanged(savedRequestIds: string[]) {
    const ids = new Set(savedRequestIds);
    if (ids.size === 0) return;
    this.tabs = this.tabs.map((tab) => tab.savedRequestId && ids.has(tab.savedRequestId)
      ? { ...tab, externallyChanged: true }
      : tab);
    void this.persistNow();
  }

  replaceSavedTab(tabId: string, savedRequest: SavedRequestDetail) {
    this.updateTab(tabId, (tab) => {
      tab.request = cloneRequestDraft(savedRequest.request);
      tab.baselineRequest = cloneRequestDraft(savedRequest.request);
      tab.collectionId = savedRequest.collectionId;
      tab.parentId = savedRequest.parentId ?? null;
      tab.sourceUpdatedAt = savedRequest.updatedAt;
      tab.externallyChanged = false;
      tab.errorText = "";
    });
    void this.persistNow();
  }

  unlinkSavedRequestsForCollection(collectionId: string) {
    if (!collectionId) {
      return 0;
    }

    return this.unlinkSavedRequestTabs((tab) => tab.collectionId === collectionId);
  }

  unlinkSavedRequests(savedRequestIds: string[]) {
    const ids = new Set(savedRequestIds.filter(Boolean));
    if (ids.size === 0) {
      return 0;
    }

    return this.unlinkSavedRequestTabs((tab) => Boolean(tab.savedRequestId && ids.has(tab.savedRequestId)));
  }

  unlinkSavedRequestsFromMissingCollections(validCollectionIds: Set<string>) {
    return this.unlinkSavedRequestTabs((tab) => Boolean(tab.collectionId && !validCollectionIds.has(tab.collectionId)));
  }

  markSendStarted(tabId: string) {
    this.inFlightTabId = tabId;
    this.isCanceling = false;
  }

  markSendFinished(tabId: string) {
    if (this.inFlightTabId === tabId) {
      this.inFlightTabId = "";
      this.isCanceling = false;
    }
  }

  markCanceling() {
    if (this.inFlightTabId) {
      this.isCanceling = true;
    }
  }

  closeTab(tabId: string) {
    const tab = this.tabs.find((item) => item.id === tabId) ?? null;
    if (!tab) {
      return "missing" as const;
    }

    if (tab.response?.body.mode === "file") {
      void releaseResponseBody(tab.response.body.handleId);
    }

    if (this.inFlightTabId === tabId) {
      return "blocked-sending" as const;
    }

    const remainingTabs = this.tabs.filter((item) => item.id !== tabId);

    if (remainingTabs.length === 0) {
      const fallbackTab = createBlankWorkspaceTab();
      this.tabs = [fallbackTab];
      this.activeTabId = fallbackTab.id;
      void this.persistNow();
      return "closed" as const;
    }

    const closedIndex = this.tabs.findIndex((item) => item.id === tabId);
    this.tabs = remainingTabs;

    if (this.activeTabId === tabId) {
      const nextActive = remainingTabs[Math.max(0, closedIndex - 1)] ?? remainingTabs[0];
      this.activeTabId = nextActive.id;
    }

    void this.persistNow();
    return "closed" as const;
  }

  serialize(): RequestWorkspaceState {
    return this.serializeForPersistence();
  }

  serializeForPersistence(): RequestWorkspaceState {
    return cloneRequestWorkspaceState({
      tabs: this.tabs.map(normalizePersistedWorkspaceTab),
      activeTabId: this.activeTabId
    });
  }

  schedulePersist() {
    if (!browser || !this.initialized) {
      return;
    }

    this.writeCache();

    if (this.persistTimer) {
      window.clearTimeout(this.persistTimer);
    }

    this.persistTimer = window.setTimeout(() => {
      this.persistTimer = null;
      void this.persistNow();
    }, REQUEST_PERSIST_DEBOUNCE_MS);
  }

  async persistNow() {
    if (!browser || !this.initialized) {
      return;
    }

    if (this.persistTimer) {
      window.clearTimeout(this.persistTimer);
      this.persistTimer = null;
    }

    this.writeCache();
    await saveRequestWorkspaceState(this.serializeForPersistence());
  }

  private insertAfterActive(nextTab: RequestWorkspaceTab) {
    const currentIndex = this.tabs.findIndex((tab) => tab.id === this.activeTabId);
    if (currentIndex < 0) {
      this.tabs = [...this.tabs, nextTab];
      return;
    }

    this.tabs = [
      ...this.tabs.slice(0, currentIndex + 1),
      nextTab,
      ...this.tabs.slice(currentIndex + 1)
    ];
  }

  private unlinkSavedRequestTabs(matches: (tab: RequestWorkspaceTab) => boolean) {
    let changedCount = 0;

    this.tabs = this.tabs.map((tab) => {
      if (!matches(tab)) {
        return tab;
      }

      changedCount += 1;
      return normalizeWorkspaceTab({
        ...tab,
        source: "blank",
        savedRequestId: null,
        collectionId: null,
        parentId: null,
        baselineRequest: null,
        errorText: ""
      });
    });

    if (changedCount > 0) {
      void this.persistNow();
    }

    return changedCount;
  }

  private updateTab(tabId: string, mutate: (tab: RequestWorkspaceTab) => void) {
    this.tabs = this.tabs.map((tab) => {
      if (tab.id !== tabId) {
        return tab;
      }

      const nextTab = normalizeWorkspaceTab(tab);
      mutate(nextTab);
      return nextTab;
    });
  }
}

export const requestWorkspace = new RequestWorkspaceStore();
