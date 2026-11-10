import {useEffect,useMemo,useRef,useState} from "react";
import {Modal} from "./Modal";
import type {FolioAdapter} from "../editor/adapter";
import type {AnnotationRect,DocumentInfo,EditorDocument,PageContentReplacement,PagePlan} from "../editor/types";
import {fieldSourceBounds,formAuthorSelection,initialFieldBounds,validateFieldDraft,type CreateFormFieldRequest,type NewFormField} from "../editor/form-authoring";
import {replacementPages} from "../editor/page-replacement";
import {discardOpenedDocument} from "../editor/discard-opened-document";
import {pageVisibleRect} from "../editor/geometry";
import {useComposedPage} from "../editor/use-composed-page";
import "./form-author.css";

interface Props {adapter:FolioAdapter;document:EditorDocument;page:PagePlan;sourceIds:string[];onApply(result:PageContentReplacement):boolean;onCancel():void}
const strings=(r:AnnotationRect)=>({x:String(r.x),y:String(r.y),width:String(r.width),height:String(r.height)});
export function FormAuthorDialog({adapter,document,page,sourceIds,onApply,onCancel}:Props) {
 const borrowed=useMemo(()=>new Set(sourceIds),[sourceIds]);
 const [name,setName]=useState("New field"),[label,setLabel]=useState(""),[required,setRequired]=useState(false);
 const [kind,setKind]=useState<NewFormField["kind"]>("text"),[multiline,setMultiline]=useState(false),[multiSelect,setMultiSelect]=useState(false),[choices,setChoices]=useState("One\nTwo"),[fontSize,setFontSize]=useState("12");
 const [area,setArea]=useState(()=>strings(initialFieldBounds(page)));
 const [candidate,setCandidate]=useState<{result:PageContentReplacement;page:PagePlan}|null>(null),owner=useRef<DocumentInfo|null>(null);
 const [busy,setBusy]=useState(false),running=useRef(false),[error,setError]=useState<string|null>(null),[applied,setApplied]=useState(false);
 const alive=useRef(false),closed=useRef(false),generation=useRef(0),submitted=useRef(false);
 const drag=useRef<{id:number;x:number;y:number;area:AnnotationRect;resize:boolean}|null>(null);
 const release=(info:DocumentInfo|null)=>{if(info?.id&&!borrowed.has(info.id))void discardOpenedDocument(info,id=>adapter.closeDocument(id)).catch(()=>{});};
 const dispose=()=>{const info=owner.current;owner.current=null;release(info);};
 useEffect(()=>{alive.current=true;return()=>{alive.current=false;generation.current++;dispose();};},[]);
 const rect={x:area.x.trim()?Number(area.x):NaN,y:area.y.trim()?Number(area.y):NaN,width:area.width.trim()?Number(area.width):NaN,height:area.height.trim()?Number(area.height):NaN};
 const field:NewFormField=kind==="text"?{kind,multiline}:kind==="checkbox"?{kind}:(kind==="dropdown"||kind==="radio")?{kind,options:choices.replace(/\r\n?/g,"\n").split("\n")}:{kind,options:choices.replace(/\r\n?/g,"\n").split("\n"),multiSelect};
 const request:CreateFormFieldRequest={name,label,required,fontSize:fontSize.trim()?Number(fontSize):NaN,field,bounds:fieldSourceBounds(page,rect),rotation:page.rotation};
 let admission:string|null=null;
 try{formAuthorSelection(document,page);if(!adapter.createFormField||!adapter.renderPagePlan)admission="Creating fields requires the desktop PDF service.";}catch(e){admission=e instanceof Error?e.message:String(e);}
 const invalid=rect.width<4||rect.height<4?"Fields must be at least 4 pt wide and tall.":validateFieldDraft(page,request);
 const preview=useComposedPage(adapter,candidate?.page??page,1000,true,true);
 const ready=!!candidate&&!!preview.url&&!preview.loading&&!preview.error&&!busy&&!applied&&!admission&&!invalid;
 const invalidate=()=>{generation.current++;dispose();setCandidate(null);setError(null);};
 const close=()=>{closed.current=true;invalidate();onCancel();};
 const prepare=async()=>{
  if(running.current||submitted.current||closed.current||invalid||admission||!adapter.createFormField)return;
  invalidate();const version=generation.current;running.current=true;setBusy(true);let orphan:DocumentInfo|null=null;
  try {
   const selection=formAuthorSelection(document,page);
   const result=await adapter.createFormField(selection.sourceId,selection.occurrences,selection.selectedOccurrence,request);
   if(!borrowed.has(result.document.id))orphan=result.document;
   if(!alive.current||closed.current||version!==generation.current)return;
   if(borrowed.has(result.document.id))throw new Error("Field creation must return a new source version.");
   const pages=replacementPages(document,page,result);
   owner.current=result.document;orphan=null;setCandidate({result,page:pages.find(p=>p.id===page.id)!});
  }catch(e){if(alive.current&&!closed.current&&version===generation.current)setError(e instanceof Error?e.message:String(e));}
  finally{release(orphan);running.current=false;if(alive.current)setBusy(false);}
 };
 const apply=()=>{
  if(!ready||!candidate||submitted.current)return;
  submitted.current=true;const owned=owner.current;owner.current=null;
  try{if(!onApply(candidate.result))throw new Error("The document changed. Close this dialog and try again.");setApplied(true);}
  catch(e){submitted.current=false;if(alive.current&&!closed.current){owner.current=owned;setError(e instanceof Error?e.message:String(e));}else release(owned);}
 };
 const visible=pageVisibleRect(page);
 const bounded=(r:AnnotationRect):AnnotationRect=>{
  const width=Math.max(4,Math.min(r.width,visible.width)),height=Math.max(4,Math.min(r.height,visible.height));
  return {x:Math.max(visible.x,Math.min(r.x,visible.x+visible.width-width)),y:Math.max(visible.y,Math.min(r.y,visible.y+visible.height-height)),width,height};
 };
 const move=(r:AnnotationRect)=>{invalidate();setArea(strings(bounded(r)));};
 const keyboard=(event:React.KeyboardEvent,resize:boolean)=>{
  const dx=event.key==="ArrowLeft"?-1:event.key==="ArrowRight"?1:0,dy=event.key==="ArrowUp"?-1:event.key==="ArrowDown"?1:0;
  if((dx||dy)&&!applied&&Object.values(rect).every(Number.isFinite)){event.preventDefault();const step=event.shiftKey?10:1;move(resize?{...rect,width:rect.width+dx*step,height:rect.height+dy*step}:{...rect,x:rect.x+dx*step,y:rect.y+dy*step});}
 };
 const drawn=Object.values(rect).every(Number.isFinite)&&rect.width>0&&rect.height>0;
 return <Modal title="Create form field" description="Place a field, preview it, then apply. Use Fill forms to enter values after creating it." className="form-author-dialog" onClose={close}>
  <div className="form-author-grid"><div className="form-author-controls">
   <label>Field type<select value={kind} disabled={applied} onChange={e=>{invalidate();setKind(e.target.value as NewFormField["kind"]);if(e.target.value==="radio"&&Object.values(rect).every(Number.isFinite))setArea(strings(bounded({...rect,height:Math.max(rect.height,Math.min(32,choices.split(/\r\n?|\n/).length)*28)})));}}><option value="text">Text field</option><option value="checkbox">Checkbox</option><option value="dropdown">Dropdown</option><option value="list">List</option><option value="radio">Radio group</option></select></label>
   <label>Field name<input value={name} disabled={applied} onChange={e=>{invalidate();setName(e.target.value);}}/></label>
   <label>Accessible label<input value={label} disabled={applied} onChange={e=>{invalidate();setLabel(e.target.value);}}/></label>
   <label className="form-author-check"><input type="checkbox" checked={required} disabled={applied} onChange={e=>{invalidate();setRequired(e.target.checked);}}/>Required</label>
   {kind==="text"&&<label className="form-author-check"><input type="checkbox" checked={multiline} onChange={e=>{invalidate();setMultiline(e.target.checked);}}/>Multiple lines</label>}
   {(kind==="dropdown"||kind==="list"||kind==="radio")&&<label>Choices (one per line)<textarea rows={5} value={choices} onChange={e=>{invalidate();setChoices(e.target.value);}}/></label>}
   {kind==="radio"&&<p>Choices appear in vertical rows with visible labels. Only one choice can be selected in the group.</p>}
   {kind==="list"&&<label className="form-author-check"><input type="checkbox" checked={multiSelect} onChange={e=>{invalidate();setMultiSelect(e.target.checked);}}/>Allow multiple selections</label>}
   {kind!=="checkbox"&&<label>Font size (pt)<input type="number" min="1" max="300" value={fontSize} onChange={e=>{invalidate();setFontSize(e.target.value);}}/></label>}
   <div className="form-author-position">{([["x","Left"],["y","Top"],["width","Width"],["height","Height"]] as const).map(([key,title])=><label key={key}>{title} (pt)<input type="number" step="0.5" value={area[key]} disabled={applied} onChange={e=>{invalidate();setArea(old=>({...old,[key]:e.target.value}));}}/></label>)}</div>
   <p>Drag the field or its corner. Arrow keys move or resize; Shift uses 10 pt steps. Text uses Helvetica.</p>
  </div><div className="form-author-preview"><div style={{aspectRatio:`${visible.width}/${visible.height}`,width:`min(100%, calc(var(--form-preview-height) * ${visible.width/visible.height}))`}}>
   {preview.url&&<img src={preview.url} alt={candidate?"Created field preview":"Page preview"}/>}
   <svg aria-label="Field placement" viewBox={`${visible.x} ${visible.y} ${visible.width} ${visible.height}`} onPointerDown={e=>{
    if(applied||e.button!==0||!drawn)return;const handle=(e.target as Element).closest("[data-field-handle]");if(!handle)return;
    e.preventDefault();(handle as SVGElement).focus();e.currentTarget.setPointerCapture(e.pointerId);drag.current={id:e.pointerId,x:e.clientX,y:e.clientY,area:rect,resize:handle.getAttribute("data-field-handle")==="resize"};
   }} onPointerMove={e=>{const d=drag.current;if(!d||d.id!==e.pointerId)return;const box=e.currentTarget.getBoundingClientRect();if(!box.width||!box.height)return;const dx=(e.clientX-d.x)*visible.width/box.width,dy=(e.clientY-d.y)*visible.height/box.height;move(d.resize?{...d.area,width:d.area.width+dx,height:d.area.height+dy}:{...d.area,x:d.area.x+dx,y:d.area.y+dy});}}
    onPointerUp={()=>{drag.current=null;}} onPointerCancel={()=>{drag.current=null;}} onLostPointerCapture={()=>{drag.current=null;}}>
    {drawn&&<><rect {...rect} data-field-handle="move" className="form-author-area" role="button" aria-label="Move field" tabIndex={applied?-1:0} onKeyDown={e=>keyboard(e,false)}/><rect x={rect.x+rect.width-5} y={rect.y+rect.height-5} width="10" height="10" data-field-handle="resize" className="form-author-handle" role="button" aria-label="Resize field" tabIndex={applied?-1:0} onKeyDown={e=>keyboard(e,true)}/></>}
   </svg>
  </div>{preview.loading&&<p role="status">Rendering page…</p>}{preview.error&&<p role="alert">{preview.error}</p>}</div></div>
  {(admission||error||invalid)&&<p role="alert" className="error-note">{admission||error||invalid}</p>}
  {busy&&<p role="status">Preparing field…</p>}
  <div className="dialog-actions"><button className="button" onClick={close}>Cancel</button><button className="button" disabled={busy||applied||!!invalid||!!admission} onClick={()=>void prepare()}>Preview field</button><button className="button primary" disabled={!ready} onClick={apply}>Apply field</button></div>
 </Modal>;
}
