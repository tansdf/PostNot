import { describe, expect, it } from "vitest";

import { createRealtimeConnectionDraft, createRealtimeMessageDraft, type RealtimeTranscriptEntry } from "$lib/api/types";
import {
  createRealtimeWorkspaceTab,
  normalizeRealtimeWorkspaceState,
  serializeRealtimeWorkspaceState,
  trimRealtimeTranscript
} from "$lib/realtime-workspace";

function transcriptEntry(sequence: number, sizeBytes: number): RealtimeTranscriptEntry {
  return {
    id: `entry-${sequence}`,
    sessionId: "tab-1",
    generation: 2,
    sequence,
    occurredAt: "2026-07-30T00:00:00.000Z",
    direction: "received",
    kind: "text",
    label: "Text message",
    eventName: null,
    payload: {
      mode: "inline",
      text: String(sequence),
      sizeBytes,
      encoding: "utf8",
      truncated: false
    }
  };
}

describe("realtime workspace persistence", () => {
  it("restores drafts but always resets live state and transcript", () => {
    const dirtyTab = createRealtimeWorkspaceTab(createRealtimeConnectionDraft("socketio"), createRealtimeMessageDraft("socketio"), { selectedMessageId: "request-1", collectionId: "collection-1" });
    Object.assign(dirtyTab, { id: "tab-1", status: "connected", generation: 9, lastSequence: 42, statusMessage: "Connected", reconnectRequired: true, transcript: [transcriptEntry(42, 100)], transcriptSizeBytes: 100, errorText: "runtime-only" });

    const restored = normalizeRealtimeWorkspaceState({
      tabs: [dirtyTab],
      activeTabId: dirtyTab.id
    });

    expect(restored.tabs[0]).toMatchObject({
      id: "tab-1",
      selectedMessageId: "request-1",
      status: "disconnected",
      generation: 0,
      lastSequence: 0,
      statusMessage: "Disconnected",
      reconnectRequired: false,
      transcript: [],
      transcriptSizeBytes: 0,
      errorText: ""
    });
    expect(restored.tabs[0].connectionDraft.protocol).toBe("socketio");
  });

  it("serializes disconnected workspace state only", () => {
    const tab = createRealtimeWorkspaceTab();
    Object.assign(tab, { id: "tab-1", status: "reconnecting", generation: 3, lastSequence: 4, statusMessage: "Retrying", transcript: [transcriptEntry(4, 10)], transcriptSizeBytes: 10 });
    const serialized = serializeRealtimeWorkspaceState({
      activeTabId: "tab-1",
      tabs: [
        tab
      ]
    });

    expect(serialized.tabs[0].status).toBe("disconnected");
    expect(serialized.tabs[0].transcript).toEqual([]);
  });
});

describe("realtime transcript bounds", () => {
  it("keeps the newest entries when either limit is exceeded", () => {
    const result = trimRealtimeTranscript(
      [transcriptEntry(1, 4), transcriptEntry(2, 4), transcriptEntry(3, 4)],
      2,
      8
    );

    expect(result.entries.map((entry) => entry.sequence)).toEqual([2, 3]);
    expect(result.sizeBytes).toBe(8);
    expect(result.trimmedCount).toBe(1);
  });

  it("removes an individually oversized payload", () => {
    const result = trimRealtimeTranscript([transcriptEntry(1, 20)], 5, 10);
    expect(result.entries).toEqual([]);
    expect(result.sizeBytes).toBe(0);
    expect(result.trimmedCount).toBe(1);
  });

  it("honors caller-provided limits above the defaults", () => {
    const entries = Array.from({ length: 2_001 }, (_, index) => transcriptEntry(index + 1, 1));
    const result = trimRealtimeTranscript(entries, 10_000, 512 * 1024 * 1024);

    expect(result.entries).toHaveLength(2_001);
    expect(result.trimmedCount).toBe(0);
  });
});
