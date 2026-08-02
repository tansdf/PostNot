import {
  cloneRealtimeConnectionDraft,
  cloneRealtimeMessageDraft,
  createRealtimeConnectionDraft,
  createRealtimeMessageDraft,
  type RealtimeConnectionDraft,
  type RealtimeConnectionStatus,
  type RealtimeMessageDraft,
  type RealtimeTranscriptEntry,
  type RealtimeWorkspaceState,
  type RealtimeWorkspaceTab
} from "$lib/api/types";

export const REALTIME_TRANSCRIPT_ENTRY_LIMIT = 2_000;
export const REALTIME_TRANSCRIPT_BYTE_LIMIT = 64 * 1024 * 1024;

export function createRealtimeWorkspaceId() {
  if (typeof crypto !== "undefined" && "randomUUID" in crypto) return crypto.randomUUID();
  return `realtime-tab-${Date.now()}-${Math.random().toString(36).slice(2, 10)}`;
}

export function createRealtimeWorkspaceTab(
  connectionDraft: RealtimeConnectionDraft = createRealtimeConnectionDraft(),
  messageDraft: RealtimeMessageDraft = createRealtimeMessageDraft(connectionDraft.protocol),
  options: Partial<Pick<RealtimeWorkspaceTab,
    "selectedProfileId" | "profileUpdatedAt" | "baselineConnectionDraft" |
    "selectedMessageId" | "collectionId" | "parentId" | "sourceUpdatedAt" | "baselineMessageDraft">> = {}
): RealtimeWorkspaceTab {
  return {
    id: createRealtimeWorkspaceId(),
    selectedProfileId: options.selectedProfileId ?? null,
    profileUpdatedAt: options.profileUpdatedAt ?? null,
    connectionExternallyChanged: false,
    connectionDraft: cloneRealtimeConnectionDraft(connectionDraft),
    baselineConnectionDraft: options.baselineConnectionDraft ? cloneRealtimeConnectionDraft(options.baselineConnectionDraft) : cloneRealtimeConnectionDraft(connectionDraft),
    selectedMessageId: options.selectedMessageId ?? null,
    collectionId: options.collectionId ?? null,
    parentId: options.parentId ?? null,
    sourceUpdatedAt: options.sourceUpdatedAt ?? null,
    messageExternallyChanged: false,
    messageDraft: cloneRealtimeMessageDraft(messageDraft),
    baselineMessageDraft: options.baselineMessageDraft ? cloneRealtimeMessageDraft(options.baselineMessageDraft) : cloneRealtimeMessageDraft(messageDraft),
    status: "disconnected",
    generation: 0,
    lastSequence: 0,
    statusMessage: "Disconnected",
    reconnectRequired: false,
    transcript: [],
    transcriptSizeBytes: 0,
    errorText: ""
  };
}

export function normalizeRealtimeWorkspaceTab(tab: Partial<RealtimeWorkspaceTab>): RealtimeWorkspaceTab {
  const connection = cloneRealtimeConnectionDraft(tab.connectionDraft ?? createRealtimeConnectionDraft());
  const message = cloneRealtimeMessageDraft(tab.messageDraft ?? createRealtimeMessageDraft(connection.protocol));
  const normalized = createRealtimeWorkspaceTab(connection, message, {
    selectedProfileId: tab.selectedProfileId,
    profileUpdatedAt: tab.profileUpdatedAt,
    baselineConnectionDraft: tab.baselineConnectionDraft ?? connection,
    selectedMessageId: tab.selectedMessageId,
    collectionId: tab.collectionId,
    parentId: tab.parentId,
    sourceUpdatedAt: tab.sourceUpdatedAt,
    baselineMessageDraft: tab.baselineMessageDraft ?? message
  });
  normalized.id = tab.id || normalized.id;
  normalized.connectionExternallyChanged = Boolean(tab.connectionExternallyChanged);
  normalized.messageExternallyChanged = Boolean(tab.messageExternallyChanged);
  return normalized;
}

export function normalizeRealtimeWorkspaceState(state: Partial<RealtimeWorkspaceState> | null | undefined): RealtimeWorkspaceState {
  const tabs = state?.tabs?.length ? state.tabs.map(normalizeRealtimeWorkspaceTab) : [createRealtimeWorkspaceTab()];
  return { tabs, activeTabId: tabs.some((tab) => tab.id === state?.activeTabId) ? state!.activeTabId! : tabs[0].id };
}

export function serializeRealtimeWorkspaceState(state: RealtimeWorkspaceState): RealtimeWorkspaceState {
  return { activeTabId: state.activeTabId, tabs: state.tabs.map(normalizeRealtimeWorkspaceTab) };
}

export function realtimeConnectionEquals(left: RealtimeConnectionDraft | null | undefined, right: RealtimeConnectionDraft | null | undefined) {
  return Boolean(left && right && JSON.stringify(left) === JSON.stringify(right));
}
export function realtimeMessageEquals(left: RealtimeMessageDraft | null | undefined, right: RealtimeMessageDraft | null | undefined) {
  return Boolean(left && right && JSON.stringify(left) === JSON.stringify(right));
}
export function transcriptEntrySize(entry: RealtimeTranscriptEntry) { return entry.payload?.sizeBytes ?? 0; }
export function trimRealtimeTranscript(entries: RealtimeTranscriptEntry[], entryLimit = REALTIME_TRANSCRIPT_ENTRY_LIMIT, byteLimit = REALTIME_TRANSCRIPT_BYTE_LIMIT) {
  let sizeBytes = entries.reduce((total, entry) => total + transcriptEntrySize(entry), 0);
  let start = 0;
  while (start < entries.length && (entries.length - start > Math.max(1, entryLimit) || sizeBytes > Math.max(0, byteLimit))) sizeBytes -= transcriptEntrySize(entries[start++]);
  return { entries: start ? entries.slice(start) : entries, sizeBytes: Math.max(0, sizeBytes), trimmedCount: start };
}
