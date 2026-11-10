import {useLayoutEffect,useRef,useState} from 'react';
import {Modal} from './Modal';
import type {FolioAdapter} from '../editor/adapter';
import type {FormField,PagePlan} from '../editor/types';
import type {FormRemovalInfo,RemoveFormFieldRequest} from '../editor/form-removal';
import './form-properties.css';

interface Props {adapter:FolioAdapter;page:PagePlan;field:FormField;onApply(request:RemoveFormFieldRequest):Promise<void>;onClose():void}
export function FormRemovalDialog({adapter,page,field,onApply,onClose}:Props) {
 const [info,setInfo]=useState<FormRemovalInfo|null>(null),[error,setError]=useState<string|null>(null),[busy,setBusy]=useState(false),[applied,setApplied]=useState(false);
 const generation=useRef(0),pending=useRef(false),alive=useRef(false);
 useLayoutEffect(()=>{alive.current=true;return()=>{alive.current=false;};},[]);
 useLayoutEffect(()=>{
  const token=++generation.current;setInfo(null);setError(null);setApplied(false);
  void Promise.resolve().then(()=>{
   if(!adapter.inspectFormRemoval)throw new Error('Field removal requires the desktop PDF service.');
   return adapter.inspectFormRemoval(page.sourceId,page.pageIndex,field.annotationIndex);
  }).then(value=>{if(token===generation.current)setInfo(value);}).catch(e=>{if(token===generation.current)setError(e instanceof Error?e.message:String(e));});
  return()=>{generation.current++;};
 },[adapter,page.sourceId,page.pageIndex,field.annotationIndex]);
 const close=()=>{if(!pending.current)onClose();};
 const apply=async()=>{
  if(pending.current||!info||info.removalReason||applied)return;
  const token=generation.current;pending.current=true;setBusy(true);setError(null);
  try{await onApply({pageIndex:page.pageIndex,annotationIndex:field.annotationIndex,expectedKey:info.expectedKey});if(token===generation.current)setApplied(true);}
  catch(e){if(token===generation.current)setError(e instanceof Error?e.message:String(e));}
  finally{pending.current=false;if(alive.current)setBusy(false);}
 };
 return <Modal title="Remove form field" description="Remove this field and its linked appearances from the active document." className="form-properties-dialog" onClose={close}>
  {!info&&!error&&<p role="status">Checking field removal…</p>}
  {info&&<div className="form-properties-controls">
   <p>Field: <strong>{info.fullName||'Unnamed field'}</strong></p>
   <p>{info.linkedWidgets>1?`Remove all ${info.linkedWidgets} linked appearances of this field.`:'Remove this field and its appearance.'}</p>
   {field.kind==='radio'&&<p>This removes the entire radio group, including every option.</p>}
   {!!info.pageIndices.length&&<p>Source pages: {info.pageIndices.map(index=>index+1).join(', ')}.</p>}
   <p>Every copy of these source pages in this document is affected. Linked fields on source pages outside the current page list are also removed.</p>
   <p>Printed labels and page artwork stay in place. You can restore the field with Undo.</p>
   {info.removalReason&&<p role="alert" className="error-note">{info.removalReason}</p>}
  </div>}
  {error&&<p role="alert" className="error-note">{error}</p>}
  {busy&&<p role="status">Removing field…</p>}
  <div className="dialog-actions"><button className="button" disabled={busy} onClick={close}>Cancel</button><button className="button" disabled={busy||applied||!info||!!info.removalReason} onClick={()=>void apply()}>Remove field</button></div>
 </Modal>;
}
