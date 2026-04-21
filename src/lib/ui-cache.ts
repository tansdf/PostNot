/**
 * Lightweight synchronous localStorage cache for UI state that needs to be
 * correct on the very first paint (before async backend reads resolve).
 *
 * All helpers swallow storage failures (private mode, disabled storage,
 * quota exceeded, corrupted JSON, etc.) and behave as "cache miss" so
 * callers can always fall back to a safe default without branching on
 * runtime environment.
 */

export function readCachedJson<T>(key: string): T | null {
  if (typeof window === "undefined") {
    return null;
  }

  try {
    const raw = window.localStorage.getItem(key);
    if (raw === null) {
      return null;
    }
    return JSON.parse(raw) as T;
  } catch {
    return null;
  }
}

export function writeCachedJson(key: string, value: unknown): void {
  if (typeof window === "undefined") {
    return;
  }

  try {
    window.localStorage.setItem(key, JSON.stringify(value));
  } catch {
    // Ignore storage failures.
  }
}

export function clearCachedJson(key: string): void {
  if (typeof window === "undefined") {
    return;
  }

  try {
    window.localStorage.removeItem(key);
  } catch {
    // Ignore storage failures.
  }
}

export const UI_CACHE_KEYS = {
  settings: "postnot.settings",
  environmentsList: "postnot.environments.list",
  environmentsActiveId: "postnot.environments.activeId",
  environmentsActiveVarCount: "postnot.environments.activeVarCount",
  environmentsActiveDetail: "postnot.environments.activeDetailMeta",
  workspaceTabs: "postnot.workspace.tabs",
  workspaceActiveTabId: "postnot.workspace.activeTabId",
  collectionsList: "postnot.collections.list",
  collectionsSelectedId: "postnot.collections.selectedId",
  collectionsItemsByCollection: "postnot.collections.itemsByCollection",
  sidebarExpanded: "postnot.sidebar.expanded"
} as const;
