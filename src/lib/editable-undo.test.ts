import { describe, expect, it, vi } from "vitest";

import {
  createEditableUndoHistory,
  installEditableUndoFallback,
  isEditableUndoShortcut,
  isTextEditableControl
} from "./editable-undo";
import { EDITABLE_UNDO_BASELINE_EVENT } from "./dom-editing";

type FakeEditable = {
  tagName: string;
  type?: string;
  value: string;
  selectionStart: number;
  selectionEnd: number;
  disabled: boolean;
  readOnly: boolean;
  setSelectionRange: ReturnType<typeof vi.fn>;
  dispatchEvent: ReturnType<typeof vi.fn>;
};

function createFakeEditable(value = ""): FakeEditable {
  const control: FakeEditable = {
    tagName: "INPUT",
    type: "text",
    value,
    selectionStart: value.length,
    selectionEnd: value.length,
    disabled: false,
    readOnly: false,
    setSelectionRange: vi.fn((start: number, end: number) => {
      control.selectionStart = start;
      control.selectionEnd = end;
    }),
    dispatchEvent: vi.fn()
  };

  return control;
}

function keyEvent(key: string, patch: Partial<KeyboardEvent> = {}) {
  return {
    key,
    code: `Key${key.toUpperCase()}`,
    ctrlKey: true,
    metaKey: false,
    shiftKey: false,
    altKey: false,
    ...patch
  } as KeyboardEvent;
}

describe("isTextEditableControl", () => {
  it("accepts text inputs and textareas but ignores non-text controls", () => {
    expect(isTextEditableControl(createFakeEditable() as unknown as EventTarget)).toBe(true);
    expect(isTextEditableControl({ ...createFakeEditable(), type: "password" } as unknown as EventTarget)).toBe(true);
    expect(isTextEditableControl({ ...createFakeEditable(), type: "checkbox" } as unknown as EventTarget)).toBe(false);
    expect(isTextEditableControl({ ...createFakeEditable(), disabled: true } as unknown as EventTarget)).toBe(false);
    expect(isTextEditableControl({ ...createFakeEditable(), tagName: "TEXTAREA", type: undefined } as unknown as EventTarget)).toBe(true);
  });
});

describe("isEditableUndoShortcut", () => {
  it("recognizes undo and redo shortcuts", () => {
    expect(isEditableUndoShortcut(keyEvent("z"))).toBe("undo");
    expect(isEditableUndoShortcut(keyEvent("y"))).toBe("redo");
    expect(isEditableUndoShortcut(keyEvent("z", { shiftKey: true }))).toBe("redo");
    expect(isEditableUndoShortcut(keyEvent("z", { altKey: true }))).toBeNull();
  });
});

describe("createEditableUndoHistory", () => {
  it("restores previous input values and selections", () => {
    const control = createFakeEditable("abc");
    const history = createEditableUndoHistory(control as unknown as HTMLInputElement);

    history.captureBeforeInput();
    control.value = "abcd";
    control.selectionStart = 4;
    control.selectionEnd = 4;
    history.recordInput();

    expect(history.undo()).toBe(true);
    expect(control.value).toBe("abc");
    expect(control.setSelectionRange).toHaveBeenLastCalledWith(3, 3);
    expect(control.dispatchEvent).toHaveBeenCalledOnce();

    expect(history.redo()).toBe(true);
    expect(control.value).toBe("abcd");
    expect(control.setSelectionRange).toHaveBeenLastCalledWith(4, 4);
  });

  it("records programmatic insertion input as one undoable edit", () => {
    const control = createFakeEditable("{{ba");
    const history = createEditableUndoHistory(control as unknown as HTMLInputElement);

    control.value = "{{base_url}}";
    control.selectionStart = 12;
    control.selectionEnd = 12;
    history.recordInput();

    expect(history.undo()).toBe(true);
    expect(control.value).toBe("{{ba");
  });

  it("restores the pre-edit caret instead of the initial focus caret", () => {
    const control = createFakeEditable("abc");
    control.selectionStart = 0;
    control.selectionEnd = 0;
    const history = createEditableUndoHistory(control as unknown as HTMLInputElement);

    control.selectionStart = 3;
    control.selectionEnd = 3;
    history.captureBeforeInput();
    control.value = "abcd";
    control.selectionStart = 4;
    control.selectionEnd = 4;
    history.recordInput();

    expect(history.undo()).toBe(true);
    expect(control.value).toBe("abc");
    expect(control.setSelectionRange).toHaveBeenLastCalledWith(3, 3);
  });
});

describe("installEditableUndoFallback", () => {
  it("uses the focused control when an undo shortcut is retargeted to the document", () => {
    const listeners = new Map<string, EventListener[]>();
    const input = createFakeEditable("abc");
    const fakeDocument = { activeElement: input };

    class FakeKeyboardEvent {
      altKey = false;
      code = "KeyZ";
      ctrlKey = true;
      defaultPrevented = false;
      key = "z";
      metaKey = false;
      shiftKey = false;
      target: EventTarget | null = fakeDocument as unknown as EventTarget;

      preventDefault() {
        this.defaultPrevented = true;
      }

      stopPropagation() {}
    }

    const root = {
      ownerDocument: fakeDocument,
      addEventListener: vi.fn((type: string, listener: EventListener) => {
        listeners.set(type, [...(listeners.get(type) ?? []), listener]);
      }),
      removeEventListener: vi.fn()
    };
    const dispatch = (type: string, event: Event) => {
      for (const listener of listeners.get(type) ?? []) {
        listener(event);
      }
    };
    const originalKeyboardEvent = globalThis.KeyboardEvent;

    vi.stubGlobal("KeyboardEvent", FakeKeyboardEvent);

    const uninstall = installEditableUndoFallback(root as unknown as HTMLElement);

    dispatch("focusin", { target: input } as unknown as Event);
    dispatch("beforeinput", { target: input } as unknown as Event);
    input.value = "abcd";
    input.selectionStart = 4;
    input.selectionEnd = 4;
    dispatch("input", { target: input } as unknown as Event);

    const event = new (globalThis.KeyboardEvent as unknown as typeof FakeKeyboardEvent)();

    expect(event).toBeInstanceOf(KeyboardEvent);

    dispatch("keydown", event as unknown as Event);

    expect(input.value).toBe("abc");
    expect(input.setSelectionRange).toHaveBeenLastCalledWith(3, 3);
    expect(event.defaultPrevented).toBe(true);

    uninstall();
    vi.stubGlobal("KeyboardEvent", originalKeyboardEvent);
  });

  it("uses app insertion hints to avoid selecting the replaced autocomplete prefix on undo", () => {
    const listeners = new Map<string, EventListener[]>();
    const input = createFakeEditable("pn.");
    const fakeDocument = { activeElement: input };

    class FakeKeyboardEvent {
      altKey = false;
      code = "KeyZ";
      ctrlKey = true;
      defaultPrevented = false;
      key = "z";
      metaKey = false;
      shiftKey = false;
      target: EventTarget | null = fakeDocument as unknown as EventTarget;

      preventDefault() {
        this.defaultPrevented = true;
      }

      stopPropagation() {}
    }

    const root = {
      ownerDocument: fakeDocument,
      addEventListener: vi.fn((type: string, listener: EventListener) => {
        listeners.set(type, [...(listeners.get(type) ?? []), listener]);
      }),
      removeEventListener: vi.fn()
    };
    const dispatch = (type: string, event: Event) => {
      for (const listener of listeners.get(type) ?? []) {
        listener(event);
      }
    };
    const originalKeyboardEvent = globalThis.KeyboardEvent;

    vi.stubGlobal("KeyboardEvent", FakeKeyboardEvent);

    const uninstall = installEditableUndoFallback(root as unknown as HTMLElement);

    dispatch("focusin", { target: input } as unknown as Event);
    input.selectionStart = 0;
    input.selectionEnd = 3;
    dispatch(EDITABLE_UNDO_BASELINE_EVENT, {
      detail: { selectionStart: 3, selectionEnd: 3 },
      target: input
    } as unknown as Event);
    dispatch("beforeinput", { target: input } as unknown as Event);
    input.value = "pn.variables";
    input.selectionStart = 12;
    input.selectionEnd = 12;
    dispatch("input", { target: input } as unknown as Event);

    dispatch("keydown", new (globalThis.KeyboardEvent as unknown as typeof FakeKeyboardEvent)() as unknown as Event);

    expect(input.value).toBe("pn.");
    expect(input.setSelectionRange).toHaveBeenLastCalledWith(3, 3);

    uninstall();
    vi.stubGlobal("KeyboardEvent", originalKeyboardEvent);
  });
});
