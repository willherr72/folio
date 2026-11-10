import {useEffect,useMemo,useRef,useState} from "react";
import {Modal} from "./Modal";
import type {FolioAdapter} from "../editor/adapter";
import type {DocumentInfo,EditorDocument,PageContentReplacement,PagePlan} from "../editor/types";
import {pageReplacementRequest,replacementPages} from "../editor/page-replacement";
import {discardOpenedDocument} from "../editor/discard-opened-document";
import {useComposedPage} from "../editor/use-composed-page";
import "./page-replacement.css";

interface Props {
 adapter:FolioAdapter;document:EditorDocument;page:PagePlan;sourceIds:string[];
 onApply(result:PageContentReplacement):boolean;onCancel():void;
}
export function PageReplacementDialog({adapter,document,page,sourceIds,onApply,onCancel}:Props) {
 const borrowed=useMemo(()=>new Set(sourceIds),[sourceIds]);
 const [incoming,setIncoming]=useState<DocumentInfo|null>(null),incomingOwner=useRef<DocumentInfo|null>(null);
 const pendingIncoming=useRef<DocumentInfo|null>(null);
 const [pageNumber,setPageNumber]=useState("1");
 const [candidate,setCandidate]=useState<{result:PageContentReplacement;page:PagePlan}|null>(null),candidateOwner=useRef<DocumentInfo|null>(null);
 const [busy,setBusy]=useState<"choosing"|"preparing"|null>(null),running=useRef(false);
 const [error,setError]=useState<string|null>(null),[applied,setApplied]=useState(false);
 const alive=useRef(false),closed=useRef(false),generation=useRef(0),submitted=useRef(false);
 const release=(info:DocumentInfo|null)=>{if(info?.id&&!borrowed.has(info.id))void discardOpenedDocument(info,id=>adapter.closeDocument(id)).catch(()=>{});};
 const disposeCandidate=()=>{const info=candidateOwner.current;candidateOwner.current=null;release(info);};
 const disposeIncoming=()=>{const info=incomingOwner.current;incomingOwner.current=null;if(info?.id!==pendingIncoming.current?.id)release(info);};
 useEffect(()=>{alive.current=true;return()=>{alive.current=false;generation.current++;disposeCandidate();disposeIncoming();};},[]);
 const currentPreview=useComposedPage(adapter,page,900,true,true);
 const preview=useComposedPage(adapter,candidate?.page??page,900,!!candidate,true);
 const validPage=!!incoming&&pageNumber.trim()!==""&&Number.isInteger(Number(pageNumber))&&Number(pageNumber)>=1&&Number(pageNumber)<=incoming.pages.length;
 let admission:string|null=null;
 try {pageReplacementRequest(document,page);if(!adapter.replacePageContent||!adapter.renderPagePlan)admission="Page replacement requires the desktop PDF service.";}catch(cause){admission=cause instanceof Error?cause.message:String(cause);}
 const ready=!!candidate&&!!preview.url&&!preview.loading&&!preview.error&&!busy&&!applied&&!admission&&validPage;
 const invalidate=()=>{generation.current++;disposeCandidate();setCandidate(null);setError(null);};
 const close=()=>{closed.current=true;invalidate();disposeIncoming();onCancel();};
 const choose=async()=>{
  if(running.current||submitted.current||closed.current)return;
  invalidate();const request=generation.current;running.current=true;setBusy("choosing");
  let orphan:DocumentInfo|null=null;
  try {
   const opened=await adapter.openPdf();
   if(!opened)return;
   if(!opened.id||borrowed.has(opened.id)||opened.id===incomingOwner.current?.id)throw new Error("Choose a PDF as a new source.");
   orphan=opened;
   if(!alive.current||closed.current||request!==generation.current)return;
   if(!opened.pages.length)throw new Error("The incoming PDF has no pages.");
   disposeIncoming();incomingOwner.current=opened;orphan=null;setIncoming(opened);setPageNumber("1");
  }catch(cause){if(alive.current&&!closed.current&&request===generation.current)setError(cause instanceof Error?cause.message:String(cause));}
  finally {release(orphan);running.current=false;if(alive.current)setBusy(null);}
 };
 const prepare=async()=>{
  if(running.current||submitted.current||closed.current||!incoming||!validPage||admission||!adapter.replacePageContent)return;
  invalidate();const request=generation.current;running.current=true;setBusy("preparing");
  let orphan:DocumentInfo|null=null;
  try {
   const selection=pageReplacementRequest(document,page);
   pendingIncoming.current=incoming;
   const result=await adapter.replacePageContent(selection.sourceId,selection.occurrences,selection.selectedOccurrence,incoming.id,Number(pageNumber)-1);
   if(result.document.id!==incoming.id&&!borrowed.has(result.document.id))orphan=result.document;
   if(!alive.current||closed.current||request!==generation.current)return;
   if(result.document.id===incoming.id||borrowed.has(result.document.id))throw new Error("Replacement must return a new source version.");
   const pages=replacementPages(document,page,result);
   const replaced=pages.find(value=>value.id===page.id)!;
   candidateOwner.current=result.document;orphan=null;setCandidate({result,page:replaced});
  }catch(cause){if(alive.current&&!closed.current&&request===generation.current)setError(cause instanceof Error?cause.message:String(cause));}
  finally {
   release(orphan);const pending=pendingIncoming.current;pendingIncoming.current=null;
   if(pending?.id!==incomingOwner.current?.id)release(pending);
   running.current=false;if(alive.current)setBusy(null);
  }
 };
 const apply=()=>{
  if(!ready||!candidate||submitted.current)return;
  submitted.current=true;const owned=candidateOwner.current;candidateOwner.current=null;
  try {
   if(!onApply(candidate.result))throw new Error("The document changed. Close this dialog and try again.");
   setApplied(true);
  }catch(cause){
   submitted.current=false;
   if(alive.current&&!closed.current){candidateOwner.current=owned;setError(cause instanceof Error?cause.message:String(cause));}
   else release(owned);
  }
 };
 return <Modal title="Replace page content" className="page-replacement-dialog" description="Fit content from another PDF into this page. Its size, crop, rotation, comments, forms, and added edits stay in place." onClose={close}>
  <div className="replacement-source-controls">
   <button className="button" disabled={!!busy||applied||!!admission} onClick={()=>void choose()}>Choose PDF…</button>
   {incoming&&<><strong title={incoming.name}>{incoming.name}</strong><label>Incoming page<input aria-label="Incoming page" type="number" min="1" max={incoming.pages.length} step="1" value={pageNumber} disabled={busy==="choosing"||applied} onChange={event=>{invalidate();setPageNumber(event.target.value);}}/></label><span>of {incoming.pages.length}</span></>}
  </div>
  <p>Only this selected copy changes. Incoming comments and form fields are not imported. You can undo after applying.</p>
  {admission&&<p role="alert" className="error-note">{admission}</p>}
  {error&&<p role="alert" className="error-note">{error}</p>}
  {incoming&&!validPage&&<p role="alert" className="error-note">Choose a whole page number from 1 to {incoming.pages.length}.</p>}
  {busy&&<p role="status">{busy==="choosing"?"Choosing incoming PDF…":"Preparing replacement…"}</p>}
  <div className="replacement-preview-grid">
   <figure><figcaption>Current page</figcaption>{currentPreview.url?<img src={currentPreview.url} alt="Current page preview"/>:<p role="status">Rendering current page…</p>}{currentPreview.error&&<p role="alert" className="error-note">{currentPreview.error}</p>}</figure>
   <figure><figcaption>Replacement</figcaption>{candidate&&preview.url&&<img src={preview.url} alt="Replacement page preview"/>}{preview.loading&&<p role="status">Rendering replacement…</p>}{preview.error&&<p role="alert" className="error-note">{preview.error}</p>}{!candidate&&<p>Choose a PDF and preview its content here.</p>}</figure>
  </div>
  <div className="dialog-actions"><button className="button" onClick={close}>Cancel</button><button className="button" disabled={!validPage||!!busy||applied||!!admission} onClick={()=>void prepare()}>Preview replacement</button><button className="button primary" disabled={!ready} onClick={apply}>Apply replacement</button></div>
 </Modal>;
}
