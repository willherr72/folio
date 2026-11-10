import {useEffect,useMemo,useRef,useState} from "react";
import {Modal} from "./Modal";
import {splitPdfSelection,startPdfSplit,type SplitMode,type SplitPdfProgress,type SplitPdfResult} from "../editor/pdf-split";
import type {DocumentBookmark,PagePlan} from "../editor/types";
import "./release-dialogs.css";
interface Props{pages:PagePlan[];bookmarks?:DocumentBookmark[];bookmarkError?:string;metadata?:import("../editor/document-properties").DocumentMetadataEdit;onClose():void;onComplete(result:SplitPdfResult):void;onBusyChange(busy:boolean):void}
export function SplitPdfDialog({pages,bookmarks,bookmarkError,metadata,onClose,onComplete,onBusyChange}:Props){
 const[mode,setMode]=useState<SplitMode>("every"),[every,setEvery]=useState("1"),[after,setAfter]=useState("");
 const[busy,setBusy]=useState(false),[cancelling,setCancelling]=useState(false),[failure,setFailure]=useState<string|null>(null),[notice,setNotice]=useState<string|null>(null),[progress,setProgress]=useState<SplitPdfProgress|null>(null);
 const task=useRef<ReturnType<typeof startPdfSplit>|null>(null),pending=useRef(false),alive=useRef(true),notifyBusy=useRef(onBusyChange);notifyBusy.current=onBusyChange;
 useEffect(()=>{alive.current=true;return()=>{alive.current=false;void task.current?.cancel().catch(()=>{});};},[]);
 const{selection,error}=useMemo(()=>{
  try{if(bookmarkError)throw Error("Unreadable bookmarks must be resolved explicitly in the Bookmarks panel before splitting.");return{selection:splitPdfSelection(pages,mode,mode==="every"?every:after,bookmarks,metadata),error:null};}
  catch(value){return{selection:null,error:value instanceof Error?value.message:String(value)};}
 },[pages,bookmarks,bookmarkError,metadata,mode,every,after]);
 const cancel=async()=>{if(!pending.current){onClose();return;}setCancelling(true);try{await task.current?.cancel();}catch(value){if(alive.current){setCancelling(false);setFailure("Couldn’t request cancellation: "+String(value));}}};
 const start=async()=>{
  if(pending.current||!selection||error)return;
  pending.current=true;setBusy(true);notifyBusy.current(true);setCancelling(false);setFailure(null);setNotice(null);setProgress(null);
  const work=startPdfSplit(selection.request,event=>{if(alive.current)setProgress(event);});task.current=work;
  try{const result=await work.result;if(alive.current){if(result)onComplete(result);else setNotice("Export cancelled. The PDF is unchanged.");}}
  catch(value){if(alive.current)setFailure(value instanceof Error?value.message:String(value));}
  finally{task.current=null;pending.current=false;notifyBusy.current(false);if(alive.current){setBusy(false);setCancelling(false);}}
 };
 const status=cancelling?"Cancelling… the current native operation may need to finish first.":progress?.stage==="choosingFolder"?"Choose a destination in the folder dialog. To cancel now, close that dialog.":progress?`${progress.completed} of ${progress.total} PDFs exported — ${progress.stage}.`:"Preparing export…";
 return <Modal className="release-dialog pdf-split-dialog" title="Split PDF" description="Create editable PDFs from the current page order, including your edits, rotation and crop." onClose={()=>void cancel()}>
  <div className="release-form">
   <label>Split method<select disabled={busy} value={mode} onChange={e=>{setMode(e.target.value as SplitMode);setFailure(null);}}><option value="every">Every N pages</option><option value="after">After selected pages</option><option value="bookmarks">At top-level bookmarks</option></select></label>
   {mode==="every"&&<label>Pages per PDF<input type="number" min="1" max="10000" step="1" disabled={busy} value={every} onChange={e=>{setEvery(e.target.value);setFailure(null);}}/></label>}
   {mode==="after"&&<><label>Split after pages<input disabled={busy} value={after} onChange={e=>{setAfter(e.target.value);setFailure(null);}} placeholder="2, 5-7" aria-invalid={!!error}/></label><p>Use current page numbers before the final page. Each number ends a part.</p></>}
   {mode==="bookmarks"&&<p>Each top-level bookmark that points to a page starts a part. Leading pages are retained; nested and external bookmarks do not define splits.</p>}
   {selection&&<><p>{selection.parts.length} PDF{selection.parts.length===1?"":"s"} · All {pages.length} page{pages.length===1?"":"s"} included</p><div className="pdf-split-parts" tabIndex={0} aria-label="PDF parts preview"><table><thead><tr><th>File</th><th>Pages</th></tr></thead><tbody>{selection.parts.map(part=><tr key={part.name}><td>{part.name}</td><td>{part.start+1===part.end?part.end:`${part.start+1}–${part.end}`}</td></tr>)}</tbody></table></div></>}
   <p>A new Folio split folder will be created inside your chosen folder. Links to pages outside a part are removed. Splitting does not save changes to the open PDF.</p>
   {(error||failure)&&<p className="error-note" role="alert">{error||failure}</p>}{busy&&<p role="status" aria-live="polite">{status}</p>}{notice&&<p role="status">{notice}</p>}
  </div>
  <div className="dialog-actions"><button className="button" disabled={cancelling} onClick={()=>void cancel()}>{busy?"Cancel export":"Cancel"}</button><button className="button primary" disabled={busy||!!error} onClick={()=>void start()}>Choose folder and split…</button></div>
 </Modal>;
}
