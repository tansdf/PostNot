<script lang="ts">
  import { onMount } from "svelte";

  import {
    buildItemMoveInput,
    buildRootMoveInput,
    resolveItemDropPlacement
  } from "$lib/collections/drag-and-drop";
  import { collectionDnd } from "$lib/stores/collection-dnd.svelte";
  import { collections } from "$lib/stores/collections.svelte";

  type DropTargetData = {
    collectionId: string;
    itemId: string | null;
    itemKind: "folder" | "request" | null;
    placement: "root" | "before" | "after" | "inside";
  };

  onMount(() => {
    function handlePointerMove(event: PointerEvent) {
      const isDragging = collectionDnd.updatePointer(event.pointerId, {
        x: event.clientX,
        y: event.clientY
      });

      if (!isDragging) {
        return;
      }

      event.preventDefault();
      collectionDnd.setDropIndicator(resolveDropIndicator(event.clientX, event.clientY));
    }

    async function handlePointerUp(event: PointerEvent) {
      if (collectionDnd.pointerId !== event.pointerId) {
        return;
      }

      const draggedRequest = collectionDnd.draggedRequest;
      const dropIndicator = collectionDnd.dropIndicator;

      if (!collectionDnd.isDragging || !draggedRequest || !dropIndicator) {
        collectionDnd.finishDrag();
        return;
      }

      const sourceItems = collections.collectionItemsByCollection[draggedRequest.collectionId] ?? [];
      const targetItems = collections.collectionItemsByCollection[dropIndicator.collectionId] ?? [];

      const input =
        dropIndicator.placement === "root"
          ? buildRootMoveInput(draggedRequest, sourceItems, dropIndicator.collectionId)
          : buildItemMoveInput({
              dragged: draggedRequest,
              sourceItems,
              targetItems,
              targetCollectionId: dropIndicator.collectionId,
              targetItemId: dropIndicator.itemId ?? "",
              placement: dropIndicator.placement
            });

      collectionDnd.finishDrag();

      if (!input) {
        return;
      }

      await collections.moveSavedRequest(draggedRequest.itemId, draggedRequest.collectionId, input);
    }

    function handlePointerCancel(event: PointerEvent) {
      if (collectionDnd.pointerId === event.pointerId) {
        collectionDnd.cancelInteraction();
      }
    }

    window.addEventListener("pointermove", handlePointerMove, true);
    window.addEventListener("pointerup", handlePointerUp, true);
    window.addEventListener("pointercancel", handlePointerCancel, true);

    return () => {
      window.removeEventListener("pointermove", handlePointerMove, true);
      window.removeEventListener("pointerup", handlePointerUp, true);
      window.removeEventListener("pointercancel", handlePointerCancel, true);
    };
  });

  function resolveDropIndicator(clientX: number, clientY: number) {
    const rawTarget = document.elementFromPoint(clientX, clientY);
    const dropTarget = rawTarget instanceof HTMLElement ? rawTarget.closest<HTMLElement>("[data-collection-drop]") : null;
    if (!dropTarget) {
      return null;
    }

    const targetData = readDropTargetData(dropTarget, clientY);
    if (!targetData || !collectionDnd.draggedRequest) {
      return null;
    }

    if (
      targetData.itemId === collectionDnd.draggedRequest.itemId &&
      targetData.placement !== "root"
    ) {
      return null;
    }

    return {
      collectionId: targetData.collectionId,
      itemId: targetData.itemId,
      placement: targetData.placement
    };
  }

  function readDropTargetData(element: HTMLElement, clientY: number): DropTargetData | null {
    const collectionId = element.dataset.collectionId;
    const dropType = element.dataset.collectionDrop;
    if (!collectionId || !dropType) {
      return null;
    }

    if (dropType === "root") {
      return {
        collectionId,
        itemId: null,
        itemKind: null,
        placement: "root"
      };
    }

    const itemId = element.dataset.itemId;
    const itemKind = element.dataset.itemKind === "folder" || element.dataset.itemKind === "request"
      ? element.dataset.itemKind
      : null;
    if (!itemId || !itemKind) {
      return null;
    }

    return {
      collectionId,
      itemId,
      itemKind,
      placement: resolveItemDropPlacement(clientY, element.getBoundingClientRect(), itemKind === "folder")
    };
  }
</script>

{#if collectionDnd.isDragging && collectionDnd.draggedRequest && collectionDnd.pointer}
  <div
    class="collection-drag-overlay"
    style={`left:${collectionDnd.pointer.x + 14}px; top:${collectionDnd.pointer.y + 14}px;`}
    aria-hidden="true"
  >
    <strong>{collectionDnd.draggedRequest.name || "Saved request"}</strong>
    <span>Move request</span>
  </div>
{/if}
