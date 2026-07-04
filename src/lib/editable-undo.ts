import { EDITABLE_UNDO_BASELINE_EVENT } from "$lib/dom-editing";

type TextEditableControl = HTMLInputElement | HTMLTextAreaElement;

type UndoEntry = {
  value: string;
  selectionStart: number;
  selectionEnd: number;
};

type EditableUndoAction = "undo" | "redo";

const TEXT_INPUT_TYPES = new Set([
  "",
  "email",
  "password",
  "search",
  "tel",
  "text",
  "url"
]);

export function shouldInstallEditableUndoFallback() {
  if (typeof window === "undefined") {
    return false;
  }

  const testWindow = window as Window & {
    __POSTNOT_FORCE_EDITABLE_UNDO_FALLBACK__?: boolean;
  };

  if (testWindow.__POSTNOT_FORCE_EDITABLE_UNDO_FALLBACK__) {
    return true;
  }

  return (
    ("__TAURI_INTERNALS__" in window || "__TAURI__" in window) &&
    navigator.platform.toLowerCase().includes("linux")
  );
}

export function installEditableUndoFallback(root: Document | HTMLElement = document) {
  const histories = new WeakMap<TextEditableControl, EditableUndoHistory>();
  const ownerDocument =
    typeof Document !== "undefined" && root instanceof Document ? root : root.ownerDocument;

  const getHistory = (control: TextEditableControl, syncExternalValue = true) => {
    let history = histories.get(control);

    if (!history) {
      history = createEditableUndoHistory(control);
      histories.set(control, history);
    } else if (syncExternalValue) {
      history.syncExternalValue();
    }

    return history;
  };

  const handleFocusIn = (event: Event) => {
    const control = getTextEditableControl(event.target);
    if (control) {
      getHistory(control);
    }
  };

  const handleBeforeInput = (event: Event) => {
    const control = getTextEditableControl(event.target);
    if (control) {
      getHistory(control).captureBeforeInput();
    }
  };

  const handleUndoBaselineHint = (event: Event) => {
    const control = getTextEditableControl(event.target);
    if (!control || !("detail" in event)) {
      return;
    }

    const detail = event.detail as Partial<UndoEntry>;
    if (
      typeof detail.selectionStart !== "number" ||
      typeof detail.selectionEnd !== "number"
    ) {
      return;
    }

    getHistory(control, false).captureNextBeforeInputSelection(
      detail.selectionStart,
      detail.selectionEnd
    );
  };

  const handleInput = (event: Event) => {
    const control = getTextEditableControl(event.target);
    if (control) {
      getHistory(control, false).recordInput();
    }
  };

  const handleKeydown = (event: Event) => {
    if (!(event instanceof KeyboardEvent)) {
      return;
    }

    const action = isEditableUndoShortcut(event);
    if (!action) {
      return;
    }

    const control =
      getTextEditableControl(event.target) ??
      getTextEditableControl(ownerDocument?.activeElement ?? null);
    if (!control) {
      return;
    }

    const history = getHistory(control);
    const didRestore = action === "undo" ? history.undo() : history.redo();

    if (didRestore) {
      event.preventDefault();
      event.stopPropagation();
    }
  };

  root.addEventListener("focusin", handleFocusIn, true);
  root.addEventListener(EDITABLE_UNDO_BASELINE_EVENT, handleUndoBaselineHint, true);
  root.addEventListener("beforeinput", handleBeforeInput, true);
  root.addEventListener("input", handleInput, true);
  root.addEventListener("keydown", handleKeydown, true);

  return () => {
    root.removeEventListener("focusin", handleFocusIn, true);
    root.removeEventListener(EDITABLE_UNDO_BASELINE_EVENT, handleUndoBaselineHint, true);
    root.removeEventListener("beforeinput", handleBeforeInput, true);
    root.removeEventListener("input", handleInput, true);
    root.removeEventListener("keydown", handleKeydown, true);
  };
}

export function isTextEditableControl(target: EventTarget | null): target is TextEditableControl {
  if (!target || typeof target !== "object" || !("tagName" in target)) {
    return false;
  }

  const control = target as Partial<TextEditableControl> & { tagName: string };
  if (control.disabled || control.readOnly) {
    return false;
  }

  if (control.tagName === "TEXTAREA") {
    return true;
  }

  if (control.tagName !== "INPUT") {
    return false;
  }

  return TEXT_INPUT_TYPES.has((control.type ?? "").toLowerCase());
}

export function isEditableUndoShortcut(event: KeyboardEvent): EditableUndoAction | null {
  if (!(event.ctrlKey || event.metaKey) || event.altKey) {
    return null;
  }

  const key = event.key.toLowerCase();
  const code = event.code;

  if (!event.shiftKey && (key === "z" || code === "KeyZ")) {
    return "undo";
  }

  if ((key === "y" || code === "KeyY") || (event.shiftKey && (key === "z" || code === "KeyZ"))) {
    return "redo";
  }

  return null;
}

export function createEditableUndoHistory(control: TextEditableControl) {
  return new EditableUndoHistory(control);
}

class EditableUndoHistory {
  private entries: UndoEntry[];
  private index = 0;
  private beforeInputEntry: UndoEntry | null = null;
  private nextBeforeInputSelection: Pick<UndoEntry, "selectionStart" | "selectionEnd"> | null =
    null;
  private isRestoring = false;

  constructor(private readonly control: TextEditableControl) {
    this.entries = [this.createEntry()];
  }

  syncExternalValue() {
    if (this.entries[this.index]?.value !== this.control.value && !this.isRestoring) {
      this.entries = [this.createEntry()];
      this.index = 0;
      this.beforeInputEntry = null;
      this.nextBeforeInputSelection = null;
    }
  }

  captureBeforeInput() {
    if (!this.isRestoring) {
      this.beforeInputEntry = this.createEntry(this.nextBeforeInputSelection ?? undefined);
      this.nextBeforeInputSelection = null;
    }
  }

  captureNextBeforeInputSelection(selectionStart: number, selectionEnd: number) {
    if (!this.isRestoring) {
      this.nextBeforeInputSelection = { selectionStart, selectionEnd };
    }
  }

  recordInput() {
    if (this.isRestoring) {
      return;
    }

    const nextEntry = this.createEntry();
    const currentEntry = this.entries[this.index];

    if (currentEntry?.value === nextEntry.value) {
      this.beforeInputEntry = null;
      this.nextBeforeInputSelection = null;
      return;
    }

    const baselineEntry =
      this.beforeInputEntry && this.beforeInputEntry.value !== nextEntry.value
        ? this.beforeInputEntry
        : currentEntry;
    let nextEntries = this.entries.slice(0, this.index + 1);

    if (baselineEntry && currentEntry) {
      if (currentEntry.value === baselineEntry.value) {
        nextEntries[nextEntries.length - 1] = baselineEntry;
      } else {
        nextEntries = [...nextEntries, baselineEntry];
      }
    }

    nextEntries = [...nextEntries, nextEntry].slice(-200);
    this.entries = nextEntries;
    this.index = nextEntries.length - 1;
    this.beforeInputEntry = null;
    this.nextBeforeInputSelection = null;
  }

  undo() {
    if (this.index <= 0) {
      return false;
    }

    this.index -= 1;
    this.restore(this.entries[this.index]);
    return true;
  }

  redo() {
    if (this.index >= this.entries.length - 1) {
      return false;
    }

    this.index += 1;
    this.restore(this.entries[this.index]);
    return true;
  }

  private createEntry(selectionOverride?: Pick<UndoEntry, "selectionStart" | "selectionEnd">): UndoEntry {
    const selectionStart =
      selectionOverride?.selectionStart ?? this.control.selectionStart ?? this.control.value.length;
    const selectionEnd = selectionOverride?.selectionEnd ?? this.control.selectionEnd ?? selectionStart;

    return {
      value: this.control.value,
      selectionStart,
      selectionEnd
    };
  }

  private restore(entry: UndoEntry | undefined) {
    if (!entry) {
      return;
    }

    this.isRestoring = true;
    this.control.value = entry.value;
    this.control.setSelectionRange(entry.selectionStart, entry.selectionEnd);
    this.control.dispatchEvent(createSyntheticInputEvent());
    this.isRestoring = false;
    this.beforeInputEntry = null;
    this.nextBeforeInputSelection = null;
  }
}

function getTextEditableControl(target: EventTarget | null) {
  return isTextEditableControl(target) ? target : null;
}

function createSyntheticInputEvent() {
  if (typeof InputEvent === "function") {
    return new InputEvent("input", {
      bubbles: true,
      cancelable: false,
      inputType: "historyUndo"
    });
  }

  return new Event("input", { bubbles: true, cancelable: false });
}
