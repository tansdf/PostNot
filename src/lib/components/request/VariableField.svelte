<script lang="ts">
  import { tick } from "svelte";

  import type { KeyValueRow } from "$lib/api/types";

  type VariableOption = {
    key: string;
    value: string;
  };

  type UsedVariable = {
    key: string;
    value: string | null;
    isResolved: boolean;
    tooltip: string;
  };

  export let value = "";
  export let variables: KeyValueRow[] = [];
  export let onValueInput: (value: string) => void = () => {};
  export let className = "";
  export let placeholder = "";
  export let id = "";
  export let type = "text";
  export let spellcheck = true;
  export let multiline = false;
  export let disabled = false;

  let fieldElement: HTMLInputElement | HTMLTextAreaElement | null = null;
  let mirrorElement: HTMLDivElement | null = null;
  let mirrorTextElement: HTMLSpanElement | null = null;
  let mirrorCaretElement: HTMLSpanElement | null = null;
  let isSuggestionsOpen = false;
  let activeSuggestionIndex = 0;
  let currentQuery = "";
  let replacementStart = -1;
  let replacementEnd = -1;
  let suggestionLeft = 12;
  let suggestionTop = 0;
  let suggestionPlacement: "above" | "below" = "above";
  let hasMeasuredSuggestionPosition = false;
  let blurTimeout: ReturnType<typeof setTimeout> | null = null;

  const variablePattern = /{{\s*([A-Za-z0-9_.-]+)\s*}}/g;

  $: availableVariables = getAvailableVariables(variables);
  $: filteredVariables = getFilteredVariables(availableVariables, currentQuery);
  $: if (!filteredVariables.length) {
    activeSuggestionIndex = 0;
  } else if (activeSuggestionIndex >= filteredVariables.length) {
    activeSuggestionIndex = 0;
  }
  $: usedVariables = getUsedVariables(value, availableVariables);
  $: if (isSuggestionsOpen) {
    void tick().then(() => updateSuggestionAnchor());
  }

  function getAvailableVariables(rows: KeyValueRow[]): VariableOption[] {
    const seen = new Set<string>();

    return rows
      .filter((row) => row.enabled && row.key.trim())
      .map((row) => ({
        key: row.key.trim(),
        value: row.value
      }))
      .filter((row) => {
        const lookupKey = row.key.toLowerCase();

        if (seen.has(lookupKey)) {
          return false;
        }

        seen.add(lookupKey);
        return true;
      });
  }

  function getFilteredVariables(rows: VariableOption[], query: string) {
    if (!query.trim()) {
      return rows;
    }

    const normalizedQuery = query.trim().toLowerCase();
    return rows.filter((row) => row.key.toLowerCase().includes(normalizedQuery));
  }

  function getUsedVariables(fieldValue: string, rows: VariableOption[]): UsedVariable[] {
    const variableLookup = new Map(rows.map((row) => [row.key, row.value]));
    const seen = new Set<string>();
    const result: UsedVariable[] = [];

    for (const match of fieldValue.matchAll(variablePattern)) {
      const key = match[1]?.trim();

      if (!key || seen.has(key)) {
        continue;
      }

      seen.add(key);

      if (variableLookup.has(key)) {
        const resolvedValue = variableLookup.get(key) ?? "";
        result.push({
          key,
          value: resolvedValue,
          isResolved: true,
          tooltip: `${key}: ${resolvedValue || "(empty value)"}`
        });
      } else {
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

  function updateValue(nextValue: string) {
    value = nextValue;
    onValueInput(nextValue);
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
    if (!fieldElement || !availableVariables.length) {
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

    mirrorTextElement.textContent = beforeCursor;
    mirrorCaretElement.textContent = "\u200b";

    const caretLeft = mirrorCaretElement.offsetLeft - fieldElement.scrollLeft;
    const caretTop = mirrorCaretElement.offsetTop - fieldElement.scrollTop;
    const shellWidth = fieldElement.clientWidth;

    suggestionLeft = Math.max(8, Math.min(caretLeft, Math.max(8, shellWidth - 240)));
    suggestionPlacement = caretTop > 88 ? "above" : "below";
    suggestionTop = suggestionPlacement === "above" ? caretTop - 8 : caretTop + 28;
    hasMeasuredSuggestionPosition = true;
  }

  async function applySuggestion(variableKey: string) {
    const nextValue = `${value.slice(0, replacementStart)}{{${variableKey}}}${value.slice(replacementEnd)}`;
    const nextCursor = replacementStart + variableKey.length + 4;

    updateValue(nextValue);
    closeSuggestions();
    await tick();
    fieldElement?.focus();
    fieldElement?.setSelectionRange(nextCursor, nextCursor);
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
    if (!isSuggestionsOpen || !filteredVariables.length) {
      return;
    }

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
    }
  }

  function handleSuggestionPointerDown(event: MouseEvent) {
    event.preventDefault();
    clearBlurTimeout();
  }

  function getFieldClasses() {
    return [className, usedVariables.length ? "variable-aware-active" : ""].filter(Boolean).join(" ");
  }

  function getSuggestionStyle() {
    return `left: ${suggestionLeft}px; top: ${suggestionTop}px;`;
  }
</script>

<div class="variable-field">
  <div class="variable-input-shell">
    {#if multiline}
      <textarea
        bind:this={fieldElement}
        {id}
        class={getFieldClasses()}
        {placeholder}
        {spellcheck}
        {disabled}
        value={value}
        on:blur={handleBlur}
        on:click={handleCursorMovement}
        on:focus={handleFocus}
        on:input={handleInput}
        on:keydown={handleKeydown}
        on:keyup={handleCursorMovement}
      ></textarea>
    {:else}
      <input
        bind:this={fieldElement}
        {id}
        class={getFieldClasses()}
        {placeholder}
        {spellcheck}
        {disabled}
        {type}
        value={value}
        on:blur={handleBlur}
        on:click={handleCursorMovement}
        on:focus={handleFocus}
        on:input={handleInput}
        on:keydown={handleKeydown}
        on:keyup={handleCursorMovement}
      />
    {/if}

    {#if isSuggestionsOpen}
      <div
        class:variable-suggestions-hidden={!hasMeasuredSuggestionPosition}
        class:variable-suggestions-below={suggestionPlacement === "below"}
        class="variable-suggestions"
        role="listbox"
        aria-label="Environment variable suggestions"
        style={getSuggestionStyle()}
      >
        {#if filteredVariables.length}
          {#each filteredVariables as variable, index (variable.key)}
            <button
              class:variable-suggestion-active={index === activeSuggestionIndex}
              class="variable-suggestion"
              type="button"
              on:click={() => applySuggestion(variable.key)}
              on:mousedown={handleSuggestionPointerDown}
            >
              <strong>{variable.key}</strong>
              <span>{variable.value || "(empty value)"}</span>
            </button>
          {/each}
        {:else}
          <div class="variable-suggestion-empty">No matching environment variables.</div>
        {/if}
      </div>
    {/if}

    <div aria-hidden="true" class:variable-input-mirror-multiline={multiline} class="variable-input-mirror" bind:this={mirrorElement}>
      <span bind:this={mirrorTextElement}></span><span bind:this={mirrorCaretElement} class="variable-input-caret-marker"></span>
    </div>
  </div>

  {#if usedVariables.length}
    <div class="variable-pill-list">
      {#each usedVariables as variable (variable.key)}
        <span
          class:variable-pill-unresolved={!variable.isResolved}
          class="variable-pill"
          title={variable.tooltip}
        >
          {variable.key}
        </span>
      {/each}
    </div>
  {/if}
</div>
