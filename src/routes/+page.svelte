<script lang="ts">
  import { goto } from "$app/navigation";
  import { resolve } from "$app/paths";
  import { page } from "$app/state";
  import { onMount, tick } from "svelte";

  import {
    cancelActiveRequest,
    clearHistory,
    getEnvironment,
    getHistoryEntry,
    getSavedRequest,
    getSettings,
    importCurlRequestToDraft,
    importOpenApiRequestToDraft,
    listEnvironments,
    listHistory,
    previewRequest,
    setActiveEnvironment,
    sendRequest,
    updateSettings,
    updateEnvironment
  } from "$lib/api/commands";
  import type {
    AppSettings,
    CollectionItemSummary,
    EnvironmentDetail,
    EnvironmentSummary,
    HistoryEntryDetail,
    HistoryEntrySummary,
    KeyValueRow,
    RequestDraft,
    RequestPreview,
    RequestWorkspaceTab,
    RequestScriptExecution
  } from "$lib/api/types";
  import {
    cloneRequestDraft,
    createDefaultSettings,
    createEnvironmentVariable,
    createRequestDraft
  } from "$lib/api/types";
  import HistoryPanel from "$lib/components/history/HistoryPanel.svelte";
  import RequestEditor from "$lib/components/request/RequestEditor.svelte";
  import RequestTabs from "$lib/components/request/RequestTabs.svelte";
  import JsonViewer from "$lib/components/response/JsonViewer.svelte";
  import ResponseViewer from "$lib/components/response/ResponseViewer.svelte";
  import { createStaleGuard } from "$lib/async-stale-guard";
  import { modalFocusTrap } from "$lib/modal-focus-trap";
  import {
    createEmptyRequestScriptExecution,
    type InheritedRequestScripts,
    runPreRequestScript,
    runTestScript
  } from "$lib/request-scripts";
  import { collections } from "$lib/stores/collections.svelte";
  import { notifications } from "$lib/stores/notifications.svelte";
  import { requestWorkspace } from "$lib/stores/request-workspace.svelte";
  import { readCachedJson, writeCachedJson, UI_CACHE_KEYS } from "$lib/ui-cache";

  function mergeCachedSettings(): AppSettings {
    const defaults = createDefaultSettings();
    const cached = readCachedJson<Partial<AppSettings>>(UI_CACHE_KEYS.settings);
    if (!cached || typeof cached !== "object") {
      return defaults;
    }
    return { ...defaults, ...cached };
  }

  let request = $state(createRequestDraft());
  let settings: AppSettings = $state(mergeCachedSettings());
  let history: HistoryEntrySummary[] = $state([]);
  let isHistoryLoading = $state(true);
  let isHistoryDetailLoading = $state(false);
  let isClearingHistory = $state(false);
  let restoringHistoryId = $state("");
  let historyErrorText = $state("");
  let historyDetailErrorText = $state("");
  let settingsErrorText = $state("");
  let environments: EnvironmentSummary[] = $state(
    readCachedJson<EnvironmentSummary[]>(UI_CACHE_KEYS.environmentsList) ?? []
  );
  let activeEnvironmentId = $state(
    readCachedJson<string>(UI_CACHE_KEYS.environmentsActiveId) ?? ""
  );
  let activeEnvironmentDetail: EnvironmentDetail | null = $state(null);
  let cachedActiveEnvironmentVarCount: number | null = $state(
    readCachedJson<number>(UI_CACHE_KEYS.environmentsActiveVarCount)
  );
  let isEnvironmentsLoading = $state(true);
  let isEnvironmentChanging = $state(false);
  let environmentsErrorText = $state("");
  let selectedHistoryId = $state("");
  let selectedHistoryDetail: HistoryEntryDetail | null = $state(null);
  let isSaveDialogOpen = $state(false);
  let saveDialogMode = $state<"replace-tab" | "save-as">("replace-tab");
  let saveDialogTabId = $state("");
  let isRequestImportDialogOpen = $state(false);
  let requestImportFormat = $state<"curl" | "openapi">("curl");
  let curlImportSource = $state("");
  let openApiImportSource = $state("");
  let isRequestPreviewDialogOpen = $state(false);
  let isRequestPreviewLoading = $state(false);
  let requestPreview: RequestPreview | null = $state(null);
  let requestPreviewErrorText = $state("");
  let isRequestExportDialogOpen = $state(false);
  let requestExportFormat = $state<"curl" | "json">("curl");
  let requestExportSafety = $state<"redacted" | "full">("redacted");
  let isImportingRequest = $state(false);
  let isHistoryCollapseSaving = $state(false);
  let requestImportErrorText = $state("");
  let openApiImportFileInput: HTMLInputElement | null = $state(null);
  let saveTargetCollectionId = $state("");
  let saveTargetParentId: string | null = $state(null);
  let requestTabsScrollRequest = $state({ n: 0, tabId: "" });
  let requestedSavedRequestId = $derived(page.url.searchParams.get("savedRequestId") ?? "");
  let activeTab = $derived(
    requestWorkspace.tabs.find((tab) => tab.id === requestWorkspace.activeTabId) ?? null
  );
  let activeTabResponse = $derived(activeTab?.response ?? null);
  let activeTabScriptExecution = $derived(activeTab?.scriptExecution ?? null);
  let activeTabErrorText = $derived(activeTab?.errorText ?? "");
  let activeTabIsSending = $derived(activeTab?.id === requestWorkspace.inFlightTabId);
  let activeTabSendLocked = $derived(
    Boolean(requestWorkspace.inFlightTabId && requestWorkspace.inFlightTabId !== activeTab?.id)
  );
  let activeEnvironmentVarCount: number | null = $derived(
    activeEnvironmentDetail
      ? computeActiveEnvironmentVarCount(activeEnvironmentDetail)
      : cachedActiveEnvironmentVarCount
  );
  let isSyncingRequestFromWorkspace = false;
  let requestOwnerTabId = "";
  let lastSyncedCollectionId = "";
  let lastHandledRequestedSavedRequestId = "";

  const savedRequestRoute = createStaleGuard();

  const openApiRequestImportPlaceholder = `openapi: 3.0.3
info:
  title: Example API
paths:
  /items:
    get:
      summary: List items`;

  type RedactionDetail = {
    field: string;
    reason: string;
  };

  type RequestExportBuild = {
    source: string;
    redactions: RedactionDetail[];
  };

  const REDACTED_EXPORT_VALUE = "{{redacted}}";

  let requestExportBuild = $derived(buildRequestExportSource(request, requestExportFormat, requestExportSafety));
  let requestExportSource = $derived(requestExportBuild.source);
  let requestExportRedactions = $derived(requestExportBuild.redactions);

  function shellQuote(value: string) {
    if (value.length === 0) {
      return "''";
    }

    return `'${value.replace(/'/g, "'\\''")}'`;
  }

  function hasHeader(requestDraft: RequestDraft, headerName: string) {
    return requestDraft.headers.some(
      (header) => header.enabled && header.key.trim().toLowerCase() === headerName.toLowerCase()
    );
  }

  function buildUrlWithQueryParams(requestDraft: RequestDraft, redactions?: RedactionDetail[]) {
    const baseUrl = redactions ? redactUrlQueryString(requestDraft.url, redactions) : requestDraft.url;
    const activeQueryRows = requestDraft.queryParams.filter((row) => row.enabled && row.key.trim());
    const apiKeyQueryRows =
      requestDraft.auth.type === "api-key" &&
      requestDraft.auth.apiKeyIn === "query" &&
      requestDraft.auth.apiKeyName.trim()
        ? [
            {
              key: requestDraft.auth.apiKeyName,
              value: redactions
                ? redactValue(requestDraft.auth.apiKeyValue, "API key query parameter", "API key values are credentials.", redactions)
                : requestDraft.auth.apiKeyValue
            }
          ]
        : [];
    const queryRows = [
      ...activeQueryRows.map((row) => ({
        key: row.key,
        value: redactions && isSensitiveKey(row.key)
          ? redactValue(row.value, `Query parameter "${row.key.trim()}"`, "The parameter name looks like a token, key, secret, or password.", redactions)
          : row.value
      })),
      ...apiKeyQueryRows
    ];

    if (queryRows.length === 0) {
      return baseUrl;
    }

    const hashIndex = baseUrl.indexOf("#");
    const hash = hashIndex >= 0 ? baseUrl.slice(hashIndex) : "";
    const beforeHash = hashIndex >= 0 ? baseUrl.slice(0, hashIndex) : baseUrl;
    const separator = beforeHash.includes("?") ? "&" : "?";
    const queryString = queryRows
      .map((row) => `${row.key}${row.value.length > 0 ? `=${row.value}` : ""}`)
      .join("&");

    return `${beforeHash}${separator}${queryString}${hash}`;
  }

  function addRedaction(redactions: RedactionDetail[], field: string, reason: string) {
    if (!redactions.some((item) => item.field === field && item.reason === reason)) {
      redactions.push({ field, reason });
    }
  }

  function redactValue(value: string, field: string, reason: string, redactions: RedactionDetail[]) {
    if (value.length === 0) {
      return value;
    }

    addRedaction(redactions, field, reason);
    return REDACTED_EXPORT_VALUE;
  }

  function normalizedSecretName(value: string) {
    return value.trim().toLowerCase().replace(/[^a-z0-9]/g, "");
  }

  function isSensitiveKey(value: string) {
    const normalized = normalizedSecretName(value);
    if (!normalized) {
      return false;
    }

    return (
      normalized === "authorization" ||
      normalized === "proxyauthorization" ||
      normalized === "cookie" ||
      normalized === "setcookie" ||
      normalized === "apikey" ||
      normalized === "xapikey" ||
      normalized === "clientsecret" ||
      normalized === "access_token" ||
      normalized.includes("accesstoken") ||
      normalized.includes("apikey") ||
      normalized.includes("secret") ||
      normalized.includes("token") ||
      normalized.includes("password") ||
      normalized.includes("passwd")
    );
  }

  function decodeKeyForRedaction(value: string) {
    try {
      return decodeURIComponent(value.replace(/\+/g, " "));
    } catch {
      return value;
    }
  }

  function redactUrlQueryString(url: string, redactions: RedactionDetail[]) {
    const hashIndex = url.indexOf("#");
    const hash = hashIndex >= 0 ? url.slice(hashIndex) : "";
    const beforeHash = hashIndex >= 0 ? url.slice(0, hashIndex) : url;
    const queryIndex = beforeHash.indexOf("?");
    if (queryIndex < 0) {
      return url;
    }

    const base = beforeHash.slice(0, queryIndex);
    const query = beforeHash.slice(queryIndex + 1);
    const redactedQuery = query
      .split("&")
      .map((part) => {
        const separatorIndex = part.indexOf("=");
        const key = separatorIndex >= 0 ? part.slice(0, separatorIndex) : part;
        const value = separatorIndex >= 0 ? part.slice(separatorIndex + 1) : "";
        if (!isSensitiveKey(decodeKeyForRedaction(key)) || value.length === 0) {
          return part;
        }

        addRedaction(redactions, `URL query parameter "${decodeKeyForRedaction(key)}"`, "The URL parameter name looks like a token, key, secret, or password.");
        return `${key}=${encodeURIComponent(REDACTED_EXPORT_VALUE)}`;
      })
      .join("&");

    return `${base}?${redactedQuery}${hash}`;
  }

  function redactHeaderValue(headerName: string, value: string, redactions: RedactionDetail[]) {
    const normalized = normalizedSecretName(headerName);
    if (normalized === "authorization" || normalized === "proxyauthorization") {
      return redactValue(value, `Header "${headerName.trim()}"`, "Authorization headers often contain bearer tokens or basic credentials.", redactions);
    }
    if (normalized === "cookie" || normalized === "setcookie") {
      return redactValue(value, `Header "${headerName.trim()}"`, "Cookies can carry session secrets.", redactions);
    }
    if (isSensitiveKey(headerName)) {
      return redactValue(value, `Header "${headerName.trim()}"`, "The header name looks like a token, key, secret, or password.", redactions);
    }
    return value;
  }

  function redactJsonSecrets(value: unknown, redactions: RedactionDetail[], path = "Body JSON"): unknown {
    if (Array.isArray(value)) {
      return value.map((item, index) => redactJsonSecrets(item, redactions, `${path}[${index}]`));
    }

    if (value && typeof value === "object") {
      return Object.fromEntries(
        Object.entries(value).map(([key, item]) => {
          if (isSensitiveKey(key) && item !== null && item !== undefined && String(item).length > 0) {
            addRedaction(redactions, `${path}.${key}`, "The JSON property name looks like a token, key, secret, or password.");
            return [key, REDACTED_EXPORT_VALUE];
          }
          return [key, redactJsonSecrets(item, redactions, `${path}.${key}`)];
        })
      );
    }

    return value;
  }

  function redactRawBody(raw: string, redactions: RedactionDetail[]) {
    if (!raw.trim()) {
      return raw;
    }

    try {
      return JSON.stringify(redactJsonSecrets(JSON.parse(raw), redactions), null, 2);
    } catch {
      return redactUrlEncodedRawBody(raw, redactions);
    }
  }

  function redactUrlEncodedRawBody(raw: string, redactions: RedactionDetail[]) {
    const trimmed = raw.trim();
    if (!trimmed.includes("=") || /[\r\n{}]/.test(trimmed)) {
      return raw;
    }

    return raw
      .split("&")
      .map((part) => {
        const separatorIndex = part.indexOf("=");
        if (separatorIndex < 0) {
          return part;
        }

        const key = part.slice(0, separatorIndex);
        const value = part.slice(separatorIndex + 1);
        if (!isSensitiveKey(decodeKeyForRedaction(key)) || value.length === 0) {
          return part;
        }

        addRedaction(redactions, `Raw body field "${decodeKeyForRedaction(key)}"`, "The field name looks like a token, key, secret, or password.");
        return `${key}=${encodeURIComponent(REDACTED_EXPORT_VALUE)}`;
      })
      .join("&");
  }

  function buildRedactedRequestDraft(requestDraft: RequestDraft) {
    const redactions: RedactionDetail[] = [];
    const redacted = cloneRequestDraft(requestDraft);
    redacted.url = redactUrlQueryString(redacted.url, redactions);

    redacted.headers = redacted.headers.map((header) =>
      header.enabled && header.key.trim()
        ? { ...header, value: redactHeaderValue(header.key, header.value, redactions) }
        : header
    );

    redacted.queryParams = redacted.queryParams.map((row) =>
      row.enabled && row.key.trim() && isSensitiveKey(row.key)
        ? {
            ...row,
            value: redactValue(row.value, `Query parameter "${row.key.trim()}"`, "The parameter name looks like a token, key, secret, or password.", redactions)
          }
        : row
    );

    redacted.auth = {
      ...redacted.auth,
      basicPassword: redactValue(redacted.auth.basicPassword, "Basic auth password", "Basic-auth passwords are credentials.", redactions),
      bearerToken: redactValue(redacted.auth.bearerToken, "Bearer token", "Bearer tokens grant API access.", redactions),
      apiKeyValue: redactValue(redacted.auth.apiKeyValue, "API key value", "API key values are credentials.", redactions),
      oauth2AccessToken: redactValue(redacted.auth.oauth2AccessToken, "OAuth2 access token", "OAuth2 access tokens grant API access.", redactions),
      oauth2ClientSecret: redactValue(redacted.auth.oauth2ClientSecret, "OAuth2 client secret", "OAuth2 client secrets are credentials.", redactions)
    };

    redacted.body = {
      ...redacted.body,
      raw: redacted.body.mode === "json" || redacted.body.mode === "raw"
        ? redactRawBody(redacted.body.raw, redactions)
        : redacted.body.raw,
      form: redacted.body.form.map((row) =>
        row.enabled && row.key.trim() && isSensitiveKey(row.key)
          ? {
              ...row,
              value: redactValue(row.value, `Body field "${row.key.trim()}"`, "The field name looks like a token, key, secret, or password.", redactions)
            }
          : row
      )
    };

    return { request: redacted, redactions };
  }

  function buildCurlExport(requestDraft: RequestDraft, safety: "redacted" | "full", redactions: RedactionDetail[]) {
    const lines = ["curl"];
    lines.push(`  --request ${shellQuote(requestDraft.method)}`);
    lines.push(`  --url ${shellQuote(buildUrlWithQueryParams(requestDraft, safety === "redacted" ? redactions : undefined))}`);

    for (const header of requestDraft.headers.filter((row) => row.enabled && row.key.trim())) {
      const headerValue = safety === "redacted" ? redactHeaderValue(header.key, header.value, redactions) : header.value;
      lines.push(`  --header ${shellQuote(`${header.key.trim()}: ${headerValue}`)}`);
    }

    switch (requestDraft.auth.type) {
      case "basic":
        if (requestDraft.auth.basicUsername.trim() || requestDraft.auth.basicPassword.length > 0) {
          const password = safety === "redacted"
            ? redactValue(requestDraft.auth.basicPassword, "Basic auth password", "Basic-auth passwords are credentials.", redactions)
            : requestDraft.auth.basicPassword;
          lines.push(
            `  --user ${shellQuote(`${requestDraft.auth.basicUsername}:${password}`)}`
          );
        }
        break;
      case "bearer":
        if (requestDraft.auth.bearerToken.trim()) {
          const token = safety === "redacted"
            ? redactValue(requestDraft.auth.bearerToken, "Bearer token", "Bearer tokens grant API access.", redactions)
            : requestDraft.auth.bearerToken;
          lines.push(`  --header ${shellQuote(`Authorization: Bearer ${token}`)}`);
        }
        break;
      case "oauth2":
        if (requestDraft.auth.oauth2AccessToken.trim()) {
          const token = safety === "redacted"
            ? redactValue(requestDraft.auth.oauth2AccessToken, "OAuth2 access token", "OAuth2 access tokens grant API access.", redactions)
            : requestDraft.auth.oauth2AccessToken;
          lines.push(`  --header ${shellQuote(`Authorization: Bearer ${token}`)}`);
        }
        break;
      case "api-key":
        if (
          requestDraft.auth.apiKeyIn === "header" &&
          requestDraft.auth.apiKeyName.trim()
        ) {
          const keyValue = safety === "redacted"
            ? redactValue(requestDraft.auth.apiKeyValue, "API key header", "API key values are credentials.", redactions)
            : requestDraft.auth.apiKeyValue;
          lines.push(
            `  --header ${shellQuote(`${requestDraft.auth.apiKeyName.trim()}: ${keyValue}`)}`
          );
        }
        break;
    }

    switch (requestDraft.body.mode) {
      case "json":
        if (!hasHeader(requestDraft, "Content-Type")) {
          lines.push(`  --header ${shellQuote("Content-Type: application/json")}`);
        }
        if (requestDraft.body.raw.length > 0) {
          lines.push(`  --data-raw ${shellQuote(safety === "redacted" ? redactRawBody(requestDraft.body.raw, redactions) : requestDraft.body.raw)}`);
        }
        break;
      case "raw":
        if (requestDraft.body.raw.length > 0) {
          lines.push(`  --data-raw ${shellQuote(safety === "redacted" ? redactRawBody(requestDraft.body.raw, redactions) : requestDraft.body.raw)}`);
        }
        break;
      case "form-urlencoded":
        for (const field of requestDraft.body.form.filter((row) => row.enabled && row.key.trim())) {
          const value = safety === "redacted" && isSensitiveKey(field.key)
            ? redactValue(field.value, `Body field "${field.key.trim()}"`, "The field name looks like a token, key, secret, or password.", redactions)
            : field.value;
          lines.push(`  --data-urlencode ${shellQuote(`${field.key}=${value}`)}`);
        }
        break;
      case "multipart":
        for (const field of requestDraft.body.form.filter((row) => row.enabled && row.key.trim())) {
          const value = safety === "redacted" && isSensitiveKey(field.key)
            ? redactValue(field.value, `Body field "${field.key.trim()}"`, "The field name looks like a token, key, secret, or password.", redactions)
            : field.value;
          lines.push(`  --form ${shellQuote(`${field.key}=${value}`)}`);
        }
        for (const file of requestDraft.body.files.filter((row) => row.enabled && row.name.trim() && row.path.trim())) {
          lines.push(`  --form ${shellQuote(`${file.name}=@${file.path}`)}`);
        }
        break;
    }

    return lines.map((line, index) => (index === lines.length - 1 ? line : `${line} \\`)).join("\n");
  }

  function buildRequestExportSource(requestDraft: RequestDraft, format: "curl" | "json", safety: "redacted" | "full"): RequestExportBuild {
    const redactedExport = safety === "redacted" ? buildRedactedRequestDraft(requestDraft) : { request: cloneRequestDraft(requestDraft), redactions: [] };

    if (format === "json") {
      return {
        source: JSON.stringify(redactedExport.request, null, 2),
        redactions: redactedExport.redactions
      };
    }

    const redactions: RedactionDetail[] = [];
    return {
      source: buildCurlExport(requestDraft, safety, redactions),
      redactions
    };
  }

  onMount(() => {
    void initializePage();
  });

  $effect(() => {
    const nextActiveTab = activeTab;
    if (!nextActiveTab) {
      return;
    }

    if (nextActiveTab.id === requestOwnerTabId) {
      return;
    }

    if (requestOwnerTabId) {
      const previousTab = getTabById(requestOwnerTabId);
      if (previousTab && !requestEquals(previousTab.request, request)) {
        requestWorkspace.updateTabRequest(requestOwnerTabId, request);
      }
    }

    isSyncingRequestFromWorkspace = true;
    requestOwnerTabId = nextActiveTab.id;
    request = cloneRequestDraft(nextActiveTab.request);
  });

  $effect(() => {
    const nextActiveTab = activeTab;
    if (!nextActiveTab) {
      return;
    }

    if (isSyncingRequestFromWorkspace) {
      isSyncingRequestFromWorkspace = false;
      return;
    }

    if (nextActiveTab.id !== requestOwnerTabId) {
      return;
    }

    if (!requestEquals(nextActiveTab.request, request)) {
      requestWorkspace.updateTabRequest(requestOwnerTabId, request);
    }
  });

  $effect(() => {
    const requestedId = requestedSavedRequestId;
    if (!requestWorkspace.initialized) {
      return;
    }

    if (requestedId === lastHandledRequestedSavedRequestId) {
      return;
    }

    lastHandledRequestedSavedRequestId = requestedId;

    if (!requestedId) {
      return;
    }

    if (activeTab?.savedRequestId === requestedId) {
      if (activeTab) {
        bumpRequestTabsScrollIntoView(activeTab.id);
      }
      return;
    }

    void openSavedRequestFromRoute(requestedId);
  });

  $effect(() => {
    const collectionId = activeTab?.collectionId ?? "";
    if (!collectionId) {
      lastSyncedCollectionId = "";
      return;
    }

    if (collectionId === lastSyncedCollectionId) {
      return;
    }

    lastSyncedCollectionId = collectionId;
    void syncActiveCollection(collectionId);
  });

  async function initializePage() {
    await Promise.all([loadSettings(), loadHistory(), collections.ensureLoaded(), loadEnvironments(), requestWorkspace.ensureInitialized()]);

    if (requestedSavedRequestId) {
      await openSavedRequestFromRoute(requestedSavedRequestId);
      return;
    }

    await syncRouteToActiveTab();
  }

  function requestEquals(left: RequestDraft, right: RequestDraft) {
    return JSON.stringify(left) === JSON.stringify(right);
  }

  function getTabById(tabId: string) {
    return requestWorkspace.tabs.find((tab) => tab.id === tabId) ?? null;
  }

  async function syncRouteToActiveTab() {
    const savedRequestId = activeTab?.savedRequestId ?? "";
    const currentSavedRequestId = page.url.searchParams.get("savedRequestId") ?? "";

    if (savedRequestId === currentSavedRequestId) {
      return;
    }

    const gotoOpts = {
      replaceState: true,
      noScroll: true,
      keepFocus: true
    } as const;

    if (savedRequestId) {
      await goto(resolve(`/?savedRequestId=${encodeURIComponent(savedRequestId)}`), gotoOpts);
    } else {
      await goto(resolve("/"), gotoOpts);
    }
  }

  async function syncActiveCollection(collectionId: string) {
    await collections.ensureLoaded(collectionId);
    await collections.selectCollection(collectionId);
  }

  async function loadSettings() {
    try {
      settings = await getSettings();
      writeCachedJson(UI_CACHE_KEYS.settings, settings);
      settingsErrorText = "";
    } catch (error) {
      settingsErrorText = error instanceof Error ? error.message : String(error);
    }
  }

  function isPrimarySaveShortcut(event: KeyboardEvent) {
    return (event.ctrlKey || event.metaKey) && !event.shiftKey && !event.altKey && event.key.toLowerCase() === "s";
  }

  async function handleHistoryCollapsedChange(isCollapsed: boolean) {
    if (isHistoryCollapseSaving || settings.isHistoryCollapsed === isCollapsed) {
      return;
    }

    const previousSettings = settings;
    settings = {
      ...settings,
      isHistoryCollapsed: isCollapsed
    };
    writeCachedJson(UI_CACHE_KEYS.settings, settings);
    isHistoryCollapseSaving = true;

    try {
      settings = await updateSettings(settings);
      writeCachedJson(UI_CACHE_KEYS.settings, settings);
      settingsErrorText = "";
    } catch (error) {
      settings = previousSettings;
      writeCachedJson(UI_CACHE_KEYS.settings, previousSettings);
      settingsErrorText = error instanceof Error ? error.message : String(error);
      notifications.error(settingsErrorText, "History preference not saved");
    } finally {
      isHistoryCollapseSaving = false;
    }
  }

  async function loadHistory() {
    isHistoryLoading = true;

    try {
      history = await listHistory(12);
      historyErrorText = "";

      if (selectedHistoryId && !history.some((entry) => entry.id === selectedHistoryId)) {
        closeHistoryDetail();
      }
    } catch (error) {
      historyErrorText = error instanceof Error ? error.message : String(error);
    } finally {
      isHistoryLoading = false;
    }
  }

  async function loadEnvironments(preferredEnvironmentId = activeEnvironmentId) {
    isEnvironmentsLoading = true;

    try {
      environments = await listEnvironments();
      const activeEnvironment = environments.find((environment) => environment.isActive) ?? null;
      activeEnvironmentId = activeEnvironment?.id ?? "";
      environmentsErrorText = "";

      writeCachedJson(UI_CACHE_KEYS.environmentsList, environments);
      writeCachedJson(UI_CACHE_KEYS.environmentsActiveId, activeEnvironmentId);

      const detailEnvironmentId =
        preferredEnvironmentId && environments.some((environment) => environment.id === preferredEnvironmentId)
          ? preferredEnvironmentId
          : activeEnvironment?.id ?? "";

      if (detailEnvironmentId) {
        activeEnvironmentDetail = await getEnvironment(detailEnvironmentId);
      } else {
        activeEnvironmentDetail = null;
      }

      cacheActiveEnvironmentVarCount(activeEnvironmentDetail);
    } catch (error) {
      environmentsErrorText = error instanceof Error ? error.message : String(error);
      activeEnvironmentDetail = null;
      cacheActiveEnvironmentVarCount(null);
    } finally {
      isEnvironmentsLoading = false;
    }
  }

  function computeActiveEnvironmentVarCount(detail: EnvironmentDetail | null): number | null {
    if (!detail) {
      return null;
    }
    return detail.variables.filter((variable) => variable.enabled && variable.key.trim()).length;
  }

  function cacheActiveEnvironmentVarCount(detail: EnvironmentDetail | null) {
    const count = computeActiveEnvironmentVarCount(detail);
    cachedActiveEnvironmentVarCount = count;
    if (count === null) {
      writeCachedJson(UI_CACHE_KEYS.environmentsActiveVarCount, null);
    } else {
      writeCachedJson(UI_CACHE_KEYS.environmentsActiveVarCount, count);
    }
  }

  async function inspectHistoryEntry(id: string, shouldKeepExistingDetail = false) {
    const scrollY = window.scrollY;
    selectedHistoryId = id;
    isHistoryDetailLoading = true;
    historyDetailErrorText = "";

    if (!shouldKeepExistingDetail) {
      selectedHistoryDetail = null;
    }

    try {
      selectedHistoryDetail = await getHistoryEntry(id);
    } catch (error) {
      selectedHistoryDetail = null;
      historyDetailErrorText = error instanceof Error ? error.message : String(error);
    } finally {
      isHistoryDetailLoading = false;
      await tick();
      window.scrollTo({ top: scrollY });
    }
  }

  function closeHistoryDetail() {
    selectedHistoryId = "";
    selectedHistoryDetail = null;
    historyDetailErrorText = "";
    isHistoryDetailLoading = false;
  }

  async function handleEnvironmentChange(environmentId: string) {
    isEnvironmentChanging = true;

    try {
      await setActiveEnvironment(environmentId || null);
      await loadEnvironments(environmentId);
      if (environmentId) {
        const environmentName = environments.find((environment) => environment.id === environmentId)?.name ?? "Environment";
        notifications.info(environmentName, "Active environment changed");
      } else {
        notifications.info("Requests will now run without an active environment.", "Environment cleared");
      }
    } catch (error) {
      environmentsErrorText = error instanceof Error ? error.message : String(error);
    } finally {
      isEnvironmentChanging = false;
    }
  }

  async function handleClearHistory() {
    if (!window.confirm("Clear all stored request history? This cannot be undone.")) {
      return;
    }

    isClearingHistory = true;

    try {
      await clearHistory();
      closeHistoryDetail();
      await loadHistory();
      historyErrorText = "";
      notifications.success("Stored request history was cleared.", "History cleared");
    } catch (error) {
      historyErrorText = error instanceof Error ? error.message : String(error);
    } finally {
      isClearingHistory = false;
    }
  }

  async function handleRestoreHistoryEntry(id: string) {
    if (!requestWorkspace.initialized || restoringHistoryId) {
      return;
    }

    restoringHistoryId = id;

    try {
      const detail =
        selectedHistoryDetail?.id === id ? selectedHistoryDetail : await getHistoryEntry(id);
      const openedTab = requestWorkspace.openHistoryRequest(detail.requestSnapshot);
      bumpRequestTabsScrollIntoView(openedTab.id);
      await syncRouteToActiveTab();

      const restoredLabel = detail.requestSnapshot.name.trim() || detail.url;
      notifications.success(`${restoredLabel} is now open in a new request tab.`, "Request restored");
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      notifications.error(message, "Restore failed");
    } finally {
      restoringHistoryId = "";
    }
  }

  async function persistActiveEnvironmentFromScript(nextEnvironment: EnvironmentDetail): Promise<EnvironmentDetail> {
    const updated = await updateEnvironment(nextEnvironment.id, {
      name: nextEnvironment.name.trim(),
      variables: nextEnvironment.variables
    });

    activeEnvironmentDetail = updated;
    activeEnvironmentId = updated.id;
    cacheActiveEnvironmentVarCount(updated);
    writeCachedJson(UI_CACHE_KEYS.environmentsActiveId, activeEnvironmentId);
    environments = environments.map((environment) =>
      environment.id === updated.id
        ? {
            ...environment,
            name: updated.name,
            isActive: updated.isActive,
            variableCount: updated.variables.length,
            updatedAt: updated.updatedAt
          }
        : environment
    );
    writeCachedJson(UI_CACHE_KEYS.environmentsList, environments);

    return updated;
  }

  function responseSnippet(value: string) {
    const normalized = value.trim().replace(/\s+/g, " ");
    if (!normalized) {
      return "";
    }

    return normalized.length > 240 ? `${normalized.slice(0, 240)}...` : normalized;
  }

  async function handleFetchOAuth2Token(options: { persistToEnvironment: boolean }) {
    const auth = request.auth;
    const tokenUrl = auth.oauth2TokenUrl.trim();
    const clientId = auth.oauth2ClientId.trim();
    const clientSecret = auth.oauth2ClientSecret.trim();
    const scope = auth.oauth2Scope.trim();

    if (!tokenUrl || !clientId || !clientSecret) {
      throw new Error("Token URL, client ID, and client secret are required.");
    }

    if (options.persistToEnvironment && !activeEnvironmentDetail) {
      throw new Error("Activate an environment before saving the OAuth2 token.");
    }

    const tokenRequest: RequestDraft = {
      ...createRequestDraft(),
      name: "OAuth2 token refresh",
      method: "POST",
      url: tokenUrl,
      queryParams: [],
      headers: [
        {
          id: "oauth2-accept",
          key: "Accept",
          value: "application/json",
          enabled: true
        }
      ],
      body: {
        mode: "form-urlencoded",
        raw: "",
        form: [
          { id: "oauth2-grant-type", key: "grant_type", value: "client_credentials", enabled: true },
          { id: "oauth2-client-id", key: "client_id", value: clientId, enabled: true },
          { id: "oauth2-client-secret", key: "client_secret", value: clientSecret, enabled: true },
          { id: "oauth2-scope", key: "scope", value: scope, enabled: Boolean(scope) }
        ],
        files: []
      }
    };

    const sendResult = await sendRequest(tokenRequest, { persistHistory: false });
    const { response } = sendResult;

    if (response.errorText) {
      throw new Error(response.errorText);
    }

    if (!response.statusCode || response.statusCode < 200 || response.statusCode >= 300) {
      const details = responseSnippet(response.bodyText);
      throw new Error(
        `Token endpoint returned ${response.statusCode ?? "no status"}${details ? `: ${details}` : "."}`
      );
    }

    let tokenBody: unknown;
    try {
      tokenBody = JSON.parse(response.bodyText);
    } catch {
      throw new Error("Token endpoint did not return JSON.");
    }

    if (!tokenBody || typeof tokenBody !== "object" || !("access_token" in tokenBody)) {
      throw new Error("Token response did not include access_token.");
    }

    const accessToken = String((tokenBody as { access_token: unknown }).access_token ?? "");
    if (!accessToken.trim()) {
      throw new Error("Token response returned an empty access_token.");
    }

    const expiresInValue = (tokenBody as { expires_in?: unknown }).expires_in;
    const expiresIn = typeof expiresInValue === "number" && Number.isFinite(expiresInValue) ? expiresInValue : null;
    const tokenTypeValue = (tokenBody as { token_type?: unknown }).token_type;
    const tokenType = typeof tokenTypeValue === "string" ? tokenTypeValue : "Bearer";

    if (options.persistToEnvironment && activeEnvironmentDetail) {
      const existingIndex = activeEnvironmentDetail.variables.findIndex(
        (variable) => variable.key.trim() === "oauth_access_token"
      );
      const nextVariables =
        existingIndex >= 0
          ? activeEnvironmentDetail.variables.map((variable, index) =>
              index === existingIndex
                ? {
                    ...variable,
                    value: accessToken,
                    enabled: true,
                    isSecret: true
                  }
                : variable
            )
          : [
              ...activeEnvironmentDetail.variables,
              {
                ...createEnvironmentVariable(),
                key: "oauth_access_token",
                value: accessToken,
                enabled: true,
                isSecret: true
              }
            ];

      await persistActiveEnvironmentFromScript({
        ...activeEnvironmentDetail,
        variables: nextVariables
      });
      notifications.success("Saved as {{oauth_access_token}} in the active environment.", "OAuth2 token fetched");

      return {
        accessToken,
        persistedToEnvironment: true,
        expiresIn,
        tokenType
      };
    }

    notifications.success("The token was placed in this request's OAuth2 access token field.", "OAuth2 token fetched");

    return {
      accessToken,
      persistedToEnvironment: false,
      expiresIn,
      tokenType
    };
  }

  function activeCollectionScripts(tab: RequestWorkspaceTab): InheritedRequestScripts | null {
    if (!tab.collectionId) {
      return null;
    }

    const collection = collections.collections.find((item) => item.id === tab.collectionId);
    if (!collection) {
      return null;
    }

    return {
      preRequestScript: collection.preRequestScript,
      testScript: collection.testScript,
      folderScripts: folderScriptPath(
        collections.collectionItemsByCollection[tab.collectionId] ?? [],
        tab.parentId ?? null
      )
    };
  }

  function folderScriptPath(
    items: CollectionItemSummary[],
    targetFolderId: string | null
  ): InheritedRequestScripts["folderScripts"] {
    if (!targetFolderId) {
      return [];
    }

    const path = findFolderPath(items, targetFolderId);
    return path.map((folder) => ({
      name: folder.name,
      preRequestScript: folder.preRequestScript,
      testScript: folder.testScript
    }));
  }

  function findFolderPath(items: CollectionItemSummary[], targetFolderId: string): CollectionItemSummary[] {
    for (const item of items) {
      if (item.kind !== "folder") {
        continue;
      }

      if (item.id === targetFolderId) {
        return [item];
      }

      const childPath = findFolderPath(item.children, targetFolderId);
      if (childPath.length > 0) {
        return [item, ...childPath];
      }
    }

    return [];
  }

  function bumpRequestTabsScrollIntoView(tabId?: string) {
    const id = tabId ?? requestWorkspace.activeTabId;
    requestTabsScrollRequest = {
      n: requestTabsScrollRequest.n + 1,
      tabId: id
    };
  }

  async function openSavedRequestFromRoute(itemId: string) {
    const existingTab = requestWorkspace.findTabBySavedRequestId(itemId);
    if (existingTab) {
      if (existingTab.id !== requestWorkspace.activeTabId) {
        requestWorkspace.activateTab(existingTab.id);
      }
      bumpRequestTabsScrollIntoView(existingTab.id);
      return;
    }

    const seq = savedRequestRoute.next();

    try {
      const savedRequest = await getSavedRequest(itemId);
      if (savedRequestRoute.isStale(seq)) {
        return;
      }

      const openedTab = requestWorkspace.openSavedRequest(savedRequest);
      bumpRequestTabsScrollIntoView(openedTab.id);
    } catch (error) {
      if (savedRequestRoute.isStale(seq)) {
        return;
      }

      const message = error instanceof Error ? error.message : String(error);
      if (activeTab) {
        requestWorkspace.setTabError(activeTab.id, message);
      } else {
        notifications.error(message, "Request load failed");
      }
    }
  }

  async function handleSend() {
    if (!requestWorkspace.initialized) {
      return;
    }

    const tab = activeTab ?? getTabById(requestOwnerTabId);
    if (!tab || requestWorkspace.inFlightTabId) {
      return;
    }

    const tabId = tab.id;
    const requestToSend = cloneRequestDraft(request);
    requestWorkspace.clearTabError(tabId);
    requestWorkspace.markSendStarted(tabId);

    try {
      const inheritedScripts = activeCollectionScripts(tab);
      const preparedRequest = await runPreRequestScript(
        requestToSend,
        activeEnvironmentDetail?.variables ?? [],
        inheritedScripts,
        {
          activeEnvironment: activeEnvironmentDetail,
          persistActiveEnvironment: persistActiveEnvironmentFromScript
        }
      );

      if (preparedRequest.errorText) {
        const execution = {
          ...createEmptyRequestScriptExecution(),
          preRequestErrorText: preparedRequest.errorText
        };
        requestWorkspace.setTabResponse(tabId, {
          statusCode: null,
          statusText: "Pre-request script failed",
          durationMs: 0,
          sizeBytes: 0,
          headers: [],
          bodyText: "",
          errorText: "",
          executedAt: new Date().toISOString()
        }, execution);
        return;
      }

      const sendResult = await sendRequest(preparedRequest.request);
      const scriptExecution = await runTestScript(
        requestToSend,
        sendResult.response,
        activeEnvironmentDetail?.variables ?? [],
        inheritedScripts,
        {
          activeEnvironment: activeEnvironmentDetail,
          persistActiveEnvironment: persistActiveEnvironmentFromScript
        }
      );

      requestWorkspace.setTabResponse(tabId, sendResult.response, scriptExecution);

      if (scriptExecution.testScriptErrorText) {
        notifications.warning(
          `The response was received, but the test script stopped early: ${scriptExecution.testScriptErrorText}`,
          "Test script error"
        );
      } else if (scriptExecution.tests.some((test) => test.status === "failed")) {
        const failedCount = scriptExecution.tests.filter((test) => test.status === "failed").length;
        notifications.warning(
          `${failedCount} scripted test${failedCount === 1 ? "" : "s"} failed for this response.`,
          "Tests failed"
        );
      }

      if (sendResult.historyPersistenceError) {
        notifications.warning(
          `The response is shown, but this run was not saved to history: ${sendResult.historyPersistenceError}`,
          "History not saved"
        );
      }
    } catch (error) {
      const errorText = error instanceof Error ? error.message : String(error);

      requestWorkspace.setTabResponse(tabId, {
        statusCode: null,
        statusText: errorText === "Request canceled." ? "Request canceled" : "Request failed",
        durationMs: 0,
        sizeBytes: 0,
        headers: [],
        bodyText: "",
        errorText,
        executedAt: new Date().toISOString()
      });
    } finally {
      requestWorkspace.markSendFinished(tabId);
      await loadHistory();

      if (selectedHistoryId) {
        await inspectHistoryEntry(selectedHistoryId, true);
      }
    }
  }

  async function handleCancelRequest() {
    if (!requestWorkspace.inFlightTabId || requestWorkspace.isCanceling || requestWorkspace.inFlightTabId !== activeTab?.id) {
      return;
    }

    requestWorkspace.markCanceling();

    try {
      await cancelActiveRequest();
    } catch {
      requestWorkspace.isCanceling = false;
    }
  }

  async function handleSaveRequest() {
    if (!requestWorkspace.initialized) {
      return;
    }

    const tab = activeTab ?? getTabById(requestOwnerTabId);
    if (!tab) {
      return;
    }

    const requestToSave = cloneRequestDraft(request);
    requestWorkspace.clearTabError(tab.id);
    collections.resetError();

    const hasSavedRequest =
      !!tab.savedRequestId &&
      !!tab.collectionId &&
      collections.collections.some((collection) => collection.id === tab.collectionId);

    if (hasSavedRequest) {
      const savedRequest = await collections.updateExistingSavedRequest(tab.savedRequestId!, tab.collectionId!, requestToSave);

      if (!savedRequest) {
        requestWorkspace.setTabError(tab.id, collections.errorText);
        return;
      }

      requestWorkspace.setTabSaved(tab.id, savedRequest, requestToSave);
      await syncRouteToActiveTab();
      return;
    }

    if (collections.collections.length === 0) {
      requestWorkspace.setTabError(tab.id, "Create a collection first from the sidebar.");
      return;
    }

    saveDialogMode = "replace-tab";
    saveDialogTabId = tab.id;
    saveTargetCollectionId = collections.selectedCollectionId || tab.collectionId || collections.collections[0].id;
    saveTargetParentId = tab.parentId ?? null;
    isSaveDialogOpen = true;
  }

  async function handleSaveAsNewRequest() {
    if (!requestWorkspace.initialized) {
      return;
    }

    const tab = activeTab ?? getTabById(requestOwnerTabId);
    if (!tab) {
      return;
    }

    requestWorkspace.clearTabError(tab.id);
    collections.resetError();

    if (collections.collections.length === 0) {
      requestWorkspace.setTabError(tab.id, "Create a collection first from the sidebar.");
      return;
    }

    saveDialogMode = "save-as";
    saveDialogTabId = tab.id;
    saveTargetCollectionId = collections.selectedCollectionId || tab.collectionId || collections.collections[0].id;
    saveTargetParentId = tab.parentId ?? null;
    isSaveDialogOpen = true;
  }

  async function confirmSaveRequest() {
    if (!saveTargetCollectionId) {
      const saveTab = getTabById(saveDialogTabId);
      if (saveTab) {
        requestWorkspace.setTabError(saveTab.id, "Choose a collection first.");
      }
      return;
    }

    const saveTab = getTabById(saveDialogTabId);
    if (!saveTab) {
      closeSaveDialog();
      return;
    }

    const draftToSave =
      saveDialogTabId === requestOwnerTabId ? cloneRequestDraft(request) : cloneRequestDraft(saveTab.request);

    if (saveDialogMode === "save-as") {
      const savedSummary = await collections.saveNewRequest(saveTargetCollectionId, draftToSave, saveTargetParentId);

      if (!savedSummary) {
        requestWorkspace.setTabError(saveTab.id, collections.errorText);
        return;
      }

      requestWorkspace.setTabSaved(saveTab.id, savedSummary, draftToSave);
      bumpRequestTabsScrollIntoView(saveTab.id);

      isSaveDialogOpen = false;
      saveDialogMode = "replace-tab";
      saveDialogTabId = "";
      saveTargetParentId = null;
      await collections.selectCollection(savedSummary.collectionId);
      await syncRouteToActiveTab();
      return;
    }

    const savedRequest = await collections.saveNewRequest(saveTargetCollectionId, draftToSave, saveTargetParentId);

    if (!savedRequest) {
      requestWorkspace.setTabError(saveTab.id, collections.errorText);
      return;
    }

    requestWorkspace.setTabSaved(saveTab.id, savedRequest, draftToSave);
    isSaveDialogOpen = false;
    saveDialogMode = "replace-tab";
    saveDialogTabId = "";
    await collections.selectCollection(savedRequest.collectionId);
    await syncRouteToActiveTab();
  }

  function closeSaveDialog() {
    isSaveDialogOpen = false;
    saveDialogMode = "replace-tab";
    saveDialogTabId = "";
    saveTargetParentId = null;
  }

  async function handleNewRequest() {
    requestWorkspace.createBlankTab();
    await syncRouteToActiveTab();
  }

  async function handleActivateTab(tabId: string) {
    if (tabId === requestWorkspace.activeTabId) {
      return;
    }

    requestWorkspace.activateTab(tabId);
    await syncRouteToActiveTab();
  }

  async function handleCloseTab(tabId: string) {
    const tab = getTabById(tabId);
    if (!tab) {
      return;
    }

    if (requestWorkspace.inFlightTabId === tabId) {
      notifications.info("Cancel the in-flight request before closing this tab.", "Request still running");
      return;
    }

    if (tabId === requestWorkspace.activeTabId && tabId === requestOwnerTabId) {
      const stored = getTabById(tabId);
      if (stored && !requestEquals(stored.request, request)) {
        requestWorkspace.updateTabRequest(tabId, request);
      }
    }

    const tabForClose = getTabById(tabId) ?? tab;

    if (requestWorkspace.isDirty(tabForClose) && !window.confirm("Close this tab and discard unsaved changes?")) {
      return;
    }

    if (saveDialogTabId === tabId) {
      closeSaveDialog();
    }

    requestWorkspace.closeTab(tabId);
    await syncRouteToActiveTab();
  }

  function openRequestImportDialog() {
    requestImportFormat = "curl";
    curlImportSource = "";
    openApiImportSource = "";
    requestImportErrorText = "";
    isRequestImportDialogOpen = true;
  }

  function closeRequestImportDialog() {
    isRequestImportDialogOpen = false;
    curlImportSource = "";
    openApiImportSource = "";
    requestImportErrorText = "";
  }

  async function openRequestPreviewDialog() {
    isRequestPreviewDialogOpen = true;
    isRequestPreviewLoading = true;
    requestPreview = null;
    requestPreviewErrorText = "";

    try {
      requestPreview = await previewRequest(cloneRequestDraft(request));
    } catch (error) {
      requestPreviewErrorText = error instanceof Error ? error.message : String(error);
    } finally {
      isRequestPreviewLoading = false;
    }
  }

  function closeRequestPreviewDialog() {
    isRequestPreviewDialogOpen = false;
    isRequestPreviewLoading = false;
    requestPreview = null;
    requestPreviewErrorText = "";
  }

  function handleModalBackdropKeydown(event: KeyboardEvent, close: () => void) {
    if (event.target === event.currentTarget && (event.key === "Enter" || event.key === " ")) {
      event.preventDefault();
      close();
    }
  }

  function openRequestExportDialog() {
    requestExportFormat = "curl";
    requestExportSafety = "redacted";
    isRequestExportDialogOpen = true;
  }

  function closeRequestExportDialog() {
    isRequestExportDialogOpen = false;
  }

  async function handleCopyRequestExport() {
    try {
      await navigator.clipboard.writeText(requestExportSource);
      notifications.success(
        requestExportSafety === "redacted" && requestExportRedactions.length > 0
          ? "The redacted export is on your clipboard."
          : "The exported request text is on your clipboard.",
        "Export copied"
      );
    } catch (error) {
      notifications.error(error instanceof Error ? error.message : String(error), "Copy failed");
    }
  }

  function filterPreviewRows(rows: KeyValueRow[]) {
    return rows.filter((row) => row.enabled && (row.key.trim() || row.value.trim()));
  }

  function filterPreviewFiles(rows: RequestPreview["body"]["files"]) {
    return rows.filter((row) => row.enabled && (row.name.trim() || row.path.trim()));
  }

  function previewDisplayValue(value: string) {
    return value || "(empty value)";
  }

  function previewAuthRows(preview: RequestPreview): KeyValueRow[] {
    const auth = preview.auth;

    switch (auth.type) {
      case "basic":
        return [
          { id: "preview-auth-username", key: "Username", value: auth.basicUsername, enabled: Boolean(auth.basicUsername) },
          { id: "preview-auth-password", key: "Password", value: auth.basicPassword, enabled: Boolean(auth.basicPassword) }
        ];
      case "bearer":
        return [
          { id: "preview-auth-bearer", key: "Bearer token", value: auth.bearerToken, enabled: Boolean(auth.bearerToken) }
        ];
      case "api-key":
        return [
          { id: "preview-auth-api-key-name", key: "Key", value: auth.apiKeyName, enabled: Boolean(auth.apiKeyName) },
          { id: "preview-auth-api-key-value", key: "Value", value: auth.apiKeyValue, enabled: Boolean(auth.apiKeyValue) },
          { id: "preview-auth-api-key-placement", key: "Send in", value: auth.apiKeyIn, enabled: true }
        ];
      case "oauth2":
        return [
          { id: "preview-auth-oauth-token", key: "Access token", value: auth.oauth2AccessToken, enabled: Boolean(auth.oauth2AccessToken) },
          { id: "preview-auth-oauth-token-url", key: "Token URL", value: auth.oauth2TokenUrl, enabled: Boolean(auth.oauth2TokenUrl) },
          { id: "preview-auth-oauth-client-id", key: "Client ID", value: auth.oauth2ClientId, enabled: Boolean(auth.oauth2ClientId) },
          { id: "preview-auth-oauth-client-secret", key: "Client secret", value: auth.oauth2ClientSecret, enabled: Boolean(auth.oauth2ClientSecret) },
          { id: "preview-auth-oauth-scope", key: "Scope", value: auth.oauth2Scope, enabled: Boolean(auth.oauth2Scope) }
        ];
      default:
        return [];
    }
  }

  async function handleImportRequest() {
    requestImportErrorText = "";
    const source = requestImportFormat === "curl" ? curlImportSource.trim() : openApiImportSource.trim();
    if (!source) {
      requestImportErrorText =
        requestImportFormat === "curl"
          ? "Paste a complete cURL command to import."
          : "Open an OpenAPI 3 JSON or YAML file, or paste the document payload to import.";
      return;
    }

    isImportingRequest = true;

    try {
      const imported =
        requestImportFormat === "curl"
          ? await importCurlRequestToDraft({ source })
          : await importOpenApiRequestToDraft({ source });

      requestWorkspace.openImportedTab(imported.request);
      closeRequestImportDialog();
      await syncRouteToActiveTab();
      notifications.success(
        requestImportFormat === "curl"
          ? "The imported cURL command is now loaded into a new request tab."
          : "The imported OpenAPI request is now loaded into a new request tab.",
        requestImportFormat === "curl" ? "cURL imported" : "OpenAPI request imported"
      );
    } catch (error) {
      requestImportErrorText = error instanceof Error ? error.message : String(error);
    } finally {
      isImportingRequest = false;
    }
  }

  function handleSaveDialogBackdropKeydown(event: KeyboardEvent) {
    if (event.target === event.currentTarget && (event.key === "Enter" || event.key === " ")) {
      event.preventDefault();
      closeSaveDialog();
    }
  }

  function handleWindowKeydown(event: KeyboardEvent) {
    if (!isPrimarySaveShortcut(event)) {
      return;
    }

    if (isRequestImportDialogOpen || isRequestExportDialogOpen || isRequestPreviewDialogOpen) {
      event.preventDefault();
      return;
    }

    event.preventDefault();

    if (isSaveDialogOpen) {
      void confirmSaveRequest();
      return;
    }

    void handleSaveRequest();
  }
</script>

<svelte:head>
  <title>PostNot</title>
</svelte:head>

<div class="workspace-grid">
  <div class="profile-bar">
    <div class="profile-facts">
      <span class="profile-fact">Timeout <strong>{settings.requestTimeoutMs} ms</strong></span>
      <span class="profile-fact">Redirects <strong>{settings.followRedirects ? "Follow" : "Off"}</strong></span>
      <span class="profile-fact">TLS <strong>{settings.validateTls ? "Validated" : "Relaxed"}</strong></span>
      <span class="profile-fact">History <strong>{settings.historyLimit}</strong></span>
    </div>

    <span class="profile-divider"></span>

    <div class="profile-env-section">
      <label>
        <span class="sr-only">Active environment</span>
        <select
          class="text-input profile-env-select"
          value={activeEnvironmentId}
          onchange={(event) => handleEnvironmentChange(event.currentTarget.value)}
          disabled={isEnvironmentChanging}
        >
          <option value="">No environment</option>
          {#each environments as environment (environment.id)}
            <option value={environment.id}>{environment.name}</option>
          {/each}
        </select>
      </label>

      {#if activeEnvironmentVarCount !== null}
        <span class="profile-env-hint">
          {activeEnvironmentVarCount} var{activeEnvironmentVarCount === 1 ? "" : "s"}
        </span>
      {/if}
    </div>

    {#if settingsErrorText}
      <span class="profile-env-hint" style="color: var(--danger)">{settingsErrorText}</span>
    {/if}

    {#if environmentsErrorText}
      <span class="profile-env-hint" style="color: var(--danger)">{environmentsErrorText}</span>
    {/if}
  </div>

  <RequestTabs
    tabs={requestWorkspace.tabs}
    activeTabId={requestWorkspace.activeTabId}
    inFlightTabId={requestWorkspace.inFlightTabId}
    scrollRequest={requestTabsScrollRequest}
    onIsDirty={(tab) => requestWorkspace.isDirty(tab)}
    onActivate={handleActivateTab}
    onClose={handleCloseTab}
    onCreate={handleNewRequest}
  />

  <RequestEditor
    bind:request
    environmentVariables={activeEnvironmentDetail?.variables ?? []}
    isSending={activeTabIsSending}
    isCanceling={requestWorkspace.isCanceling}
    isSaving={collections.isSavingRequest}
    saveLabel={activeTab?.savedRequestId ? "Update" : "Save"}
    saveDisabled={activeTabIsSending}
    sendDisabled={activeTabSendLocked}
    handleNewRequest={handleNewRequest}
    handleOpenImport={openRequestImportDialog}
    handleOpenExport={openRequestExportDialog}
    handleOpenPreview={openRequestPreviewDialog}
    handleSendRequest={handleSend}
    handleCancelRequest={handleCancelRequest}
    handleSaveRequest={handleSaveRequest}
    showSaveMenu={collections.collections.length > 0}
    handleSaveAsRequest={handleSaveAsNewRequest}
    activeEnvironmentName={activeEnvironmentDetail?.name ?? ""}
    handleFetchOAuth2Token={handleFetchOAuth2Token}
  />

  {#if activeTabErrorText}
    <div class="response-error">{activeTabErrorText}</div>
  {/if}

  <ResponseViewer response={activeTabResponse} scriptExecution={activeTabScriptExecution} />

  <HistoryPanel
    items={history}
    isLoading={isHistoryLoading}
    errorText={historyErrorText}
    isCollapsed={settings.isHistoryCollapsed}
    selectedId={selectedHistoryId}
    detail={selectedHistoryDetail}
    detailErrorText={historyDetailErrorText}
    isDetailLoading={isHistoryDetailLoading}
    isClearing={isClearingHistory}
    restoringId={restoringHistoryId}
    onToggleCollapse={handleHistoryCollapsedChange}
    onInspect={inspectHistoryEntry}
    onRestore={handleRestoreHistoryEntry}
    onClear={handleClearHistory}
    onCloseDetail={closeHistoryDetail}
  />
</div>

<svelte:window onkeydown={handleWindowKeydown} />

{#if isSaveDialogOpen}
  <div
    class="modal-backdrop"
    role="button"
    tabindex="0"
    aria-label="Close save request dialog"
    use:modalFocusTrap={{ onEscape: closeSaveDialog }}
    onclick={(event) => {
      if (event.target === event.currentTarget) {
        closeSaveDialog();
      }
    }}
    onkeydown={handleSaveDialogBackdropKeydown}
  >
    <div
      class="panel save-dialog request-save-dialog"
      role="dialog"
      tabindex="-1"
      aria-modal="true"
      aria-labelledby="save-request-title"
    >
      <div class="editor-header">
        <h2 id="save-request-title">{saveDialogMode === "save-as" ? "Save as" : "Save request"}</h2>
      </div>

      <div class="editor-block request-save-dialog-body">
        <div class="request-save-target-section">
          <span class="field-label">Choose a collection</span>
          <div class="save-target-list save-collection-list" role="listbox" aria-label="Choose a collection">
            {#each collections.collections as collection (collection.id)}
              <button
                class={["save-target-button", saveTargetCollectionId === collection.id && "save-target-active"]}
                type="button"
                role="option"
                aria-selected={saveTargetCollectionId === collection.id}
                onclick={async () => {
                  saveTargetCollectionId = collection.id;
                  saveTargetParentId = null;
                  await collections.loadCollectionItems(collection.id);
                }}
              >
                <strong>{collection.name}</strong>
                <span>{collection.requestCount} request{collection.requestCount === 1 ? "" : "s"}</span>
              </button>
            {/each}
          </div>
        </div>

        {#if saveTargetCollectionId}
          <div class="request-save-target-section">
            <span class="field-label">Choose a folder</span>
            <div class="save-target-list save-folder-list" role="listbox" aria-label="Choose a folder">
              {#each collections.folderTargets(saveTargetCollectionId) as folderTarget (`${saveTargetCollectionId}-${folderTarget.id ?? "root"}`)}
                <button
                  class={[
                    "save-target-button",
                    folderTarget.id ? "save-target-folder" : "save-target-root",
                    saveTargetParentId === folderTarget.id && "save-target-active"
                  ]}
                  type="button"
                  role="option"
                  aria-selected={saveTargetParentId === folderTarget.id}
                  onclick={() => (saveTargetParentId = folderTarget.id)}
                  style={`--tree-depth:${folderTarget.depth};`}
                >
                  <strong>{folderTarget.name}</strong>
                  <span>{folderTarget.id ? "Folder" : "Collection root"}</span>
                </button>
              {/each}
            </div>
          </div>
        {/if}

        <div class="collections-page-actions">
          <button class="send-button" type="button" onclick={confirmSaveRequest} disabled={collections.isSavingRequest}>
            {collections.isSavingRequest ? "Saving..." : saveDialogMode === "save-as" ? "Save as" : "Save request"}
          </button>
          <button class="ghost-button" type="button" onclick={closeSaveDialog}>
            Cancel
          </button>
        </div>
      </div>
    </div>
  </div>
{/if}

{#if isRequestPreviewDialogOpen}
  <div
    class="modal-backdrop"
    role="button"
    tabindex="0"
    aria-label="Close request preview dialog"
    use:modalFocusTrap={{ onEscape: closeRequestPreviewDialog }}
    onclick={(event) => {
      if (event.target === event.currentTarget) {
        closeRequestPreviewDialog();
      }
    }}
    onkeydown={(event) => handleModalBackdropKeydown(event, closeRequestPreviewDialog)}
  >
    <div class="panel request-preview-dialog" role="dialog" tabindex="-1" aria-modal="true" aria-labelledby="request-preview-title">
      <div class="editor-header import-dialog-header">
        <div>
          <h2 id="request-preview-title">Resolved Request Preview</h2>
          <span class="history-meta">Read-only · secrets masked · no network call</span>
        </div>
        <button class="ghost-button" type="button" onclick={closeRequestPreviewDialog}>
          Close
        </button>
      </div>

      {#if isRequestPreviewLoading}
        <div class="empty-state">Calculating the resolved request...</div>
      {:else if requestPreviewErrorText}
        <div class="response-error">{requestPreviewErrorText}</div>
      {:else if requestPreview}
        {#if requestPreview.warnings.length}
          <div class="request-preview-callouts">
            {#each requestPreview.warnings as warning}
              <div class="response-error request-preview-callout">{warning}</div>
            {/each}
          </div>
        {/if}

        {#if requestPreview.notes.length}
          <details class="request-preview-notes">
            <summary>
              <span>Preview notes</span>
              <svg class="request-preview-notes-chevron" viewBox="0 0 24 24" aria-hidden="true">
                <path d="M6 9l6 6 6-6" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round" />
              </svg>
            </summary>
            {#each requestPreview.notes as note}
              <p class="field-help request-preview-note">{note}</p>
            {/each}
          </details>
        {/if}

        <div class="detail-grid request-preview-grid">
          <section class="detail-card detail-card-span">
            <h4 class="detail-section-title">Overview</h4>
            <div class="detail-facts request-preview-facts">
              <div class="status-item">
                <span class="status-label">Request</span>
                <strong>{requestPreview.name || requestPreview.finalUrl}</strong>
              </div>
              <div class="status-item">
                <span class="status-label">Method</span>
                <strong class={`method-badge method-${requestPreview.method.toLowerCase()}`}>{requestPreview.method}</strong>
              </div>
              <div class="status-item">
                <span class="status-label">Environment</span>
                <strong>{requestPreview.settings.activeEnvironmentName ?? "No environment"}</strong>
              </div>
              <div class="status-item">
                <span class="status-label">Auth</span>
                <strong>{requestPreview.auth.type}</strong>
              </div>
              <div class="status-item">
                <span class="status-label">Timeout</span>
                <strong>{requestPreview.settings.requestTimeoutMs} ms</strong>
              </div>
              <div class="status-item">
                <span class="status-label">Redirects</span>
                <strong>{requestPreview.settings.followRedirects ? "Follow" : "Off"}</strong>
              </div>
              <div class="status-item">
                <span class="status-label">TLS</span>
                <strong>{requestPreview.settings.validateTls ? "Validated" : "Relaxed"}</strong>
              </div>
              <div class="status-item">
                <span class="status-label">Body mode</span>
                <strong>{requestPreview.body.mode}</strong>
              </div>
              <div class="status-item detail-wide request-preview-url-item">
                <span class="status-label">Final URL</span>
                <strong class="detail-url-value" title={requestPreview.finalUrl}>{requestPreview.finalUrl}</strong>
              </div>
            </div>
          </section>

          <section class="detail-card detail-card-span">
            <h4 class="detail-section-title">Outgoing Request</h4>
            <div class="detail-response-columns">
              <div class="detail-response-column">
                <h5 class="detail-subtitle">Headers and Query</h5>

                {#if filterPreviewRows(requestPreview.queryParams).length || filterPreviewRows(requestPreview.headers).length}
                  <div class="detail-stack">
                    {#if filterPreviewRows(requestPreview.queryParams).length}
                      <div class="detail-block">
                        <h6 class="detail-micro-title">Query Parameters</h6>
                        <div class="detail-kv-list detail-kv-list-compact">
                          {#each filterPreviewRows(requestPreview.queryParams) as row (row.id)}
                            <div class="detail-kv-item">
                              <strong>{row.key || "(empty key)"}</strong>
                              <span>{previewDisplayValue(row.value)}</span>
                            </div>
                          {/each}
                        </div>
                      </div>
                    {/if}

                    {#if filterPreviewRows(requestPreview.headers).length}
                      <div class="detail-block">
                        <h6 class="detail-micro-title">Request Headers</h6>
                        <div class="detail-kv-list detail-kv-list-compact">
                          {#each filterPreviewRows(requestPreview.headers) as row (row.id)}
                            <div class="detail-kv-item">
                              <strong>{row.key || "(empty key)"}</strong>
                              <span>{previewDisplayValue(row.value)}</span>
                            </div>
                          {/each}
                        </div>
                      </div>
                    {/if}
                  </div>
                {:else}
                  <div class="empty-state">No query parameters or request headers will be sent.</div>
                {/if}
              </div>

              <div class="detail-response-column">
                <h5 class="detail-subtitle">Body</h5>

                {#if requestPreview.body.mode === "multipart"}
                  {#if filterPreviewRows(requestPreview.body.form).length || filterPreviewFiles(requestPreview.body.files).length}
                    <div class="detail-stack">
                      {#if filterPreviewRows(requestPreview.body.form).length}
                        <div class="detail-block">
                          <h6 class="detail-micro-title">Text Fields</h6>
                          <div class="detail-kv-list detail-kv-list-compact">
                            {#each filterPreviewRows(requestPreview.body.form) as row (row.id)}
                              <div class="detail-kv-item">
                                <strong>{row.key || "(empty field)"}</strong>
                                <span>{previewDisplayValue(row.value)}</span>
                              </div>
                            {/each}
                          </div>
                        </div>
                      {/if}

                      {#if filterPreviewFiles(requestPreview.body.files).length}
                        <div class="detail-block">
                          <h6 class="detail-micro-title">Files</h6>
                          <div class="detail-kv-list detail-kv-list-compact">
                            {#each filterPreviewFiles(requestPreview.body.files) as file (file.id)}
                              <div class="detail-kv-item">
                                <strong>{file.name || "(empty field)"}</strong>
                                <span>{file.path || "(empty path)"}</span>
                              </div>
                            {/each}
                          </div>
                        </div>
                      {/if}
                    </div>
                  {:else}
                    <div class="empty-state">No multipart fields or files will be sent.</div>
                  {/if}
                {:else if requestPreview.body.mode === "form-urlencoded"}
                  {#if filterPreviewRows(requestPreview.body.form).length}
                    <div class="detail-kv-list detail-kv-list-compact">
                      {#each filterPreviewRows(requestPreview.body.form) as row (row.id)}
                        <div class="detail-kv-item">
                          <strong>{row.key || "(empty field)"}</strong>
                          <span>{previewDisplayValue(row.value)}</span>
                        </div>
                      {/each}
                    </div>
                  {:else}
                    <div class="empty-state">No form fields will be sent.</div>
                  {/if}
                {:else if requestPreview.body.raw}
                  <JsonViewer source={requestPreview.body.raw} maxHeight="clamp(12rem, 38vh, 26rem)" />
                {:else}
                  <div class="empty-state">This request will be sent without a body.</div>
                {/if}
              </div>
            </div>
          </section>

          {#if previewAuthRows(requestPreview).length}
            <section class="detail-card detail-card-span">
              <h4 class="detail-section-title">Auth Inputs</h4>
              <div class="detail-kv-list request-preview-auth-list">
                {#each previewAuthRows(requestPreview) as row (row.id)}
                  <div class="detail-kv-item">
                    <strong>{row.key}</strong>
                    <span>{previewDisplayValue(row.value)}</span>
                  </div>
                {/each}
              </div>
            </section>
          {/if}
        </div>
      {/if}
    </div>
  </div>
{/if}

{#if isRequestExportDialogOpen}
  <div
    class="modal-backdrop"
    role="button"
    tabindex="0"
    aria-label="Close request export dialog"
    use:modalFocusTrap={{ onEscape: closeRequestExportDialog }}
    onclick={(event) => {
      if (event.target === event.currentTarget) {
        closeRequestExportDialog();
      }
    }}
    onkeydown={(event) => handleModalBackdropKeydown(event, closeRequestExportDialog)}
  >
    <div class="panel save-dialog" role="dialog" tabindex="-1" aria-modal="true" aria-labelledby="request-export-title">
      <div class="editor-header import-dialog-header">
        <h2 id="request-export-title">Export Request</h2>
        <span class="history-meta">
          {requestExportFormat === "curl" ? "cURL command" : "PostNot request JSON"}
          {requestExportSafety === "redacted" ? " · secrets redacted" : " · full values included"}
        </span>
      </div>

      <div class="editor-block modal-scroll-body">
        <div class="import-format-toggle" role="tablist" aria-label="Choose request export format">
          <button
            class={["system-button", requestExportFormat === "curl" && "toggle-active"]}
            type="button"
            role="tab"
            aria-selected={requestExportFormat === "curl"}
            onclick={() => (requestExportFormat = "curl")}
          >
            cURL
          </button>
          <button
            class={["system-button", requestExportFormat === "json" && "toggle-active"]}
            type="button"
            role="tab"
            aria-selected={requestExportFormat === "json"}
            onclick={() => (requestExportFormat = "json")}
          >
            JSON
          </button>
        </div>

        <label class="inline-checkbox">
          <input
            type="checkbox"
            checked={requestExportSafety === "full"}
            onchange={(event) => (requestExportSafety = event.currentTarget.checked ? "full" : "redacted")}
          />
          <span>Include secrets in this export</span>
        </label>

        {#if requestExportSafety === "full"}
          <p class="auth-error-text">
            Full export includes bearer tokens, OAuth2 access tokens, client secrets, API keys, cookies, and basic-auth passwords.
          </p>
        {:else if requestExportRedactions.length > 0}
          <div class="request-export-redactions" aria-live="polite">
            <p class="field-help">
              PostNot redacted credential-looking values so this export is safer to paste into chat, tickets, or docs.
            </p>
            <ul>
              {#each requestExportRedactions as redaction}
                <li><strong>{redaction.field}</strong>: {redaction.reason}</li>
              {/each}
            </ul>
          </div>
        {:else}
          <p class="field-help">No credential-looking values were found in this request export.</p>
        {/if}

        <label>
          <span class="field-label">
            {requestExportFormat === "curl" ? "cURL command" : "Request JSON"}
            {requestExportSafety === "redacted" ? " (redacted)" : " (full)"}
          </span>
          <textarea
            class="text-input collections-import-source"
            value={requestExportSource}
            readonly
          ></textarea>
        </label>

        <div class="collections-page-actions">
          <button class="send-button" type="button" onclick={handleCopyRequestExport}>
            Copy
          </button>
          <button class="ghost-button" type="button" onclick={closeRequestExportDialog}>
            Close
          </button>
        </div>
      </div>
    </div>
  </div>
{/if}

{#if isRequestImportDialogOpen}
  <div
    class="modal-backdrop"
    role="button"
    tabindex="0"
    aria-label="Close request import dialog"
    use:modalFocusTrap={{ onEscape: closeRequestImportDialog }}
    onclick={(event) => {
      if (event.target === event.currentTarget) {
        closeRequestImportDialog();
      }
    }}
    onkeydown={(event) => {
      if (event.target === event.currentTarget && (event.key === "Enter" || event.key === " ")) {
        event.preventDefault();
        closeRequestImportDialog();
      }
    }}
  >
    <div class="panel save-dialog" role="dialog" tabindex="-1" aria-modal="true" aria-labelledby="request-import-title">
      <div class="editor-header import-dialog-header">
        <h2 id="request-import-title">Import Request</h2>
        <span class="history-meta">
          {requestImportFormat === "curl" ? "cURL command" : "OpenAPI 3 JSON or YAML"}
        </span>
      </div>

      <div class="editor-block modal-scroll-body">
        <div class="import-format-toggle" role="tablist" aria-label="Choose request import format">
          <button
            class={["system-button", requestImportFormat === "curl" && "toggle-active"]}
            type="button"
            role="tab"
            aria-selected={requestImportFormat === "curl"}
            onclick={() => {
              requestImportFormat = "curl";
              requestImportErrorText = "";
            }}
          >
            cURL
          </button>
          <button
            class={["system-button", requestImportFormat === "openapi" && "toggle-active"]}
            type="button"
            role="tab"
            aria-selected={requestImportFormat === "openapi"}
            onclick={() => {
              requestImportFormat = "openapi";
              requestImportErrorText = "";
            }}
          >
            OpenAPI 3
          </button>
        </div>

        {#if requestImportFormat === "curl"}
          <label>
            <span class="field-label">Paste cURL command</span>
            <textarea
              class="text-input collections-import-source"
              bind:value={curlImportSource}
              placeholder='curl --request GET https://api.example.com/items -H "Authorization: Bearer token"'
            ></textarea>
          </label>
        {:else}
          <p class="field-help">Load an OpenAPI 3 document from JSON or YAML. Single-operation files open directly in a new request tab.</p>

          <label>
            <span class="field-label">Paste source</span>
            <textarea
              class="text-input collections-import-source"
              bind:value={openApiImportSource}
              placeholder={openApiRequestImportPlaceholder}
            ></textarea>
          </label>

          <input
            bind:this={openApiImportFileInput}
            class="sr-only"
            type="file"
            accept=".json,.yaml,.yml,application/json,application/yaml,text/yaml,text/x-yaml"
            onchange={async (event: Event & { currentTarget: HTMLInputElement }) => {
              const file = event.currentTarget.files?.[0];
              if (!file) {
                return;
              }

              openApiImportSource = await file.text();
              requestImportErrorText = "";
              event.currentTarget.value = "";
            }}
          />
        {/if}

        {#if requestImportErrorText}
          <div class="response-error">{requestImportErrorText}</div>
        {/if}

        <div class="collections-page-actions">
          {#if requestImportFormat === "openapi"}
            <button class="ghost-button" type="button" onclick={() => openApiImportFileInput?.click()}>
              Open file
            </button>
          {/if}
          <button class="send-button" type="button" onclick={handleImportRequest} disabled={isImportingRequest}>
            {isImportingRequest ? "Importing..." : "Import request"}
          </button>
          <button class="ghost-button" type="button" onclick={closeRequestImportDialog}>
            Cancel
          </button>
        </div>
      </div>
    </div>
  </div>
{/if}
