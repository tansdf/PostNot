import type { Attachment } from "svelte/attachments";

/** Converts the dominant wheel axis into horizontal movement for overflowing strips. */
export const horizontalWheelScroll: Attachment<HTMLElement> = (node) => {
  const onWheel = (event: WheelEvent) => {
    if (node.scrollWidth <= node.clientWidth + 1) return;

    const delta = event.shiftKey || Math.abs(event.deltaY) >= Math.abs(event.deltaX)
      ? event.deltaY
      : event.deltaX;
    if (!delta) return;

    node.scrollLeft += delta;
    event.preventDefault();
  };

  node.addEventListener("wheel", onWheel, { passive: false });
  return () => node.removeEventListener("wheel", onWheel);
};
