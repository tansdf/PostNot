import type { CollectionItemSummary, MoveCollectionItemInput } from "$lib/api/types";

export type DraggedCollectionItem = {
  itemId: string;
  collectionId: string;
  parentId: string | null;
  name: string;
  kind: CollectionItemSummary["kind"];
};

export type DraggedCollectionRequest = DraggedCollectionItem;

export type CollectionDropPlacement = "before" | "after" | "inside";

export type CollectionDropIndicator = {
  collectionId: string;
  itemId: string | null;
  placement: CollectionDropPlacement | "root";
};

type CollectionItemLocation = {
  item: CollectionItemSummary;
  parentId: string | null;
  index: number;
  siblings: CollectionItemSummary[];
};

export function resolveItemDropPlacement(clientY: number, rect: DOMRect, allowInside: boolean): CollectionDropPlacement {
  if (rect.height <= 0) {
    return allowInside ? "inside" : "after";
  }

  const y = (clientY - rect.top) / rect.height;

  if (allowInside && y >= 0.32 && y <= 0.68) {
    return "inside";
  }

  return y < 0.5 ? "before" : "after";
}

export function buildRootMoveInput(
  dragged: DraggedCollectionItem,
  sourceItems: CollectionItemSummary[],
  targetCollectionId: string
): MoveCollectionItemInput | null {
  const sourceLocation = findCollectionItemLocation(sourceItems, dragged.itemId);
  if (!sourceLocation) {
    return null;
  }

  if (
    dragged.collectionId === targetCollectionId &&
    sourceLocation.parentId === null &&
    sourceLocation.index === sourceLocation.siblings.length - 1
  ) {
    return null;
  }

  return {
    targetCollectionId,
    targetParentId: null,
    targetIndex: null
  };
}

export function buildItemMoveInput(args: {
  dragged: DraggedCollectionItem;
  sourceItems: CollectionItemSummary[];
  targetItems: CollectionItemSummary[];
  targetCollectionId: string;
  targetItemId: string;
  placement: CollectionDropPlacement;
}): MoveCollectionItemInput | null {
  const sourceLocation = findCollectionItemLocation(args.sourceItems, args.dragged.itemId);
  const targetLocation = findCollectionItemLocation(args.targetItems, args.targetItemId);

  if (!sourceLocation || !targetLocation || targetLocation.item.id === args.dragged.itemId) {
    return null;
  }

  if (
    sourceLocation.item.kind === "folder" &&
    isCollectionItemDescendant(sourceLocation.item, targetLocation.item.id)
  ) {
    return null;
  }

  let targetParentId: string | null;
  let targetIndex: number | null;

  if (args.placement === "inside") {
    if (targetLocation.item.kind !== "folder") {
      return null;
    }

    targetParentId = targetLocation.item.id;
    targetIndex = targetLocation.item.children.length;
  } else {
    targetParentId = targetLocation.parentId;
    targetIndex = targetLocation.index + (args.placement === "after" ? 1 : 0);
  }

  if (
    sourceLocation.item.collectionId === args.targetCollectionId &&
    sourceLocation.parentId === targetParentId &&
    targetIndex !== null &&
    sourceLocation.index < targetIndex
  ) {
    targetIndex -= 1;
  }

  if (
    sourceLocation.item.collectionId === args.targetCollectionId &&
    sourceLocation.parentId === targetParentId &&
    targetIndex === sourceLocation.index
  ) {
    return null;
  }

  return {
    targetCollectionId: args.targetCollectionId,
    targetParentId,
    targetIndex
  };
}

function isCollectionItemDescendant(item: CollectionItemSummary, maybeDescendantId: string): boolean {
  if (item.kind !== "folder") {
    return false;
  }

  return item.children.some((child) => {
    if (child.id === maybeDescendantId) {
      return true;
    }

    return isCollectionItemDescendant(child, maybeDescendantId);
  });
}

function findCollectionItemLocation(
  items: CollectionItemSummary[],
  itemId: string,
  parentId: string | null = null
): CollectionItemLocation | null {
  for (const [index, item] of items.entries()) {
    if (item.id === itemId) {
      return {
        item,
        parentId,
        index,
        siblings: items
      };
    }

    if (item.kind === "folder") {
      const nestedLocation = findCollectionItemLocation(item.children, itemId, item.id);
      if (nestedLocation) {
        return nestedLocation;
      }
    }
  }

  return null;
}
