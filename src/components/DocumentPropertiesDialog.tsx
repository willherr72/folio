import {useLayoutEffect,useRef,useState} from 'react';
import {Modal} from './Modal';
import {copyDocumentProperties,metadataEqual,metadataFromDraft,propertiesDraft,propertyKeys,type PropertyDraft,type DocumentMetadataEdit,type DocumentProperties} from '../editor/document-properties';
import './form-properties.css';
interface Props{adapter:{inspectDocumentProperties?(sourceId:string):Promise<DocumentProperties>};sourceId:string;metadata?:DocumentMetadataEdit;canEdit:boolean;onApply(edit:DocumentMetadataEdit|undefined):void;onClose():void}
export function DocumentPropertiesDialog({adapter,sourceId,metadata,canEdit,onApply,onClose}:Props){
 const [source,setSource]=useState<DocumentProperties|null>(null),[draft,setDraft]=useState<PropertyDraft|null>(null),[failure,setFailure]=useState<string|null>(null),[remove,setRemove]=useState(metadata?.mode==='remove'),[reset,setReset]=useState(false),[applied,setApplied]=useState(false);
 const generation=useRef(0);
 useLayoutEffect(()=>{
  const token=++generation.current;setSource(null);setDraft(null);setFailure(null);setReset(false);setRemove(metadata?.mode==='remove');setApplied(false);
  void Promise.resolve().then(()=>{if(!adapter.inspectDocumentProperties)throw Error('Document properties require the desktop PDF service.');return adapter.inspectDocumentProperties(sourceId);}).then(value=>{
   const owned=copyDocumentProperties(value);if(token!==generation.current)return;setSource(owned);setDraft(propertiesDraft(owned,metadata));
  }).catch(error=>{if(token===generation.current)setFailure(error instanceof Error?error.message:String(error));});
  return()=>{generation.current++;};
 },[adapter,sourceId,metadata]);
 const cleared=!reset&&(metadata?.mode==='remove'||(metadata?.mode==='update'&&metadata.clearExisting));
 let next:DocumentMetadataEdit|undefined,validation:string|null=null;
 if(source&&draft)try{next=metadataFromDraft(source,reset?undefined:metadata,draft,remove);}catch(error){validation=error instanceof Error?error.message:String(error);}
 const unsupported=!!source?.error&&!cleared&&!remove;
 const enabled=!!source&&!!draft&&canEdit&&!applied&&!validation&&(!unsupported||(reset&&next===undefined))&&!metadataEqual(metadata,next);
 const change=(key:typeof propertyKeys[number],value:string)=>{setDraft(current=>current?{...current,[key]:value}:current);setFailure(null);};
 const apply=()=>{if(!enabled)return;try{onApply(next);setApplied(true);}catch(error){setFailure(error instanceof Error?error.message:String(error));}};
 return <Modal title="Document properties" description="Inspect document information and edit properties saved with this PDF." className="form-properties-dialog document-properties-dialog" onClose={onClose}>
  {!source&&!failure&&<p role="status">Reading document properties…</p>}
  {source&&draft&&<div className="form-properties-controls">
   <dl><dt>Source PDF information</dt><dd>{source.hasInfo?'Present':'Not present'}</dd><dt>Source XMP metadata</dt><dd>{source.hasXmp?'Present':'Not present'}</dd></dl>
   <p className="muted">Presence describes the source PDF before draft changes.</p>
   {!canEdit&&<p>This PDF's permissions allow inspection but do not allow changing document properties.</p>}
   {source.conflicts.length>0&&!cleared&&<p>PDF and XMP values differ for {source.conflicts.join(', ')}. Editing a field updates both; untouched fields keep their existing values.</p>}
   {source.error&&!cleared&&<p role="alert">{source.error}</p>}
   <label className="form-properties-check"><input type="checkbox" checked={remove} disabled={!canEdit||applied} onChange={event=>{setRemove(event.target.checked);setFailure(null);}}/>Remove document properties</label>
   {remove&&<p>Removes document Info and XMP, including custom properties. Page/object metadata, attachments, comments and visible text remain. This change can be undone before saving.</p>}
   {cleared&&!remove&&<p>New properties will be added to the cleared document information. Original custom properties will remain removed.</p>}
   {propertyKeys.map(key=><label key={key}>{({title:'Title',author:'Author',subject:'Subject',keywords:'Keywords'})[key]}{key==='subject'?<textarea rows={3} value={draft[key]} disabled={!canEdit||applied||remove||unsupported} onChange={event=>change(key,event.target.value)}/>:<input value={draft[key]} disabled={!canEdit||applied||remove||unsupported} onChange={event=>change(key,event.target.value)}/>}</label>)}
   {!remove&&<p className="muted">Title and Subject update the default language. Existing translations are retained.</p>}
   {source.multipleAuthors&&!cleared&&!remove&&<p>The source lists multiple authors. Editing Author replaces that list with the literal text entered here.</p>}
   <dl><dt>Creator</dt><dd>{cleared||remove?'—':source.creator||'—'}</dd><dt>Producer</dt><dd>{cleared||remove?'—':source.producer||'—'}</dd><dt>Created</dt><dd>{cleared||remove?'—':source.creationDate||'—'}</dd><dt>Modified</dt><dd>{cleared||remove?'—':source.modificationDate||'—'}</dd></dl>
   <p className="muted">Changes apply to this document draft and are written when you save a PDF copy.</p>
   <button className="button" disabled={!canEdit||applied} onClick={()=>{setReset(true);setRemove(false);setDraft(propertiesDraft(source));setFailure(null);}}>Reset to source properties</button>
  </div>}
  {(failure||validation)&&<p role="alert" className="error-note">{failure||validation}</p>}
  <div className="dialog-actions"><button className="button" onClick={onClose}>Cancel</button><button className="button primary" disabled={!enabled} onClick={apply}>Apply properties</button></div>
 </Modal>;
}
