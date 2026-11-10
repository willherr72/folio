import {useEffect,useRef,useState} from "react";
import {Modal} from "./Modal";
import {imageExportSelection,startPageImageExport,type PageImageOptions,type PageImageProgress,type PageImageResult} from "../editor/page-image-export";
import type {PagePlan} from "../editor/types";
import "./release-dialogs.css";
interface Props{pages:PagePlan[];currentPageId:string|null;onClose():void;onComplete(result:PageImageResult):void;onBusyChange(busy:boolean):void}
export function ExportImagesDialog({pages,currentPageId,onClose,onComplete,onBusyChange}:Props){
 const [mode,setMode]=useState<"current"|"all"|"range">("current"),[range,setRange]=useState("");
 const [format,setFormat]=useState<PageImageOptions["format"]>("png"),[dpi,setDpi]=useState<PageImageOptions["dpi"]>(150),[quality,setQuality]=useState("90");
 const [busy,setBusy]=useState(false),[cancelling,setCancelling]=useState(false),[failure,setFailure]=useState<string|null>(null),[notice,setNotice]=useState<string|null>(null),[progress,setProgress]=useState<PageImageProgress|null>(null);
 const task=useRef<ReturnType<typeof startPageImageExport>|null>(null),pending=useRef(false),alive=useRef(true),notifyBusy=useRef(onBusyChange);notifyBusy.current=onBusyChange;
 useEffect(()=>{alive.current=true;return()=>{alive.current=false;void task.current?.cancel().catch(()=>{});};},[]);
 let selection:ReturnType<typeof imageExportSelection>|null=null,error:string|null=null;
 try{selection=imageExportSelection(pages,mode,currentPageId,range,{format,dpi,jpegQuality:format==="png"?90:(quality.trim()?Number(quality):NaN)});}catch(value){error=value instanceof Error?value.message:String(value);}
 const cancel=async()=>{if(!pending.current){onClose();return;}setCancelling(true);try{await task.current?.cancel();}catch(value){if(alive.current)setFailure("Couldn’t request cancellation: "+String(value));}};
 const start=async()=>{
  if(pending.current||!selection||error)return;
  pending.current=true;setBusy(true);notifyBusy.current(true);setCancelling(false);setFailure(null);setNotice(null);setProgress(null);
  const work=startPageImageExport(selection.request,event=>{if(alive.current)setProgress(event);});task.current=work;
  try{const result=await work.result;if(alive.current){if(result)onComplete(result);else setNotice("Export cancelled. The PDF is unchanged.");}}
  catch(value){if(alive.current)setFailure(value instanceof Error?value.message:String(value));}
  finally{task.current=null;pending.current=false;notifyBusy.current(false);if(alive.current){setBusy(false);setCancelling(false);}}
 };
 const status=cancelling?"Cancelling… the current native operation may need to finish first.":progress?.stage==="choosingFolder"?"Choose a destination in the folder dialog. Cancelling that dialog leaves the PDF unchanged.":progress?`${progress.completed} of ${progress.total} pages exported — ${progress.stage}.`:"Preparing export…";
 return <Modal className="release-dialog" title="Export pages as images" description="Save the visible pages with your current edits, rotation and crop. Images contain no editable text or interactive form fields." onClose={()=>void cancel()}>
  <div className="release-form">
   <label>Pages<select disabled={busy} value={mode} onChange={event=>setMode(event.target.value as typeof mode)}><option value="current">Current page</option><option value="all">All pages ({pages.length})</option><option value="range">Page range</option></select></label>
   {mode==="range"&&<label>Page range<input disabled={busy} value={range} onChange={event=>setRange(event.target.value)} placeholder="1, 3-5" aria-invalid={!!error}/></label>}
   <label>Format<select disabled={busy} value={format} onChange={event=>setFormat(event.target.value as typeof format)}><option value="png">PNG — lossless</option><option value="jpeg">JPEG — smaller files</option></select></label>
   <label>Resolution<select disabled={busy} value={dpi} onChange={event=>setDpi(Number(event.target.value) as typeof dpi)}>{[72,150,300,600].map(value=><option key={value} value={value}>{value} dpi</option>)}</select></label>
   {format==="jpeg"&&<label>JPEG quality<input disabled={busy} type="number" min="1" max="100" step="1" value={quality} onChange={event=>setQuality(event.target.value)}/></label>}
   {selection&&<p>{selection.request.pages.length} page{selection.request.pages.length===1?"":"s"} selected. First image: approximately {selection.firstSize.width} × {selection.firstSize.height} pixels.</p>}
   <p>A new Folio images folder will be created inside your chosen folder. Filenames keep the current page numbers. Both formats use a white background.</p>
   <p>Exporting images does not save changes to your PDF.</p>
   {(error||failure)&&<p className="error-note" role="alert">{error||failure}</p>}
   {busy&&<p role="status" aria-live="polite">{status}</p>}{notice&&<p role="status">{notice}</p>}
  </div>
  <div className="dialog-actions"><button className="button" disabled={cancelling} onClick={()=>void cancel()}>{busy?"Cancel export":"Cancel"}</button><button className="button primary" disabled={busy||!!error} onClick={()=>void start()}>Choose folder and export…</button></div>
 </Modal>;
}
