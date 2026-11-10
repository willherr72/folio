import {useLayoutEffect,useRef,useState} from 'react';
import {Modal} from './Modal';
import type {FolioAdapter} from '../editor/adapter';
import type {FormField,PagePlan} from '../editor/types';
import {formPropertiesRequest,propertiesDraft,type FormProperties,type FormPropertyDraft,type SetFormPropertiesRequest} from '../editor/form-properties';
import './form-properties.css';
interface Props {adapter:FolioAdapter;page:PagePlan;field:FormField;onApply(request:SetFormPropertiesRequest):Promise<void>;onClose():void}
export function FormPropertiesDialog({adapter,page,field,onApply,onClose}:Props){
 const [original,setOriginal]=useState<FormProperties|null>(null),[draft,setDraft]=useState<FormPropertyDraft|null>(null),[error,setError]=useState<string|null>(null),[busy,setBusy]=useState(false),[applied,setApplied]=useState(false);
 const generation=useRef(0),pending=useRef(false),alive=useRef(false);
 useLayoutEffect(()=>{alive.current=true;return()=>{alive.current=false;};},[]);
 useLayoutEffect(()=>{const token=++generation.current;setOriginal(null);setDraft(null);setError(null);setApplied(false);
  void Promise.resolve().then(()=>{if(!adapter.inspectFormProperties)throw new Error('Field properties require the desktop PDF service.');return adapter.inspectFormProperties(page.sourceId,page.pageIndex,field.annotationIndex);}).then(p=>{if(token===generation.current){setOriginal(p);setDraft(propertiesDraft(p));}}).catch(e=>{if(token===generation.current)setError(String(e instanceof Error?e.message:e));});
  return()=>{generation.current++;};
 },[adapter,page.sourceId,page.pageIndex,field.annotationIndex]);
 let request:SetFormPropertiesRequest|null=null,validation:string|null=null;
 if(original&&draft)try{request=formPropertiesRequest(original,draft,page.pageIndex,field.annotationIndex);}catch(e){validation=String(e instanceof Error?e.message:e);}
 const change=(next:Partial<FormPropertyDraft>)=>{if(!pending.current&&!applied){setDraft(previous=>previous?{...previous,...next}:previous);setError(null);}};
 const close=()=>{if(!pending.current)onClose();};
 const apply=async()=>{if(pending.current||!request||applied)return;const token=generation.current;pending.current=true;setBusy(true);setError(null);
  try{await onApply(request);if(token===generation.current)setApplied(true);}
  catch(e){if(token===generation.current)setError(String(e instanceof Error?e.message:e));}
  finally{pending.current=false;if(alive.current)setBusy(false);}
 };
 return <Modal title="Field properties" description="Change the field definition while keeping its answer and appearance." className="form-properties-dialog" onClose={close}>
  {!original&&!error&&<p role="status">Reading field properties…</p>}
  {original&&draft&&<div className="form-properties-controls">
   <p>Full field name: <strong>{original.fullName||'Unnamed field'}</strong></p>
   <p>{original.linkedWidgets>1?`These settings apply to all ${original.linkedWidgets} linked appearances of this field.`:'These settings apply to this field.'}</p>
   <label>Field name<input value={draft.name} disabled={busy||applied||!!original.renameReason} onChange={e=>change({name:e.target.value})}/></label>
   <p className="muted">The field name identifies the answer; it does not change printed page text.</p>
   {original.renameReason&&<p>{original.renameReason}</p>}
   <label>Tooltip<textarea rows={3} value={draft.label} disabled={busy||applied} onChange={e=>change({label:e.target.value})}/></label>
   <label className="form-properties-check"><input type="checkbox" checked={draft.readOnly} disabled={busy||applied} onChange={e=>change({readOnly:e.target.checked})}/>Read only</label>
   <label className="form-properties-check"><input type="checkbox" checked={draft.required} disabled={busy||applied} onChange={e=>change({required:e.target.checked})}/>Required</label>
   <p className="muted">Required marks the field for form submission. Folio does not run submission or validation scripts.</p>
   <button className="button" disabled={busy||applied} onClick={()=>{setDraft(propertiesDraft(original));setError(null);}}>Reset properties</button>
  </div>}
  {(error||validation)&&<p role="alert" className="error-note">{error||validation}</p>}
  {busy&&<p role="status">Applying field properties…</p>}
  <div className="dialog-actions"><button className="button" disabled={busy} onClick={close}>Cancel</button><button className="button primary" disabled={busy||applied||!request||!!validation} onClick={()=>void apply()}>Apply properties</button></div>
 </Modal>;
}
