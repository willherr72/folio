import {useEffect,useMemo,useRef,useState} from "react";
import {Modal} from "./Modal";
import type {FolioAdapter} from "../editor/adapter";
import type {AnnotationRect,DocumentInfo,EditorDocument,FormField,PageContentReplacement,PagePlan} from "../editor/types";
import {formAuthorSelection} from "../editor/form-authoring";
import {positionedFieldBounds,fieldPositionRequest,type PositionFormFieldRequest} from "../editor/form-positioning";
import {replacementPages} from "../editor/page-replacement";
import {discardOpenedDocument} from "../editor/discard-opened-document";
import {pageVisibleRect} from "../editor/geometry";
import {useComposedPage} from "../editor/use-composed-page";
import "./form-author.css";

interface Props {adapter:FolioAdapter;document:EditorDocument;page:PagePlan;field:FormField;sourceIds:string[];onApply(result:PageContentReplacement):boolean;onCancel():void}
const strings=(r:AnnotationRect)=>({x:String(r.x),y:String(r.y),width:String(r.width),height:String(r.height)});
export function FormPositionDialog({adapter,document,page,field,sourceIds,onApply,onCancel}:Props) {
 const borrowed=useMemo(()=>new Set(sourceIds),[sourceIds]);
 const initial=useMemo(()=>{try{return positionedFieldBounds(page,field);}catch{return {x:0,y:0,width:0,height:0};}},[page,field]);
 const [reflowText,setReflowText]=useState(field.kind==="text");
 const [area,setArea]=useState(()=>strings(initial));
 const [candidate,setCandidate]=useState<{result:PageContentReplacement;page:PagePlan}|null>(null),owner=useRef<DocumentInfo|null>(null);
 const [busy,setBusy]=useState(false),running=useRef(false),[error,setError]=useState<string|null>(null),[applied,setApplied]=useState(false);
 const alive=useRef(false),closed=useRef(false),generation=useRef(0),submitted=useRef(false);
 const drag=useRef<{id:number;x:number;y:number;area:AnnotationRect;resize:boolean}|null>(null);
 const release=(info:DocumentInfo|null)=>{if(info?.id&&!borrowed.has(info.id))void discardOpenedDocument(info,id=>adapter.closeDocument(id)).catch(()=>{});};
 const dispose=()=>{const info=owner.current;owner.current=null;release(info);};
 useEffect(()=>{alive.current=true;return()=>{alive.current=false;generation.current++;dispose();};},[]);
 const rect={x:area.x.trim()?Number(area.x):NaN,y:area.y.trim()?Number(area.y):NaN,width:area.width.trim()?Number(area.width):NaN,height:area.height.trim()?Number(area.height):NaN};
 let request:PositionFormFieldRequest|null=null,admission:string|null=null;
 try{request=fieldPositionRequest(document,page,field,rect,reflowText);if(!adapter.positionFormField||!adapter.renderPagePlan)admission="Positioning fields requires the desktop PDF service.";}catch(e){admission=e instanceof Error?e.message:String(e);}
 const invalid=Object.entries(rect).every(([key,value])=>Math.abs(value-initial[key as keyof AnnotationRect])<0.0001)?"Change the position or size before previewing.":null;
 const preview=useComposedPage(adapter,candidate?.page??page,1000,true,true);
 const ready=!!candidate&&!!preview.url&&!preview.loading&&!preview.error&&!busy&&!applied&&!admission&&!invalid;
 const invalidate=()=>{generation.current++;dispose();setCandidate(null);setError(null);};
 const close=()=>{closed.current=true;invalidate();onCancel();};
 const prepare=async()=>{
  if(running.current||submitted.current||closed.current||invalid||admission||!adapter.positionFormField||!request)return;
  invalidate();const version=generation.current;running.current=true;setBusy(true);let orphan:DocumentInfo|null=null;
  try {
   const selection=formAuthorSelection(document,page);
   const result=await adapter.positionFormField(selection.sourceId,selection.occurrences,selection.selectedOccurrence,request);
   if(!borrowed.has(result.document.id))orphan=result.document;
   if(!alive.current||closed.current||version!==generation.current)return;
   if(borrowed.has(result.document.id))throw new Error("Field positioning must return a new source version.");
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
 return <Modal title="Field position and size" description="Adjust this field, preview the result, then apply." className="form-author-dialog" onClose={close}>
  <div className="form-author-grid"><div className="form-author-controls">
   <p>{field.label||field.name||"Selected field"}</p>
   <p>Moving preserves the field’s appearance.</p>
   {field.kind==="text"&&<label>When resizing<select value={reflowText?"reflow":"scale"} disabled={applied} onChange={e=>{invalidate();setReflowText(e.target.value==="reflow");}}><option value="reflow">Reflow with field settings</option><option value="scale">Scale saved appearance</option></select></label>}
   <p>{reflowText?"Text uses the field’s original font settings. Multiline text wraps to fit the new width. Preview to check for clipped text.":"Resizing scales the saved appearance; later value edits use the original text settings."}</p>
   <div className="form-author-position">{([["x","Left"],["y","Top"],["width","Width"],["height","Height"]] as const).map(([key,title])=><label key={key}>{title} (pt)<input type="number" step="0.5" value={area[key]} disabled={applied} onChange={e=>{invalidate();setArea(old=>({...old,[key]:e.target.value}));}}/></label>)}</div>
   <button className="button" disabled={applied} onClick={()=>{invalidate();setArea(strings(initial));}}>Reset position</button>
   <p>Drag the field or its corner. Arrow keys move or resize; Shift uses 10 pt steps. </p>
  </div><div className="form-author-preview"><div style={{aspectRatio:`${visible.width}/${visible.height}`,width:`min(100%, calc(var(--form-preview-height) * ${visible.width/visible.height}))`}}>
   {preview.url&&<img src={preview.url} alt={candidate?"Positioned field preview":"Page preview"}/>}
   <svg aria-label="Field placement" viewBox={`${visible.x} ${visible.y} ${visible.width} ${visible.height}`} onPointerDown={e=>{
    if(applied||e.button!==0||!drawn)return;const handle=(e.target as Element).closest("[data-field-handle]");if(!handle)return;
    e.preventDefault();(handle as SVGElement).focus();e.currentTarget.setPointerCapture(e.pointerId);drag.current={id:e.pointerId,x:e.clientX,y:e.clientY,area:rect,resize:handle.getAttribute("data-field-handle")==="resize"};
   }} onPointerMove={e=>{const d=drag.current;if(!d||d.id!==e.pointerId)return;const box=e.currentTarget.getBoundingClientRect();if(!box.width||!box.height)return;const dx=(e.clientX-d.x)*visible.width/box.width,dy=(e.clientY-d.y)*visible.height/box.height;move(d.resize?{...d.area,width:d.area.width+dx,height:d.area.height+dy}:{...d.area,x:d.area.x+dx,y:d.area.y+dy});}}
    onPointerUp={()=>{drag.current=null;}} onPointerCancel={()=>{drag.current=null;}} onLostPointerCapture={()=>{drag.current=null;}}>
    {drawn&&<><rect {...rect} data-field-handle="move" className="form-author-area" role="button" aria-label="Move field" tabIndex={applied?-1:0} onKeyDown={e=>keyboard(e,false)}/><rect x={rect.x+rect.width-5} y={rect.y+rect.height-5} width="10" height="10" data-field-handle="resize" className="form-author-handle" role="button" aria-label="Resize field" tabIndex={applied?-1:0} onKeyDown={e=>keyboard(e,true)}/></>}
   </svg>
  </div>{preview.loading&&<p role="status">Rendering page…</p>}{preview.error&&<p role="alert">{preview.error}</p>}</div></div>
  {(admission||error||invalid)&&<p role="alert" className="error-note">{admission||error||invalid}</p>}
  {busy&&<p role="status">Preparing position…</p>}
  <div className="dialog-actions"><button className="button" onClick={close}>Cancel</button><button className="button" disabled={busy||applied||!!invalid||!!admission} onClick={()=>void prepare()}>Preview position</button><button className="button primary" disabled={!ready} onClick={apply}>Apply position</button></div>
 </Modal>;
}
