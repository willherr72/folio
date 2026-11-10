import {useRef,useState} from "react";
import {Modal} from "./Modal";
import {selectPages} from "../editor/page-selection";
import type {PagePlan} from "../editor/types";
import "./release-dialogs.css";

export function ExtractPagesDialog({pages,currentPageId,onClose,onExport}:{pages:PagePlan[];currentPageId:string|null;onClose():void;onExport(pages:PagePlan[]):Promise<boolean>}){
 const [mode,setMode]=useState<"all"|"current"|"range">("current"),[range,setRange]=useState("");
 const [busy,setBusy]=useState(false),[failure,setFailure]=useState<string|null>(null);
 const pending=useRef(false);
 let selected:PagePlan[]=[];let error:string|null=null;
 try{selected=selectPages(pages,mode,currentPageId,range);}catch(value){error=value instanceof Error?value.message:String(value);}
 const close=()=>{if(!pending.current)onClose();};
 const save=async()=>{
  if(pending.current || error)return;
  pending.current=true;setBusy(true);setFailure(null);
  try{if(await onExport(selected))onClose();}
  catch(value){setFailure(value instanceof Error?value.message:String(value));}
  finally{pending.current=false;setBusy(false);}
 };
 return <Modal className="release-dialog" title="Extract pages" description="Save selected pages as a separate editable PDF, including your current edits. This document stays unchanged." onClose={close}>
  <div className="release-form">
   <label>Pages<select disabled={busy} value={mode} onChange={event=>{setMode(event.target.value as typeof mode);setFailure(null);}}><option value="current">Current page</option><option value="all">All pages ({pages.length})</option><option value="range">Page range</option></select></label>
   {mode==="range"&&<label>Page range<input autoFocus disabled={busy} value={range} onChange={event=>{setRange(event.target.value);setFailure(null);}} placeholder="1, 3-5" aria-invalid={!!error}/></label>}
   <p>Page numbers follow the current arrangement. Overlapping ranges include each page once, in document order.</p>
   {(error||failure)&&<p className="error-note" role="alert">{error||failure}</p>}
   <p>{selected.length} page{selected.length===1?"":"s"} selected.</p>
   {busy&&<p role="status">Saving extracted PDF…</p>}
  </div>
  <div className="dialog-actions"><button className="button" disabled={busy} onClick={close}>Cancel</button><button className="button primary" disabled={busy||!!error} onClick={()=>void save()}>Save extracted PDF…</button></div>
 </Modal>;
}
