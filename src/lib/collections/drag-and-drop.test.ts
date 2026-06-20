import { describe, expect, it } from "vitest";
import type { CollectionItemSummary } from "$lib/api/types";
import { buildAccessibleMoveInput, type DraggedCollectionItem } from "./drag-and-drop";

function item(
  id: string,
  collectionId: string,
  kind: CollectionItemSummary["kind"] = "request",
  children: CollectionItemSummary[] = [],
  parentId: string | null = null
): CollectionItemSummary {
  return {
    id,
    collectionId,
    parentId,
    kind,
    name: id,
    method: kind === "request" ? "GET" : null,
    url: kind === "request" ? `https://example.com/${id}` : null,
    preRequestScript: "",
    testScript: "",
    updatedAt: "2026-06-20T00:00:00Z",
    children
  };
}

function dragged(source: CollectionItemSummary): DraggedCollectionItem {
  return {
    itemId: source.id,
    collectionId: source.collectionId,
    parentId: source.parentId ?? null,
    name: source.name,
    kind: source.kind
  };
}

describe("buildAccessibleMoveInput", () => {
  it("maps the first position in another collection", () => {
    const source = item("source", "one");
    const target = item("target", "two");

    expect(buildAccessibleMoveInput({
      dragged: dragged(source),
      sourceItems: [source],
      targetItems: [target],
      target: { targetCollectionId: "two", targetParentId: null, afterItemId: null }
    })).toEqual({ targetCollectionId: "two", targetParentId: null, targetIndex: 0 });
  });

  it("maps an after-sibling position", () => {
    const source = item("source", "one");
    const first = item("first", "two");
    const second = item("second", "two");

    expect(buildAccessibleMoveInput({
      dragged: dragged(source),
      sourceItems: [source],
      targetItems: [first, second],
      target: { targetCollectionId: "two", targetParentId: null, afterItemId: first.id }
    })?.targetIndex).toBe(1);
  });

  it("rejects a no-op position", () => {
    const first = item("first", "one");
    const source = item("source", "one");

    expect(buildAccessibleMoveInput({
      dragged: dragged(source),
      sourceItems: [first, source],
      targetItems: [first, source],
      target: { targetCollectionId: "one", targetParentId: null, afterItemId: first.id }
    })).toBeNull();
  });

  it("rejects moving a folder into its descendant", () => {
    const descendant = item("child", "one", "folder", [], "parent");
    const parent = item("parent", "one", "folder", [descendant]);

    expect(buildAccessibleMoveInput({
      dragged: dragged(parent),
      sourceItems: [parent],
      targetItems: [parent],
      target: { targetCollectionId: "one", targetParentId: descendant.id, afterItemId: null }
    })).toBeNull();
  });
});
