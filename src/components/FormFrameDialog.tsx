import {useEffect,useLayoutEffect,useRef,useState} from 'react';
import {Modal} from './Modal';
import type {FolioAdapter} from '../editor/adapter';
import type {DocumentInfo,EditorDocument,FormField,PageContentReplacement,PagePlan} from '../editor/types';
import {frameDraft,fieldFrameRequest,validateFrameAppearance,type FrameDraft,type FormFrameAppearance,type SetFormFrameRequest} from '../editor/form-frame';
import {formAuthorSelection} from '../editor/form-authoring';
import {replacementPages} from '../editor/page-replacement';
import {discardOpenedDocument} from '../editor/discard-opened-document';
import {pageVisibleRect} from '../editor/geometry';
import {useComposedPage} from '../editor/use-composed-page';
import './form-author.css';

interface Props {
 adapter:FolioAdapter;document:EditorDocument;page:PagePlan;field:FormField;sourceIds:string[];
 isBorrowed?(id:string):boolean;
 onApply(result:PageContentReplacement):boolean;onCancel():void;
}
interface CandidateOwner {document:DocumentInfo;adapter:FolioAdapter;borrowed:Set<string>}
const styleLabels={solid:'Solid',dashed:'Dashed',underline:'Underline',beveled:'Beveled',inset:'Inset'};
export function FormFrameDialog({adapter,document,page,field,sourceIds,isBorrowed,onApply,onCancel}:Props){
 const borrowedNow=useRef({sourceIds,isBorrowed});borrowedNow.current={sourceIds,isBorrowed};
 const isCurrentSource=(id:string)=>borrowedNow.current.sourceIds.includes(id)||!!borrowedNow.current.isBorrowed?.(id);
 const [style,setStyle]=useState<FormFrameAppearance|null>(null),[draft,setDraft]=useState<FrameDraft|null>(null);
 const [inspectionError,setInspectionError]=useState<string|null>(null),[error,setError]=useState<string|null>(null);
 const [candidate,setCandidate]=useState<{result:PageContentReplacement;page:PagePlan}|null>(null),owner=useRef<CandidateOwner|null>(null);
 const [busy,setBusy]=useState(false),running=useRef(false),[applied,setApplied]=useState(false);
 const alive=useRef(false),closed=useRef(false),generation=useRef(0),submitted=useRef(false);
 const release=(owned:CandidateOwner|null)=>{
  if(owned?.document.id&&!owned.borrowed.has(owned.document.id)&&!isCurrentSource(owned.document.id))void discardOpenedDocument(owned.document,id=>owned.adapter.closeDocument(id)).catch(()=>{});
 };
 const dispose=()=>{const info=owner.current;owner.current=null;release(info);};
 useEffect(()=>{alive.current=true;return()=>{alive.current=false;generation.current++;dispose();};},[]);
 useLayoutEffect(()=>{
  generation.current++;dispose();setCandidate(null);setError(null);
  let cancelled=false;setStyle(null);setDraft(null);setInspectionError(null);
  void Promise.resolve().then(()=>{
   if(!adapter.inspectFormFrame)throw new Error('Border editing requires the desktop PDF service.');
   formAuthorSelection(document,page);
   return adapter.inspectFormFrame(page.sourceId,page.pageIndex,field.annotationIndex);
  }).then(value=>{
   if(cancelled||closed.current)return;
   validateFrameAppearance(value);setStyle(value);setDraft(frameDraft(value));
  }).catch(e=>{if(!cancelled&&!closed.current)setInspectionError(e instanceof Error?e.message:String(e));});
  return()=>{cancelled=true;};
 },[adapter,document,page,field]);
 let request:SetFormFrameRequest|null=null,admission:string|null=null;
 if(style&&draft){try{
  request=fieldFrameRequest(document,page,field,style,draft);
  if(!adapter.setFormFrame||!adapter.renderPagePlan)admission='Border editing requires the desktop PDF service.';
 }catch(e){admission=e instanceof Error?e.message:String(e);}}
 const preview=useComposedPage(adapter,candidate?.page??page,1000,true,true);
 const ready=!!candidate&&!!preview.url&&!preview.loading&&!preview.error&&!busy&&!applied&&!closed.current&&!admission&&!!request;
 const invalidate=()=>{generation.current++;dispose();setCandidate(null);setError(null);};
 const change=(next:Partial<FrameDraft>)=>{invalidate();setDraft(old=>old?{...old,...next}:null);};
 const close=()=>{closed.current=true;invalidate();onCancel();};
 const prepare=async()=>{
  if(running.current||submitted.current||closed.current||admission||!adapter.setFormFrame||!request)return;
  invalidate();const version=generation.current,borrowed=new Set(sourceIds);running.current=true;setBusy(true);let orphan:CandidateOwner|null=null;
  try{
   const selection=formAuthorSelection(document,page);
   const result=await adapter.setFormFrame(selection.sourceId,selection.occurrences,selection.selectedOccurrence,request);
   if(!borrowed.has(result.document.id)&&!isCurrentSource(result.document.id))orphan={document:result.document,adapter,borrowed};
   if(!alive.current||closed.current||version!==generation.current)return;
   if(!orphan)throw new Error('Border editing must return a new source version.');
   const pages=replacementPages(document,page,result);
   owner.current=orphan;orphan=null;setCandidate({result,page:pages.find(p=>p.id===page.id)!});
  }catch(e){if(alive.current&&!closed.current&&version===generation.current)setError(e instanceof Error?e.message:String(e));}
  finally{release(orphan);running.current=false;if(alive.current)setBusy(false);}
 };
 const apply=()=>{
  if(!ready||!candidate||submitted.current)return;
  if(isCurrentSource(candidate.result.document.id)){invalidate();setError('Border editing must return a new source version.');return;}
  submitted.current=true;const owned=owner.current;owner.current=null;
  try{if(!onApply(candidate.result))throw new Error('The document changed. Close this dialog and try again.');setApplied(true);}
  catch(e){submitted.current=false;if(alive.current&&!closed.current){owner.current=owned;setError(e instanceof Error?e.message:String(e));}else release(owned);}
 };
 const visible=pageVisibleRect(page);
 return <Modal title="Border and background" description="Preview the selected field’s border and background, then apply." className="form-author-dialog" onClose={close}>
  <div className="form-author-grid"><div className="form-author-controls">
   <p>{field.label||field.name||'Selected field'}</p>
   {!style&&!inspectionError&&<p role="status">Reading border settings…</p>}
   {style&&draft&&<>
    <label>Border width (pt)<input type="number" min="0" max="32" step="0.25" value={draft.width} disabled={applied} onChange={e=>change({width:e.target.value})}/></label>
    <p>Set width to 0 for no border.</p>
    <label>Border style<select value={draft.style} disabled={applied} onChange={e=>change({style:e.target.value as FrameDraft['style']})}>
     <option value="original">{style.style?`Original (${styleLabels[style.style]})`:'Original'}</option>
     {style.supportedStyles.map(s=><option key={s} value={s}>{styleLabels[s]}</option>)}
    </select></label>
    {(['border','background'] as const).map(key=><div key={key}>
     <label>{key==='border'?'Border color':'Background'}<select value={draft[`${key}Mode`]} disabled={applied} onChange={e=>change({[`${key}Mode`]:e.target.value})}>
      <option value="original">Original</option><option value="transparent">Transparent</option><option value="rgb">Custom RGB</option>
     </select></label>
     {draft[`${key}Mode`]==='rgb'?<label>{key==='border'?'Border RGB color':'Background RGB color'}<input type="text" maxLength={7} value={draft[`${key}Color`]} disabled={applied} onChange={e=>change({[`${key}Color`]:e.target.value})} placeholder="#RRGGBB"/></label>:draft[`${key}Mode`]==='original'&&<p>{style[key].colorSpace==='transparent'?'Originally transparent.':`Original ${style[key].colorSpace.toUpperCase()} color is preserved.`}</p>}
    </div>)}
    <button className="button" disabled={applied} onClick={()=>{invalidate();setDraft(frameDraft(style));}}>Reset appearance</button>
    <p>Answers stay unchanged. Only the selected field gets the new border and background.</p>
   </>}
  </div><div className="form-author-preview"><div style={{aspectRatio:`${visible.width}/${visible.height}`,width:`min(100%, calc(var(--form-preview-height) * ${visible.width/visible.height}))`}}>{preview.url&&<img src={preview.url} alt={candidate?'Border and background preview':'Page preview'}/>}</div>{preview.loading&&<p role="status">Rendering page…</p>}{preview.error&&<p role="alert">{preview.error}</p>}</div></div>
  {(inspectionError||error||admission)&&<p role="alert" className="error-note">{inspectionError||error||admission}</p>}
  {busy&&<p role="status">Preparing appearance…</p>}
  <div className="dialog-actions"><button className="button" onClick={close}>Cancel</button><button className="button" disabled={busy||applied||closed.current||!!admission||!request} onClick={()=>void prepare()}>Preview appearance</button><button className="button primary" disabled={!ready} onClick={apply}>Apply appearance</button></div>
 </Modal>;
}
