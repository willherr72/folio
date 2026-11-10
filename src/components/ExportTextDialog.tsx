import {useEffect,useRef,useState} from 'react';
import {Modal} from './Modal';
import {textExportSelection,startTextExport,type TextExportProgress,type TextExportResult} from '../editor/text-export';
import type {PagePlan} from '../editor/types';
import './release-dialogs.css';
interface Props{pages:PagePlan[];currentPageId:string|null;onClose():void;onComplete(result:TextExportResult):void;onBusyChange(busy:boolean):void}
export function ExportTextDialog({pages,currentPageId,onClose,onComplete,onBusyChange}:Props){
 const [mode,setMode]=useState<'current'|'all'|'range'>('current'),[range,setRange]=useState(''),[includeFormValues,setIncludeFormValues]=useState(true);
 const [busy,setBusy]=useState(false),[cancelling,setCancelling]=useState(false),[failure,setFailure]=useState<string|null>(null),[notice,setNotice]=useState<string|null>(null),[progress,setProgress]=useState<TextExportProgress|null>(null),[result,setResult]=useState<TextExportResult|null>(null);
 const task=useRef<ReturnType<typeof startTextExport>|null>(null),pending=useRef(false),alive=useRef(true),notifyBusy=useRef(onBusyChange);notifyBusy.current=onBusyChange;
 useEffect(()=>{alive.current=true;return()=>{alive.current=false;void task.current?.cancel().catch(()=>{});};},[]);
 let selection:ReturnType<typeof textExportSelection>|null=null,error:string|null=null;
 try{selection=textExportSelection(pages,mode,currentPageId,range,includeFormValues);}catch(value){error=value instanceof Error?value.message:String(value);}
 const cancel=async()=>{
  if(!pending.current){onClose();return;}setCancelling(true);
  try{await task.current?.cancel();}catch(value){if(alive.current){setFailure('Could not request cancellation: '+String(value));setCancelling(false);}}
 };
 const start=async()=>{
  if(pending.current||!selection||error)return;
  pending.current=true;setBusy(true);notifyBusy.current(true);setCancelling(false);setFailure(null);setNotice(null);setProgress(null);setResult(null);
  const work=startTextExport(selection,event=>{if(alive.current)setProgress(event);});task.current=work;
  try{const output=await work.result;if(alive.current){if(output){setResult(output);onComplete(output);}else setNotice('Export cancelled. The PDF is unchanged.');}}
  catch(value){if(alive.current)setFailure(value instanceof Error?value.message:String(value));}
  finally{task.current=null;pending.current=false;notifyBusy.current(false);if(alive.current){setBusy(false);setCancelling(false);}}
 };
 const status=cancelling?'Cancelling… the current native operation may need to finish first.':progress?.stage==='choosingFile'?'Choose a destination in the Save dialog. Cancel that dialog to return.':progress?`${progress.completed} of ${progress.total} pages processed — ${progress.stage}.`:'Preparing export…';
 return <Modal className="release-dialog" title="Export text" description="Extract selectable text from your edited pages into a UTF-8 text file." onClose={()=>void cancel()}>
  <div className="release-form">
   <label>Pages<select disabled={busy} value={mode} onChange={event=>setMode(event.target.value as typeof mode)}><option value="current">Current page</option><option value="all">All pages ({pages.length})</option><option value="range">Page range</option></select></label>
   {mode==='range'&&<label>Page range<input disabled={busy} value={range} onChange={event=>setRange(event.target.value)} placeholder="1, 3-5" aria-invalid={!!error}/></label>}
   <label style={{display:'flex',alignItems:'center',gap:8}}><input type="checkbox" disabled={busy} checked={includeFormValues} onChange={event=>setIncludeFormValues(event.target.checked)}/>Include form values</label>
   <p>Form values appear in a labeled section after each page. Hidden fields and password values are omitted.</p>
   {selection&&<p>{selection.pages.length} page{selection.pages.length===1?'':'s'} selected. Pages stay in document order, separated by a page-break character.</p>}
   <p>Current text edits, added text and saved OCR are included. Scanned images need Recognize text first. Columns and tables follow the PDF’s reading order; formatting, comments and drawings are not exported.</p>
   <p>Text outside the page crop is omitted. Text covered by drawings may remain selectable and appear in the export.</p>
   <p>The text file is unencrypted. Exporting it does not save changes to your PDF.</p>
   {(error||failure)&&<p className="error-note" role="alert">{error||failure}</p>}
   {busy&&<p role="status" aria-live="polite">{status}</p>}{notice&&<p role="status">{notice}</p>}
   {result&&<div role="status"><p>Exported {result.pageCount} page{result.pageCount===1?'':'s'} to {result.path}</p>
    {result.emptyPages>0&&<p>{result.emptyPages} {result.emptyPages===1?'page had':'pages had'} no extractable text. Run Recognize text on scanned pages if needed.</p>}
    {result.skippedFields>0&&<p>{result.skippedFields} password or unsupported field{result.skippedFields===1?' was':'s were'} omitted.</p>}
   </div>}
  </div>
  <div className="dialog-actions"><button className="button" disabled={cancelling} onClick={()=>void cancel()}>{busy?'Cancel export':result?'Done':'Cancel'}</button><button className="button primary" disabled={busy||!!error} onClick={()=>void start()}>Choose file and export…</button></div>
 </Modal>;
}
