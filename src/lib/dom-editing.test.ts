import { afterEach, describe, expect, it, vi } from "vitest";

import { EDITABLE_UNDO_BASELINE_EVENT, insertTextIntoEditableControl } from "./dom-editing";

type FakeControl = {
  value: string;
  selectionStart: number;
  selectionEnd: number;
  ownerDocument: {
    execCommand: ReturnType<typeof vi.fn>;
  };
  focus: ReturnType<typeof vi.fn>;
  setSelectionRange: ReturnType<typeof vi.fn>;
  setRangeText: ReturnType<typeof vi.fn>;
  dispatchEvent: ReturnType<typeof vi.fn>;
};

function createFakeControl(value = "hello world"): FakeControl {
  const control: FakeControl = {
    value,
    selectionStart: 0,
    selectionEnd: 0,
    ownerDocument: {
      execCommand: vi.fn()
    },
    focus: vi.fn(),
    setSelectionRange: vi.fn((start: number, end: number) => {
      control.selectionStart = start;
      control.selectionEnd = end;
    }),
    setRangeText: vi.fn((text: string, start: number, end: number) => {
      control.value = `${control.value.slice(0, start)}${text}${control.value.slice(end)}`;
      control.selectionStart = start + text.length;
      control.selectionEnd = start + text.length;
    }),
    dispatchEvent: vi.fn()
  };

  return control;
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("insertTextIntoEditableControl", () => {
  it("uses the browser insertText command so edits join the native undo stack", () => {
    const control = createFakeControl();
    control.ownerDocument.execCommand.mockImplementation((_command: string, _showUi: boolean, text: string) => {
      control.value = `${control.value.slice(0, control.selectionStart)}${text}${control.value.slice(control.selectionEnd)}`;
      return true;
    });

    const usedNativeCommand = insertTextIntoEditableControl(control as unknown as HTMLTextAreaElement, "{{base_url}}", {
      selectionStart: 6,
      selectionEnd: 11
    });

    expect(usedNativeCommand).toBe(true);
    expect(control.focus).toHaveBeenCalledOnce();
    expect(control.setSelectionRange).toHaveBeenNthCalledWith(1, 6, 11);
    expect(control.ownerDocument.execCommand).toHaveBeenCalledWith("insertText", false, "{{base_url}}");
    expect(control.value).toBe("hello {{base_url}}");
    expect(control.setRangeText).not.toHaveBeenCalled();
  });

  it("falls back to setRangeText and emits an input event when the native command is unavailable", () => {
    const control = createFakeControl();
    control.ownerDocument.execCommand.mockReturnValue(false);
    vi.stubGlobal("InputEvent", undefined);
    vi.stubGlobal("Event", class {
      type: string;

      constructor(type: string) {
        this.type = type;
      }
    });

    const usedNativeCommand = insertTextIntoEditableControl(control as unknown as HTMLInputElement, "api", {
      selectionStart: 6,
      selectionEnd: 11
    });

    expect(usedNativeCommand).toBe(false);
    expect(control.value).toBe("hello api");
    expect(control.setRangeText).toHaveBeenCalledWith("api", 6, 11, "end");
    expect(control.dispatchEvent).toHaveBeenCalledWith(expect.objectContaining({ type: "input" }));
  });

  it("can hint that undo should collapse a replaced selection to its end", () => {
    const control = createFakeControl("pn.");
    control.ownerDocument.execCommand.mockReturnValue(false);
    vi.stubGlobal(
      "CustomEvent",
      class {
        type: string;
        detail: unknown;

        constructor(type: string, init: CustomEventInit) {
          this.type = type;
          this.detail = init.detail;
        }
      }
    );

    insertTextIntoEditableControl(control as unknown as HTMLTextAreaElement, "pn.variables", {
      selectionStart: 0,
      selectionEnd: 3,
      undoBaselineSelection: "collapse-end"
    });

    expect(control.dispatchEvent).toHaveBeenCalledWith(
      expect.objectContaining({
        detail: { selectionStart: 3, selectionEnd: 3 },
        type: EDITABLE_UNDO_BASELINE_EVENT
      })
    );
  });
});
