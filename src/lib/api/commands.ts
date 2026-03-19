import { invoke } from "@tauri-apps/api/core";
import type { RequestDraft, ResponsePayload } from "$lib/api/types";

function hasTauriRuntime() {
  return typeof window !== "undefined" && ("__TAURI_INTERNALS__" in window || "__TAURI__" in window);
}

function createMockResponse(payload: RequestDraft): ResponsePayload {
  return {
    statusCode: 200,
    statusText: "Frontend mock",
    durationMs: 42,
    sizeBytes: payload.url.length,
    headers: [
      {
        id: "mock-header",
        key: "content-type",
        value: "application/json",
        enabled: true
      }
    ],
    bodyText: JSON.stringify(
      {
        message: "Tauri backend is not connected yet in this environment.",
        request: payload
      },
      null,
      2
    ),
    errorText: "",
    executedAt: new Date().toISOString()
  };
}

export async function sendRequest(payload: RequestDraft): Promise<ResponsePayload> {
  if (!hasTauriRuntime()) {
    return createMockResponse(payload);
  }

  return invoke<ResponsePayload>("send_request", { payload });
}
