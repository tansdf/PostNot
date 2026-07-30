<script lang="ts">
  import type { EnvironmentVariable } from "$lib/api/types";
  import VariableField from "$lib/components/request/VariableField.svelte";
  import { insertTextIntoEditableControl } from "$lib/dom-editing";

  type HighlightToken = {
    type: string;
    value: string;
  };

  let {
    value = "",
    variables = [],
    onValueInput = () => {},
    onBlur = () => {},
    onFocus = () => {},
    className = "body-textarea",
    placeholder = "",
    ariaLabel = "JSON editor",
    ariaInvalid = false
  }: {
    value?: string;
    variables?: EnvironmentVariable[];
    onValueInput?: (value: string) => void;
    onBlur?: () => void;
    onFocus?: () => void;
    className?: string;
    placeholder?: string;
    ariaLabel?: string;
    ariaInvalid?: boolean;
  } = $props();

  const variableTokenPattern = /{{\s*(?:\$[A-Za-z0-9_.-]+(?:\[\d+\])?|[A-Za-z0-9_.-]+)\s*}}/g;

  function matchVariableToken(source: string, start: number) {
    return source.slice(start).match(/^{{\s*(?:\$[A-Za-z0-9_.-]+(?:\[\d+\])?|[A-Za-z0-9_.-]+)\s*}}/)?.[0] ?? null;
  }

  function pushVariableAwareText(tokens: HighlightToken[], tokenValue: string, baseType: HighlightToken["type"]) {
    if (!tokenValue) {
      return;
    }

    let lastIndex = 0;

    for (const match of tokenValue.matchAll(variableTokenPattern)) {
      const index = match.index ?? 0;

      if (index > lastIndex) {
        tokens.push({ type: baseType, value: tokenValue.slice(lastIndex, index) });
      }

      tokens.push({ type: "variable", value: match[0] });
      lastIndex = index + match[0].length;
    }

    if (lastIndex < tokenValue.length) {
      tokens.push({ type: baseType, value: tokenValue.slice(lastIndex) });
    } else if (lastIndex === 0) {
      tokens.push({ type: baseType, value: tokenValue });
    }
  }

  function tokenizeJson(json: string): HighlightToken[] {
    const tokens: HighlightToken[] = [];
    let index = 0;

    while (index < json.length) {
      const variableToken = matchVariableToken(json, index);

      if (variableToken) {
        tokens.push({ type: "variable", value: variableToken });
        index += variableToken.length;
        continue;
      }

      const character = json[index];

      if (character === '"') {
        const start = index;
        index += 1;
        while (index < json.length && json[index] !== '"') {
          if (json[index] === "\\") index += 1;
          index += 1;
        }
        index += 1;
        const raw = json.slice(start, index);
        let nextNonWhitespace = index;
        while (nextNonWhitespace < json.length && (json[nextNonWhitespace] === " " || json[nextNonWhitespace] === "\t")) {
          nextNonWhitespace += 1;
        }
        pushVariableAwareText(tokens, raw, json[nextNonWhitespace] === ":" ? "key" : "string");
        continue;
      }

      if (character === "-" || (character >= "0" && character <= "9")) {
        const start = index;
        while (index < json.length && /[0-9.eE+\-]/.test(json[index])) index += 1;
        tokens.push({ type: "number", value: json.slice(start, index) });
        continue;
      }

      if (json.startsWith("true", index)) {
        tokens.push({ type: "bool", value: "true" });
        index += 4;
        continue;
      }
      if (json.startsWith("false", index)) {
        tokens.push({ type: "bool", value: "false" });
        index += 5;
        continue;
      }
      if (json.startsWith("null", index)) {
        tokens.push({ type: "null", value: "null" });
        index += 4;
        continue;
      }
      if ("{}[]".includes(character)) {
        tokens.push({ type: "bracket", value: character });
        index += 1;
        continue;
      }
      if (character === ":") {
        tokens.push({ type: "colon", value: ":" });
        index += 1;
        continue;
      }
      if (character === ",") {
        tokens.push({ type: "comma", value: "," });
        index += 1;
        continue;
      }
      if (character === "\n") {
        tokens.push({ type: "newline", value: "\n" });
        index += 1;
        continue;
      }
      if (character === " " || character === "\t") {
        const start = index;
        while (index < json.length && (json[index] === " " || json[index] === "\t")) index += 1;
        tokens.push({ type: "indent", value: json.slice(start, index) });
        continue;
      }

      tokens.push({ type: "text", value: character });
      index += 1;
    }

    return tokens;
  }

  function handleJsonKeydown(event: KeyboardEvent) {
    const textarea = event.target as HTMLTextAreaElement;
    if (textarea.tagName !== "TEXTAREA") return;

    if (event.key === "Enter") {
      event.preventDefault();
      const { selectionStart } = textarea;
      const currentValue = textarea.value;
      const lineStart = currentValue.lastIndexOf("\n", selectionStart - 1) + 1;
      const currentLine = currentValue.slice(lineStart, selectionStart);
      const indent = currentLine.match(/^(\s*)/)?.[1] ?? "";
      const characterBefore = currentValue[selectionStart - 1];
      const characterAfter = currentValue[selectionStart];

      if ((characterBefore === "{" || characterBefore === "[") && (characterAfter === "}" || characterAfter === "]")) {
        insertTextIntoEditableControl(textarea, `\n${indent}  \n${indent}`, {
          selectionStart,
          selectionEnd: selectionStart,
          cursorOffset: indent.length + 3
        });
      } else {
        const nextIndent = characterBefore === "{" || characterBefore === "[" ? `${indent}  ` : indent;
        insertTextIntoEditableControl(textarea, `\n${nextIndent}`, {
          selectionStart,
          selectionEnd: selectionStart
        });
      }
      return;
    }

    if (event.key === "Tab") {
      event.preventDefault();
      insertTextIntoEditableControl(textarea, "  ", {
        selectionStart: textarea.selectionStart,
        selectionEnd: textarea.selectionEnd
      });
    }
  }

  let highlightTokens = $derived(tokenizeJson(value));
</script>

<div class="json-editor-shell" onfocusout={onBlur} onfocusin={onFocus}>
  <VariableField
    className={`${className} json-editor-textarea`}
    multiline={true}
    {value}
    {variables}
    {highlightTokens}
    highlightOverlayClassName="json-editor-overlay"
    {placeholder}
    spellcheck={false}
    {ariaLabel}
    {ariaInvalid}
    onValueInput={onValueInput}
    onExtraKeydown={handleJsonKeydown}
  />
</div>
