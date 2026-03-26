<script lang="ts">
  type Token = { type: string; value: string };

  let {
    source = "",
    maxHeight = "clamp(16rem, 62vh, 44rem)"
  }: {
    source?: string;
    maxHeight?: string;
  } = $props();

  function tokenize(json: string): Token[] {
    const tokens: Token[] = [];
    let i = 0;

    while (i < json.length) {
      const ch = json[i];

      if (ch === '"') {
        const start = i;
        i++;
        while (i < json.length && json[i] !== '"') {
          if (json[i] === '\\') i++;
          i++;
        }
        i++;
        const raw = json.slice(start, i);

        let j = i;
        while (j < json.length && (json[j] === ' ' || json[j] === '\t')) j++;
        if (json[j] === ':') {
          tokens.push({ type: "key", value: raw });
        } else {
          tokens.push({ type: "string", value: raw });
        }
        continue;
      }

      if (ch === '-' || (ch >= '0' && ch <= '9')) {
        const start = i;
        if (ch === '-') i++;
        while (i < json.length && ((json[i] >= '0' && json[i] <= '9') || json[i] === '.' || json[i] === 'e' || json[i] === 'E' || json[i] === '+' || json[i] === '-')) {
          if ((json[i] === 'e' || json[i] === 'E') && i > start) { i++; continue; }
          if ((json[i] === '+' || json[i] === '-') && i > start + 1 && (json[i-1] === 'e' || json[i-1] === 'E')) { i++; continue; }
          if (json[i] >= '0' && json[i] <= '9') { i++; continue; }
          if (json[i] === '.' && i > start) { i++; continue; }
          break;
        }
        tokens.push({ type: "number", value: json.slice(start, i) });
        continue;
      }

      if (json.startsWith("true", i)) {
        tokens.push({ type: "boolean", value: "true" });
        i += 4;
        continue;
      }
      if (json.startsWith("false", i)) {
        tokens.push({ type: "boolean", value: "false" });
        i += 5;
        continue;
      }
      if (json.startsWith("null", i)) {
        tokens.push({ type: "null", value: "null" });
        i += 4;
        continue;
      }

      if (ch === '{' || ch === '}' || ch === '[' || ch === ']') {
        tokens.push({ type: "bracket", value: ch });
        i++;
        continue;
      }

      if (ch === ':') {
        tokens.push({ type: "colon", value: ": " });
        i++;
        if (json[i] === ' ') i++;
        continue;
      }

      if (ch === ',') {
        tokens.push({ type: "comma", value: "," });
        i++;
        continue;
      }

      if (ch === '\n') {
        tokens.push({ type: "newline", value: "\n" });
        i++;
        continue;
      }

      if (ch === ' ' || ch === '\t') {
        const start = i;
        while (i < json.length && (json[i] === ' ' || json[i] === '\t')) i++;
        tokens.push({ type: "indent", value: json.slice(start, i) });
        continue;
      }

      tokens.push({ type: "text", value: ch });
      i++;
    }

    return tokens;
  }

  function tryFormat(raw: string): string {
    if (!raw) return "";
    try {
      return JSON.stringify(JSON.parse(raw), null, 2);
    } catch {
      return raw;
    }
  }

  let isJson = $derived((() => {
    if (!source) return false;
    try { JSON.parse(source); return true; } catch { return false; }
  })());

  let formatted = $derived(tryFormat(source));
  let tokens = $derived(isJson ? tokenize(formatted) : []);

  function handleCopy() {
    void navigator.clipboard.writeText(formatted || source);
  }
</script>

{#if !source}
  <pre class="json-viewer" style:max-height={maxHeight}></pre>
{:else if isJson}
  <div class="json-viewer-wrap">
    <button class="json-copy-button" type="button" onclick={handleCopy} title="Copy to clipboard">Copy</button>
    <pre class="json-viewer json-highlighted" style:max-height={maxHeight}>{#each tokens as token, i (i)}{#if token.type === "key"}<span class="jt-key">{token.value}</span>{:else if token.type === "string"}<span class="jt-string">{token.value}</span>{:else if token.type === "number"}<span class="jt-number">{token.value}</span>{:else if token.type === "boolean"}<span class="jt-bool">{token.value}</span>{:else if token.type === "null"}<span class="jt-null">{token.value}</span>{:else if token.type === "bracket"}<span class="jt-bracket">{token.value}</span>{:else if token.type === "colon"}<span class="jt-colon">{token.value}</span>{:else if token.type === "comma"}<span class="jt-comma">{token.value}</span>{:else}{token.value}{/if}{/each}</pre>
  </div>
{:else}
  <pre class="json-viewer" style:max-height={maxHeight}>{source}</pre>
{/if}
