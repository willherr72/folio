import {useEffect,useLayoutEffect,useMemo,useRef,useState} from 'react';
import {Modal} from './Modal';
import type {FolioAdapter} from '../editor/adapter';
import type {DocumentInfo,EditorDocument,FormField,PageContentReplacement,PagePlan} from '../editor/types';
import {appearanceDraft,fieldAppearanceRequest,type AppearanceDraft,type FormTextAppearance,type SetFormTextAppearanceRequest} from '../editor/form-text-appearance';
import {formAuthorSelection} from '../editor/form-authoring';
import {replacementPages} from '../editor/page-replacement';
import {discardOpenedDocument} from '../editor/discard-opened-document';
import {pageVisibleRect} from '../editor/geometry';
import {useComposedPage} from '../editor/use-composed-page';
import './form-author.css';

interface Props {adapter:FolioAdapter;document:EditorDocument;page:PagePlan;field:FormField;sourceIds:string[];onApply(result:PageContentReplacement):boolean;onCancel():void}
interface CandidateOwner {document:DocumentInfo;adapter:FolioAdapter;borrowed:Set<string>}
export function FormTextAppearanceDialog({adapter,document,page,field,sourceIds,onApply,onCancel}:Props){
 const borrowed=useMemo(()=>new Set(sourceIds),[sourceIds]);
 const [style,setStyle]=useState<FormTextAppearance|null>(null),[draft,setDraft]=useState<AppearanceDraft|null>(null);
 const [inspectionError,setInspectionError]=useState<string|null>(null),[error,setError]=useState<string|null>(null);
 const [candidate,setCandidate]=useState<{result:PageContentReplacement;page:PagePlan}|null>(null),owner=useRef<CandidateOwner|null>(null);
 const [busy,setBusy]=useState(false),running=useRef(false),[applied,setApplied]=useState(false);
 const alive=useRef(false),closed=useRef(false),generation=useRef(0),submitted=useRef(false);
 const release=(owned:CandidateOwner|null)=>{if(owned?.document.id&&!owned.borrowed.has(owned.document.id))void discardOpenedDocument(owned.document,id=>owned.adapter.closeDocument(id)).catch(()=>{});};
 const dispose=()=>{const info=owner.current;owner.current=null;release(info);};
 useEffect(()=>{alive.current=true;return()=>{alive.current=false;generation.current++;dispose();};},[]);
 useLayoutEffect(()=>{
  generation.current++;dispose();setCandidate(null);setError(null);
  let cancelled=false;setStyle(null);setDraft(null);setInspectionError(null);
  void Promise.resolve().then(()=>{if(!adapter.inspectFormTextAppearance)throw new Error('Text appearance requires the desktop PDF service.');return adapter.inspectFormTextAppearance(page.sourceId,page.pageIndex,field.annotationIndex);}).then(value=>{
   if(cancelled||closed.current)return;
   if(!value.styleKey||!Number.isFinite(value.fontSize)||value.fontSize<0||!/^#[0-9a-f]{6}$/i.test(value.color)||!['gray','rgb','cmyk'].includes(value.colorSpace)||!['left','center','right'].includes(value.alignment))throw new Error('The field returned invalid text settings.');
   setStyle(value);setDraft(appearanceDraft(value));
  }).catch(e=>{if(!cancelled&&!closed.current)setInspectionError(e instanceof Error?e.message:String(e));});
  return()=>{cancelled=true;};
 },[adapter,document,page,field]);
 let request:SetFormTextAppearanceRequest|null=null,admission:string|null=null;
 if(style&&draft){try{request=fieldAppearanceRequest(document,page,field,style,draft);if(!adapter.setFormTextAppearance||!adapter.renderPagePlan)admission='Text appearance requires the desktop PDF service.';}catch(e){admission=e instanceof Error?e.message:String(e);}}
 const preview=useComposedPage(adapter,candidate?.page??page,1000,true,true);
 const ready=!!candidate&&!!preview.url&&!preview.loading&&!preview.error&&!busy&&!applied&&!admission&&!!request;
 const invalidate=()=>{generation.current++;dispose();setCandidate(null);setError(null);};
 const change=(next:Partial<AppearanceDraft>)=>{invalidate();setDraft(old=>old?{...old,...next}:null);};
 const close=()=>{closed.current=true;invalidate();onCancel();};
 const prepare=async()=>{
  if(running.current||submitted.current||closed.current||admission||!adapter.setFormTextAppearance||!request)return;
  invalidate();const version=generation.current;running.current=true;setBusy(true);let orphan:CandidateOwner|null=null;
  try{
   const selection=formAuthorSelection(document,page);
   const result=await adapter.setFormTextAppearance(selection.sourceId,selection.occurrences,selection.selectedOccurrence,request);
   if(!borrowed.has(result.document.id))orphan={document:result.document,adapter,borrowed};
   if(!alive.current||closed.current||version!==generation.current)return;
   if(borrowed.has(result.document.id))throw new Error('Text appearance must return a new source version.');
   const pages=replacementPages(document,page,result);
   owner.current=orphan;orphan=null;setCandidate({result,page:pages.find(p=>p.id===page.id)!});
  }catch(e){if(alive.current&&!closed.current&&version===generation.current)setError(e instanceof Error?e.message:String(e));}
  finally{release(orphan);running.current=false;if(alive.current)setBusy(false);}
 };
 const apply=()=>{
  if(!ready||!candidate||submitted.current)return;
  submitted.current=true;const owned=owner.current;owner.current=null;
  try{if(!onApply(candidate.result))throw new Error('The document changed. Close this dialog and try again.');setApplied(true);}
  catch(e){submitted.current=false;if(alive.current&&!closed.current){owner.current=owned;setError(e instanceof Error?e.message:String(e));}else release(owned);}
 };
 const visible=pageVisibleRect(page);
 return <Modal title="Text field appearance" description="Preview font size, color and alignment, then apply." className="form-author-dialog" onClose={close}>
  <div className="form-author-grid"><div className="form-author-controls">
   <p>{field.label||field.name||'Selected text field'}</p>
   {!style&&!inspectionError&&<p role="status">Reading text settings…</p>}
   {style&&draft&&<>
    <p>Original font: <strong>{style.fontLabel}</strong></p>
    <label className="form-author-check"><input type="checkbox" checked={draft.autoFit} disabled={applied} onChange={e=>change({autoFit:e.target.checked,...(!e.target.checked&&Number(draft.size)===0?{size:'12'}:{})})}/>Auto fit</label>
    <label>Font size (pt)<input type="number" step="0.1" value={draft.size} disabled={applied||draft.autoFit} onChange={e=>change({size:e.target.value})}/></label>
    <label>Alignment<select value={draft.alignment} disabled={applied} onChange={e=>change({alignment:e.target.value as AppearanceDraft['alignment']})}><option value="left">Left</option><option value="center">Center</option><option value="right">Right</option></select></label>
    <label>Text color<select value={draft.colorMode} disabled={applied} onChange={e=>change({colorMode:e.target.value as AppearanceDraft['colorMode']})}><option value="original">Original color</option><option value="custom">Custom RGB</option></select></label>
    {draft.colorMode==='custom'?<label>RGB color<input type="text" value={draft.color} maxLength={7} disabled={applied} onChange={e=>change({color:e.target.value})} placeholder="#RRGGBB"/></label>:<p>Original {style.colorSpace.toUpperCase()} color is preserved.</p>}
    <button className="button" disabled={applied} onClick={()=>{invalidate();setDraft(appearanceDraft(style));}}>Reset appearance</button>
    <p>Check the preview for clipped text. Shared fields keep their values; this field gets the new appearance.</p>
   </>}
  </div><div className="form-author-preview"><div style={{aspectRatio:`${visible.width}/${visible.height}`,width:`min(100%, calc(var(--form-preview-height) * ${visible.width/visible.height}))`}}>{preview.url&&<img src={preview.url} alt={candidate?'Text appearance preview':'Page preview'}/>}</div>{preview.loading&&<p role="status">Rendering page…</p>}{preview.error&&<p role="alert">{preview.error}</p>}</div></div>
  {(inspectionError||error||admission)&&<p role="alert" className="error-note">{inspectionError||error||admission}</p>}
  {busy&&<p role="status">Preparing appearance…</p>}
  <div className="dialog-actions"><button className="button" onClick={close}>Cancel</button><button className="button" disabled={busy||applied||!!admission||!request} onClick={()=>void prepare()}>Preview appearance</button><button className="button primary" disabled={!ready} onClick={apply}>Apply appearance</button></div>
 </Modal>;
}
