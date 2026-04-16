import type { Action } from "svelte/action";

const FOCUSABLE_SELECTOR =
  'button:not([disabled]), [href], input:not([disabled]):not([type="hidden"]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

export type ModalFocusTrapParams = {
  onEscape: () => void;
};

function listFocusables(dialog: HTMLElement): HTMLElement[] {
  return Array.from(dialog.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR)).filter(
    (el) => !el.closest(".sr-only") && !el.hasAttribute("disabled")
  );
}

/**
 * Traps Tab within the dialog, focuses the first focusable control on open,
 * restores focus on teardown, and invokes onEscape for Escape.
 */
export const modalFocusTrap: Action<HTMLElement, ModalFocusTrapParams> = (backdrop, { onEscape }) => {
  const dialogEl = backdrop.querySelector('[role="dialog"]');
  if (!(dialogEl instanceof HTMLElement)) {
    return {};
  }

  const panel: HTMLElement = dialogEl;
  const previousFocus = document.activeElement as HTMLElement | null;

  const rafId = requestAnimationFrame(() => {
    const focusables = listFocusables(panel);
    if (focusables.length > 0) {
      focusables[0].focus();
    } else {
      panel.focus();
    }
  });

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      onEscape();
      return;
    }

    if (e.key !== "Tab") {
      return;
    }

    const focusables = listFocusables(panel);
    if (focusables.length === 0) {
      e.preventDefault();
      return;
    }

    const first = focusables[0];
    const last = focusables[focusables.length - 1];
    const active = document.activeElement as Node | null;

    if (e.shiftKey) {
      if (active === first || active === panel) {
        e.preventDefault();
        last.focus();
      }
    } else if (active === last) {
      e.preventDefault();
      first.focus();
    }
  }

  backdrop.addEventListener("keydown", handleKeydown);

  return {
    destroy() {
      cancelAnimationFrame(rafId);
      backdrop.removeEventListener("keydown", handleKeydown);
      previousFocus?.focus?.({ preventScroll: true });
    }
  };
};
