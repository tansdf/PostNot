import { Channel, invoke } from "@tauri-apps/api/core";

import { hasTauriRuntime } from "$lib/api/commands";
import type {
  ExportResult,
  RealtimeConnectionDraft,
  RealtimeConnectionProfileDetail,
  RealtimeConnectionProfileSummary,
  RealtimeMessageDraft,
  RealtimeRuntimeEvent,
  RealtimeSessionSnapshot,
  RealtimeWorkspaceState,
  SavedRealtimeMessageDetail,
  SavedRealtimeMessageSummary
} from "$lib/api/types";
import { createRealtimeConnectionDraft, createRealtimeMessageDraft } from "$lib/api/types";

export type RealtimeConnectInput = { sessionId: string; connection: RealtimeConnectionDraft };
export type RealtimeSendMessage = RealtimeMessageDraft;
export type RealtimeEventSubscription = { close: () => void };

const mockSavedMessages = new Map<string, SavedRealtimeMessageDetail>();
const mockProfiles = new Map<string, RealtimeConnectionProfileDetail>();
let mockWorkspaceState: RealtimeWorkspaceState | null = null;
const mockSessions = new Map<string, { snapshot: RealtimeSessionSnapshot; protocol: RealtimeConnectionDraft["protocol"]; onEvent: (event: RealtimeRuntimeEvent) => void }>();

const now = () => new Date().toISOString();
const inlinePayload = (text: string, encoding: "utf8" | "base64" = "utf8") => ({
  mode: "inline" as const, text, sizeBytes: new TextEncoder().encode(text).byteLength, encoding, truncated: false
});

function createMockSnapshot(sessionId: string): RealtimeSessionSnapshot {
  return { sessionId, generation: 1, lastSequence: 1, status: "connected", statusMessage: "Connected", transcript: [{
    id: `${sessionId}-1`, sessionId, generation: 1, sequence: 1, occurredAt: now(), direction: "system",
    kind: "lifecycle", label: "Connected", eventName: null, payload: null
  }], transcriptSizeBytes: 0 };
}

function cloneSummary(detail: SavedRealtimeMessageDetail): SavedRealtimeMessageSummary {
  const { message: _message, ...summary } = detail;
  return structuredClone(summary);
}

function seedMessage(itemId: string): SavedRealtimeMessageDetail | null {
  const protocol = itemId === "mock-realtime-socketio-1" ? "socketio" : itemId === "mock-realtime-websocket-1" ? "websocket" : null;
  if (!protocol) return null;
  const message = createRealtimeMessageDraft(protocol);
  message.name = protocol === "socketio" ? "Support presence" : "Live order events";
  return { id: itemId, collectionId: "mock-collection-1", parentId: null, name: message.name, requestType: protocol, updatedAt: "2026-07-30T12:00:00.000Z", message };
}

function seedProfiles() {
  if (mockProfiles.size) return;
  for (const protocol of ["websocket", "socketio"] as const) {
    const connection = createRealtimeConnectionDraft(protocol);
    connection.name = protocol === "websocket" ? "Order events" : "Support presence";
    connection.url = protocol === "websocket" ? "wss://events.example.test/orders" : "https://presence.example.test";
    const id = `mock-${protocol}-profile`;
    mockProfiles.set(id, { id, name: connection.name, protocol, url: connection.url, updatedAt: now(), connection });
  }
}

export async function getRealtimeWorkspaceState() {
  if (!hasTauriRuntime()) return mockWorkspaceState;
  return invoke<RealtimeWorkspaceState | null>("get_realtime_workspace_state");
}
export async function saveRealtimeWorkspaceState(state: RealtimeWorkspaceState) {
  if (!hasTauriRuntime()) { mockWorkspaceState = structuredClone(state); return; }
  await invoke("save_realtime_workspace_state", { state });
}

export async function listRealtimeConnectionProfiles(): Promise<RealtimeConnectionProfileSummary[]> {
  if (!hasTauriRuntime()) { seedProfiles(); return [...mockProfiles.values()].map(({ connection: _, ...item }) => structuredClone(item)); }
  return invoke("list_realtime_connection_profiles");
}
export async function getRealtimeConnectionProfile(profileId: string): Promise<RealtimeConnectionProfileDetail> {
  if (!hasTauriRuntime()) { seedProfiles(); const item = mockProfiles.get(profileId); if (item) return structuredClone(item); throw new Error("Realtime connection profile not found."); }
  return invoke("get_realtime_connection_profile", { profileId });
}
export async function createRealtimeConnectionProfile(connection: RealtimeConnectionDraft): Promise<RealtimeConnectionProfileDetail> {
  if (!hasTauriRuntime()) { const detail = { id: `mock-profile-${Date.now()}`, name: connection.name, protocol: connection.protocol, url: connection.url, updatedAt: now(), connection: structuredClone(connection) }; mockProfiles.set(detail.id, detail); return detail; }
  return invoke("create_realtime_connection_profile", { connection });
}
export async function updateRealtimeConnectionProfile(profileId: string, connection: RealtimeConnectionDraft, expectedUpdatedAt?: string | null): Promise<RealtimeConnectionProfileDetail> {
  if (!hasTauriRuntime()) { const detail = { id: profileId, name: connection.name, protocol: connection.protocol, url: connection.url, updatedAt: now(), connection: structuredClone(connection) }; mockProfiles.set(profileId, detail); return detail; }
  return invoke("update_realtime_connection_profile", { profileId, connection, expectedUpdatedAt: expectedUpdatedAt ?? null });
}
export async function deleteRealtimeConnectionProfile(profileId: string, expectedUpdatedAt?: string | null): Promise<void> {
  if (!hasTauriRuntime()) { mockProfiles.delete(profileId); return; }
  await invoke("delete_realtime_connection_profile", { profileId, expectedUpdatedAt: expectedUpdatedAt ?? null });
}
export async function importRealtimeConnectionProfiles(): Promise<RealtimeConnectionProfileDetail[]> {
  if (!hasTauriRuntime()) return [];
  return invoke("import_realtime_connection_profiles");
}
export async function exportRealtimeConnectionProfiles(profileIds: string[], includeSensitive = false): Promise<ExportResult | null> {
  if (!hasTauriRuntime()) return { filePath: "/tmp/postnot-realtime-connections.json" };
  return invoke("export_realtime_connection_profiles", { profileIds, includeSensitive });
}

export async function saveRealtimeMessageToCollection(collectionId: string, message: RealtimeMessageDraft, parentId?: string | null): Promise<SavedRealtimeMessageSummary> {
  if (!hasTauriRuntime()) { const detail: SavedRealtimeMessageDetail = { id: `mock-realtime-message-${Date.now()}`, collectionId, parentId: parentId ?? null, name: message.name, requestType: message.protocol, updatedAt: now(), message: structuredClone(message) }; mockSavedMessages.set(detail.id, detail); return cloneSummary(detail); }
  return invoke("save_realtime_message_to_collection", { collectionId, parentId: parentId ?? null, message });
}
export async function updateSavedRealtimeMessage(itemId: string, message: RealtimeMessageDraft, expectedUpdatedAt?: string | null): Promise<SavedRealtimeMessageSummary> {
  if (!hasTauriRuntime()) { const old = mockSavedMessages.get(itemId); const detail: SavedRealtimeMessageDetail = { id: itemId, collectionId: old?.collectionId ?? "mock-collection-1", parentId: old?.parentId ?? null, name: message.name, requestType: message.protocol, updatedAt: now(), message: structuredClone(message) }; mockSavedMessages.set(itemId, detail); return cloneSummary(detail); }
  return invoke("update_saved_realtime_message", { itemId, message, expectedUpdatedAt: expectedUpdatedAt ?? null });
}
export async function getSavedRealtimeMessage(itemId: string): Promise<SavedRealtimeMessageDetail> {
  if (!hasTauriRuntime()) { const item = mockSavedMessages.get(itemId) ?? seedMessage(itemId); if (item) return structuredClone(item); throw new Error("Saved realtime message not found."); }
  return invoke("get_saved_realtime_message", { itemId });
}
export async function listSavedRealtimeMessages(collectionId: string): Promise<SavedRealtimeMessageSummary[]> {
  if (!hasTauriRuntime()) return [seedMessage("mock-realtime-websocket-1"), seedMessage("mock-realtime-socketio-1"), ...mockSavedMessages.values()].filter((item): item is SavedRealtimeMessageDetail => Boolean(item && item.collectionId === collectionId)).map(cloneSummary);
  return invoke("list_saved_realtime_messages", { collectionId });
}
export async function deleteSavedRealtimeMessage(itemId: string): Promise<void> {
  if (!hasTauriRuntime()) { mockSavedMessages.delete(itemId); return; }
  await invoke("delete_saved_realtime_message", { itemId });
}

function emitMockTranscript(sessionId: string, entry: Omit<RealtimeSessionSnapshot["transcript"][number], "id" | "sessionId" | "generation" | "sequence" | "occurredAt">) {
  const session = mockSessions.get(sessionId); if (!session) throw new Error("Connect before sending a message.");
  const sequence = ++session.snapshot.lastSequence;
  const complete = { ...entry, id: `${sessionId}-${sequence}`, sessionId, generation: session.snapshot.generation, sequence, occurredAt: now() };
  session.snapshot.transcript.push(complete); session.snapshot.transcriptSizeBytes += complete.payload?.sizeBytes ?? 0;
  session.onEvent({ type: "transcript", sessionId, generation: session.snapshot.generation, sequence, entry: structuredClone(complete) });
}

function mockPresentation(message: RealtimeMessageDraft) {
  if (message.protocol === "websocket") {
    if (message.composer.mode === "binary") { const binary = message.composer.binary; return { kind: "binary" as const, label: "Binary message", eventName: null, payload: inlinePayload(binary?.source === "file" ? binary.path : binary?.value ?? "", "base64") }; }
    return { kind: message.composer.mode, label: message.composer.mode === "json" ? "JSON message" : "Text message", eventName: null, payload: inlinePayload(message.composer.content) };
  }
  const text = message.composer.binary ? (message.composer.binary.source === "file" ? message.composer.binary.path : message.composer.binary.value) : JSON.stringify(message.composer.arguments);
  return { kind: message.composer.binary ? "binary" as const : "event" as const, label: message.composer.waitForAck ? "Event · awaiting ACK" : "Event", eventName: message.composer.event, payload: inlinePayload(text, message.composer.binary ? "base64" : "utf8") };
}

export async function connectRealtimeConnection(input: RealtimeConnectInput, onEvent: (event: RealtimeRuntimeEvent) => void): Promise<{ result: RealtimeSessionSnapshot; subscription: RealtimeEventSubscription }> {
  if (!hasTauriRuntime()) { const snapshot = createMockSnapshot(input.sessionId); mockSessions.set(input.sessionId, { snapshot, protocol: input.connection.protocol, onEvent }); return { result: structuredClone(snapshot), subscription: { close: () => { const session = mockSessions.get(input.sessionId); if (session) session.onEvent = () => {}; } } }; }
  const channel = new Channel<RealtimeRuntimeEvent>(); channel.onmessage = onEvent;
  const result = await invoke<RealtimeSessionSnapshot>("connect_realtime_connection", { input, onEvent: channel });
  return { result, subscription: { close: () => { channel.onmessage = () => {}; } } };
}
export async function disconnectRealtimeConnection(sessionId: string) { if (!hasTauriRuntime()) { const session = mockSessions.get(sessionId); if (session) { session.snapshot.status = "disconnected"; session.snapshot.statusMessage = "Disconnected"; } return; } await invoke("disconnect_realtime_connection", { sessionId }); }
export async function releaseRealtimeConnection(sessionId: string) { if (!hasTauriRuntime()) { mockSessions.delete(sessionId); return; } await invoke("release_realtime_connection", { sessionId }); }
export async function sendRealtimeMessage(sessionId: string, message: RealtimeMessageDraft) {
  if (!hasTauriRuntime()) { const session = mockSessions.get(sessionId); if (!session) throw new Error("Connect before sending a message."); if (session.protocol !== message.protocol) throw new Error(`Connected ${session.protocol} session cannot send a ${message.protocol} message.`); const p = mockPresentation(message); emitMockTranscript(sessionId, { ...p, direction: "sent" }); emitMockTranscript(sessionId, { ...p, direction: "received", label: message.protocol === "socketio" ? "Mock server event" : "Mock echo" }); return; }
  await invoke("send_realtime_message", { sessionId, message });
}
export async function pingRealtimeConnection(sessionId: string, payload?: string) { if (!hasTauriRuntime()) { emitMockTranscript(sessionId, { direction: "sent", kind: "ping", label: "Ping", eventName: null, payload: payload ? inlinePayload(payload) : null }); return; } await invoke("ping_realtime_connection", { sessionId, payload: payload ?? null }); }
export async function closeRealtimeConnection(sessionId: string, code = 1000, reason = "") { if (!hasTauriRuntime()) { const session = mockSessions.get(sessionId); if (session) { session.snapshot.status = "disconnected"; session.snapshot.statusMessage = "Disconnected"; } return; } await invoke("close_realtime_connection", { sessionId, code, reason }); }
export async function getRealtimeSessionSnapshot(sessionId: string): Promise<RealtimeSessionSnapshot> { if (!hasTauriRuntime()) { const snapshot = mockSessions.get(sessionId)?.snapshot; if (!snapshot) throw new Error("Realtime session not found."); return structuredClone(snapshot); } return invoke("get_realtime_session_snapshot", { sessionId }); }
export async function clearRealtimeTranscript(sessionId: string) { if (!hasTauriRuntime()) { const session = mockSessions.get(sessionId); if (session) { session.snapshot.transcript = []; session.snapshot.transcriptSizeBytes = 0; } return; } await invoke("clear_realtime_transcript", { sessionId }); }
export async function readRealtimePayload(handleId: string): Promise<string> { if (!hasTauriRuntime()) return ""; return invoke("read_realtime_payload", { handleId }); }
export async function saveRealtimePayload(handleId: string, suggestedName?: string): Promise<string | null> { if (!hasTauriRuntime()) return null; return invoke("save_realtime_payload", { handleId, suggestedName: suggestedName ?? null }); }
export async function exportRealtimeTranscript(sessionId: string): Promise<ExportResult | null> { if (!hasTauriRuntime()) return { filePath: `/tmp/${sessionId}-transcript.json` }; return invoke("export_realtime_transcript", { sessionId }); }
