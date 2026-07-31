<script lang="ts">
  import { tick } from "svelte";

  import type { EnvironmentVariable } from "$lib/api/types";
  import { insertTextIntoEditableControl } from "$lib/dom-editing";
  import { tokenizeScript, type ScriptToken } from "$lib/script-formatting";

  type ScriptCompletion = {
    id: string;
    label: string;
    detail: string;
    insertText: string;
  };

  type CompletionContext = {
    start: number;
    end: number;
    query: string;
    kind: "token" | "variable";
    previousChar: string;
  };

  type ScriptEditorKind = "preRequest" | "test";

  const CURSOR_MARKER = "__CURSOR__";
  const TOKEN_PATTERN = /[A-Za-z0-9_.]/;
  const BASE_TOP_LEVEL_COMPLETIONS: ScriptCompletion[] = [
    {
      id: "pn-root",
      label: "pn",
      detail: "PostNot scripting API root",
      insertText: "pn"
    },
    {
      id: "pn-request",
      label: "pn.request",
      detail: "Outgoing request helpers",
      insertText: "pn.request"
    },
    {
      id: "pn-response",
      label: "pn.response",
      detail: "Response helpers",
      insertText: "pn.response"
    },
    {
      id: "pn-variables",
      label: "pn.variables",
      detail: "Active environment variables",
      insertText: "pn.variables"
    },
    {
      id: "pn-http",
      label: "pn.http",
      detail: "Helper HTTP requests inside scripts",
      insertText: "pn.http"
    },
    {
      id: "pn-test",
      label: "pn.test(name, fn)",
      detail: "Register a sync or async response test block",
      insertText: "await pn.test('name', async () => {\n  __CURSOR__\n});"
    },
    {
      id: "pn-expect",
      label: "pn.expect(value)",
      detail: "Start an assertion chain",
      insertText: "pn.expect(__CURSOR__)"
    }
  ];
  const REQUEST_COMPLETIONS: ScriptCompletion[] = [
    { id: "req-name", label: "name", detail: "Request display name", insertText: "pn.request.name" },
    { id: "req-method", label: "method", detail: "HTTP method", insertText: "pn.request.method" },
    { id: "req-url", label: "url", detail: "Request URL", insertText: "pn.request.url" },
    {
      id: "req-add-header",
      label: "addHeader(key, value)",
      detail: "Append a request header row",
      insertText: "pn.request.addHeader('__CURSOR__', '')"
    },
    {
      id: "req-upsert-header",
      label: "upsertHeader(key, value)",
      detail: "Create or replace a request header",
      insertText: "pn.request.upsertHeader('__CURSOR__', '')"
    },
    {
      id: "req-remove-header",
      label: "removeHeader(key)",
      detail: "Remove a request header by name",
      insertText: "pn.request.removeHeader('__CURSOR__')"
    },
    {
      id: "req-add-query",
      label: "addQueryParam(key, value)",
      detail: "Append a query parameter",
      insertText: "pn.request.addQueryParam('__CURSOR__', '')"
    },
    {
      id: "req-upsert-query",
      label: "upsertQueryParam(key, value)",
      detail: "Create or replace a query parameter",
      insertText: "pn.request.upsertQueryParam('__CURSOR__', '')"
    },
    {
      id: "req-remove-query",
      label: "removeQueryParam(key)",
      detail: "Remove a query parameter by name",
      insertText: "pn.request.removeQueryParam('__CURSOR__')"
    },
    {
      id: "req-set-raw",
      label: "setRawBody(value)",
      detail: "Switch to raw body mode and set the body",
      insertText: "pn.request.setRawBody(__CURSOR__)"
    },
    {
      id: "req-set-json",
      label: "setJsonBody(value)",
      detail: "Switch to JSON mode and set the body",
      insertText: "pn.request.setJsonBody(__CURSOR__)"
    },
    {
      id: "req-clear-body",
      label: "clearBody()",
      detail: "Send the request without a body",
      insertText: "pn.request.clearBody()"
    },
    {
      id: "req-set-bearer",
      label: "setBearerToken(token)",
      detail: "Switch auth to bearer token",
      insertText: "pn.request.setBearerToken(__CURSOR__)"
    },
    {
      id: "req-set-oauth2",
      label: "setOAuth2Token(token)",
      detail: "Switch auth to OAuth2 bearer token",
      insertText: "pn.request.setOAuth2Token(__CURSOR__)"
    },
    {
      id: "req-set-basic",
      label: "setBasicAuth(username, password)",
      detail: "Switch auth to basic auth",
      insertText: "pn.request.setBasicAuth('__CURSOR__', '')"
    },
    {
      id: "req-set-api-key",
      label: "setApiKey(name, value, placement)",
      detail: "Switch auth to API key",
      insertText: "pn.request.setApiKey('__CURSOR__', '', 'header')"
    },
    {
      id: "req-clear-auth",
      label: "clearAuth()",
      detail: "Clear request auth configuration",
      insertText: "pn.request.clearAuth()"
    },
    {
      id: "req-get-header",
      label: "getHeader(name)",
      detail: "Read a request header value",
      insertText: "pn.request.getHeader('__CURSOR__')"
    }
  ];
  const RESPONSE_COMPLETIONS: ScriptCompletion[] = [
    { id: "res-code", label: "code", detail: "HTTP status code", insertText: "pn.response.code" },
    { id: "res-status", label: "status", detail: "HTTP status text", insertText: "pn.response.status" },
    { id: "res-headers", label: "headers", detail: "Response header list", insertText: "pn.response.headers" },
    {
      id: "res-header",
      label: "header(name)",
      detail: "Read a response header value",
      insertText: "pn.response.header('__CURSOR__')"
    },
    {
      id: "res-text",
      label: "text()",
      detail: "Read the raw response body",
      insertText: "pn.response.text()"
    },
    {
      id: "res-json",
      label: "json()",
      detail: "Parse the response body as JSON",
      insertText: "pn.response.json()"
    }
  ];
  const VARIABLE_COMPLETIONS: ScriptCompletion[] = [
    {
      id: "var-get",
      label: "get(name)",
      detail: "Read one active environment variable",
      insertText: "pn.variables.get('__CURSOR__')"
    },
    {
      id: "var-has",
      label: "has(name)",
      detail: "Check whether a variable exists",
      insertText: "pn.variables.has('__CURSOR__')"
    },
    {
      id: "var-all",
      label: "all()",
      detail: "Read all active environment variables",
      insertText: "pn.variables.all()"
    },
    {
      id: "var-set",
      label: "set(name, value, options)",
      detail: "Create or update an active environment variable",
      insertText: "await pn.variables.set('__CURSOR__', '')"
    },
    {
      id: "var-remove",
      label: "remove(name)",
      detail: "Remove an active environment variable",
      insertText: "await pn.variables.remove('__CURSOR__')"
    }
  ];
  const HTTP_COMPLETIONS: ScriptCompletion[] = [
    {
      id: "http-send",
      label: "send(request)",
      detail: "Send a helper request without adding a history entry",
      insertText: "await pn.http.send({\n  method: 'GET',\n  url: '__CURSOR__'\n})"
    }
  ];
  const EXPECTATION_COMPLETIONS: ScriptCompletion[] = [
    { id: "exp-to-be", label: "toBe(expected)", detail: "Strict equality assertion", insertText: "toBe(__CURSOR__)" },
    { id: "exp-to-equal", label: "toEqual(expected)", detail: "Deep equality assertion", insertText: "toEqual(__CURSOR__)" },
    {
      id: "exp-to-include",
      label: "toInclude(expected)",
      detail: "String or array inclusion assertion",
      insertText: "toInclude(__CURSOR__)"
    },
    { id: "exp-to-match", label: "toMatch(expected)", detail: "String or regex match", insertText: "toMatch(__CURSOR__)" },
    { id: "exp-to-truthy", label: "toBeTruthy()", detail: "Truthy assertion", insertText: "toBeTruthy()" },
    { id: "exp-to-falsy", label: "toBeFalsy()", detail: "Falsy assertion", insertText: "toBeFalsy()" },
    {
      id: "exp-to-gt",
      label: "toBeGreaterThan(value)",
      detail: "Numeric greater-than assertion",
      insertText: "toBeGreaterThan(__CURSOR__)"
    },
    {
      id: "exp-to-lt",
      label: "toBeLessThan(value)",
      detail: "Numeric less-than assertion",
      insertText: "toBeLessThan(__CURSOR__)"
    }
  ];

  let {
    value = $bindable(),
    placeholder = "",
    environmentVariables = [],
    disabled = false,
    scriptKind = "preRequest",
    onValueInput = () => {}
  }: {
    value: string;
    placeholder?: string;
    environmentVariables?: EnvironmentVariable[];
    disabled?: boolean;
    scriptKind?: ScriptEditorKind;
    onValueInput?: (value: string) => void;
  } = $props();

  let textareaElement: HTMLTextAreaElement | null = $state(null);
  let highlightOverlayElement: HTMLPreElement | null = $state(null);
  let highlightContentElement: HTMLElement | null = $state(null);
  let mirrorElement: HTMLDivElement | null = $state(null);
  let mirrorCaretElement: HTMLSpanElement | null = $state(null);
  let suggestionsElement: HTMLDivElement | null = $state(null);
  let isSuggestionsOpen = $state(false);
  let suggestions = $state<ScriptCompletion[]>([]);
  let activeSuggestionIndex = $state(0);
  let replacementStart = $state(-1);
  let replacementEnd = $state(-1);
  let suggestionLeft = $state(12);
  let suggestionTop = $state(0);
  let suggestionPlacement: "above" | "below" = $state("above");
  let hasMeasuredSuggestionPosition = $state(false);
  let blurTimeout: ReturnType<typeof setTimeout> | null = $state(null);
  let mirrorBeforeText = $state(" ");
  let highlightSyncFrame: number | null = null;
  let scriptTokens = $derived(tokenizeScript(value ?? ""));

  $effect(() => {
    if (!textareaElement || !highlightOverlayElement || !highlightContentElement) {
      return;
    }

    const activeTextarea = textareaElement;
    const resizeObserver = new ResizeObserver(() => scheduleHighlightOverlaySync());

    scheduleHighlightOverlaySync();
    resizeObserver.observe(activeTextarea);
    activeTextarea.addEventListener("scroll", syncHighlightOverlay);

    return () => {
      activeTextarea.removeEventListener("scroll", syncHighlightOverlay);
      resizeObserver.disconnect();
      if (highlightSyncFrame !== null) {
        cancelAnimationFrame(highlightSyncFrame);
        highlightSyncFrame = null;
      }
    };
  });

  $effect(() => {
    value;
    scriptTokens;
    void tick().then(() => scheduleHighlightOverlaySync());
  });

  function topLevelCompletionsFor(kind: ScriptEditorKind) {
    return BASE_TOP_LEVEL_COMPLETIONS.filter((item) => {
      if (item.id === "pn-request") {
        return kind === "preRequest";
      }

      if (item.id === "pn-response" || item.id === "pn-test") {
        return kind === "test";
      }

      return true;
    });
  }

  function uniqueEnvironmentVariableNames(rows: EnvironmentVariable[]) {
    const seen: Record<string, true> = {};
    const result: ScriptCompletion[] = [];

    for (const row of rows) {
      const key = row.key.trim();
      const lookupKey = key.toLowerCase();
      if (!row.enabled || !key || seen[lookupKey]) {
        continue;
      }

      seen[lookupKey] = true;
      result.push({
        id: `env-${lookupKey}`,
        label: key,
        detail: row.isSecret ? "Secret environment variable" : "Environment variable",
        insertText: key
      });
    }

    return result;
  }

  function filterCompletions(items: ScriptCompletion[], query: string) {
    const normalizedQuery = query.trim().toLowerCase();
    if (!normalizedQuery) {
      return items;
    }

    return items.filter((item) => item.label.toLowerCase().includes(normalizedQuery));
  }

  function getVariableNameContext(source: string, cursor: number): CompletionContext | null {
    const beforeCursor = source.slice(0, cursor);
    const match = beforeCursor.match(/pn\.variables\.(?:get|has)\(\s*(['"])([^'"]*)$/);
    if (!match) {
      return null;
    }

    const quote = match[1];
    const query = match[2] ?? "";
    let end = cursor;
    while (end < source.length && source[end] !== quote) {
      end += 1;
    }

    return {
      start: cursor - query.length,
      end,
      query,
      kind: "variable",
      previousChar: quote
    };
  }

  function getTokenContext(source: string, cursor: number): CompletionContext | null {
    let start = cursor;
    while (start > 0 && TOKEN_PATTERN.test(source[start - 1] ?? "")) {
      start -= 1;
    }

    let end = cursor;
    while (end < source.length && TOKEN_PATTERN.test(source[end] ?? "")) {
      end += 1;
    }

    if (start === end) {
      return null;
    }

    return {
      start,
      end,
      query: source.slice(start, cursor),
      kind: "token",
      previousChar: start > 0 ? source[start - 1] ?? "" : ""
    };
  }

  function resolveTokenSuggestions(
    query: string,
    previousChar: string,
    kind: ScriptEditorKind
  ) {
    const normalized = query.trim();
    if (!normalized) {
      return [];
    }

    const expectationQuery = normalized.startsWith(".") ? normalized.slice(1) : normalized;
    if (
      (previousChar === "." || normalized.startsWith(".")) &&
      /^to[a-z0-9]*$/i.test(expectationQuery)
    ) {
      return filterCompletions(EXPECTATION_COMPLETIONS, expectationQuery);
    }

    const topLevelCompletions = topLevelCompletionsFor(kind);
    if (normalized === "p" || normalized === "pn") {
      return topLevelCompletions;
    }

    if (!normalized.startsWith("pn")) {
      return [];
    }

    if (!normalized.startsWith("pn.")) {
      return filterCompletions(topLevelCompletions, normalized.slice(2));
    }

    const path = normalized.slice(3);
    const [firstSegment, secondSegment = ""] = path.split(".", 2);

    if (!path) {
      return topLevelCompletions.filter((item) => item.id !== "pn-root");
    }

    if (!path.includes(".")) {
      const topLevelMembers = topLevelCompletions.filter((item) => item.id !== "pn-root");
      return filterCompletions(topLevelMembers, firstSegment);
    }

    if (firstSegment === "request" && kind === "preRequest") {
      return filterCompletions(REQUEST_COMPLETIONS, secondSegment);
    }

    if (firstSegment === "response" && kind === "test") {
      return filterCompletions(RESPONSE_COMPLETIONS, secondSegment);
    }

    if (firstSegment === "variables") {
      return filterCompletions(VARIABLE_COMPLETIONS, secondSegment);
    }

    if (firstSegment === "http") {
      return filterCompletions(HTTP_COMPLETIONS, secondSegment);
    }

    return [];
  }

  function closeSuggestions() {
    isSuggestionsOpen = false;
    suggestions = [];
    activeSuggestionIndex = 0;
    replacementStart = -1;
    replacementEnd = -1;
    hasMeasuredSuggestionPosition = false;
  }

  function clearBlurTimeout() {
    if (!blurTimeout) {
      return;
    }

    clearTimeout(blurTimeout);
    blurTimeout = null;
  }

  function copyTextareaStyles() {
    if (!textareaElement || !mirrorElement) {
      return;
    }

    const computedStyle = window.getComputedStyle(textareaElement);
    const mirroredProperties = [
      "boxSizing",
      "fontFamily",
      "fontSize",
      "fontWeight",
      "fontStyle",
      "fontVariant",
      "letterSpacing",
      "lineHeight",
      "paddingTop",
      "paddingRight",
      "paddingBottom",
      "paddingLeft",
      "borderTopWidth",
      "borderRightWidth",
      "borderBottomWidth",
      "borderLeftWidth",
      "textIndent",
      "textTransform",
      "textAlign",
      "tabSize"
    ] as const;

    for (const property of mirroredProperties) {
      mirrorElement.style[property] = computedStyle[property];
    }

    mirrorElement.style.width = `${textareaElement.clientWidth}px`;
    mirrorElement.style.whiteSpace = "pre-wrap";
    mirrorElement.style.overflowWrap = "break-word";
    mirrorElement.style.wordBreak = "break-word";
  }

  function scheduleHighlightOverlaySync() {
    if (!textareaElement || !highlightOverlayElement) {
      return;
    }

    if (highlightSyncFrame !== null) {
      cancelAnimationFrame(highlightSyncFrame);
    }

    highlightSyncFrame = requestAnimationFrame(() => {
      highlightSyncFrame = null;
      syncHighlightOverlay();
    });
  }

  function syncHighlightOverlay() {
    if (!textareaElement || !highlightOverlayElement || !highlightContentElement) {
      return;
    }

    const computedStyle = window.getComputedStyle(textareaElement);

    highlightOverlayElement.style.width = `${textareaElement.offsetWidth}px`;
    highlightOverlayElement.style.height = `${textareaElement.offsetHeight}px`;
    highlightOverlayElement.style.boxSizing = computedStyle.boxSizing;
    highlightOverlayElement.style.padding = "0";
    highlightOverlayElement.style.borderWidth = computedStyle.borderWidth;
    highlightOverlayElement.style.borderStyle = computedStyle.borderStyle;
    highlightOverlayElement.style.borderColor = "transparent";
    highlightOverlayElement.style.font = computedStyle.font;
    highlightOverlayElement.style.letterSpacing = computedStyle.letterSpacing;
    highlightOverlayElement.style.lineHeight = computedStyle.lineHeight;
    highlightOverlayElement.style.textAlign = computedStyle.textAlign;
    highlightOverlayElement.style.textIndent = computedStyle.textIndent;
    highlightOverlayElement.style.textTransform = computedStyle.textTransform;
    highlightOverlayElement.style.tabSize = computedStyle.tabSize;
    highlightOverlayElement.style.overflowX = "hidden";
    highlightOverlayElement.style.overflowY = "hidden";
    highlightOverlayElement.style.resize = "none";

    highlightContentElement.style.width = `${textareaElement.clientWidth}px`;
    highlightContentElement.style.boxSizing = computedStyle.boxSizing;
    highlightContentElement.style.padding = computedStyle.padding;
    highlightContentElement.style.font = computedStyle.font;
    highlightContentElement.style.letterSpacing = computedStyle.letterSpacing;
    highlightContentElement.style.lineHeight = computedStyle.lineHeight;
    highlightContentElement.style.textAlign = computedStyle.textAlign;
    highlightContentElement.style.textIndent = computedStyle.textIndent;
    highlightContentElement.style.textTransform = computedStyle.textTransform;
    highlightContentElement.style.tabSize = computedStyle.tabSize;
    highlightContentElement.style.whiteSpace = computedStyle.whiteSpace;
    highlightContentElement.style.overflowWrap = computedStyle.overflowWrap;
    highlightContentElement.style.wordBreak = computedStyle.wordBreak;
    highlightContentElement.style.transform = `translate(${-textareaElement.scrollLeft}px, ${-textareaElement.scrollTop}px)`;
  }

  function updateSuggestionAnchor() {
    if (!textareaElement || !mirrorElement || !mirrorCaretElement) {
      return;
    }

    copyTextareaStyles();
    const selectionStart = textareaElement.selectionStart ?? value.length;
    mirrorBeforeText = value.slice(0, selectionStart) || " ";

    const caretLeft = mirrorCaretElement.offsetLeft - textareaElement.scrollLeft;
    const caretTop = mirrorCaretElement.offsetTop - textareaElement.scrollTop;
    const shellWidth = textareaElement.clientWidth;

    suggestionLeft = Math.max(8, Math.min(caretLeft, Math.max(8, shellWidth - 260)));
    suggestionPlacement = caretTop > 88 ? "above" : "below";
    suggestionTop = suggestionPlacement === "above" ? caretTop - 8 : caretTop + 28;
    hasMeasuredSuggestionPosition = true;
  }

  function updateAutocompleteState() {
    if (!textareaElement || disabled) {
      closeSuggestions();
      return;
    }

    const cursor = textareaElement.selectionStart ?? value.length;
    const variableContext = getVariableNameContext(value, cursor);
    const context = variableContext ?? getTokenContext(value, cursor);
    if (!context) {
      closeSuggestions();
      return;
    }

    const nextSuggestions =
      context.kind === "variable"
        ? filterCompletions(uniqueEnvironmentVariableNames(environmentVariables), context.query)
        : resolveTokenSuggestions(context.query, context.previousChar, scriptKind);

    if (nextSuggestions.length === 0) {
      closeSuggestions();
      return;
    }

    const isSameSuggestionSet =
      replacementStart === context.start &&
      replacementEnd === context.end &&
      suggestions.length === nextSuggestions.length &&
      suggestions.every((suggestion, index) => suggestion.id === nextSuggestions[index]?.id);

    replacementStart =
      context.kind === "token" && context.query.startsWith(".")
        ? context.start + 1
        : context.start;
    replacementEnd = context.end;
    suggestions = nextSuggestions;
    activeSuggestionIndex = isSameSuggestionSet
      ? Math.min(activeSuggestionIndex, nextSuggestions.length - 1)
      : 0;
    hasMeasuredSuggestionPosition = false;
    isSuggestionsOpen = true;
    updateSuggestionAnchor();
    void tick().then(() => {
      updateSuggestionAnchor();
      scrollActiveSuggestionIntoView();
    });
  }

  function scrollActiveSuggestionIntoView() {
    if (!suggestionsElement) {
      return;
    }

    const suggestionButtons = suggestionsElement.querySelectorAll<HTMLButtonElement>(".variable-suggestion");
    suggestionButtons[activeSuggestionIndex]?.scrollIntoView({ block: "nearest" });
  }

  async function applySuggestion(item: ScriptCompletion) {
    if (!textareaElement || replacementStart < 0 || replacementEnd < 0 || disabled) {
      return;
    }

    const markerIndex = item.insertText.indexOf(CURSOR_MARKER);
    const insertedText = item.insertText.replace(CURSOR_MARKER, "");

    insertTextIntoEditableControl(textareaElement, insertedText, {
      selectionStart: replacementStart,
      selectionEnd: replacementEnd,
      cursorOffset: markerIndex >= 0 ? markerIndex : insertedText.length,
      undoBaselineSelection: "collapse-end"
    });
    closeSuggestions();
    await tick();
    updateAutocompleteState();
  }

  function handleInput(event: Event) {
    const target = event.currentTarget as HTMLTextAreaElement;
    value = target.value;
    onValueInput(target.value);
    updateAutocompleteState();
    scheduleHighlightOverlaySync();
  }

  function handleFocus() {
    clearBlurTimeout();
    updateAutocompleteState();
  }

  function handleBlur() {
    clearBlurTimeout();
    blurTimeout = setTimeout(() => {
      closeSuggestions();
    }, 120);
  }

  function handleCursorMovement(event?: KeyboardEvent | MouseEvent) {
    if (
      event instanceof KeyboardEvent &&
      ["ArrowDown", "ArrowUp", "Enter", "Tab", "Escape"].includes(event.key)
    ) {
      return;
    }

    updateAutocompleteState();
  }

  function handleKeydown(event: KeyboardEvent) {
    if (isSuggestionsOpen && suggestions.length > 0) {
      if (event.key === "ArrowDown") {
        event.preventDefault();
        activeSuggestionIndex = (activeSuggestionIndex + 1) % suggestions.length;
        void tick().then(() => scrollActiveSuggestionIntoView());
        return;
      }

      if (event.key === "ArrowUp") {
        event.preventDefault();
        activeSuggestionIndex = (activeSuggestionIndex - 1 + suggestions.length) % suggestions.length;
        void tick().then(() => scrollActiveSuggestionIntoView());
        return;
      }

      if (event.key === "Enter" || event.key === "Tab") {
        event.preventDefault();
        void applySuggestion(suggestions[activeSuggestionIndex]);
        return;
      }

      if (event.key === "Escape") {
        event.preventDefault();
        closeSuggestions();
        return;
      }
    }

    if (handleScriptFormattingKeydown(event)) {
      return;
    }
  }

  function handleScriptFormattingKeydown(event: KeyboardEvent) {
    if (!textareaElement || disabled) {
      return false;
    }

    if (event.key === "Enter") {
      event.preventDefault();
      const smartNewline = getSmartNewline(textareaElement);
      insertTextIntoEditableControl(textareaElement, smartNewline.text, {
        cursorOffset: smartNewline.cursorOffset
      });
      return true;
    }

    if (event.key === "Tab") {
      event.preventDefault();
      handleScriptTab(event.shiftKey);
      return true;
    }

    return false;
  }

  function getSmartNewline(textarea: HTMLTextAreaElement) {
    const { selectionStart, value: source } = textarea;
    const lineStart = source.lastIndexOf("\n", selectionStart - 1) + 1;
    const currentLine = source.slice(lineStart, selectionStart);
    const baseIndent = currentLine.match(/^(\s*)/)?.[1] ?? "";
    const beforeCursor = source.slice(0, selectionStart).trimEnd();
    const afterCursor = source.slice(selectionStart).trimStart();
    const extraIndent = /(?:\{|\(|\[|=>)$/.test(beforeCursor) ? "  " : "";

    if (extraIndent && /^(?:\}|\)|\])/.test(afterCursor)) {
      const text = `\n${baseIndent}${extraIndent}\n${baseIndent}`;
      return {
        text,
        cursorOffset: `\n${baseIndent}${extraIndent}`.length
      };
    }

    const text = `\n${baseIndent}${extraIndent}`;
    return {
      text,
      cursorOffset: text.length
    };
  }

  function handleScriptTab(isOutdent: boolean) {
    if (!textareaElement) {
      return;
    }

    const { selectionStart, selectionEnd, value: source } = textareaElement;
    if (selectionStart === selectionEnd) {
      if (isOutdent) {
        const lineStart = source.lastIndexOf("\n", selectionStart - 1) + 1;
        const linePrefix = source.slice(lineStart, selectionStart);
        const removableIndent = linePrefix.match(/(?:  |\t)$/)?.[0] ?? "";

        if (removableIndent) {
          insertTextIntoEditableControl(textareaElement, "", {
            selectionStart: selectionStart - removableIndent.length,
            selectionEnd: selectionStart
          });
        }
        return;
      }

      insertTextIntoEditableControl(textareaElement, isOutdent ? "" : "  ");
      return;
    }

    const lineStart = source.lastIndexOf("\n", selectionStart - 1) + 1;
    const selectedBlock = source.slice(lineStart, selectionEnd);
    const replacement = selectedBlock
      .split("\n")
      .map((line) => isOutdent ? line.replace(/^(?:  |\t)/, "") : `  ${line}`)
      .join("\n");

    insertTextIntoEditableControl(textareaElement, replacement, {
      selectionStart: lineStart,
      selectionEnd,
      cursorOffset: replacement.length
    });
  }

  function handleSuggestionPointerDown(event: MouseEvent) {
    event.preventDefault();
    clearBlurTimeout();
  }

  function getSuggestionStyle() {
    return `left: ${suggestionLeft}px; top: ${suggestionTop}px;`;
  }

  function getScriptTokenClass(type: ScriptToken["type"]) {
    switch (type) {
      case "keyword":
        return "jt-key";
      case "api":
      case "property":
        return "jt-variable";
      case "string":
      case "template":
        return "jt-string";
      case "comment":
        return "jt-comment";
      case "number":
        return "jt-number";
      case "boolean":
        return "jt-bool";
      case "null":
        return "jt-null";
      case "operator":
        return "jt-comma";
      default:
        return "";
    }
  }
</script>

<div class="variable-input-shell">
  <pre
    class="variable-highlight-overlay variable-highlight-overlay-multiline script-editor-overlay"
    aria-hidden="true"
    spellcheck={false}
    bind:this={highlightOverlayElement}
  ><code class="variable-highlight-content" bind:this={highlightContentElement}>{#each scriptTokens as token}{#if getScriptTokenClass(token.type)}<span class={getScriptTokenClass(token.type)}>{token.value}</span>{:else}{token.value}{/if}{/each}</code></pre>

  <textarea
    bind:this={textareaElement}
    class="text-input body-textarea request-script-editor variable-input-highlighted"
    bind:value={value}
    {placeholder}
    spellcheck={false}
    {disabled}
    onblur={handleBlur}
    onclick={handleCursorMovement}
    onfocus={handleFocus}
    oninput={handleInput}
    onkeydown={handleKeydown}
    onkeyup={handleCursorMovement}
  ></textarea>

  {#if isSuggestionsOpen}
    <div
      bind:this={suggestionsElement}
      class={["variable-suggestions", !hasMeasuredSuggestionPosition && "variable-suggestions-hidden", suggestionPlacement === "below" && "variable-suggestions-below"]}
      role="listbox"
      aria-label="Script autocomplete suggestions"
      style={getSuggestionStyle()}
    >
      {#each suggestions as suggestion, index (suggestion.id)}
        <button
          class={["variable-suggestion", index === activeSuggestionIndex && "variable-suggestion-active"]}
          type="button"
          onclick={() => applySuggestion(suggestion)}
          onmousedown={handleSuggestionPointerDown}
        >
          <strong>{suggestion.label}</strong>
          <span>{suggestion.detail}</span>
        </button>
      {/each}
    </div>
  {/if}

  <div aria-hidden="true" class={["variable-input-mirror", "variable-input-mirror-multiline"]} bind:this={mirrorElement}>
    {mirrorBeforeText}<span bind:this={mirrorCaretElement} class="variable-input-caret-marker">&#8203;</span>
  </div>
</div>
