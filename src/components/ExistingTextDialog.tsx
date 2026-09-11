import { FontPicker } from "./FontPicker";
import { customFontState, customTextError, holdCustomFont, type FontInfo } from "../editor/custom-fonts";
import { useEffect, useId, useLayoutEffect, useRef, useState } from "react";
import type { EditableTextRun } from "../editor/types";
import { Modal } from "./Modal";
import "./existing-text.css";

interface ExistingTextDialogProps {
  run: EditableTextRun;
  busy: boolean;
  error: string | null;
  onApply(replacement: string, fontId?: string): void | Promise<void>;
  onDraftChange?(): void;
  onCancel(): void;
}

export function ExistingTextDialog({ run, busy, error, onApply, onCancel, onDraftChange }: ExistingTextDialogProps) {
  const [replacement, setReplacement] = useState(run.text);
  const [font, setFont] = useState<FontInfo | null>(null);
  const [pickerOpen, setPickerOpen] = useState(false);
  const fontLease = useRef<(() => Promise<void>) | null>(null);
  const applying = useRef<Promise<void> | null>(null);
  const alive = useRef(false);
  useEffect(() => {
    alive.current = true;
    return () => { alive.current = false; queueMicrotask(() => { if (!alive.current) {
      const release = fontLease.current; fontLease.current = null;
      void Promise.resolve(applying.current).catch(() => {}).then(() => release?.());
    } }); };
  }, []);
  const chooseFont = (info: FontInfo) => {
    const previous = fontLease.current;
    fontLease.current = holdCustomFont(info.id);
    setFont(info); setPickerOpen(false); onDraftChange?.();
    void previous?.();
  };
  const useOriginal = () => {
    const previous = fontLease.current; fontLease.current = null;
    setFont(null); onDraftChange?.(); void previous?.();
  };
  const submit = () => {
    if (!canApply || applying.current) return;
    const result = font ? onApply(replacement, font.id) : onApply(replacement);
    if (result) {
      const pending = Promise.resolve(result); applying.current = pending;
      void pending.catch(() => {}).finally(() => { if (applying.current === pending) applying.current = null; });
    }
  };
  const editable = run.supported || !!run.canSubstitute;
  const [pasteError, setPasteError] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const hintId = useId();
  const errorId = useId();
  useLayoutEffect(() => { inputRef.current?.select(); }, []);
  const invalid = !replacement.trim() ? "Enter replacement text."
    : /[\u0000-\u001f\u007f-\u009f]/.test(replacement) ? "Use a single line without control characters."
    : replacement.length > 1000 ? "Use at most 1,000 characters." : font ? customTextError(font, replacement) : null;
  const canApply = (run.supported || (!!run.canSubstitute && !!font)) && !busy && !invalid && (replacement !== run.text || !!font);
  const message = pasteError ?? error ?? (replacement !== run.text ? invalid : null);
  const cancel = () => { if (!busy) onCancel(); };
  if (pickerOpen) return <FontPicker text={replacement} currentFontId={font?.id} title="Choose a substitute font" description="Preview a font for this replacement. Your PDF changes only when you apply the text edit." onChoose={chooseFont} onClose={()=>setPickerOpen(false)}/>;
  return <Modal title="Edit existing text" onClose={cancel} className="existing-text-dialog" initialFocusRef={editable ? inputRef : undefined}>
    <form aria-busy={busy} onSubmit={event => { event.preventDefault(); submit(); }}>
      <div className="existing-text-fields">
        <p className="existing-text-font">{run.fontName} · {Number(run.fontSize.toFixed(2))} pt</p>
        {editable ? <>
          <label>Replacement text<input ref={inputRef} type="text" value={replacement} maxLength={1000} readOnly={busy}
            aria-describedby={`${hintId}${message ? ` ${errorId}` : ""}`} aria-invalid={!!message}
            onChange={event => { if (!busy) { setReplacement(event.target.value); setPasteError(null); onDraftChange?.(); } }}
            onPaste={event => { if (/[\u0000-\u001f\u007f-\u009f]/.test(event.clipboardData.getData("text/plain"))) { event.preventDefault(); setPasteError("Use a single line without control characters."); } }} />
          </label>
          <p id={hintId}>Longer text can extend into available space at the same size and position. Folio checks page edges and nearby source text and graphics; the page does not reflow.</p>
        </> : <><p className="existing-text-original">{run.text}</p><p>{run.reason ?? "This text cannot be edited safely with its original font and layout."}</p></>}
        {run.isEmbedded && <p>The embedded font may contain only some characters.</p>}
        {run.canSubstitute && <div className="existing-text-font-choice">
          <div><span>Replacement font</span><strong>{font?.name ?? "Original PDF font"}</strong></div>
          <button type="button" className="button" disabled={busy} onClick={()=>setPickerOpen(true)}>Choose substitute font…</button>
          {font && <>
            <p className="existing-text-preview" aria-label="Replacement font preview" style={{fontFamily:customFontState(font.id)?.family,fontSize:Math.min(28,Math.max(16,run.fontSize)),fontKerning:"none",fontVariantLigatures:"none",fontSynthesis:"none"}}>{invalid ? "Update the replacement text to preview this font." : replacement}</p>
            <button type="button" className="existing-text-original-button" disabled={busy} onClick={useOriginal}>Use original font</button>
          </>}
        </div>}
        {message && <p id={errorId} className="existing-text-error" role="alert">{message}</p>}
      </div>
      <footer className="dialog-actions">
        <button type="button" className="button" disabled={busy} onClick={cancel}>Cancel</button>
        {editable && <button type="submit" className="button primary" disabled={!canApply}>{busy ? "Applying…" : "Apply changes"}</button>}
      </footer>
    </form>
  </Modal>;
}
