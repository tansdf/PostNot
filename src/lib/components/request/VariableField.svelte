<script lang="ts">
  import { tick } from "svelte";

  import type { EnvironmentVariable } from "$lib/api/types";

  type VariableOption = {
    key: string;
    value: string;
    isSecret: boolean;
  };

  type UsedVariable = {
    key: string;
    value: string | null;
    isResolved: boolean;
    tooltip: string;
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
    onExtraKeydown = undefined
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
    onExtraKeydown?: ((event: KeyboardEvent) => void) | undefined;
  } = $props();

  let fieldElement: HTMLInputElement | HTMLTextAreaElement | null = $state(null);
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

  const variablePattern = /{{\s*([A-Za-z0-9_.-]+)\s*}}/g;

  let availableVariables = $derived(getAvailableVariables(variables));
  let filteredVariables = $derived(getFilteredVariables(availableVariables, currentQuery));
  let usedVariables = $derived(getUsedVariables(value, availableVariables));

  $effect(() => {
    if (!filteredVariables.length) {
      activeSuggestionIndex = 0;
    } else if (activeSuggestionIndex >= filteredVariables.length) {
      activeSuggestionIndex = 0;
    }
  });

  $effect(() => {
    if (isSuggestionsOpen) {
      void tick().then(() => updateSuggestionAnchor());
    }
  });

  function getAvailableVariables(rows: EnvironmentVariable[]): VariableOption[] {
    const seen = new Set<string>();

    return rows
      .filter((row) => row.enabled && row.key.trim())
      .map((row) => ({
        key: row.key.trim(),
        value: row.value,
        isSecret: row.isSecret
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
    const variableLookup = new Map(rows.map((row) => [row.key, row]));
    const seen = new Set<string>();
    const result: UsedVariable[] = [];

    for (const match of fieldValue.matchAll(variablePattern)) {
      const key = match[1]?.trim();

      if (!key || seen.has(key)) {
        continue;
      }

      seen.add(key);

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
    if (isSuggestionsOpen && filteredVariables.length) {
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
        {disabled}
        {type}
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
      <span bind:this={mirrorTextElement}></span><span bind:this={mirrorCaretElement} class="variable-input-caret-marker"></span>
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
