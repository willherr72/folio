import {useEffect, useId, useRef, useState} from "react";
import type {EditableTextRun, TextGroupPreview} from "../editor/types";
import {Modal} from "./Modal";
import "./existing-text.css";

interface TextGroupDialogProps {
  anchor: EditableTextRun;
  runs: EditableTextRun[];
  busy: boolean;
  error: string | null;
  loading?: boolean;
  onInspect(indices: number[]): Promise<TextGroupPreview>;
  onApply(preview: TextGroupPreview, replacement: string): void | Promise<void>;
  onDraftChange?(): void;
  onCancel(): void;
}

export function TextGroupDialog({anchor,runs,busy,error,loading=false,onInspect,onApply,onDraftChange,onCancel}: TextGroupDialogProps) {
  const [selected,setSelected]=useState([anchor.objectIndex]);
  const [preview,setPreview]=useState<TextGroupPreview|null>(null);
  const [replacement,setReplacement]=useState("");
  const [checking,setChecking]=useState(false);
  const [checkError,setCheckError]=useState<string|null>(null);
  const [pasteError,setPasteError]=useState<string|null>(null);
  const request=useRef(0);
  const alive=useRef(true);
  const applying=useRef(false);
  const input=useRef<HTMLInputElement>(null);
  const hintId=useId();
  const errorId=useId();
  useEffect(()=>{alive.current=true;return ()=>{alive.current=false;request.current++;};},[]);
  useEffect(()=>{if(preview)input.current?.select();},[preview]);
  const ordered=[...runs].sort((a,b)=>a.objectIndex-b.objectIndex);
  const anchorPosition=ordered.findIndex(run=>run.objectIndex===anchor.objectIndex);
  const nearby=anchorPosition<0 ? [anchor] : ordered.slice(Math.max(0,anchorPosition-8),anchorPosition+9);
  const consecutive=selected.every((index,position)=>position===0 || index===selected[position-1]+1);
  const canCheck=!busy && !loading && !checking && selected.length>=2 && consecutive;
  const invalid=!replacement.trim() ? "Enter replacement text."
    : /[\u0000-\u001f\u007f-\u009f]/.test(replacement) ? "Use a single line without control characters."
    : replacement.length>1000 ? "Use at most 1,000 characters." : null;
  const message=pasteError ?? checkError ?? error ?? (preview && replacement!==preview.text ? invalid : null);
  const canApply=!!preview && !busy && !invalid && replacement!==preview.text;
  const toggle=(index:number)=>{
    if(busy || applying.current || index===anchor.objectIndex)return;
    if(!selected.includes(index) && selected.length>=8)return;
    request.current++;setChecking(false);setPreview(null);setCheckError(null);setPasteError(null);onDraftChange?.();
    setSelected(value=>value.includes(index) ? value.filter(item=>item!==index) : [...value,index].sort((a,b)=>a-b));
  };
  const check=async()=>{
    if(!canCheck)return;
    const token=++request.current;setChecking(true);setPreview(null);setCheckError(null);onDraftChange?.();
    try {
      const result=await onInspect([...selected]);
      if(!alive.current || token!==request.current)return;
      setPreview(result);setReplacement(result.text);
    } catch(error) {
      if(alive.current && token===request.current)setCheckError(error instanceof Error ? error.message : String(error));
    } finally {if(alive.current && token===request.current)setChecking(false);}
  };
  const cancel=()=>{if(busy || applying.current)return;request.current++;setChecking(false);onCancel();};
  const submit=async()=>{
    if(!canApply || !preview || applying.current)return;
    applying.current=true;
    try {await onApply(preview,replacement);}
    catch(error) {if(alive.current)setCheckError(error instanceof Error ? error.message : String(error));}
    finally {applying.current=false;}
  };
  return <Modal title="Edit text together" className="existing-text-dialog" onClose={cancel}>
    <form aria-busy={busy || checking || loading} onSubmit={event=>{event.preventDefault();void submit();}}>
      <div className="existing-text-fields">
        <p>Select 2–8 consecutive pieces of the same line. Check the selection to see whether they can be edited together without changing the original layout.</p>
        {loading ? <p role="status">Loading nearby text…</p> : <fieldset className="text-group-pieces">
          <legend>Nearby text · {selected.length} selected</legend>
          {nearby.map(run=><label key={run.objectIndex}>
            <input type="checkbox" checked={selected.includes(run.objectIndex)} disabled={busy || run.objectIndex===anchor.objectIndex || (selected.length>=8 && !selected.includes(run.objectIndex))} onChange={()=>toggle(run.objectIndex)}/>
            <span>{run.text || "(Empty text)"}</span>{run.objectIndex===anchor.objectIndex && <small>Starting piece</small>}
          </label>)}
        </fieldset>}
        {!consecutive && <p>Select consecutive pieces, including the text between them.</p>}
        <button type="button" className="button text-group-check" disabled={!canCheck} onClick={()=>void check()}>{checking ? "Checking selection…" : "Check selection"}</button>
        {preview && <>
          <p className="existing-text-font">{preview.fontName} · {Number(preview.fontSize.toFixed(2))} pt</p>
          <p className="existing-text-original" aria-label="Original text">{preview.text}</p>
          <label>Replacement text<input ref={input} value={replacement} maxLength={1000} readOnly={busy} aria-describedby={`${hintId}${message ? ` ${errorId}` : ""}`} aria-invalid={!!message}
            onChange={event=>{if(!busy){setReplacement(event.target.value);setPasteError(null);setCheckError(null);onDraftChange?.();}}}
            onPaste={event=>{if(/[\u0000-\u001f\u007f-\u009f]/.test(event.clipboardData.getData("text/plain"))){event.preventDefault();setPasteError("Use a single line without control characters.");}}}/></label>
          <p id={hintId}>The replacement uses the original font, size and starting position. Longer text can use available space; the page does not reflow.</p>
        </>}
        {message && <p id={errorId} className="existing-text-error" role="alert">{message}</p>}
      </div>
      <footer className="dialog-actions"><button type="button" className="button" disabled={busy} onClick={cancel}>Cancel</button>
        {preview && <button type="submit" className="button primary" disabled={!canApply}>{busy ? "Applying…" : "Apply changes"}</button>}
      </footer>
    </form>
  </Modal>;
}
