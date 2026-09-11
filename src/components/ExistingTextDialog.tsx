import { useId, useLayoutEffect, useRef, useState } from "react";
import type { EditableTextRun } from "../editor/types";
import { Modal } from "./Modal";
import "./existing-text.css";

interface ExistingTextDialogProps {
  run: EditableTextRun;
  busy: boolean;
  error: string | null;
  onApply(replacement: string): void;
  onCancel(): void;
}

export function ExistingTextDialog({ run, busy, error, onApply, onCancel }: ExistingTextDialogProps) {
  const [replacement, setReplacement] = useState(run.text);
  const [pasteError, setPasteError] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const hintId = useId();
  const errorId = useId();
  useLayoutEffect(() => { inputRef.current?.select(); }, []);
  const invalid = !replacement.trim() ? "Enter replacement text."
    : /[\u0000-\u001f\u007f-\u009f]/.test(replacement) ? "Use a single line without control characters."
    : replacement.length > 1000 ? "Use at most 1,000 characters." : null;
  const canApply = run.supported && !busy && !invalid && replacement !== run.text;
  const message = pasteError ?? error ?? (replacement !== run.text ? invalid : null);
  const cancel = () => { if (!busy) onCancel(); };
  return <Modal title="Edit existing text" onClose={cancel} className="existing-text-dialog" initialFocusRef={run.supported ? inputRef : undefined}>
    <form aria-busy={busy} onSubmit={event => { event.preventDefault(); if (canApply) onApply(replacement); }}>
      <div className="existing-text-fields">
        <p className="existing-text-font">{run.fontName} · {Number(run.fontSize.toFixed(2))} pt</p>
        {run.supported ? <>
          <label>Replacement text<input ref={inputRef} type="text" value={replacement} maxLength={1000} readOnly={busy}
            aria-describedby={`${hintId}${message ? ` ${errorId}` : ""}`} aria-invalid={!!message}
            onChange={event => { if (!busy) { setReplacement(event.target.value); setPasteError(null); } }}
            onPaste={event => { if (/[\u0000-\u001f\u007f-\u009f]/.test(event.clipboardData.getData("text/plain"))) { event.preventDefault(); setPasteError("Use a single line without control characters."); } }} />
          </label>
          <p id={hintId}>The replacement must fit the existing text run. Folio preserves the original font and position; it does not substitute fonts or reflow the page.</p>
        </> : <><p className="existing-text-original">{run.text}</p><p>{run.reason ?? "This text cannot be edited safely with its original font and layout."}</p></>}
        {message && <p id={errorId} className="existing-text-error" role="alert">{message}</p>}
      </div>
      <footer className="dialog-actions">
        <button type="button" className="button" disabled={busy} onClick={cancel}>Cancel</button>
        {run.supported && <button type="submit" className="button primary" disabled={!canApply}>{busy ? "Applying…" : "Apply changes"}</button>}
      </footer>
    </form>
  </Modal>;
}
