import { describe, expect, it } from "vitest";

import { computeVirtualWindowStart, moveWrappedMatchIndex, prepareRepresentationSwitch } from "$lib/response-window";

describe("response virtual window", () => {
  it("keeps overscan before the visible row without going negative", () => {
    expect(computeVirtualWindowStart(0, 23, 40)).toBe(0);
    expect(computeVirtualWindowStart(2300, 23, 40)).toBe(60);
  });

  it("wraps next and previous search navigation", () => {
    expect(moveWrappedMatchIndex(2, 1, 3)).toBe(0);
    expect(moveWrappedMatchIndex(0, -1, 3)).toBe(2);
    expect(moveWrappedMatchIndex(-1, 1, 0)).toBe(-1);
  });

  it("invalidates an active search before switching representations", () => {
    expect(prepareRepresentationSwitch("search-7", 7)).toEqual({
      searchIdToCancel: "search-7",
      nextSearchSequence: 8
    });
  });
});
