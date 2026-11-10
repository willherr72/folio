import {useId,useMemo,useLayoutEffect,useEffect,useRef,useState,type KeyboardEvent,type DragEvent} from "react";
import {Modal} from "./Modal";
import {usePageWindow} from "./usePageWindow";
import {Thumbnail} from "./PageView";
import {createHistory,undo,redo,planDigest,uniqueId} from "../editor/model";
import {createOrganizer,selectOrganizer,selectAllOrganizer,organizerKeyTarget,commitOrganizer,type OrganizerAction,type OrganizerFrame} from "../editor/page-organizer";
import {canArrangePages} from "../editor/protection";
import type {FolioAdapter} from "../editor/adapter";
import type {EditorDocument,History,PagePlan} from "../editor/types";
import "./page-organizer.css";
interface Props{document:EditorDocument;adapter:FolioAdapter;onClose():void;onApply(draft:EditorDocument):void;onExtract(draft:EditorDocument,pages:PagePlan[]):Promise<string|null>}
export function PageOrganizerDialog({document,adapter,onClose,onApply,onExtract}:Props){
 const[history,setHistory]=useState(()=>createHistory(createOrganizer(document))),[busy,setBusy]=useState(false),[failure,setFailure]=useState<string|null>(null),[notice,setNotice]=useState<string|null>(null),[moveBefore,setMoveBefore]=useState("1");
 const[dragged,setDragged]=useState<string[]>([]),[dragOrigin,setDragOrigin]=useState<string|null>(null),[dropTarget,setDropTarget]=useState<{id:string;side:"before"|"after"}|null>(null);
 const state=useRef(history);state.current=history;
 const historyFocus=useRef<HTMLElement|null>(null);
 const grid=useRef<HTMLDivElement>(null),initialFocus=useRef<HTMLButtonElement>(null),focusRequested=useRef(false),revealRequested=useRef(false),pending=useRef(false),alive=useRef(true);
 const drag=useRef<{ids:string[];pages:PagePlan[]}|null>(null);const helpId=useId();
 const frame=history.present,selected=new Set(frame.selection.ids),count=selected.size,allowed=canArrangePages(document.pages),disabled=busy||!allowed;
 const changed=useMemo(()=>planDigest(frame.document)!==planDigest(document),[frame.document,document]),moveTarget=moveBefore.trim()?Number(moveBefore):NaN,validMove=Number.isInteger(moveTarget)&&moveTarget>=1&&moveTarget<=frame.document.pages.length+1;
 const windowed=usePageWindow(grid,{count:frame.document.pages.length,initialIndex:frame.document.pages.findIndex(p=>p.id===document.selectedPageId),retained:[frame.document.pages.findIndex(p=>p.id===frame.selection.focusId),frame.document.pages.findIndex(p=>p.id===dragOrigin)]});
 const install=(next:History<OrganizerFrame>)=>{state.current=next;setHistory(next);};
 const cancelDrag=()=>{drag.current=null;setDragged([]);setDragOrigin(null);setDropTarget(null);};
 useEffect(()=>{alive.current=true;return()=>{alive.current=false;};},[]);
 useEffect(()=>{cancelDrag();},[frame.document.pages]);
 useLayoutEffect(()=>{
  const owner=historyFocus.current;historyFocus.current=null;
  // A native disabled button can drop focus to BODY without a focusin event.
  // Recover only the control that initiated history, and never steal new focus.
  const active=window.document.activeElement;
  if(owner&&(!owner.isConnected||owner.matches(":disabled"))&&(active===owner||active===window.document.body))focusRequested.current=true;
  if(busy||(!focusRequested.current&&!revealRequested.current))return;revealRequested.current=false;windowed.reveal(frame.document.pages.findIndex(p=>p.id===frame.selection.focusId));if(!focusRequested.current)return;focusRequested.current=false;const target=Array.from(grid.current?.querySelectorAll<HTMLButtonElement>('[role="option"]')??[]).find(e=>e.dataset.pageId===frame.selection.focusId);target?.focus({preventScroll:true});target?.scrollIntoView?.({block:"nearest",inline:"nearest"});},[frame,busy,windowed.reveal]);
 const choose=(id:string,options:Parameters<typeof selectOrganizer>[2],focus=false)=>{if(pending.current)return;focusRequested.current=focus;if(focus)windowed.reveal(state.current.present.document.pages.findIndex(p=>p.id===id));const h=state.current;install({...h,present:selectOrganizer(h.present,id,options)});};
 const run=(action:OrganizerAction,focus=false)=>{if(pending.current||!allowed)return;try{focusRequested.current=focus;revealRequested.current=true;install(commitOrganizer(state.current,action));setFailure(null);setNotice(null);}catch(e){setFailure(e instanceof Error?e.message:String(e));}};
 const travel=(direction:"undo"|"redo",focus=false,origin?:EventTarget|null)=>{if(pending.current)return;const active=window.document.activeElement;historyFocus.current=origin===active&&active instanceof HTMLElement?active:null;cancelDrag();focusRequested.current=focus;revealRequested.current=true;install(direction==="undo"?undo(state.current):redo(state.current));setFailure(null);setNotice(null);};
 const close=()=>{if(!pending.current)onClose();};
 const all=()=>{if(pending.current)return;const h=state.current;install({...h,present:selectAllOrganizer(h.present)});};
 const extract=async()=>{
  if(pending.current||!count||!allowed||adapter.kind!=="native")return;
  if(frame.document.bookmarkError){setFailure("Resolve unreadable bookmarks in the Bookmarks panel before extracting pages.");return;}
  pending.current=true;setBusy(true);setFailure(null);setNotice(null);cancelDrag();
  const draft=state.current.present.document,ids=new Set(state.current.present.selection.ids),pages=draft.pages.filter(p=>ids.has(p.id));
  try{const path=await onExtract(draft,pages);if(alive.current)setNotice(path?`Extracted ${pages.length} page${pages.length===1?"":"s"} to ${path}. The arrangement has not been applied.`:"Extraction cancelled. Your draft is unchanged.");}
  catch(e){if(alive.current)setFailure(e instanceof Error?e.message:String(e));}
  finally{pending.current=false;if(alive.current)setBusy(false);}
 };
 const apply=()=>{if(pending.current||!changed||!allowed)return;try{onApply({...state.current.present.document,selectedPageId:state.current.present.selection.focusId});}catch(e){setFailure(e instanceof Error?e.message:String(e));}};

 const key=(e:KeyboardEvent<HTMLButtonElement>,id:string)=>{
  if(pending.current){e.preventDefault();return;}
  const command=e.ctrlKey||e.metaKey;
  if(command&&e.key.toLowerCase()==="a"){e.preventDefault();e.stopPropagation();all();return;}
  if(e.key===" "||e.key==="Enter"){e.preventDefault();choose(id,{toggle:command,range:e.shiftKey},true);return;}
  if(e.key==="Delete"){e.preventDefault();if(count&&count<frame.document.pages.length)run({type:"delete"},true);return;}
  const target=organizerKeyTarget(state.current.present,e.key,windowed.metrics.columns);
  if(target){e.preventDefault();choose(target,{range:e.shiftKey,toggle:command,focusOnly:command&&!e.shiftKey},true);}
 };
 const side=(e:DragEvent<HTMLButtonElement>)=>{const rect=e.currentTarget.getBoundingClientRect();return !rect.width||e.clientX<rect.left+rect.width/2?"before" as const:"after" as const;};
 const localKey=(e:KeyboardEvent<HTMLDivElement>)=>{
   if(e.key==="Escape"&&drag.current){e.preventDefault();e.stopPropagation();cancelDrag();return;}
   const typing=e.target instanceof HTMLElement&&e.target.matches("input,textarea,[contenteditable=true]");
   if((e.ctrlKey||e.metaKey)&&!typing&&["z","y"].includes(e.key.toLowerCase())){e.preventDefault();e.stopPropagation();travel(e.key.toLowerCase()==="y"||e.shiftKey?"redo":"undo",!!grid.current?.contains(e.target as Node),e.target);}
 };
 return <Modal className="page-organizer-dialog" title="Organize pages" description="Arrange a draft here, then Apply it as one document Undo step. Cancel leaves the document unchanged." onClose={close} initialFocusRef={initialFocus}>
  <div className="organizer-body" onKeyDownCapture={localKey}>
   <div className="organizer-actions" role="group" aria-label="Page arrangement actions">
    <button className="button" disabled={disabled||!history.past.length} onClick={event=>travel("undo",false,event.currentTarget)}>Undo page action</button><button className="button" disabled={disabled||!history.future.length} onClick={event=>travel("redo",false,event.currentTarget)}>Redo page action</button>
    <button className="button" disabled={disabled||!count} onClick={()=>run({type:"rotate",direction:-1})}>Rotate left</button><button className="button" disabled={disabled||!count} onClick={()=>run({type:"rotate",direction:1})}>Rotate right</button>
    <button className="button" disabled={disabled||!count||frame.document.pages.length+count>10000} onClick={()=>run({type:"duplicate",ids:frame.selection.ids.map(()=>uniqueId("page"))})}>Duplicate selected</button><button className="button" disabled={disabled||!count||count===frame.document.pages.length} onClick={()=>run({type:"delete"})}>Delete selected</button>
    <button className="button" disabled={disabled||!count} onClick={()=>run({type:"move",boundary:0})}>Move to start</button><button className="button" disabled={disabled||!count} onClick={()=>run({type:"move",boundary:frame.document.pages.length})}>Move to end</button>
    <label>Move before page <input type="number" min="1" max={frame.document.pages.length+1} disabled={disabled} value={moveBefore} onChange={e=>setMoveBefore(e.target.value)} aria-invalid={!validMove}/></label><button className="button" disabled={disabled||!count||!validMove} onClick={()=>run({type:"move",boundary:moveTarget-1})}>Move</button>
   </div>
   <div className="organizer-selection"><span role="status">{count} of {frame.document.pages.length} pages selected</span><button className="button" disabled={busy} onClick={all}>Select all</button><button className="button" disabled={busy||!count} onClick={()=>{const h=state.current;install({...h,present:{...h.present,selection:{...h.present.selection,ids:[]}}});}}>Clear selection</button></div>
   <p className="organizer-help" id={helpId}>Ctrl-click toggles pages; Shift-click selects a range. Arrow keys move through the grid; Shift extends selection. Drag selected pages to reorder. Move before page {frame.document.pages.length+1} places them at the end.</p>
   <div ref={grid} className="organizer-grid" role="listbox" aria-label="Pages to organize" aria-multiselectable="true" aria-describedby={helpId} onDragOver={e=>{if(disabled||!drag.current)return;e.preventDefault();e.dataTransfer.dropEffect="move";const box=e.currentTarget.getBoundingClientRect();if(e.clientY<box.top+28)e.currentTarget.scrollTop-=16;else if(e.clientY>box.bottom-28)e.currentTarget.scrollTop+=16;}}>
    <div className="organizer-grid-content" style={{position:"relative",height:windowed.totalHeight}}>
    {windowed.indices.map(index=>{const page=frame.document.pages[index];return <button key={page.id} style={windowed.style(index)} aria-posinset={index+1} aria-setsize={frame.document.pages.length} ref={page.id===document.selectedPageId?initialFocus:undefined} type="button" role="option" aria-selected={selected.has(page.id)} aria-label={`Page ${index+1} of ${frame.document.pages.length}`} data-page-id={page.id} data-page-index={page.pageIndex} tabIndex={page.id===frame.selection.focusId?0:-1} disabled={busy} draggable={!disabled} className={`organizer-page${selected.has(page.id)?" selected":""}${dragged.includes(page.id)?" dragging":""}${dropTarget?.id===page.id?` drop-${dropTarget.side}`:""}`}
     onClick={e=>choose(page.id,{toggle:e.ctrlKey||e.metaKey,range:e.shiftKey})} onKeyDown={e=>key(e,page.id)}
     onDragStart={e=>{if(disabled||!e.dataTransfer){e.preventDefault();return;}const h=state.current;const f=h.present.selection.ids.includes(page.id)?h.present:selectOrganizer(h.present,page.id,{});install({...h,present:f});drag.current={ids:f.selection.ids,pages:f.document.pages};setDragged(f.selection.ids);setDragOrigin(page.id);e.dataTransfer.setData("text/plain",page.id);e.dataTransfer.effectAllowed="move";}}
     onDragEnd={cancelDrag} onDragOver={e=>{if(disabled||!drag.current||drag.current.ids.includes(page.id))return;e.preventDefault();e.dataTransfer.dropEffect="move";setDropTarget({id:page.id,side:side(e)});}}
     onDragLeave={e=>{if(!e.currentTarget.contains(e.relatedTarget as Node|null))setDropTarget(null);}}
     onDrop={e=>{e.preventDefault();const owned=drag.current;cancelDrag();if(disabled||!owned||owned.pages!==state.current.present.document.pages||owned.ids.includes(page.id))return;const at=state.current.present.document.pages.findIndex(p=>p.id===page.id);if(at<0)return;const h=state.current;install({...h,present:{...h.present,selection:{...h.present.selection,ids:owned.ids}}});run({type:"move",boundary:at+(side(e)==="after"?1:0)});}}>
     <span className="organizer-page-preview"><Thumbnail adapter={adapter} page={page}/></span><span className="organizer-page-number">Page {index+1}{selected.has(page.id)?" ✓":""}</span><span className="organizer-page-rotation">{page.rotation?`${page.rotation}° rotation`:"Original rotation"}</span>
    </button>;})}
    </div>
   </div>
  </div>
  <div className="organizer-feedback">{failure&&<p className="error-note" role="alert">{failure}</p>}{notice&&<p role="status">{notice}</p>}{busy&&<p role="status">Saving selected pages… Finish or cancel the native Save dialog to continue.</p>}</div>
  <div className="dialog-actions" onKeyDownCapture={localKey}><button className="button" disabled={disabled||!count||adapter.kind!=="native"} onClick={()=>void extract()}>Extract selected…</button><span className="organizer-footer-spacer"/><button className="button" disabled={busy} onClick={close}>Cancel</button><button className="button primary" disabled={disabled||!changed} onClick={apply}>Apply arrangement</button></div>
 </Modal>;
}
