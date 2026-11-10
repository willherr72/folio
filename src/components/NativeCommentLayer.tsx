import {nativeCommentPageHitBounds} from "../editor/native-comments";
import {useLayoutEffect,useRef,useState,type PointerEvent} from "react";
import {clientPointToPage,rectVisibleOnPage,screenArrowDelta,type Point} from "../editor/geometry";
import {pageAllows} from "../editor/protection";
import type {NativeComment,PagePlan} from "../editor/types";
import "./native-comment-controls.css";

export interface NativeCommentLayerProps {
 page:PagePlan; nativeRotation?:boolean; notes:NativeComment[]; zoom:number; enabled:boolean; disabled:boolean;
 onSelect():void; onOpen(note:NativeComment):void;
 onMove?(note:NativeComment,position:Point):Promise<string|null>;
 onStatus(status:{text:string;error?:boolean}|null):void;
}
type Draft={note:NativeComment;position:Point;target:SVGGElement;pointer?:number;start?:Point;clientStart?:Point;moved:boolean};
const hitBounds=(note:NativeComment,page:PagePlan,nativeRotation=false)=>nativeRotation?nativeCommentPageHitBounds(note,page):note.hitBounds??note.bounds;
function limits(note:NativeComment,page:PagePlan,nativeRotation=false){const hit=hitBounds(note,page,nativeRotation),dx=hit.x-note.bounds.x,dy=hit.y-note.bounds.y;return {minX:Math.max(0,-dx),maxX:Math.min(page.width-note.bounds.width,page.width-hit.width-dx),minY:Math.max(0,-dy),maxY:Math.min(page.height-note.bounds.height,page.height-hit.height-dy)};}
const instructions="Drag to move · Arrow keys adjust · Enter edits or applies a move · Esc cancels";
export function NativeCommentLayer(props:NativeCommentLayerProps){
 const {page,notes,enabled,disabled,zoom,nativeRotation=false}=props;
 const latest=useRef(props);latest.current=props;
 const active=useRef<Draft|null>(null),alive=useRef(true),waiting=useRef(false);
 const targets=useRef(new Map<number,SVGGElement>()),restore=useRef<{source:string;index:number}|null>(null);
 const [draft,setDraft]=useState<Draft|null>(null),[selected,setSelected]=useState<number|null>(null),[pending,setPending]=useState(false);
 const movable=(note:NativeComment)=>{const range=limits(note,latest.current.page,latest.current.nativeRotation);return !note.reason&&pageAllows(latest.current.page,"annotate")&&!!latest.current.onMove&&range.minX<=range.maxX&&range.minY<=range.maxY;};
 const release=(value:Draft|null)=>{if(value?.pointer!==undefined)try{value.target.releasePointerCapture(value.pointer);}catch{/* Browser may already have released capture. */}};
 const cancel=()=>{const value=active.current;active.current=null;setDraft(null);release(value);};
 const clamp=(note:NativeComment,position:Point):Point=>{const range=limits(note,page,nativeRotation);return {x:Math.max(range.minX,Math.min(range.maxX,position.x)),y:Math.max(range.minY,Math.min(range.maxY,position.y))};};
 const point=(event:PointerEvent<SVGGElement>):Point=>clientPointToPage(event.clientX,event.clientY,event.currentTarget.ownerSVGElement!.getBoundingClientRect(),page.width,page.height,page.rotation,page.crop);
 const available=()=>latest.current.enabled&&!latest.current.disabled&&!waiting.current;
 const select=(note:NativeComment)=>{setSelected(note.annotationIndex);props.onSelect();props.onStatus({text:movable(note)?instructions:note.reason??"This PDF does not permit moving comments. Press Enter to read the comment."});};
 useLayoutEffect(()=>{cancel();setSelected(null);props.onStatus(null);},[page.id,page.sourceId,page.pageIndex,page.width,page.height,page.rotation,page.crop,zoom,enabled,nativeRotation]);
 useLayoutEffect(()=>{cancel();},[disabled]);
 useLayoutEffect(()=>{restore.current=null;},[page.id,page.pageIndex,page.rotation,zoom,enabled,nativeRotation]);
 useLayoutEffect(()=>{if(active.current&&!movable(active.current.note))cancel();},[page.protection,props.onMove]);
 useLayoutEffect(()=>{const value=active.current;if(value&&notes.find(n=>n.annotationIndex===value.note.annotationIndex)!==value.note)cancel();},[notes]);
 useLayoutEffect(()=>{
  const value=restore.current;if(!value||disabled||!enabled||page.sourceId!==value.source)return;
  const target=targets.current.get(value.index);if(target){restore.current=null;setSelected(value.index);target.focus();}
 },[notes,page.sourceId,disabled,enabled,pending]);
 useLayoutEffect(()=>{
  alive.current=true;
  const escape=(event:KeyboardEvent)=>{if(event.key==="Escape"&&active.current){event.preventDefault();cancel();latest.current.onStatus({text:"Comment move cancelled."});}};
  const blur=()=>{cancel();restore.current=null;};
  const focus=(event:FocusEvent)=>{if(![...targets.current.values()].some(target=>target===event.target||target.contains(event.target as Node)))restore.current=null;};
  const pointer=(event:globalThis.PointerEvent)=>{if(![...targets.current.values()].some(target=>target.contains(event.target as Node)))restore.current=null;};
  window.addEventListener("focusin",focus);window.addEventListener("pointerdown",pointer);
  window.addEventListener("keydown",escape);window.addEventListener("blur",blur);
  return()=>{alive.current=false;window.removeEventListener("keydown",escape);window.removeEventListener("blur",blur);window.removeEventListener("focusin",focus);window.removeEventListener("pointerdown",pointer);const value=active.current;active.current=null;release(value);};
 },[]);
 const commit=async(value:Draft)=>{
  cancel();if(!available()||!movable(value.note)||!value.moved||(value.position.x===value.note.bounds.x&&value.position.y===value.note.bounds.y))return;
  restore.current={source:"",index:value.note.annotationIndex};waiting.current=true;setPending(true);props.onStatus({text:"Moving comment…"});
  const pageId=page.id;
  try{
   const source=await props.onMove!(value.note,value.position);
   if(alive.current&&latest.current.page.id===pageId){if(source&&restore.current?.index===value.note.annotationIndex)restore.current.source=source;props.onStatus({text:source?"Comment moved. Use Undo to restore its position.":"The document changed; the comment was not moved."});}
  }catch(error){if(alive.current)props.onStatus({text:error instanceof Error?error.message:String(error),error:true});}
  finally{waiting.current=false;if(alive.current)setPending(false);}
 };
 const moved=(value:Draft,event:PointerEvent<SVGGElement>):Draft=>{
  const p=point(event),start=value.start!;
  const distance=Math.hypot(event.clientX-value.clientStart!.x,event.clientY-value.clientStart!.y);
  return {...value,moved:value.moved||distance>=3,position:clamp(value.note,{x:value.note.bounds.x+p.x-start.x,y:value.note.bounds.y+p.y-start.y})};
 };
 const visible=notes.filter(note=>{const hit=hitBounds(note,page,nativeRotation);return note.pageIndex===page.pageIndex&&note.onPage!==false&&[...Object.values(note.bounds),...Object.values(hit)].every(Number.isFinite)&&hit.width>0&&hit.height>0&&rectVisibleOnPage(hit,page);});
 if(!enabled)return null;
 return <g className="native-comment-layer">{visible.map(note=><g key={note.annotationIndex} ref={node=>{if(node)targets.current.set(note.annotationIndex,node);else targets.current.delete(note.annotationIndex);}}
  className={`native-comment-target${selected===note.annotationIndex?" selected":""}`} data-native-comment={note.annotationIndex} role="button" aria-label={`PDF comment: ${note.text.trim()||"Empty comment"}`} aria-disabled={disabled||pending} tabIndex={disabled||pending?-1:0}
  onFocus={()=>{if(available())select(note);}} onBlur={()=>{if(active.current?.note===note)cancel();setSelected(null);if(!waiting.current)props.onStatus(null);}}
  onDoubleClick={event=>{event.stopPropagation();if(available()&&!active.current){props.onOpen(note);}}}
  onPointerDown={event=>{
   event.stopPropagation();if(!available()||event.button!==0||active.current)return;
   event.preventDefault();event.currentTarget.focus();select(note);
   if(!movable(note))return;
   const value:Draft={note,position:{x:note.bounds.x,y:note.bounds.y},target:event.currentTarget,pointer:event.pointerId,start:point(event),clientStart:{x:event.clientX,y:event.clientY},moved:false};
   active.current=value;try{event.currentTarget.setPointerCapture(event.pointerId);}catch{/* Synthetic events may not have a live pointer. */}
  }}
  onPointerMove={event=>{const value=active.current;if(!value||value.pointer!==event.pointerId)return;event.stopPropagation();if(!available()){cancel();return;}const next=moved(value,event);active.current=next;if(next.moved){setDraft(next);props.onStatus({text:"Release to move comment · Esc cancels"});}}}
  onPointerUp={event=>{const value=active.current;if(!value||value.pointer!==event.pointerId)return;event.stopPropagation();void commit(moved(value,event));}}
  onPointerCancel={event=>{if(active.current?.pointer===event.pointerId){event.stopPropagation();cancel();}}}
  onLostPointerCapture={event=>{if(active.current?.pointer===event.pointerId)cancel();}}
  onKeyDown={event=>{
   if(!available())return;
   if(event.key==="Escape"&&active.current){event.preventDefault();event.stopPropagation();cancel();props.onStatus({text:"Comment move cancelled."});return;}
   if(event.ctrlKey||event.metaKey||event.altKey)return;
   if(event.key==="Enter"||event.key===" "){event.preventDefault();event.stopPropagation();if(event.repeat)return;const value=active.current;if(value?.pointer!==undefined)return;if(value)void commit(value);else props.onOpen(note);return;}
   const delta=screenArrowDelta(event.key,event.shiftKey,page.rotation);if(!delta||!movable(note)||active.current?.pointer!==undefined)return;
   event.preventDefault();event.stopPropagation();select(note);
   const value=active.current??{note,position:{x:note.bounds.x,y:note.bounds.y},target:event.currentTarget,moved:false};
   const next={...value,moved:true,position:clamp(note,{x:value.position.x+delta.x,y:value.position.y+delta.y})};active.current=next;setDraft(next);props.onStatus({text:"Arrow keys adjust · Enter applies the move · Esc cancels"});
  }}><title>{note.reason??instructions}</title><rect className="native-comment-hit" {...hitBounds(note,page,nativeRotation)}/></g>)}
  {draft&&<rect className="native-comment-draft" x={hitBounds(draft.note,page,nativeRotation).x+draft.position.x-draft.note.bounds.x} y={hitBounds(draft.note,page,nativeRotation).y+draft.position.y-draft.note.bounds.y} width={hitBounds(draft.note,page,nativeRotation).width} height={hitBounds(draft.note,page,nativeRotation).height} pointerEvents="none"/>}
 </g>;
}
