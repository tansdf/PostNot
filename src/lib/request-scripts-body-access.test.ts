import { describe, expect, it } from "vitest";

import { scriptsUseResponseBody } from "$lib/request-scripts";

describe("response script body access", () => {
  it("keeps metadata-only tests file-backed", () => {
    expect(scriptsUseResponseBody("pn.expect(pn.response.code).toBe(200);")).toBe(false);
  });

  it("detects explicit full body access", () => {
    expect(scriptsUseResponseBody("const body = pn.response.json();")).toBe(true);
    expect(scriptsUseResponseBody("const text = pn.response.text();")).toBe(true);
  });
});
