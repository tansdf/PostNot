import { Channel, invoke } from "@tauri-apps/api/core";

import { hasTauriRuntime } from "$lib/api/commands";
import type {
  ExportResult,
  RealtimeConnectResult,
  RealtimeRequestDraft,
  RealtimeRuntimeEvent,
  RealtimeSessionSnapshot,
  RealtimeWorkspaceState,
  SavedRealtimeRequestDetail,
  SavedRealtimeRequestSummary
} from "$lib/api/types";

export type RealtimeConnectInput = {
  connectionId: string;
  request: RealtimeRequestDraft;
};

export type RealtimeSendMessage =
  | {
      requestType: "websocket";
      composer: Extract<RealtimeRequestDraft, { requestType: "websocket" }>["composer"];
    }
  | {
      requestType: "socketio";
      composer: Extract<RealtimeRequestDraft, { requestType: "socketio" }>["composer"];
    };

export type RealtimeEventSubscription = {
  close: () => void;
};

const mockSavedRequests = new Map<string, SavedRealtimeRequestDetail>();
let mockWorkspaceState: RealtimeWorkspaceState | null = null;

function now() {
  return new Date().toISOString();
}

function createMockSnapshot(connectionId: string): RealtimeSessionSnapshot {
  return {
    connectionId,
    generation: 1,
    lastSequence: 1,
    status: "connected",
    statusMessage: "Connected",
    transcript: [
      {
        id: `${connectionId}-1`,
        connectionId,
        generation: 1,
        sequence: 1,
        occurredAt: now(),
        direction: "system",
        kind: "lifecycle",
        label: "Connected",
        eventName: null,
        payload: null
      }
    ],
    transcriptSizeBytes: 0
  };
}

export async function getRealtimeWorkspaceState(): Promise<RealtimeWorkspaceState | null> {
  if (!hasTauriRuntime()) return mockWorkspaceState;
  return invoke<RealtimeWorkspaceState | null>("get_realtime_workspace_state");
}

export async function saveRealtimeWorkspaceState(state: RealtimeWorkspaceState): Promise<void> {
  if (!hasTauriRuntime()) {
    mockWorkspaceState = structuredClone(state);
    return;
  }
  await invoke("save_realtime_workspace_state", { state });
}

export async function saveRealtimeRequestToCollection(
  collectionId: string,
  request: RealtimeRequestDraft,
  parentId?: string | null
): Promise<SavedRealtimeRequestSummary> {
  if (!hasTauriRuntime()) {
    const detail: SavedRealtimeRequestDetail = {
      id: `mock-realtime-request-${Date.now()}`,
      collectionId,
      parentId: parentId ?? null,
      name: request.name,
      requestType: request.requestType,
      url: request.url,
      updatedAt: now(),
      request: structuredClone(request)
    };
    mockSavedRequests.set(detail.id, detail);
    return detail;
  }
  return invoke<SavedRealtimeRequestSummary>("save_realtime_request_to_collection", {
    collectionId,
    parentId: parentId ?? null,
    request
  });
}

export async function updateSavedRealtimeRequest(
  itemId: string,
  request: RealtimeRequestDraft,
  expectedUpdatedAt?: string | null
): Promise<SavedRealtimeRequestSummary> {
  if (!hasTauriRuntime()) {
    const previous = mockSavedRequests.get(itemId);
    const detail: SavedRealtimeRequestDetail = {
      id: itemId,
      collectionId: previous?.collectionId ?? "mock-collection-1",
      parentId: previous?.parentId ?? null,
      name: request.name,
      requestType: request.requestType,
      url: request.url,
      updatedAt: now(),
      request: structuredClone(request)
    };
    mockSavedRequests.set(itemId, detail);
    return detail;
  }
  return invoke<SavedRealtimeRequestSummary>("update_saved_realtime_request", {
    itemId,
    request,
    expectedUpdatedAt: expectedUpdatedAt ?? null
  });
}

export async function getSavedRealtimeRequest(itemId: string): Promise<SavedRealtimeRequestDetail> {
  if (!hasTauriRuntime()) {
    const saved = mockSavedRequests.get(itemId);
    if (saved) return structuredClone(saved);
    throw new Error("Saved realtime request not found.");
  }
  return invoke<SavedRealtimeRequestDetail>("get_saved_realtime_request", { itemId });
}

export async function connectRealtimeConnection(
  input: RealtimeConnectInput,
  onEvent: (event: RealtimeRuntimeEvent) => void
): Promise<{ result: RealtimeConnectResult | RealtimeSessionSnapshot; subscription: RealtimeEventSubscription }> {
  if (!hasTauriRuntime()) {
    const snapshot = createMockSnapshot(input.connectionId);
    return { result: snapshot, subscription: { close: () => {} } };
  }

  const channel = new Channel<RealtimeRuntimeEvent>();
  channel.onmessage = onEvent;
  const result = await invoke<RealtimeSessionSnapshot>("connect_realtime_connection", {
    input,
    onEvent: channel
  });
  return {
    result,
    subscription: {
      close: () => {
        channel.onmessage = () => {};
      }
    }
  };
}

export async function disconnectRealtimeConnection(connectionId: string): Promise<void> {
  if (!hasTauriRuntime()) return;
  await invoke("disconnect_realtime_connection", { connectionId });
}

export async function releaseRealtimeConnection(connectionId: string): Promise<void> {
  if (!hasTauriRuntime()) return;
  await invoke("release_realtime_connection", { connectionId });
}

export async function sendRealtimeMessage(connectionId: string, message: RealtimeSendMessage): Promise<void> {
  if (!hasTauriRuntime()) return;
  await invoke("send_realtime_message", { connectionId, message });
}

export async function pingRealtimeConnection(connectionId: string, payload?: string): Promise<void> {
  if (!hasTauriRuntime()) return;
  await invoke("ping_realtime_connection", { connectionId, payload: payload ?? null });
}

export async function closeRealtimeConnection(
  connectionId: string,
  code = 1000,
  reason = ""
): Promise<void> {
  if (!hasTauriRuntime()) return;
  await invoke("close_realtime_connection", { connectionId, code, reason });
}

export async function getRealtimeSessionSnapshot(connectionId: string): Promise<RealtimeSessionSnapshot> {
  if (!hasTauriRuntime()) return createMockSnapshot(connectionId);
  return invoke<RealtimeSessionSnapshot>("get_realtime_session_snapshot", { connectionId });
}

export async function clearRealtimeTranscript(connectionId: string): Promise<void> {
  if (!hasTauriRuntime()) return;
  await invoke("clear_realtime_transcript", { connectionId });
}

export async function readRealtimePayload(handleId: string): Promise<string> {
  if (!hasTauriRuntime()) return "";
  return invoke<string>("read_realtime_payload", { handleId });
}

export async function saveRealtimePayload(handleId: string, suggestedName?: string): Promise<string | null> {
  if (!hasTauriRuntime()) return null;
  return invoke<string | null>("save_realtime_payload", { handleId, suggestedName: suggestedName ?? null });
}

export async function exportRealtimeTranscript(connectionId: string): Promise<ExportResult | null> {
  if (!hasTauriRuntime()) return { filePath: `/tmp/${connectionId}-transcript.json` };
  return invoke<ExportResult | null>("export_realtime_transcript", { connectionId });
}
