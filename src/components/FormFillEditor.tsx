import {useEffect,useLayoutEffect,useRef,useState} from "react";
import {FormPagePreview} from "./FormPagePreview";
import {FormFieldValueEditor} from './FormFieldValueEditor';
import {FormFieldPopover} from './FormFieldPopover';
import {usePropertiesReveal} from './PropertiesPanel';
import {matchesFormAnchor,type FormCanvasAnchor} from '../editor/form-canvas-entry';
import type {FolioAdapter} from "../editor/adapter";
import type {FormField,FormFieldValue,PagePlan} from "../editor/types";
import "./form-fill.css";
const initialDraft=(field:FormField):FormFieldValue=>field.editableChoice&&field.selectedIndices.length?{kind:"choice",indices:field.selectedIndices}:field.value;

export interface FormFillEditorProps {
 page:PagePlan;adapter:FolioAdapter;pageNumber:number;
 onApply(field:FormField,value:FormFieldValue):Promise<void>;onClose():void;
 preview?:boolean;selectedAnnotationIndex?:number;selectionRevision?:number;
 onDraftChange?(dirty:boolean):void;onSelectionChange?(field:FormField|null):void;
 onLocate?():void;onOverview?():void;onBusyChange?(busy:boolean):void;
 onPosition?(field:FormField):void;
 onTextAppearance?(field:FormField):void;
 onFrameAppearance?(field:FormField):void;
 onProperties?(field:FormField):void;
 onRemove?(field:FormField):void;
 onChoices?(field:FormField):void;
 canvasAnchor?:FormCanvasAnchor|null;
}
export function FormFillEditor({page,adapter,pageNumber,onApply,onClose,preview=true,selectedAnnotationIndex,selectionRevision=0,onDraftChange,onSelectionChange,onLocate,onOverview,onBusyChange,onPosition,onTextAppearance,onFrameAppearance,onProperties,onRemove,onChoices,canvasAnchor=null}:FormFillEditorProps){
 const [fields,setFields]=useState<FormField[]>([]),[selected,setSelected]=useState(0),[draft,setDraft]=useState<FormFieldValue|null>(null);
 const [loading,setLoading]=useState(true),[busy,setBusy]=useState(false),[error,setError]=useState<string|null>(null),[notice,setNotice]=useState<string|null>(null);
 const [fieldsOwner,setFieldsOwner]=useState<{adapter:FolioAdapter;sourceId:string;pageIndex:number}|null>(null);
 const valueControls=useRef<HTMLDivElement>(null),focusNext=useRef(false);
 const inlineControls=useRef<HTMLElement|null>(null),revealProperties=usePropertiesReveal();
 const [focusRequest,setFocusRequest]=useState({revision:0,properties:false});
 const [dismissedRevision,setDismissedRevision]=useState(-1),handledInlineFocus=useRef<string|null>(null);
 const waiting=loading||fieldsOwner?.adapter!==adapter||fieldsOwner.sourceId!==page.sourceId||fieldsOwner.pageIndex!==page.pageIndex;
 const focusGroup=(group:HTMLElement,inline=false)=>(group.querySelector<HTMLElement>('input:not(:disabled),select:not(:disabled),textarea:not(:disabled)')??group).focus({preventScroll:inline});
 const requestFocus=(properties:boolean)=>{if(!valueControls.current)return;revealProperties();setFocusRequest(previous=>({revision:previous.revision+1,properties}));};
 const focusValue=()=>{const group=inlineControls.current;if(group?.isConnected)focusGroup(group,true);else requestFocus(false);};
 const focusProperties=()=>requestFocus(true);
 // Reveal commits before focusing: a collapsed panel cannot receive native focus.
 // Recheck the card after that commit so a newly mounted card owns selection focus.
 useLayoutEffect(()=>{if(!focusRequest.revision)return;const inline=!focusRequest.properties&&inlineControls.current?.isConnected;const group=inline?inlineControls.current:valueControls.current;if(group)focusGroup(group,!!inline);},[focusRequest]);
 useLayoutEffect(()=>{if(focusNext.current&&!waiting){focusNext.current=false;focusValue();}},[selected,draft,waiting]);
 const pending=useRef(false),alive=useRef(true),selection=useRef(selectedAnnotationIndex??0);
 const requestedSelection=useRef(selectedAnnotationIndex);requestedSelection.current=selectedAnnotationIndex;
 const callbacks=useRef({onDraftChange,onSelectionChange});callbacks.current={onDraftChange,onSelectionChange};
 useEffect(()=>{alive.current=true;return()=>{alive.current=false;};},[]);
 useEffect(()=>{
  let cancelled=false;setLoading(true);setError(null);setNotice(null);setFields([]);setDraft(null);focusNext.current=false;callbacks.current.onSelectionChange?.(null);
  setFieldsOwner({adapter,sourceId:page.sourceId,pageIndex:page.pageIndex});
  void adapter.listFormFields!(page.sourceId,page.pageIndex).then(next=>{
   if(cancelled)return;setFields(next);const requested=requestedSelection.current??selection.current;
   const index=next.some(f=>f.annotationIndex===requested)?next.findIndex(f=>f.annotationIndex===requested):0;
   if(next[index])selection.current=next[index].annotationIndex;
   setSelected(index);setDraft(next[index]?initialDraft(next[index]):null);if(!preview)focusNext.current=true;
   callbacks.current.onSelectionChange?.(next[index]??null);
  }).catch(value=>{if(!cancelled)setError(String(value instanceof Error?value.message:value));}).finally(()=>{if(!cancelled)setLoading(false);});
  return()=>{cancelled=true;};
 },[adapter,page.sourceId,page.pageIndex]);
 const field=fields[selected];
 const dirty=!!field&&!!draft&&JSON.stringify(initialDraft(field))!==JSON.stringify(draft);
 const selectField=(index:number,focus=false)=>{
  if(waiting||pending.current||busy||!fields[index]||(dirty&&index!==selected))return;
  if(focus&&index===selected){focusValue();return;}
  selection.current=fields[index].annotationIndex;focusNext.current=focus;
  setSelected(index);setDraft(initialDraft(fields[index]));setError(null);setNotice(null);
  callbacks.current.onSelectionChange?.(fields[index]);
 };
 useLayoutEffect(()=>{callbacks.current.onDraftChange?.(!waiting&&dirty);},[waiting,dirty]);
 useLayoutEffect(()=>{
  if(waiting||selectedAnnotationIndex===undefined)return;
  const index=fields.findIndex(field=>field.annotationIndex===selectedAnnotationIndex);
  if(index>=0)selectField(index,true);
 },[selectedAnnotationIndex,selectionRevision,waiting,fields]);
 const close=()=>{if(!pending.current)onClose();};
 const change=(value:FormFieldValue)=>{setDraft(value);setError(null);setNotice(null);};
 const apply=async()=>{
  if(pending.current||waiting||!field||!draft||!dirty||field.reason)return;
  pending.current=true;onBusyChange?.(true);setBusy(true);setError(null);setNotice(null);
  try {await onApply(field,draft);if(alive.current)setNotice("Field applied. Shared fields update together. Save a copy when finished.");}
  catch(value){if(alive.current)setError(value instanceof Error?value.message:String(value));}
  finally {pending.current=false;if(alive.current){setBusy(false);onBusyChange?.(false);}}
 };
 const disabled=busy||waiting||!!field?.reason;
 const reset=()=>{if(!pending.current&&field){setDraft(initialDraft(field));setError(null);setNotice(null);}};
 const inline=!preview&&!waiting&&field&&draft&&selectionRevision>0&&dismissedRevision!==selectionRevision&&matchesFormAnchor(canvasAnchor,adapter,page,field);
 return <section className={`form-fill-editor${preview?"":" canvas-form-editor"}`} aria-label="Form editor">
  {!preview&&<><h3>Fill forms · page {pageNumber}</h3><p>Choose an outlined field on the document. Apply each change before saving.</p><p>Calculations and validation scripts are not run.</p></>}
  <div className="form-fill-body">
   {preview&&<FormPagePreview adapter={adapter} page={page} fields={fields} selected={selected} waiting={waiting} busy={busy} dirty={dirty} onSelect={index=>selectField(index,true)}/>}
   <div className="form-fill-controls">
    {waiting?<p role="status">Loading fields…</p>:fields.length===0?(!error&&<p>No interactive form fields on this page. Use Add text for a flat form.</p>):<>
     <p>Click an outlined field on the page, or choose one from the list.</p>
     <label>Field<select value={selected} disabled={busy||dirty} onChange={event=>selectField(Number(event.target.value))}>{fields.map((f,i)=><option key={f.annotationIndex} value={i}>{f.label||f.name||"Unnamed field"}{f.kind==="radio"?` — ${f.options[0]||`Option ${i+1}`}`:""}{f.reason?" (unavailable)":""}</option>)}</select></label>
     <div className="form-value-controls" ref={valueControls} tabIndex={-1}>
     {draft&&<FormFieldValueEditor field={field} draft={draft} disabled={disabled} onChange={change}/>}
     </div>
     {dirty&&<p>Apply or reset this draft before choosing another field.</p>}
     <div className="form-fill-buttons"><button className="button" disabled={busy||!dirty} onClick={reset}>Reset draft</button><button className="button primary" disabled={disabled||!dirty} onClick={()=>void apply()}>Apply field</button></div>
     {onPosition&&<button className="button" disabled={busy||waiting||dirty||!field.bounds||field.visible===false||field.noRotate||!['text','checkbox','radio','combo','list'].includes(field.kind)} onClick={()=>onPosition(field)}>Position and size…</button>}
     {onProperties&&<button className="button" disabled={busy||waiting||dirty||!['text','checkbox','radio','combo','list'].includes(field.kind)} onClick={()=>onProperties(field)}>Field properties…</button>}
     {onRemove&&<button className="button" disabled={busy||waiting||dirty||!['text','checkbox','radio','combo','list'].includes(field.kind)} onClick={()=>onRemove(field)}>Remove field…</button>}
     {onChoices&&['combo','list'].includes(field.kind)&&<button className="button" disabled={busy||waiting||dirty} onClick={()=>onChoices(field)}>Edit choices…</button>}
     {onTextAppearance&&field.kind==='text'&&<button className="button" disabled={busy||waiting||dirty||!field.bounds||field.visible===false||field.noRotate||field.password} onClick={()=>onTextAppearance(field)}>Text appearance…</button>}
     {onFrameAppearance&&['text','checkbox','radio','combo','list'].includes(field.kind)&&<button className="button" disabled={busy||waiting||dirty||!field.bounds||field.visible===false||field.noRotate} onClick={()=>onFrameAppearance(field)}>Border and background…</button>}
    </>}
    {error&&<p className="error-note" role="alert">{error}</p>}
    {busy&&<p role="status">Updating field and appearances…</p>}
    {notice&&<p role="status">{notice}</p>}
   </div>
  </div>
  {!preview&&<div className="form-fill-buttons">{onLocate&&<button className="button" disabled={busy||waiting||!field} onClick={onLocate}>Show selected field</button>}{onOverview&&<button className="button" disabled={busy||waiting||dirty} onClick={onOverview}>Full-page form overview</button>}</div>}
  {inline&&<FormFieldPopover anchor={canvasAnchor!} field={field} draft={draft!} disabled={disabled} busy={busy} dirty={dirty} error={error} focusRevision={selectionRevision} handledFocus={handledInlineFocus} entryRef={inlineControls} onFocused={()=>{focusNext.current=false;}} onFocusFallback={focusProperties} onChange={change} onApply={()=>{handledInlineFocus.current=null;void apply();}} onReset={()=>{handledInlineFocus.current=null;reset();}} onDismiss={()=>{if(!pending.current){setDismissedRevision(selectionRevision);focusProperties();}}}/>}
  <div className="dialog-actions"><button className="button" disabled={busy} onClick={close}>{dirty?"Discard draft and close":"Done"}</button></div>
 </section>;
}
