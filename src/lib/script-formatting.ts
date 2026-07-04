export type ScriptToken = {
  type: "text" | "keyword" | "api" | "property" | "string" | "template" | "comment" | "number" | "boolean" | "null" | "operator";
  value: string;
};

const KEYWORDS = new Set([
  "async",
  "await",
  "break",
  "catch",
  "const",
  "continue",
  "else",
  "finally",
  "for",
  "function",
  "if",
  "let",
  "new",
  "return",
  "throw",
  "try",
  "typeof",
  "var",
  "while"
]);

const BOOLEAN_LITERALS = new Set(["false", "true"]);
const NULL_LITERALS = new Set(["null", "undefined"]);
const OPERATORS = new Set(["{", "}", "(", ")", "[", "]", ";", ",", ".", ":", "?", "+", "-", "*", "/", "%", "=", "!", "<", ">", "&", "|"]);

export function tokenizeScript(source: string): ScriptToken[] {
  const tokens: ScriptToken[] = [];
  let index = 0;

  while (index < source.length) {
    const comment = readComment(source, index);
    if (comment) {
      tokens.push({ type: "comment", value: comment });
      index += comment.length;
      continue;
    }

    const quotedString = readQuotedString(source, index);
    if (quotedString) {
      tokens.push({ type: "string", value: quotedString });
      index += quotedString.length;
      continue;
    }

    const templateString = readTemplateString(source, index);
    if (templateString) {
      tokens.push({ type: "template", value: templateString });
      index += templateString.length;
      continue;
    }

    const numberToken = readNumber(source, index);
    if (numberToken) {
      tokens.push({ type: "number", value: numberToken });
      index += numberToken.length;
      continue;
    }

    const identifier = readIdentifier(source, index);
    if (identifier) {
      tokens.push({ type: classifyIdentifier(source, index, identifier), value: identifier });
      index += identifier.length;
      continue;
    }

    const ch = source[index] ?? "";
    tokens.push({ type: OPERATORS.has(ch) ? "operator" : "text", value: ch });
    index += 1;
  }

  return mergeAdjacentTextTokens(tokens);
}

export function formatScript(source: string) {
  const trimmedSource = source.trim();
  if (!trimmedSource) {
    return "";
  }

  const lines: string[] = [];
  let current = "";
  let indentLevel = 0;
  let index = 0;

  const pushLine = (nextIndentLevel = indentLevel) => {
    const line = current.trim();
    if (line) {
      lines.push(`${"  ".repeat(Math.max(0, nextIndentLevel))}${line}`);
    }
    current = "";
  };

  while (index < trimmedSource.length) {
    const comment = readComment(trimmedSource, index);
    if (comment) {
      if (comment.startsWith("//")) {
        current += comment.trimEnd();
        pushLine();
      } else {
        current += comment;
      }
      index += comment.length;
      continue;
    }

    const quotedString = readQuotedString(trimmedSource, index);
    if (quotedString) {
      current += quotedString;
      index += quotedString.length;
      continue;
    }

    const templateString = readTemplateString(trimmedSource, index);
    if (templateString) {
      current += templateString;
      index += templateString.length;
      continue;
    }

    const ch = trimmedSource[index] ?? "";

    if (ch === "{") {
      const prefix = trimTrailingSpaces(current);
      current = `${prefix}${prefix.endsWith("=>") ? "" : " "}{`;
      pushLine();
      indentLevel += 1;
      index += 1;
      continue;
    }

    if (ch === "}") {
      pushLine();
      indentLevel = Math.max(0, indentLevel - 1);
      current = "}";
      index += 1;
      continue;
    }

    if (ch === ";") {
      current = trimTrailingSpaces(current) + ";";
      pushLine();
      index += 1;
      continue;
    }

    if (ch === "\n" || ch === "\r") {
      pushLine();
      index += ch === "\r" && trimmedSource[index + 1] === "\n" ? 2 : 1;
      continue;
    }

    if (/\s/.test(ch)) {
      if (current && !current.endsWith(" ")) {
        current += " ";
      }
      index += 1;
      continue;
    }

    current += ch;
    index += 1;
  }

  pushLine();

  return lines.join("\n");
}

function classifyIdentifier(source: string, start: number, identifier: string): ScriptToken["type"] {
  if (identifier === "pn") {
    return "api";
  }

  if (KEYWORDS.has(identifier)) {
    return "keyword";
  }

  if (BOOLEAN_LITERALS.has(identifier)) {
    return "boolean";
  }

  if (NULL_LITERALS.has(identifier)) {
    return "null";
  }

  let cursor = start - 1;
  while (cursor >= 0 && /\s/.test(source[cursor] ?? "")) {
    cursor -= 1;
  }

  return source[cursor] === "." ? "property" : "text";
}

function readIdentifier(source: string, start: number) {
  const first = source[start] ?? "";
  if (!/[A-Za-z_$]/.test(first)) {
    return null;
  }

  let end = start + 1;
  while (end < source.length && /[A-Za-z0-9_$]/.test(source[end] ?? "")) {
    end += 1;
  }

  return source.slice(start, end);
}

function readNumber(source: string, start: number) {
  if (!/[0-9]/.test(source[start] ?? "")) {
    return null;
  }

  let end = start + 1;
  while (end < source.length && /[0-9.eE_+-]/.test(source[end] ?? "")) {
    end += 1;
  }

  return source.slice(start, end);
}

function readComment(source: string, start: number) {
  if (source.startsWith("//", start)) {
    const newline = source.indexOf("\n", start + 2);
    return newline === -1 ? source.slice(start) : source.slice(start, newline);
  }

  if (source.startsWith("/*", start)) {
    const end = source.indexOf("*/", start + 2);
    return end === -1 ? source.slice(start) : source.slice(start, end + 2);
  }

  return null;
}

function readQuotedString(source: string, start: number) {
  const quote = source[start];
  if (quote !== "'" && quote !== '"') {
    return null;
  }

  return source.slice(start, readUntilQuote(source, start + 1, quote) + 1);
}

function readTemplateString(source: string, start: number) {
  if (source[start] !== "`") {
    return null;
  }

  return source.slice(start, readUntilQuote(source, start + 1, "`") + 1);
}

function readUntilQuote(source: string, start: number, quote: string) {
  let index = start;
  while (index < source.length) {
    if (source[index] === "\\") {
      index += 2;
      continue;
    }

    if (source[index] === quote) {
      return index;
    }

    index += 1;
  }

  return source.length - 1;
}

function mergeAdjacentTextTokens(tokens: ScriptToken[]) {
  const result: ScriptToken[] = [];

  for (const token of tokens) {
    const previous = result[result.length - 1];
    if (previous && previous.type === "text" && token.type === "text") {
      previous.value += token.value;
    } else {
      result.push({ ...token });
    }
  }

  return result;
}

function trimTrailingSpaces(value: string) {
  return value.replace(/\s+$/g, "");
}
