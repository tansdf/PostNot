import { browser } from "$app/environment";

import { getRequestWorkspaceState, saveRequestWorkspaceState } from "$lib/api/commands";
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
  cloneRequestScriptExecution,
  cloneRequestWorkspaceState,
  cloneResponsePayload,
  createRequestDraft
} from "$lib/api/types";
import { createEmptyRequestScriptExecution } from "$lib/request-scripts";

const REQUEST_PERSIST_DEBOUNCE_MS = 300;

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
    baselineRequest?: RequestDraft | null;
  } = {}
): RequestWorkspaceTab {
  return {
    id: createId(),
    source,
    savedRequestId: options.savedRequestId ?? null,
    collectionId: options.collectionId ?? null,
    parentId: options.parentId ?? null,
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
  return {
    id: tab.id || createId(),
    source: tab.source ?? "blank",
    savedRequestId: tab.savedRequestId ?? null,
    collectionId: tab.collectionId ?? null,
    parentId: tab.parentId ?? null,
    request: cloneRequestDraft(tab.request ?? createRequestDraft()),
    baselineRequest: tab.baselineRequest ? cloneRequestDraft(tab.baselineRequest) : null,
    response: tab.response ? cloneResponsePayload(tab.response) : null,
    scriptExecution: normalizeScriptExecution(tab.scriptExecution),
    errorText: tab.errorText ?? ""
  };
}

function normalizeWorkspaceState(state: RequestWorkspaceState | null): RequestWorkspaceState {
  const tabs = state?.tabs?.length ? state.tabs.map(normalizeWorkspaceTab) : [createBlankWorkspaceTab()];
  const activeTabId = tabs.some((tab) => tab.id === state?.activeTabId) ? state?.activeTabId ?? tabs[0].id : tabs[0].id;

  return {
    tabs,
    activeTabId
  };
}

class RequestWorkspaceStore {
  initialized = $state(false);
  isInitializing = $state(false);
  tabs = $state.raw<RequestWorkspaceTab[]>([]);
  activeTabId = $state("");
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

    if (tab.source === "imported") {
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

      if (!restored) {
        await this.persistNow();
      }
    } finally {
      this.isInitializing = false;
    }
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
    },
    request: RequestDraft
  ) {
    this.updateTab(tabId, (tab) => {
      tab.source = "saved";
      tab.savedRequestId = savedRequest.id;
      tab.collectionId = savedRequest.collectionId;
      tab.parentId = savedRequest.parentId ?? null;
      tab.request = cloneRequestDraft(request);
      tab.baselineRequest = cloneRequestDraft(request);
      tab.errorText = "";
    });
    void this.persistNow();
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
    return cloneRequestWorkspaceState({
      tabs: this.tabs,
      activeTabId: this.activeTabId
    });
  }

  schedulePersist() {
    if (!browser || !this.initialized) {
      return;
    }

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

    await saveRequestWorkspaceState(this.serialize());
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
