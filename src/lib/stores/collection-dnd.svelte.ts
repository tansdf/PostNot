import type { CollectionDropIndicator, DraggedCollectionRequest } from "$lib/collections/drag-and-drop";

type PointerPoint = {
  x: number;
  y: number;
};

class CollectionDndStore {
  draggedRequest = $state.raw<DraggedCollectionRequest | null>(null);
  dropIndicator = $state.raw<CollectionDropIndicator | null>(null);
  pointer = $state.raw<PointerPoint | null>(null);
  isDragging = $state(false);

  pendingRequest: DraggedCollectionRequest | null = null;
  pointerId: number | null = null;
  origin: PointerPoint | null = null;
  suppressClickUntil = 0;

  beginPotentialDrag(request: DraggedCollectionRequest, pointerId: number, point: PointerPoint) {
    this.pendingRequest = request;
    this.pointerId = pointerId;
    this.origin = point;
    this.pointer = point;
    this.draggedRequest = null;
    this.dropIndicator = null;
    this.isDragging = false;
  }

  updatePointer(pointerId: number, point: PointerPoint) {
    if (this.pointerId !== pointerId) {
      return false;
    }

    this.pointer = point;

    if (!this.isDragging && this.pendingRequest && this.origin) {
      const distance = Math.hypot(point.x - this.origin.x, point.y - this.origin.y);
      if (distance >= 6) {
        this.draggedRequest = this.pendingRequest;
        this.isDragging = true;
        document.body.classList.add("collection-dnd-active");
        return true;
      }
    }

    return this.isDragging;
  }

  setDropIndicator(indicator: CollectionDropIndicator | null) {
    if (
      this.dropIndicator?.collectionId === indicator?.collectionId &&
      this.dropIndicator?.itemId === indicator?.itemId &&
      this.dropIndicator?.placement === indicator?.placement
    ) {
      return;
    }

    this.dropIndicator = indicator;
  }

  clearDropIndicator() {
    this.dropIndicator = null;
  }

  cancelInteraction() {
    this.pendingRequest = null;
    this.pointerId = null;
    this.origin = null;
    this.pointer = null;
    this.draggedRequest = null;
    this.dropIndicator = null;
    this.isDragging = false;
    document.body.classList.remove("collection-dnd-active");
  }

  finishDrag() {
    if (this.isDragging) {
      this.suppressClickUntil = Date.now() + 250;
    }

    this.cancelInteraction();
  }

  shouldSuppressClick() {
    return Date.now() < this.suppressClickUntil;
  }

  isDraggingRequest(itemId: string) {
    return this.draggedRequest?.itemId === itemId;
  }

  matchesDropIndicator(collectionId: string, itemId: string | null, placement: CollectionDropIndicator["placement"]) {
    return (
      this.dropIndicator?.collectionId === collectionId &&
      this.dropIndicator?.itemId === itemId &&
      this.dropIndicator?.placement === placement
    );
  }
}

export const collectionDnd = new CollectionDndStore();
