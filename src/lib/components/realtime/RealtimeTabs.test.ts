import { render } from "svelte/server";
import { describe, expect, it } from "vitest";

import { createRealtimeRequestDraft, type RealtimeWorkspaceTab } from "$lib/api/types";
import RealtimeTabs from "$lib/components/realtime/RealtimeTabs.svelte";

describe("RealtimeTabs", () => {
  it("renders protocol, status, selection, and dirty state without relying on color", () => {
    const tab: RealtimeWorkspaceTab = {
      id: "tab-1",
      source: "saved",
      savedRequestId: "request-1",
      collectionId: "collection-1",
      parentId: null,
      sourceUpdatedAt: "2026-07-30T00:00:00.000Z",
      externallyChanged: false,
      draft: {
        ...createRealtimeRequestDraft("socketio"),
        name: "Live order events"
      },
      baselineDraft: createRealtimeRequestDraft("socketio"),
      status: "reconnecting",
      generation: 2,
      lastSequence: 10,
      statusMessage: "Reconnecting in 1 second",
      reconnectRequired: false,
      transcript: [],
      transcriptSizeBytes: 0,
      errorText: ""
    };

    const { body } = render(RealtimeTabs, {
      props: {
        tabs: [tab],
        activeTabId: tab.id,
        onIsDirty: () => true
      }
    });

    expect(body).toContain('role="tablist"');
    expect(body).toContain('aria-selected="true"');
    expect(body).toContain("Live order events");
    expect(body).toContain("S.IO");
    expect(body).toContain("Reconnecting in 1 second.");
    expect(body).toContain("Unsaved changes");
  });
});
