import {
  cloneRealtimeRequestDraft,
  createRealtimeRequestDraft,
  type RealtimeConnectionStatus,
  type RealtimeRequestDraft,
  type RealtimeTranscriptEntry,
  type RealtimeWorkspaceState,
  type RealtimeWorkspaceTab,
  type RealtimeWorkspaceTabSource
} from "$lib/api/types";

export const REALTIME_TRANSCRIPT_ENTRY_LIMIT = 2_000;
export const REALTIME_TRANSCRIPT_BYTE_LIMIT = 64 * 1024 * 1024;

const VALID_SOURCES = new Set<RealtimeWorkspaceTabSource>(["blank", "saved", "imported"]);

export function createRealtimeWorkspaceId() {
  if (typeof crypto !== "undefined" && "randomUUID" in crypto) return crypto.randomUUID();
  return `realtime-tab-${Date.now()}-${Math.random().toString(36).slice(2, 10)}`;
}

export function createRealtimeWorkspaceTab(
  draft: RealtimeRequestDraft = createRealtimeRequestDraft(),
  options: Partial<
    Pick<
      RealtimeWorkspaceTab,
      "source" | "savedRequestId" | "collectionId" | "parentId" | "sourceUpdatedAt" | "baselineDraft"
    >
  > = {}
): RealtimeWorkspaceTab {
  return {
    id: createRealtimeWorkspaceId(),
    source: options.source ?? "blank",
    savedRequestId: options.savedRequestId ?? null,
    collectionId: options.collectionId ?? null,
    parentId: options.parentId ?? null,
    sourceUpdatedAt: options.sourceUpdatedAt ?? null,
    externallyChanged: false,
    draft: cloneRealtimeRequestDraft(draft),
    baselineDraft: options.baselineDraft ? cloneRealtimeRequestDraft(options.baselineDraft) : cloneRealtimeRequestDraft(draft),
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
  const draft = cloneRealtimeRequestDraft(tab.draft ?? createRealtimeRequestDraft());
  const status: RealtimeConnectionStatus = "disconnected";
  return {
    id: tab.id || createRealtimeWorkspaceId(),
    source: VALID_SOURCES.has(tab.source as RealtimeWorkspaceTabSource)
      ? (tab.source as RealtimeWorkspaceTabSource)
      : "blank",
    savedRequestId: tab.savedRequestId ?? null,
    collectionId: tab.collectionId ?? null,
    parentId: tab.parentId ?? null,
    sourceUpdatedAt: tab.sourceUpdatedAt ?? null,
    externallyChanged: Boolean(tab.externallyChanged),
    draft,
    baselineDraft: tab.baselineDraft ? cloneRealtimeRequestDraft(tab.baselineDraft) : cloneRealtimeRequestDraft(draft),
    status,
    generation: 0,
    lastSequence: 0,
    statusMessage: "Disconnected",
    reconnectRequired: false,
    transcript: [],
    transcriptSizeBytes: 0,
    errorText: ""
  };
}

export function normalizeRealtimeWorkspaceState(
  state: Partial<RealtimeWorkspaceState> | null | undefined
): RealtimeWorkspaceState {
  const tabs = state?.tabs?.length
    ? state.tabs.map((tab) => normalizeRealtimeWorkspaceTab(tab))
    : [createRealtimeWorkspaceTab()];
  const activeTabId = tabs.some((tab) => tab.id === state?.activeTabId)
    ? state?.activeTabId ?? tabs[0].id
    : tabs[0].id;
  return { tabs, activeTabId };
}

export function serializeRealtimeWorkspaceState(state: RealtimeWorkspaceState): RealtimeWorkspaceState {
  return {
    activeTabId: state.activeTabId,
    tabs: state.tabs.map((tab) => normalizeRealtimeWorkspaceTab(tab))
  };
}

export function realtimeDraftEquals(
  left: RealtimeRequestDraft | null | undefined,
  right: RealtimeRequestDraft | null | undefined
) {
  return Boolean(left && right && JSON.stringify(left) === JSON.stringify(right));
}

export function transcriptEntrySize(entry: RealtimeTranscriptEntry) {
  return entry.payload?.sizeBytes ?? 0;
}

export function trimRealtimeTranscript(
  entries: RealtimeTranscriptEntry[],
  entryLimit = REALTIME_TRANSCRIPT_ENTRY_LIMIT,
  byteLimit = REALTIME_TRANSCRIPT_BYTE_LIMIT
): { entries: RealtimeTranscriptEntry[]; sizeBytes: number; trimmedCount: number } {
  const boundedEntryLimit = Math.max(1, entryLimit);
  const boundedByteLimit = Math.max(0, byteLimit);
  let sizeBytes = entries.reduce((total, entry) => total + transcriptEntrySize(entry), 0);
  let start = 0;

  while (
    start < entries.length &&
    (entries.length - start > boundedEntryLimit || sizeBytes > boundedByteLimit)
  ) {
    sizeBytes -= transcriptEntrySize(entries[start]);
    start += 1;
  }

  return {
    entries: start ? entries.slice(start) : entries,
    sizeBytes: Math.max(0, sizeBytes),
    trimmedCount: start
  };
}
