import { FontPicker } from "./FontPicker";
import { customFontState, customTextError, holdCustomFont, type FontInfo } from "../editor/custom-fonts";
import { useEffect, useId, useLayoutEffect, useRef, useState } from "react";
import type { EditableTextRun } from "../editor/types";
import type {ExistingTextDraft} from "../editor/paragraph-draft";
import {prepareParagraphPaste,type ParagraphPaste} from "../editor/paragraph-paste";
import { Modal } from "./Modal";
import "./existing-text.css";

const pastedFontError="The selected font cannot display all of the pasted text. Choose another font.";

interface ExistingTextDialogProps {
  run: EditableTextRun;
  busy: boolean;
  error: string | null;
  onApply(replacement: string, fontId?: string): void | Promise<void>;
  onDraftChange?(): void;
  onEditTogether?(): void;
  onEditParagraph?(draft:ExistingTextDraft): void;
  onCancel(): void;
}

export function ExistingTextDialog({ run, busy, error, onApply, onCancel, onDraftChange, onEditTogether, onEditParagraph }: ExistingTextDialogProps) {
  const originalText = run.sourceText ?? run.text;
  const displayFontSize = run.displayFontSize ?? run.fontSize;
  const [replacement, setReplacement] = useState(originalText);
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
  const canSubstitute = run.canSubstitute;
  const editable = run.supported || !!canSubstitute;
  const [pasteError, setPasteError] = useState<string | null>(null);
  const [pendingPaste,setPendingPaste]=useState<{owner:EditableTextRun;text:string;before:{start:number;end:number;direction:"forward"|"backward"|"none"};prepared:ParagraphPaste}|null>(null);
  const paragraphPaste=pendingPaste?.owner===run&&pendingPaste.text===replacement?pendingPaste:null;
  const inputRef = useRef<HTMLInputElement>(null);
  const hintId = useId();
  const errorId = useId();
  const pasteHintId=useId();
  useLayoutEffect(() => { inputRef.current?.select(); }, []);
  const activeText=paragraphPaste?.prepared.replacement??replacement;
  const invalid = paragraphPaste ? (font&&customTextError(font,activeText)?pastedFontError:null) : !replacement.trim() ? "Enter replacement text."
    : /[\u0000-\u001f\u007f-\u009f]/.test(replacement) ? "Use a single line without control characters."
    : replacement.length > 1000 ? "Use at most 1,000 characters." : font ? customTextError(font, replacement) : null;
  const canApply = (run.supported || (!!canSubstitute && !!font)) && !busy && !paragraphPaste && !invalid && (replacement !== originalText || !!font);
  const message = pasteError ?? error ?? (paragraphPaste || replacement !== originalText ? invalid : null);
  const cancel = () => { if (!busy) onCancel(); };
  const editParagraph=(paste?:ParagraphPaste)=>{
    const input=inputRef.current;
    const copied=font?{...font,coverage:font.coverage.map(range=>Object.freeze([...range]) as [number,number]),...(font.shapedCoverage?{shapedCoverage:font.shapedCoverage.map(range=>Object.freeze([...range]) as [number,number])}:{})}:null;
    if(copied){Object.freeze(copied.coverage);if(copied.shapedCoverage)Object.freeze(copied.shapedCoverage);Object.freeze(copied);}
    onEditParagraph?.(Object.freeze({originalText,replacement:paste?.replacement??replacement,selection:paste?.selection??Object.freeze({start:input?.selectionStart??0,end:input?.selectionEnd??0,direction:input?.selectionDirection==="backward"?"backward":"forward"}),font:copied}));
  };
  const discardPaste=()=>{
    if(busy||!paragraphPaste)return;
    const previous=paragraphPaste.before;setPendingPaste(null);setPasteError(null);
    inputRef.current?.focus({preventScroll:true});inputRef.current?.setSelectionRange(previous.start,previous.end,previous.direction);
  };
  if (pickerOpen) return <FontPicker sourceFontName={run.fontDisplayName??run.fontName} text={activeText} textErrorMessage={paragraphPaste?pastedFontError:undefined} currentFontId={font?.id} title="Choose a substitute font" description="Preview a font for this replacement. Your PDF changes only when you apply the text edit." onChoose={chooseFont} onClose={()=>setPickerOpen(false)}/>;
  return <Modal title="Edit existing text" onClose={cancel} className="existing-text-dialog" initialFocusRef={editable ? inputRef : undefined}>
    <form aria-busy={busy} onSubmit={event => { event.preventDefault(); submit(); }}>
      <div className="existing-text-fields">
        <p className="existing-text-font">{run.fontDisplayName??run.fontName} · {Number(displayFontSize.toFixed(2))} pt</p>
        {run.objectPath && <p>Only this occurrence will change. Other copies of this text stay as they are.</p>}
        {editable ? <>
          <label>Replacement text<input ref={inputRef} type="text" value={replacement} maxLength={1000} readOnly={busy}
            aria-describedby={`${hintId}${paragraphPaste?` ${pasteHintId}`:""}${message ? ` ${errorId}` : ""}`} aria-invalid={!!message}
            onChange={event => { if (!busy) { setReplacement(event.target.value); setPasteError(null); setPendingPaste(null); onDraftChange?.(); } }}
            onPaste={event => {
              if(busy){event.preventDefault();return;}
              const text=event.clipboardData.getData("text/plain");setPendingPaste(null);setPasteError(null);
              if(onEditParagraph&&/[\r\n]/.test(text)){
                event.preventDefault();
                const input=event.currentTarget,before={start:input.selectionStart??0,end:input.selectionEnd??0,direction:input.selectionDirection??"none"};
                try{const prepared=prepareParagraphPaste(replacement,before,text);setPendingPaste({owner:run,text:replacement,before,prepared});onDraftChange?.();}
                catch(cause){setPasteError(cause instanceof Error?cause.message:String(cause));}
              }else if(/[\u0000-\u001f\u007f-\u009f]/.test(text)){event.preventDefault();setPasteError("Use a single line without control characters.");}
            }} />
          </label>
          <p id={hintId}>Longer text can extend into available space at the same size and position. Folio checks page edges and nearby source text and graphics; the page does not reflow.</p>
        </> : <><p className="existing-text-original">{run.text}</p><p>{run.reason ?? "This text cannot be edited safely with its original font and layout."}</p></>}
        {paragraphPaste&&<div>
          <p id={pasteHintId} role="status">The pasted text has line breaks. Use Paste as paragraph to keep them.</p>
          <button type="button" className="existing-text-original-button" disabled={busy} onClick={discardPaste}>Discard paste</button>
        </div>}
        {run.isEmbedded && <p>The embedded font may contain only some characters.</p>}
        {!canSubstitute && (run.fontSubstitutionReason
          ? <p>{run.fontSubstitutionReason}</p>
          : run.isEmbedded && run.objectPath && <p>Font substitution is not available for this occurrence.</p>)}
        {canSubstitute && <div className="existing-text-font-choice">
          <div><span>Replacement font</span><strong>{font?.name ?? "Original PDF font"}</strong></div>
          <button type="button" className="button" disabled={busy} onClick={()=>setPickerOpen(true)}>Choose substitute font…</button>
          {font && <>
            <p className="existing-text-preview" aria-label="Replacement font preview" style={{fontFamily:customFontState(font.id)?.family,fontSize:Math.min(28,Math.max(16,displayFontSize)),fontKerning:"none",fontVariantLigatures:"none",fontSynthesis:"none",whiteSpace:paragraphPaste?"pre-wrap":undefined}}>{invalid ? "Update the replacement text to preview this font." : activeText}</p>
            <button type="button" className="existing-text-original-button" disabled={busy} onClick={useOriginal}>Use original font</button>
          </>}
        </div>}
        {message && <p id={errorId} className="existing-text-error" role="alert">{message}</p>}
      </div>
      <footer className="dialog-actions">
        {onEditParagraph && <button type="button" className="button" disabled={busy} onClick={()=>editParagraph(paragraphPaste?.prepared)}>{paragraphPaste?"Paste as paragraph":"Edit paragraph…"}</button>}
        {onEditTogether && !run.objectPath && <button type="button" className="button" disabled={busy||!!paragraphPaste} onClick={onEditTogether}>Edit together…</button>}
        <button type="button" className="button" disabled={busy} onClick={cancel}>Cancel</button>
        {editable && <button type="submit" className="button primary" disabled={!canApply}>{busy ? "Applying…" : "Apply changes"}</button>}
      </footer>
    </form>
  </Modal>;
}
