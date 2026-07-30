import type { KeyValueRow } from "$lib/api/types";

const GENERAL_HEADER_NAMES = [
  "Accept",
  "Accept-Encoding",
  "Accept-Language",
  "Authorization",
  "Cache-Control",
  "Connection",
  "Content-Encoding",
  "Content-Length",
  "Content-Type",
  "Cookie",
  "Host",
  "If-Match",
  "If-Modified-Since",
  "If-None-Match",
  "If-Unmodified-Since",
  "Origin",
  "Pragma",
  "Prefer",
  "Range",
  "Referer",
  "User-Agent",
  "X-API-Key",
  "X-Request-ID",
  "X-Trace-ID"
];

const GENERAL_HEADER_VALUE_SUGGESTIONS: Record<string, string[]> = {
  accept: ["application/json", "application/xml", "text/plain", "text/html", "*/*"],
  "accept-encoding": ["gzip, deflate, br", "gzip", "identity"],
  "accept-language": ["en-US,en;q=0.9", "en-US", "en"],
  authorization: ["Bearer {{oauth_access_token}}", "Bearer ", "Basic "],
  "cache-control": ["no-cache", "no-store", "max-age=0", "max-age=3600"],
  connection: ["keep-alive", "close"],
  "content-encoding": ["gzip", "br", "deflate", "identity"],
  "content-type": [
    "application/json",
    "application/x-www-form-urlencoded",
    "multipart/form-data",
    "text/plain",
    "application/xml",
    "text/html"
  ],
  cookie: ["session=", "token="],
  "if-match": ["*"],
  "if-none-match": ["*"],
  origin: ["http://localhost:3000", "http://localhost:5173"],
  pragma: ["no-cache"],
  prefer: ["return=representation", "return=minimal"],
  range: ["bytes=0-"],
  referer: ["http://localhost:3000", "http://localhost:5173"],
  "user-agent": ["PostNot"],
  "x-api-key": ["{{api_key}}"],
  "x-request-id": ["{{$guid}}"],
  "x-trace-id": ["{{$guid}}"]
};

function normalizeHeaderName(value: string) {
  return value.trim().toLowerCase();
}

function uniqueStrings(values: string[]) {
  const seen = new Set<string>();

  return values.filter((value) => {
    const trimmedValue = value.trim();
    const lookupKey = trimmedValue.toLowerCase();
    if (!trimmedValue || seen.has(lookupKey)) return false;
    seen.add(lookupKey);
    return true;
  }).map((value) => value.trim());
}

export function getHeaderNameSuggestions(rows: KeyValueRow[]) {
  return uniqueStrings([
    ...GENERAL_HEADER_NAMES,
    ...rows.map((row) => row.key)
  ]).sort((a, b) => a.localeCompare(b));
}

export function getHeaderValueSuggestions(headerName: string, rows: KeyValueRow[]) {
  const normalizedHeaderName = normalizeHeaderName(headerName);
  if (!normalizedHeaderName) return [];

  return uniqueStrings([
    ...rows
      .filter((row) => normalizeHeaderName(row.key) === normalizedHeaderName)
      .map((row) => row.value),
    ...(GENERAL_HEADER_VALUE_SUGGESTIONS[normalizedHeaderName] ?? [])
  ]);
}
