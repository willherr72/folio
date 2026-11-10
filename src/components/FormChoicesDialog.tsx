import {useEffect,useLayoutEffect,useMemo,useRef,useState} from 'react';
import type {FolioAdapter} from '../editor/adapter';
import type {DocumentInfo,EditorDocument,FormField,PagePlan} from '../editor/types';
import {createChoiceDraft,choiceRequest,newChoiceRow,type ChoiceDraft,type ChoiceDraftSelection,type FormChoices} from '../editor/form-choices';
import {discardOpenedDocument} from '../editor/discard-opened-document';
import {Modal} from './Modal';
import './form-choices.css';
interface Props {adapter:FolioAdapter;document:EditorDocument;page:PagePlan;field:FormField;isBorrowed(id:string):boolean;onApply(candidate:DocumentInfo):Promise<void>;onClose():void}
interface Owner {info:DocumentInfo;adapter:FolioAdapter;released:boolean;adopted:boolean}
const message=(error:unknown)=>error instanceof Error?error.message:String(error);
const revoke=(url?:string)=>{if(url?.startsWith('blob:'))URL.revokeObjectURL(url);};

function useChoiceImage(adapter:FolioAdapter,sourceId:string|null,pageIndex:number,workspacePage?:PagePlan){
 const request=useMemo(()=>sourceId?{adapter,sourceId,pageIndex,workspacePage}:null,[adapter,sourceId,pageIndex,workspacePage]);
 const [state,setState]=useState<{request:typeof request;url?:string;error?:string}>({request:null});
 useEffect(()=>{
  if(!request)return;let cancelled=false,url:string|undefined;
  void Promise.resolve().then(()=>{
   if(request.workspacePage&&request.adapter.renderPagePlan)return request.adapter.renderPagePlan({...request.workspacePage,sourceId:request.sourceId,contentSourceId:undefined},900);
   return request.adapter.renderPage(request.sourceId,request.pageIndex,900);
  }).then(value=>{if(cancelled)revoke(value);else{url=value;setState({request,url:value});}}).catch(error=>{if(!cancelled)setState({request,error:message(error)});});
  return()=>{cancelled=true;revoke(url);};
 },[request]);
 return {url:state.request===request?state.url:undefined,error:state.request===request?state.error:undefined,loading:!!request&&state.request!==request};
}

function AnswerControl({title,value,draft,multiple,editable,disabled,onChange}:{title:string;value:ChoiceDraftSelection;draft:ChoiceDraft;multiple:boolean;editable:boolean;disabled:boolean;onChange(value:ChoiceDraftSelection):void}){
 return <fieldset disabled={disabled}><legend>{title}</legend>
  <label>{title} action<select value={value.kind} onChange={e=>{const kind=e.target.value;if(kind==='rows')onChange({kind,value:[]});else if(kind==='text')onChange({kind,value:''});else onChange({kind:kind as 'keep'|'clear'});}}>
   <option value="keep">Keep existing</option><option value="clear">Clear</option><option value="rows">Choose {multiple?'rows':'a row'}</option>{editable&&<option value="text">Free text</option>}
  </select></label>
  {value.kind==='rows'&&<label>{title} rows<select multiple={multiple} size={multiple?4:undefined} value={multiple?value.value:value.value[0]??''} onChange={e=>onChange({kind:'rows',value:Array.from(e.target.selectedOptions).map(o=>o.value).filter(Boolean)})}>
   {!multiple&&<option value="">No row selected</option>}{draft.rows.map((r,i)=><option key={r.id} value={r.id}>{i+1}. {r.display||'(empty label)'} — {r.exportValue||'(empty value)'}</option>)}
  </select></label>}
  {value.kind==='text'&&<label>{title} text<input value={value.value} onChange={e=>onChange({kind:'text',value:e.target.value})}/></label>}
 </fieldset>;
}

function candidateValid(info:DocumentInfo,document:EditorDocument,sourceId:string,settings:FormChoices):void {
 if(!Array.isArray(info.pages)||!info.pages.length||info.pages.some(p=>!p||!Number.isFinite(p.width)||!Number.isFinite(p.height)||p.width<=0||p.height<=0))throw new Error('The preview returned invalid page dimensions.');
 for(const p of document.pages.filter(p=>p.sourceId===sourceId)){
  const next=info.pages[p.pageIndex];if(!next||Math.abs(next.width-p.width)>0.01||Math.abs(next.height-p.height)>0.01)throw new Error('The preview changed or omitted a source page.');
 }
 if(settings.widgets.some(w=>!info.pages[w.pageIndex]))throw new Error('The preview omitted a linked field page.');
}

export function FormChoicesDialog({adapter,document,page,field,isBorrowed,onApply,onClose}:Props){
 const [settings,setSettings]=useState<FormChoices|null>(null),[draft,setDraft]=useState<ChoiceDraft|null>(null),[selected,setSelected]=useState('');
 const [error,setError]=useState<string|null>(null),[busy,setBusy]=useState(false),[applying,setApplying]=useState(false),[applied,setApplied]=useState(false);
 const [candidate,setCandidate]=useState<DocumentInfo|null>(null),[previewPage,setPreviewPage]=useState(page.pageIndex);
 const alive=useRef(false),closed=useRef(false),generation=useRef(0),pending=useRef<symbol|null>(null),submitting=useRef(false),owner=useRef<Owner|null>(null),claimed=useRef(new Set<string>());
 const borrowed=useRef(isBorrowed);borrowed.current=isBorrowed;
 const release=(owned:Owner|null)=>{
  if(!owned||owned.released||owned.adopted)return;owned.released=true;
  if(borrowed.current(owned.info.id))return;
  let attempted=false;
  void discardOpenedDocument(owned.info,id=>{attempted=true;return owned.adapter.closeDocument(id);}).catch(async()=>{
   // A malformed response can fail before the cleanup helper reaches close.
   if(!attempted)await owned.adapter.closeDocument(owned.info.id).catch(()=>{});
  });
 };
 const dispose=()=>{const previous=owner.current;owner.current=null;release(previous);};
 const invalidate=()=>{generation.current++;pending.current=null;dispose();setCandidate(null);setBusy(false);setError(null);};
 useLayoutEffect(()=>{alive.current=true;return()=>{alive.current=false;generation.current++;dispose();};},[]);
 useLayoutEffect(()=>{
  invalidate();closed.current=false;setSettings(null);setDraft(null);setApplied(false);setPreviewPage(page.pageIndex);
  const token=generation.current;
  void Promise.resolve().then(()=>{
   if(!adapter.inspectFormChoices||!adapter.setFormChoices)throw new Error('Editing choices requires the desktop PDF service.');
   return adapter.inspectFormChoices(page.sourceId,page.pageIndex,field.annotationIndex);
  }).then(info=>{
   if(!alive.current||closed.current||generation.current!==token)return;
   const next=createChoiceDraft(info);
   if(!info.widgets.some(w=>w.pageIndex===page.pageIndex&&w.annotationIndex===field.annotationIndex))throw new Error('The field returned a different source selection.');
   setSettings(info);setDraft(next);setSelected(next.rows[0]?.id??'');
  }).catch(cause=>{if(alive.current&&!closed.current&&generation.current===token)setError(message(cause));});
  return()=>{generation.current++;dispose();};
 },[adapter,document,page.sourceId,page.pageIndex,field.annotationIndex]);
 let request:ReturnType<typeof choiceRequest>=null,validation:string|null=null;
 if(settings&&draft)try{request=choiceRequest(settings,draft,page.pageIndex,field.annotationIndex);}catch(cause){validation=message(cause);}
 const workspacePage=document.pages.find(p=>p.sourceId===page.sourceId&&p.pageIndex===previewPage&&(previewPage!==page.pageIndex||p.id===page.id))??document.pages.find(p=>p.sourceId===page.sourceId&&p.pageIndex===previewPage);
 const originalImage=useChoiceImage(adapter,page.sourceId,previewPage,workspacePage),changedImage=useChoiceImage(adapter,candidate?.id??null,previewPage,workspacePage);
 const ready=!!candidate&&!!request&&!validation&&!busy&&!applying&&!applied&&!!changedImage.url&&!changedImage.loading&&!changedImage.error&&!!originalImage.url&&!originalImage.loading&&!originalImage.error;
 const change=(next:ChoiceDraft)=>{if(submitting.current||applied)return;invalidate();setDraft(next);};
 const close=()=>{if(submitting.current)return;closed.current=true;invalidate();onClose();};
 const prepare=async()=>{
  if(pending.current||submitting.current||closed.current||!settings||!request||validation||!adapter.setFormChoices)return;
  invalidate();const token=generation.current,job=Symbol();pending.current=job;setBusy(true);let orphan:Owner|null=null;
  try{
   const info=await adapter.setFormChoices(page.sourceId,request);
   if(!info||typeof info.id!=='string'||!info.id.trim()||info.id===page.sourceId||borrowed.current(info.id)||claimed.current.has(info.id))throw new Error('The preview must return a new source version.');
   claimed.current.add(info.id);orphan={info,adapter,released:false,adopted:false};
   if(!alive.current||closed.current||token!==generation.current)return;
   candidateValid(info,document,page.sourceId,settings);owner.current=orphan;orphan=null;setCandidate(info);
  }catch(cause){if(alive.current&&!closed.current&&token===generation.current)setError(message(cause));}
  finally{release(orphan);if(pending.current===job){pending.current=null;if(alive.current)setBusy(false);}}
 };
 const apply=async()=>{
  if(!ready||!candidate||submitting.current)return;
  if(borrowed.current(candidate.id)){invalidate();setError('The preview source is already in use. Make a new preview.');return;}
  const owned=owner.current;if(!owned)return;owner.current=null;submitting.current=true;setApplying(true);setError(null);const token=generation.current;
  try{await onApply(candidate);owned.adopted=true;if(alive.current&&!closed.current&&token===generation.current){setApplied(true);onClose();}}
  catch(cause){release(owned);if(alive.current&&!closed.current&&token===generation.current){setCandidate(null);setError(message(cause));}}
  finally{submitting.current=false;if(alive.current)setApplying(false);}
 };
 const row=draft?.rows.find(r=>r.id===selected),rowIndex=draft?.rows.findIndex(r=>r.id===selected)??-1;
 const ambiguousDefaults=useMemo(()=>{
  if(!settings||!draft)return [];
  const value=draft.defaults;
  const values=value.kind==='keep'?settings.model.defaults??[]:value.kind==='rows'?draft.rows.filter(r=>value.value.includes(r.id)).map(r=>r.exportValue):value.kind==='text'?[value.value]:[];
  const matches=new Map<string,string[]>();
  draft.rows.forEach((r,i)=>{const labels=matches.get(r.exportValue)??[];labels.push(`${i+1}. ${r.display||'(empty label)'}`);matches.set(r.exportValue,labels);});
  return [...new Set(values)].flatMap(value=>{const labels=matches.get(value)??[];return labels.length>1?[{value,labels}]:[];});
 },[settings,draft]);
 const pages=settings?[...new Set(settings.widgets.map(w=>w.pageIndex))].sort((a,b)=>a-b):[page.pageIndex];
 const summary=(which:'selection'|'defaults')=>{
  if(!settings||!draft)return '';
  const value=draft[which];if(value.kind==='clear')return 'None';if(value.kind==='text')return value.value||'(empty text)';
  if(value.kind==='rows')return value.value.map(id=>draft.rows.find(r=>r.id===id)?.exportValue??'(removed)').join(', ')||'None';
  if(which==='defaults')return settings.model.defaults?.join(', ')??'None';
  const current=settings.model.selection;if(current.kind==='none')return 'None';if(current.kind==='text')return current.value;
  return current.value.map(i=>draft.rows.find(r=>r.sourceIndex===i)?.exportValue??'(removed)').join(', ');
 };
 return <Modal title="Edit choices" description="Edit option labels and stored values, then check the native preview before applying." className="form-choices-dialog" onClose={close}>
  {!settings&&!error&&<p role="status">Reading choices…</p>}
  {settings&&draft&&<>
   <p><strong>{settings.name||'Unnamed field'}</strong> · {settings.widgets.length} linked {settings.widgets.length===1?'appearance':'appearances'}. Changes apply to all of them.</p>
   <div className="form-choices-grid"><div className="form-choices-controls">
    <label>Option to edit<select size={7} value={selected} disabled={applying||applied} onChange={e=>setSelected(e.target.value)}>{draft.rows.map((r,i)=><option key={r.id} value={r.id}>{i+1}. {r.display||'(empty label)'}</option>)}</select></label>
    {row&&<><label>Display label<input value={row.display} disabled={applying||applied} onChange={e=>change({...draft,rows:draft.rows.map(r=>r.id===row.id?{...r,display:e.target.value}:r)})}/></label>
     <label>Stored value<input value={row.exportValue} disabled={applying||applied} onChange={e=>change({...draft,rows:draft.rows.map(r=>r.id===row.id?{...r,exportValue:e.target.value}:r)})}/></label></>}
    <p className="muted">The label appears in the field. The stored value is the answer used by the form.</p>
    <div className="form-choices-row-actions">
     <button className="button" disabled={applying||applied||draft.rows.length>=2000} onClick={()=>{const next=newChoiceRow();change({...draft,rows:[...draft.rows,next]});setSelected(next.id);}}>Add option</button>
     <button className="button" disabled={applying||applied||!row} onClick={()=>{const rows=draft.rows.filter(r=>r.id!==selected);change({...draft,rows});setSelected(rows[Math.min(rowIndex,rows.length-1)]?.id??'');}}>Remove option</button>
     {[-1,1].map(offset=><button key={offset} className="button" disabled={applying||applied||!row||rowIndex+offset<0||rowIndex+offset>=draft.rows.length} onClick={()=>{const rows=[...draft.rows];[rows[rowIndex],rows[rowIndex+offset]]=[rows[rowIndex+offset],rows[rowIndex]];change({...draft,rows});}}>{offset<0?'Move up':'Move down'}</button>)}
    </div>
    {settings.model.sorted&&<label className="form-choices-check"><input type="checkbox" checked={draft.manualOrder} disabled={applying||applied} onChange={e=>change({...draft,manualOrder:e.target.checked})}/>Use manual order instead of the PDF’s automatic sorting</label>}
    <AnswerControl title="Current answer" value={draft.selection} draft={draft} multiple={settings.model.multiple} editable={settings.model.editable} disabled={applying||applied} onChange={selection=>change({...draft,selection})}/>
    <p className="form-choices-summary">Current stored value after edit: {summary('selection')}</p>
    <AnswerControl title="Default answer" value={draft.defaults} draft={draft} multiple={settings.model.multiple} editable={settings.model.editable} disabled={applying||applied} onChange={defaults=>change({...draft,defaults})}/>
    <p className="form-choices-summary">Default stored value after edit: {summary('defaults')}</p>
    {!!ambiguousDefaults.length&&<div role="note" aria-label="Default answer ambiguity">
     <p>These default values match multiple options. A PDF stores the default value without a unique row. Resetting this field may select a different matching row.</p>
     <ul>{ambiguousDefaults.map(({value,labels})=><li key={value}>Stored value {value?`“${value}”`:'(empty value)'}: {labels.join('; ')}</li>)}</ul>
    </div>}
    <button className="button" disabled={applying||applied} onClick={()=>{const next=createChoiceDraft(settings);change(next);setSelected(next.rows[0]?.id??'');}}>Reset choices</button>
   </div><div className="form-choices-previews">
    <label>Preview page<select value={previewPage} onChange={e=>setPreviewPage(Number(e.target.value))}>{pages.map(i=><option key={i} value={i}>Source page {i+1}</option>)}</select></label>
    <figure><figcaption>Original</figcaption>{originalImage.url&&<img src={originalImage.url} alt="Original choices preview"/>}{originalImage.loading&&<p role="status">Rendering original…</p>}{originalImage.error&&<p role="alert">{originalImage.error}</p>}</figure>
    <figure><figcaption>After edit</figcaption>{candidate&&changedImage.url&&<img src={changedImage.url} alt="Edited choices preview"/>}{changedImage.loading&&<p role="status">Rendering edited page…</p>}{changedImage.error&&<p role="alert">{changedImage.error}</p>}{!candidate&&<p>Change an option or answer, then preview the result.</p>}</figure>
   </div></div>
  </>}
  {(error||validation)&&<p role="alert" className="error-note">{error||validation}</p>}
  {busy&&<p role="status">Preparing choices…</p>}{applying&&<p role="status">Applying choices…</p>}
  <div className="dialog-actions"><button className="button" disabled={applying} onClick={close}>Cancel</button><button className="button" disabled={busy||applying||applied||!request||!!validation} onClick={()=>void prepare()}>Preview choices</button><button className="button primary" disabled={!ready} onClick={()=>void apply()}>Apply choices</button></div>
 </Modal>;
}
