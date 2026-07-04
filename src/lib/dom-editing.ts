export type EditableTextControl = HTMLInputElement | HTMLTextAreaElement;

export const EDITABLE_UNDO_BASELINE_EVENT = "postnot-editable-undo-baseline";

type InsertTextOptions = {
  selectionStart?: number;
  selectionEnd?: number;
  cursorOffset?: number;
  undoBaselineSelection?: "collapse-end";
};

export function insertTextIntoEditableControl(
  control: EditableTextControl,
  text: string,
  options: InsertTextOptions = {}
) {
  const selectionStart = options.selectionStart ?? control.selectionStart ?? control.value.length;
  const selectionEnd = options.selectionEnd ?? control.selectionEnd ?? selectionStart;
  const cursorOffset = options.cursorOffset ?? text.length;

  control.focus();
  control.setSelectionRange(selectionStart, selectionEnd);
  dispatchUndoBaselineHint(control, selectionStart, selectionEnd, options.undoBaselineSelection);

  if (runNativeInsertTextCommand(control, text)) {
    const cursor = selectionStart + cursorOffset;
    control.setSelectionRange(cursor, cursor);
    return true;
  }

  control.setRangeText(text, selectionStart, selectionEnd, "end");
  dispatchTextInput(control, text);

  const cursor = selectionStart + cursorOffset;
  control.setSelectionRange(cursor, cursor);
  return false;
}

function dispatchUndoBaselineHint(
  control: EditableTextControl,
  selectionStart: number,
  selectionEnd: number,
  undoBaselineSelection: InsertTextOptions["undoBaselineSelection"]
) {
  if (undoBaselineSelection !== "collapse-end" || typeof CustomEvent !== "function") {
    return;
  }

  control.dispatchEvent(
    new CustomEvent(EDITABLE_UNDO_BASELINE_EVENT, {
      bubbles: true,
      cancelable: false,
      detail: {
        selectionStart: selectionEnd,
        selectionEnd
      }
    })
  );
}

function runNativeInsertTextCommand(control: EditableTextControl, text: string) {
  const doc = control.ownerDocument ?? document;

  try {
    return doc.execCommand("insertText", false, text);
  } catch {
    return false;
  }
}

function dispatchTextInput(control: EditableTextControl, text: string) {
  let event: Event;

  if (typeof InputEvent === "function") {
    event = new InputEvent("input", {
      bubbles: true,
      cancelable: false,
      data: text,
      inputType: "insertText"
    });
  } else {
    event = new Event("input", { bubbles: true, cancelable: false });
  }

  control.dispatchEvent(event);
}
