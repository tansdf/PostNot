<script lang="ts">
  import { tick } from "svelte";

  import type { EnvironmentVariable } from "$lib/api/types";
  import { insertTextIntoEditableControl } from "$lib/dom-editing";

  type VariableOption = {
    key: string;
    value: string;
    isSecret: boolean;
    isDynamic?: boolean;
  };

  type UsedVariable = {
    key: string;
    value: string | null;
    isResolved: boolean;
    tooltip: string;
  };

  type HighlightToken = {
    type: string;
    value: string;
  };

  let {
    value = "",
    variables = [],
    onValueInput = () => {},
    className = "",
    placeholder = "",
    id = "",
    type = "text",
    spellcheck = true,
    multiline = false,
    disabled = false,
    list = "",
    ariaLabel = "",
    ariaInvalid = false,
    onExtraKeydown = undefined,
    highlightTokens = [],
    highlightOverlayClassName = ""
  }: {
    value?: string;
    variables?: EnvironmentVariable[];
    onValueInput?: (value: string) => void;
    className?: string;
    placeholder?: string;
    id?: string;
    type?: string;
    spellcheck?: boolean;
    multiline?: boolean;
    disabled?: boolean;
    list?: string;
    ariaLabel?: string;
    ariaInvalid?: boolean;
    onExtraKeydown?: ((event: KeyboardEvent) => void) | undefined;
    highlightTokens?: HighlightToken[];
    highlightOverlayClassName?: string;
  } = $props();

  let fieldElement: HTMLInputElement | HTMLTextAreaElement | null = $state(null);
  let highlightOverlayElement: HTMLPreElement | null = $state(null);
  let highlightContentElement: HTMLElement | null = $state(null);
  let mirrorElement: HTMLDivElement | null = $state(null);
  let mirrorTextElement: HTMLSpanElement | null = $state(null);
  let mirrorCaretElement: HTMLSpanElement | null = $state(null);
  let isSuggestionsOpen = $state(false);
  let activeSuggestionIndex = $state(0);
  let currentQuery = $state("");
  let replacementStart = $state(-1);
  let replacementEnd = $state(-1);
  let suggestionLeft = $state(12);
  let suggestionTop = $state(0);
  let suggestionPlacement: "above" | "below" = $state("above");
  let hasMeasuredSuggestionPosition = $state(false);
  let blurTimeout: ReturnType<typeof setTimeout> | null = $state(null);
  let mirrorBeforeText = $state(" ");
  let highlightSyncFrame: number | null = null;
  let pendingLocalValue: string | null = null;
  let lastObservedValue = "";

  const variablePattern = /{{\s*(\$[A-Za-z0-9_.-]+(?:\[\d+\])?|[A-Za-z0-9_.-]+)\s*}}/g;
  const dynamicVariableOptions: VariableOption[] = [
    { key: "$guid", value: "dynamic UUID v4", isSecret: false, isDynamic: true },
    { key: "$randomUUID", value: "dynamic UUID v4", isSecret: false, isDynamic: true },
    { key: "$timestamp", value: "current Unix timestamp", isSecret: false, isDynamic: true },
    { key: "$isoTimestamp", value: "current ISO 8601 timestamp", isSecret: false, isDynamic: true },
    { key: "$randomAlphaNumeric", value: "random alphanumeric value", isSecret: false, isDynamic: true },
    { key: "$randomBoolean", value: "random boolean", isSecret: false, isDynamic: true },
    { key: "$randomInt", value: "random integer", isSecret: false, isDynamic: true },
    { key: "$randomColor", value: "random color name", isSecret: false, isDynamic: true },
    { key: "$randomHexColor", value: "random hex color", isSecret: false, isDynamic: true },
    { key: "$randomAbbreviation", value: "random uppercase abbreviation", isSecret: false, isDynamic: true },
    { key: "$randomIP", value: "random IPv4 address", isSecret: false, isDynamic: true },
    { key: "$randomIPV6", value: "random IPv6 address", isSecret: false, isDynamic: true },
    { key: "$randomMACAddress", value: "random MAC address", isSecret: false, isDynamic: true },
    { key: "$randomPassword", value: "random password", isSecret: false, isDynamic: true },
    { key: "$randomLocale", value: "random locale", isSecret: false, isDynamic: true },
    { key: "$randomUserAgent", value: "random user agent", isSecret: false, isDynamic: true },
    { key: "$randomProtocol", value: "random protocol", isSecret: false, isDynamic: true },
    { key: "$randomSemver", value: "random semantic version", isSecret: false, isDynamic: true }
  ];

  let availableVariables = $derived(getAvailableVariables(variables));
  let filteredVariables = $derived(
    getFilteredVariables(availableVariables, dynamicVariableOptions, currentQuery)
  );
  let usedVariables = $derived(
    getUsedVariables(value, availableVariables, dynamicVariableOptions)
  );
  let hasHighlightOverlay = $derived(highlightTokens.length > 0);

  $effect(() => {
    const nextValue = value;

    if (nextValue === lastObservedValue) {
      return;
    }

    lastObservedValue = nextValue;

    if (pendingLocalValue === nextValue) {
      pendingLocalValue = null;
      return;
    }

    scheduleHighlightOverlaySync();
  });

  $effect(() => {
    if (!fieldElement) {
      return;
    }

    fieldElement.setAttribute('autocorrect', 'off');
  });
  $effect(() => {
    if (!hasHighlightOverlay || !fieldElement || !highlightOverlayElement || !highlightContentElement) {
      return;
    }

    const activeFieldElement = fieldElement;
    const resizeObserver = new ResizeObserver(() => scheduleHighlightOverlaySync());

    scheduleHighlightOverlaySync();
    resizeObserver.observe(activeFieldElement);
    activeFieldElement.addEventListener('scroll', syncHighlightOverlay);

    return () => {
      activeFieldElement.removeEventListener('scroll', syncHighlightOverlay);
      resizeObserver.disconnect();
      if (highlightSyncFrame !== null) {
        cancelAnimationFrame(highlightSyncFrame);
        highlightSyncFrame = null;
      }
    };
  });

  $effect(() => {
    value;
    highlightTokens;

    if (!hasHighlightOverlay) {
      return;
    }

    void tick().then(() => scheduleHighlightOverlaySync());
  });

  function clampSuggestionIndex() {
    if (!filteredVariables.length) {
      activeSuggestionIndex = 0;
    } else if (activeSuggestionIndex >= filteredVariables.length) {
      activeSuggestionIndex = 0;
    }
  }

  function getAvailableVariables(rows: EnvironmentVariable[]): VariableOption[] {
    const seen: Record<string, true> = {};

    return rows
      .filter((row) => row.enabled && row.key.trim())
      .map((row) => ({
        key: row.key.trim(),
        value: row.value,
        isSecret: row.isSecret
      }))
      .filter((row) => {
        const lookupKey = row.key.toLowerCase();

        if (seen[lookupKey]) {
          return false;
        }

        seen[lookupKey] = true;
        return true;
      });
  }

  function getFilteredVariables(
    rows: VariableOption[],
    dynamicRows: VariableOption[],
    query: string
  ) {
    const normalizedQuery = query.trim().toLowerCase();

    if (!normalizedQuery) {
      return rows;
    }

    if (normalizedQuery.startsWith("$")) {
      const dynamicQuery = normalizedQuery.replace(/\[\d*$/, "");
      return dynamicRows.filter((row) => row.key.toLowerCase().includes(dynamicQuery));
    }

    return rows.filter((row) => row.key.toLowerCase().includes(normalizedQuery));
  }

  function getUsedVariables(
    fieldValue: string,
    rows: VariableOption[],
    dynamicRows: VariableOption[]
  ): UsedVariable[] {
    const variableLookup = new Map(rows.map((row) => [row.key, row]));
    const seen: Record<string, true> = {};
    const result: UsedVariable[] = [];

    for (const match of fieldValue.matchAll(variablePattern)) {
      const key = match[1]?.trim();

      if (!key || seen[key]) {
        continue;
      }

      seen[key] = true;

      if (variableLookup.has(key)) {
        const resolvedVariable = variableLookup.get(key);
        const resolvedValue = resolvedVariable?.value ?? "";
        result.push({
          key,
          value: resolvedVariable?.isSecret ? null : resolvedValue,
          isResolved: true,
          tooltip: resolvedVariable?.isSecret
            ? `${key}: secret value`
            : `${key}: ${resolvedValue || "(empty value)"}`
        });
      } else {
        const dynamicTooltip = describeDynamicVariable(key, dynamicRows);

        if (dynamicTooltip) {
          result.push({
            key,
            value: null,
            isResolved: true,
            tooltip: `${key}: ${dynamicTooltip}`
          });
          continue;
        }

        result.push({
          key,
          value: null,
          isResolved: false,
          tooltip: `${key}: not found in the active environment`
        });
      }
    }

    return result;
  }

  function describeDynamicVariable(key: string, rows: VariableOption[]) {
    const normalizedKey = key.trim();

    const builtInMatch = rows.find((row) => row.key === normalizedKey);
    if (builtInMatch) {
      return builtInMatch.value;
    }

    const randomAlphaNumericMatch = normalizedKey.match(/^\$randomAlphaNumeric\[(\d+)\]$/);
    if (randomAlphaNumericMatch) {
      const length = Number(randomAlphaNumericMatch[1]);
      return `random alphanumeric value (${length} ${length === 1 ? "character" : "characters"})`;
    }

    return null;
  }

  function updateValue(nextValue: string) {
    pendingLocalValue = nextValue;
    lastObservedValue = nextValue;
    value = nextValue;
    onValueInput(nextValue);
    scheduleHighlightOverlaySync();
  }
  function scheduleHighlightOverlaySync() {
    if (!hasHighlightOverlay || !fieldElement || !highlightOverlayElement) {
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
    if (!fieldElement || !highlightOverlayElement || !highlightContentElement) {
      return;
    }

    const computedStyle = window.getComputedStyle(fieldElement);

    highlightOverlayElement.style.width = fieldElement.offsetWidth + 'px';
    highlightOverlayElement.style.height = fieldElement.offsetHeight + 'px';
    highlightOverlayElement.style.boxSizing = computedStyle.boxSizing;
    highlightOverlayElement.style.padding = '0';
    highlightOverlayElement.style.borderWidth = computedStyle.borderWidth;
    highlightOverlayElement.style.borderStyle = computedStyle.borderStyle;
    highlightOverlayElement.style.borderColor = 'transparent';
    highlightOverlayElement.style.font = computedStyle.font;
    highlightOverlayElement.style.letterSpacing = computedStyle.letterSpacing;
    highlightOverlayElement.style.lineHeight = computedStyle.lineHeight;
    highlightOverlayElement.style.textAlign = computedStyle.textAlign;
    highlightOverlayElement.style.textIndent = computedStyle.textIndent;
    highlightOverlayElement.style.textTransform = computedStyle.textTransform;
    highlightOverlayElement.style.tabSize = computedStyle.tabSize;
    highlightOverlayElement.style.overflowX = 'hidden';
    highlightOverlayElement.style.overflowY = 'hidden';
    highlightOverlayElement.style.resize = 'none';

    highlightContentElement.style.width = fieldElement.clientWidth + 'px';
    highlightContentElement.style.boxSizing = computedStyle.boxSizing;
    highlightContentElement.style.padding = computedStyle.padding;
    highlightContentElement.style.font = computedStyle.font;
    highlightContentElement.style.letterSpacing = computedStyle.letterSpacing;
    highlightContentElement.style.lineHeight = computedStyle.lineHeight;
    highlightContentElement.style.textAlign = computedStyle.textAlign;
    highlightContentElement.style.textIndent = computedStyle.textIndent;
    highlightContentElement.style.textTransform = computedStyle.textTransform;
    highlightContentElement.style.tabSize = computedStyle.tabSize;
    highlightContentElement.style.whiteSpace = multiline ? computedStyle.whiteSpace : 'pre';
    highlightContentElement.style.overflowWrap = multiline ? computedStyle.overflowWrap : 'normal';
    highlightContentElement.style.wordBreak = multiline ? computedStyle.wordBreak : 'normal';
    highlightContentElement.style.transform = `translate(${-fieldElement.scrollLeft}px, ${-fieldElement.scrollTop}px)`;
  }

  function getTokenContext(fieldValue: string, cursor: number) {
    const beforeCursor = fieldValue.slice(0, cursor);
    const start = beforeCursor.lastIndexOf("{{");

    if (start === -1) {
      return null;
    }

    const closedBeforeCursor = beforeCursor.lastIndexOf("}}");
    if (closedBeforeCursor > start) {
      return null;
    }

    const typedFragment = fieldValue.slice(start + 2, cursor);
    if (/[{}\n\r]/.test(typedFragment)) {
      return null;
    }

    let end = cursor;
    const closingIndex = fieldValue.indexOf("}}", cursor);

    if (closingIndex !== -1) {
      const trailingFragment = fieldValue.slice(cursor, closingIndex);
      if (!/[{}\n\r]/.test(trailingFragment)) {
        end = closingIndex + 2;
      }
    }

    return {
      start,
      end,
      query: typedFragment.trim()
    };
  }

  function updateAutocompleteState() {
    if (!fieldElement) {
      closeSuggestions();
      return;
    }

    const selectionStart = fieldElement.selectionStart ?? value.length;
    const tokenContext = getTokenContext(value, selectionStart);

    if (!tokenContext) {
      closeSuggestions();
      return;
    }

    replacementStart = tokenContext.start;
    replacementEnd = tokenContext.end;
    currentQuery = tokenContext.query;
    hasMeasuredSuggestionPosition = false;
    clampSuggestionIndex();
    updateSuggestionAnchor();
    isSuggestionsOpen = true;
    void tick().then(() => updateSuggestionAnchor());
  }

  function copyFieldStyles() {
    if (!fieldElement || !mirrorElement) {
      return;
    }

    const computedStyle = window.getComputedStyle(fieldElement);
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

    mirrorElement.style.width = `${fieldElement.clientWidth}px`;
    mirrorElement.style.whiteSpace = multiline ? "pre-wrap" : "pre";
    mirrorElement.style.overflowWrap = multiline ? "break-word" : "normal";
    mirrorElement.style.wordBreak = multiline ? "break-word" : "normal";
  }

  function updateSuggestionAnchor() {
    if (!fieldElement || !mirrorElement || !mirrorTextElement || !mirrorCaretElement) {
      return;
    }

    copyFieldStyles();

    const selectionStart = fieldElement.selectionStart ?? value.length;
    const beforeCursor = value.slice(0, selectionStart) || " ";

    mirrorBeforeText = beforeCursor;

    const caretLeft = mirrorCaretElement.offsetLeft - fieldElement.scrollLeft;
    const caretTop = mirrorCaretElement.offsetTop - fieldElement.scrollTop;
    const shellWidth = fieldElement.clientWidth;

    suggestionLeft = Math.max(8, Math.min(caretLeft, Math.max(8, shellWidth - 240)));
    suggestionPlacement = caretTop > 88 ? "above" : "below";
    suggestionTop = suggestionPlacement === "above" ? caretTop - 8 : caretTop + 28;
    hasMeasuredSuggestionPosition = true;
  }

  async function applySuggestion(variableKey: string) {
    if (!fieldElement || disabled || replacementStart < 0 || replacementEnd < 0) {
      return;
    }

    const insertText = `{{${variableKey}}}`;
    insertTextIntoEditableControl(fieldElement, insertText, {
      selectionStart: replacementStart,
      selectionEnd: replacementEnd,
      undoBaselineSelection: "collapse-end"
    });
    closeSuggestions();
    await tick();
    updateAutocompleteState();
  }

  function closeSuggestions() {
    isSuggestionsOpen = false;
    currentQuery = "";
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

  function handleInput(event: Event) {
    const target = event.currentTarget as HTMLInputElement | HTMLTextAreaElement;
    updateValue(target.value);
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

  function handleCursorMovement() {
    updateAutocompleteState();
  }

  function handleKeydown(event: KeyboardEvent) {
    if (isSuggestionsOpen && filteredVariables.length) {
      clampSuggestionIndex();
      if (event.key === "ArrowDown") {
        event.preventDefault();
        activeSuggestionIndex = (activeSuggestionIndex + 1) % filteredVariables.length;
        return;
      }

      if (event.key === "ArrowUp") {
        event.preventDefault();
        activeSuggestionIndex = (activeSuggestionIndex - 1 + filteredVariables.length) % filteredVariables.length;
        return;
      }

      if (event.key === "Enter" || event.key === "Tab") {
        event.preventDefault();
        void applySuggestion(filteredVariables[activeSuggestionIndex].key);
        return;
      }

      if (event.key === "Escape") {
        event.preventDefault();
        closeSuggestions();
        return;
      }
    }

    onExtraKeydown?.(event);
  }

  function handleSuggestionPointerDown(event: MouseEvent) {
    event.preventDefault();
    clearBlurTimeout();
  }

  function getFieldClasses() {
    return [
      className,
      usedVariables.length ? "variable-aware-active" : "",
      hasHighlightOverlay && "variable-input-highlighted"
    ].filter(Boolean).join(" ");
  }

  function getSuggestionStyle() {
    return `left: ${suggestionLeft}px; top: ${suggestionTop}px;`;
  }

  function getHighlightOverlayClasses() {
    return [
      "variable-highlight-overlay",
      multiline ? "variable-highlight-overlay-multiline" : "variable-highlight-overlay-singleline",
      highlightOverlayClassName
    ].filter(Boolean).join(" ");
  }

  function getHighlightTokenClass(type: string) {
    switch (type) {
      case "key":
        return "jt-key";
      case "string":
        return "jt-string";
      case "number":
        return "jt-number";
      case "bool":
        return "jt-bool";
      case "null":
        return "jt-null";
      case "bracket":
        return "jt-bracket";
      case "colon":
        return "jt-colon";
      case "comma":
        return "jt-comma";
      case "variable":
        return "jt-variable";
      default:
        return "";
    }
  }
</script>

<div class="variable-field">
  <div class="variable-input-shell">
    {#if hasHighlightOverlay}
      <pre
        class={getHighlightOverlayClasses()}
        aria-hidden="true"
        spellcheck={false}
        bind:this={highlightOverlayElement}
      ><code class="variable-highlight-content" bind:this={highlightContentElement}>{#each highlightTokens as token, index (index)}{#if getHighlightTokenClass(token.type)}<span class={getHighlightTokenClass(token.type)}>{token.value}</span>{:else}{token.value}{/if}{/each}</code></pre>
    {/if}

    {#if multiline}
      <textarea
        bind:this={fieldElement}
        {id}
        class={getFieldClasses()}
        {placeholder}
        {spellcheck}
        autocapitalize=off
        autocomplete=off
        {disabled}
        aria-label={ariaLabel || undefined}
        aria-invalid={ariaInvalid ? "true" : undefined}
        value={value}
        onblur={handleBlur}
        onclick={handleCursorMovement}
        onfocus={handleFocus}
        oninput={handleInput}
        onkeydown={handleKeydown}
        onkeyup={handleCursorMovement}
      ></textarea>
    {:else}
      <input
        bind:this={fieldElement}
        {id}
        class={getFieldClasses()}
        {placeholder}
        {spellcheck}
        autocapitalize=off
        autocomplete=off
        {disabled}
        {type}
        {list}
        aria-label={ariaLabel || undefined}
        aria-invalid={ariaInvalid ? "true" : undefined}
        value={value}
        onblur={handleBlur}
        onclick={handleCursorMovement}
        onfocus={handleFocus}
        oninput={handleInput}
        onkeydown={handleKeydown}
        onkeyup={handleCursorMovement}
      />
    {/if}

    {#if isSuggestionsOpen}
      <div
        class={["variable-suggestions", !hasMeasuredSuggestionPosition && "variable-suggestions-hidden", suggestionPlacement === "below" && "variable-suggestions-below"]}
        role="listbox"
        aria-label="Environment variable suggestions"
        style={getSuggestionStyle()}
      >
        {#if filteredVariables.length}
          {#each filteredVariables as variable, index (variable.key)}
            <button
              class={["variable-suggestion", index === activeSuggestionIndex && "variable-suggestion-active"]}
              type="button"
              onclick={() => applySuggestion(variable.key)}
              onmousedown={handleSuggestionPointerDown}
            >
              <strong>{variable.key}</strong>
              <span>{variable.isSecret ? "secret value" : variable.value || "(empty value)"}</span>
            </button>
          {/each}
        {:else}
          <div class="variable-suggestion-empty">No matching environment variables.</div>
        {/if}
      </div>
    {/if}

    <div aria-hidden="true" class={["variable-input-mirror", multiline && "variable-input-mirror-multiline"]} bind:this={mirrorElement}>
      <span bind:this={mirrorTextElement}>{mirrorBeforeText}</span><span bind:this={mirrorCaretElement} class="variable-input-caret-marker">&#8203;</span>
    </div>
  </div>

  {#if usedVariables.length}
    <div class="variable-pill-list">
      {#each usedVariables as variable (variable.key)}
        <span
          class={["variable-pill", !variable.isResolved && "variable-pill-unresolved"]}
          title={variable.tooltip}
        >
          {variable.key}
        </span>
      {/each}
    </div>
  {/if}
</div>
