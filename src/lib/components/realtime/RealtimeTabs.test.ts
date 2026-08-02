import { render } from "svelte/server";
import { describe, expect, it } from "vitest";

import { createRealtimeConnectionDraft, createRealtimeMessageDraft } from "$lib/api/types";
import { createRealtimeWorkspaceTab } from "$lib/realtime-workspace";
import RealtimeTabs from "$lib/components/realtime/RealtimeTabs.svelte";

describe("RealtimeTabs", () => {
  it("renders protocol, status, selection, and dirty state without relying on color", () => {
    const connection = createRealtimeConnectionDraft("socketio"); connection.name = "Live order events";
    const tab = createRealtimeWorkspaceTab(connection, createRealtimeMessageDraft("socketio"), { selectedMessageId: "request-1", collectionId: "collection-1" });
    Object.assign(tab, { id: "tab-1", status: "reconnecting", generation: 2, lastSequence: 10, statusMessage: "Reconnecting in 1 second" });

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
