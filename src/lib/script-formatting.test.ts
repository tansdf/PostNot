import { describe, expect, it } from "vitest";

import { formatScript, tokenizeScript } from "./script-formatting";

describe("tokenizeScript", () => {
  it("keeps token text identical to the script source", () => {
    const source = "await pn.variables.set('token', response.id); // save token";
    const tokens = tokenizeScript(source);

    expect(tokens.map((token) => token.value).join("")).toBe(source);
  });

  it("classifies common JavaScript and PostNot scripting tokens", () => {
    const tokens = tokenizeScript("await pn.test('ok', () => true);");

    expect(tokens).toEqual(
      expect.arrayContaining([
        { type: "keyword", value: "await" },
        { type: "api", value: "pn" },
        { type: "property", value: "test" },
        { type: "string", value: "'ok'" },
        { type: "boolean", value: "true" }
      ])
    );
  });

  it("keeps comments and template strings as single metric-safe tokens", () => {
    const source = "const url = `${pn.request.url}`; // current URL";
    const tokens = tokenizeScript(source);

    expect(tokens).toContainEqual({ type: "keyword", value: "const" });
    expect(tokens).toContainEqual({ type: "template", value: "`${pn.request.url}`" });
    expect(tokens).toContainEqual({ type: "comment", value: "// current URL" });
  });
});

describe("formatScript", () => {
  it("formats braces and semicolon-separated statements with two-space indentation", () => {
    expect(formatScript("if (ok) {pn.test('ok',()=>{return true;});}")).toBe(
      "if (ok) {\n  pn.test('ok',()=>{\n    return true;\n  });\n}"
    );
  });

  it("preserves strings while formatting surrounding punctuation", () => {
    expect(formatScript("await pn.variables.set('brace } ;', value);pn.request.url='x';")).toBe(
      "await pn.variables.set('brace } ;', value);\npn.request.url='x';"
    );
  });
});
