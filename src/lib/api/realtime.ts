import { Channel, invoke } from "@tauri-apps/api/core";

import { hasTauriRuntime } from "$lib/api/commands";
import type {
  ExportResult,
  RealtimeRequestDraft,
  RealtimeRuntimeEvent,
  RealtimeSessionSnapshot,
  RealtimeWorkspaceState,
  SavedRealtimeRequestDetail,
  SavedRealtimeRequestSummary
} from "$lib/api/types";
import { createRealtimeRequestDraft } from "$lib/api/types";

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
const mockSessions = new Map<
  string,
  {
    snapshot: RealtimeSessionSnapshot;
    onEvent: (event: RealtimeRuntimeEvent) => void;
  }
>();

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

function seededMockRealtimeRequest(itemId: string): SavedRealtimeRequestDetail | null {
  const updatedAt = "2026-07-30T12:00:00.000Z";
  if (itemId === "mock-realtime-websocket-1") {
    const request = {
      ...createRealtimeRequestDraft("websocket"),
      name: "Live order events",
      url: "wss://events.example.test/orders",
      subprotocols: ["json"]
    };
    return {
      id: itemId,
      collectionId: "mock-collection-1",
      parentId: null,
      name: request.name,
      requestType: "websocket",
      url: request.url,
      updatedAt,
      request
    };
  }
  if (itemId === "mock-realtime-socketio-1") {
    const request = {
      ...createRealtimeRequestDraft("socketio"),
      name: "Support presence",
      url: "https://presence.example.test",
      namespace: "/support"
    };
    return {
      id: itemId,
      collectionId: "mock-collection-1",
      parentId: null,
      name: request.name,
      requestType: "socketio",
      url: request.url,
      updatedAt,
      request
    };
  }
  return null;
}

function cloneSnapshot(snapshot: RealtimeSessionSnapshot) {
  return structuredClone(snapshot);
}

function inlinePayload(text: string, encoding: "utf8" | "base64" = "utf8") {
  return {
    mode: "inline" as const,
    text,
    sizeBytes: new TextEncoder().encode(text).byteLength,
    encoding,
    truncated: false
  };
}

function emitMockTranscript(
  connectionId: string,
  entry: Omit<RealtimeSessionSnapshot["transcript"][number], "id" | "connectionId" | "generation" | "sequence" | "occurredAt">
) {
  const session = mockSessions.get(connectionId);
  if (!session) throw new Error("Connect the realtime request before sending a message.");
  const sequence = session.snapshot.lastSequence + 1;
  const complete = {
    ...entry,
    id: `${connectionId}-${sequence}`,
    connectionId,
    generation: session.snapshot.generation,
    sequence,
    occurredAt: now()
  };
  session.snapshot.lastSequence = sequence;
  session.snapshot.transcript.push(complete);
  session.snapshot.transcriptSizeBytes += complete.payload?.sizeBytes ?? 0;
  session.onEvent({
    type: "transcript",
    connectionId,
    generation: session.snapshot.generation,
    sequence,
    entry: structuredClone(complete)
  });
}

function mockMessagePresentation(message: RealtimeSendMessage) {
  if (message.requestType === "websocket") {
    const composer = message.composer;
    if (composer.mode === "binary") {
      const binary = composer.binary;
      const text = binary?.source === "file" ? binary.path : binary?.value ?? "";
      return {
        kind: "binary" as const,
        label: "Binary message",
        eventName: null,
        payload: inlinePayload(text, "base64")
      };
    }
    return {
      kind: composer.mode,
      label: composer.mode === "json" ? "JSON message" : "Text message",
      eventName: null,
      payload: inlinePayload(composer.content)
    };
  }
  const composer = message.composer;
  const text = composer.binary
    ? composer.binary.source === "file"
      ? composer.binary.path
      : composer.binary.value
    : JSON.stringify(composer.arguments);
  return {
    kind: composer.binary ? "binary" as const : "event" as const,
    label: composer.waitForAck ? "Event · awaiting ACK" : "Event",
    eventName: composer.event,
    payload: inlinePayload(text, composer.binary ? "base64" : "utf8")
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
    const saved = mockSavedRequests.get(itemId) ?? seededMockRealtimeRequest(itemId);
    if (saved) return structuredClone(saved);
    throw new Error("Saved realtime request not found.");
  }
  return invoke<SavedRealtimeRequestDetail>("get_saved_realtime_request", { itemId });
}

export async function listSavedRealtimeRequests(collectionId: string): Promise<SavedRealtimeRequestSummary[]> {
  if (!hasTauriRuntime()) {
    const seeded = ["mock-realtime-websocket-1", "mock-realtime-socketio-1"]
      .map(seededMockRealtimeRequest)
      .filter((item): item is SavedRealtimeRequestDetail => Boolean(item));
    return [...seeded, ...mockSavedRequests.values()]
      .filter((item) => item.collectionId === collectionId)
      .map(({ request: _, ...summary }) => structuredClone(summary));
  }
  return invoke<SavedRealtimeRequestSummary[]>("list_saved_realtime_requests", { collectionId });
}

export async function deleteSavedRealtimeRequest(itemId: string): Promise<void> {
  if (!hasTauriRuntime()) {
    mockSavedRequests.delete(itemId);
    return;
  }
  await invoke("delete_saved_realtime_request", { itemId });
}

export async function connectRealtimeConnection(
  input: RealtimeConnectInput,
  onEvent: (event: RealtimeRuntimeEvent) => void
): Promise<{ result: RealtimeSessionSnapshot; subscription: RealtimeEventSubscription }> {
  if (!hasTauriRuntime()) {
    const snapshot = createMockSnapshot(input.connectionId);
    mockSessions.set(input.connectionId, { snapshot, onEvent });
    return {
      result: cloneSnapshot(snapshot),
      subscription: {
        close: () => {
          const session = mockSessions.get(input.connectionId);
          if (session) session.onEvent = () => {};
        }
      }
    };
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
  if (!hasTauriRuntime()) {
    const session = mockSessions.get(connectionId);
    if (session) {
      session.snapshot.status = "disconnected";
      session.snapshot.statusMessage = "Disconnected";
    }
    return;
  }
  await invoke("disconnect_realtime_connection", { connectionId });
}

export async function releaseRealtimeConnection(connectionId: string): Promise<void> {
  if (!hasTauriRuntime()) {
    mockSessions.delete(connectionId);
    return;
  }
  await invoke("release_realtime_connection", { connectionId });
}

export async function sendRealtimeMessage(connectionId: string, message: RealtimeSendMessage): Promise<void> {
  if (!hasTauriRuntime()) {
    const presentation = mockMessagePresentation(message);
    emitMockTranscript(connectionId, { ...presentation, direction: "sent" });
    emitMockTranscript(connectionId, {
      ...presentation,
      direction: "received",
      label: message.requestType === "socketio" ? "Mock server event" : "Mock echo"
    });
    if (message.requestType === "socketio" && message.composer.waitForAck) {
      emitMockTranscript(connectionId, {
        direction: "received",
        kind: "ack",
        label: "Acknowledgement",
        eventName: message.composer.event,
        payload: inlinePayload('[{"accepted":true}]')
      });
    }
    return;
  }
  await invoke("send_realtime_message", { connectionId, message });
}

export async function pingRealtimeConnection(connectionId: string, payload?: string): Promise<void> {
  if (!hasTauriRuntime()) {
    emitMockTranscript(connectionId, {
      direction: "sent",
      kind: "ping",
      label: "Ping",
      eventName: null,
      payload: payload ? inlinePayload(payload) : null
    });
    emitMockTranscript(connectionId, {
      direction: "received",
      kind: "pong",
      label: "Pong",
      eventName: null,
      payload: payload ? inlinePayload(payload) : null
    });
    return;
  }
  await invoke("ping_realtime_connection", { connectionId, payload: payload ?? null });
}

export async function closeRealtimeConnection(
  connectionId: string,
  code = 1000,
  reason = ""
): Promise<void> {
  if (!hasTauriRuntime()) {
    emitMockTranscript(connectionId, {
      direction: "system",
      kind: "lifecycle",
      label: `Closed · ${code}${reason ? ` · ${reason}` : ""}`,
      eventName: null,
      payload: null
    });
    const session = mockSessions.get(connectionId);
    if (session) {
      session.snapshot.status = "disconnected";
      session.snapshot.statusMessage = "Disconnected";
    }
    return;
  }
  await invoke("close_realtime_connection", { connectionId, code, reason });
}

export async function getRealtimeSessionSnapshot(connectionId: string): Promise<RealtimeSessionSnapshot> {
  if (!hasTauriRuntime()) {
    const existing = mockSessions.get(connectionId);
    return cloneSnapshot(existing?.snapshot ?? createMockSnapshot(connectionId));
  }
  return invoke<RealtimeSessionSnapshot>("get_realtime_session_snapshot", { connectionId });
}

export async function clearRealtimeTranscript(connectionId: string): Promise<void> {
  if (!hasTauriRuntime()) {
    const session = mockSessions.get(connectionId);
    if (session) {
      session.snapshot.transcript = [];
      session.snapshot.transcriptSizeBytes = 0;
    }
    return;
  }
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
