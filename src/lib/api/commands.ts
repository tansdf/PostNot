import { invoke } from "@tauri-apps/api/core";
import {
  createDefaultSettings,
  type AppSettings,
  type HistoryEntrySummary,
  type RequestDraft,
  type ResponsePayload
} from "$lib/api/types";

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

function createMockHistory(limit = 10): HistoryEntrySummary[] {
  return [
    {
      id: "mock-history-1",
      requestName: "Sample request",
      method: "GET" as const,
      url: "https://jsonplaceholder.typicode.com/todos/1",
      statusCode: 200,
      durationMs: 42,
      responseBodyPreview: '{\n  "message": "Tauri backend is not connected yet in this environment."\n}',
      errorText: "",
      executedAt: new Date().toISOString()
    }
  ].slice(0, limit);
}

export async function sendRequest(payload: RequestDraft): Promise<ResponsePayload> {
  if (!hasTauriRuntime()) {
    return createMockResponse(payload);
  }

  return invoke<ResponsePayload>("send_request", { payload });
}

export async function getSettings(): Promise<AppSettings> {
  if (!hasTauriRuntime()) {
    return createDefaultSettings();
  }

  return invoke<AppSettings>("get_settings");
}

export async function updateSettings(settings: AppSettings): Promise<AppSettings> {
  if (!hasTauriRuntime()) {
    return settings;
  }

  return invoke<AppSettings>("update_settings", { settings });
}

export async function listHistory(limit = 25): Promise<HistoryEntrySummary[]> {
  if (!hasTauriRuntime()) {
    return createMockHistory(limit);
  }

  return invoke<HistoryEntrySummary[]>("list_history", { limit });
}
