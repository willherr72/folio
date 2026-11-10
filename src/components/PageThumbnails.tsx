import {useLayoutEffect,useRef,useState,type DragEvent} from "react";
import {GripVertical} from "lucide-react";
import {Thumbnail} from "./PageView";
import {usePageWindow} from "./usePageWindow";
import type {FolioAdapter} from "../editor/adapter";
import type {PagePlan} from "../editor/types";
interface Props{
 pages:PagePlan[];selectedPageId:string|null;adapter:FolioAdapter;blocked:boolean;readingBlocked:boolean;canArrange:boolean;
 draggedPageId:string|null;dropTarget:{pageId:string;side:"before"|"after"}|null;
 onSelect(id:string):void;onDragStart(event:DragEvent<HTMLButtonElement>,id:string):void;
 onDragOver(event:DragEvent<HTMLButtonElement>,id:string):void;onDrop(event:DragEvent<HTMLButtonElement>,id:string):void;
 onDragEnd():void;onDragLeave(event:DragEvent<HTMLButtonElement>):void;
}
export function PageThumbnails(props:Props){
 const host=useRef<HTMLDivElement>(null),[focusId,setFocusId]=useState(props.selectedPageId??props.pages[0]?.id),requestFocus=useRef(false);
 const selectedIndex=Math.max(0,props.pages.findIndex(p=>p.id===props.selectedPageId));
 const focusIndex=Math.max(0,props.pages.findIndex(p=>p.id===focusId));
 const windowed=usePageWindow(host,{count:props.pages.length,initialIndex:selectedIndex,retained:[focusIndex,props.pages.findIndex(p=>p.id===props.draggedPageId)],singleColumn:true});
 useLayoutEffect(()=>{windowed.reveal(selectedIndex);setFocusId(props.selectedPageId??props.pages[0]?.id);},[props.selectedPageId,selectedIndex,windowed.reveal]);
 useLayoutEffect(()=>{if(!requestFocus.current)return;requestFocus.current=false;const target=Array.from(host.current?.querySelectorAll<HTMLButtonElement>('[data-thumbnail-id]')??[]).find(x=>x.dataset.thumbnailId===focusId);target?.focus({preventScroll:true});},[focusId,windowed.indices]);
 return <div className="thumbnails" role="group" aria-label="Page thumbnails" ref={host}>
  <div style={{position:"relative",height:windowed.totalHeight}}>
   {windowed.indices.map(index=>{const page=props.pages[index];return <button key={page.id} style={windowed.style(index)} data-thumbnail-id={page.id} data-page-index={index}
    className={`thumbnail-item ${page.id===props.selectedPageId?"selected":""} ${props.draggedPageId===page.id?"dragging":""} ${props.dropTarget?.pageId===page.id?`drop-${props.dropTarget.side}`:""}`}
    aria-label={`Page ${index+1} of ${props.pages.length}`} aria-current={page.id===props.selectedPageId?"page":undefined}
    tabIndex={index===focusIndex?0:-1} title={props.canArrange?"Drag to reorder pages. Arrow keys navigate pages.":"This PDF does not permit rearranging pages."}
    draggable={!props.blocked&&props.canArrange} disabled={props.readingBlocked} onFocus={()=>setFocusId(page.id)}
    onKeyDown={event=>{if(props.blocked||props.readingBlocked||event.altKey||event.ctrlKey||event.metaKey)return;
     const at=event.key==="Home"?0:event.key==="End"?props.pages.length-1:event.key==="ArrowUp"?Math.max(0,index-1):event.key==="ArrowDown"?Math.min(props.pages.length-1,index+1):null;
     if(at===null)return;event.preventDefault();event.stopPropagation();requestFocus.current=true;windowed.reveal(at);setFocusId(props.pages[at].id);props.onSelect(props.pages[at].id);
    }}
    onClick={()=>props.onSelect(page.id)} onDragStart={event=>props.onDragStart(event,page.id)} onDragOver={event=>props.onDragOver(event,page.id)}
    onDrop={event=>props.onDrop(event,page.id)} onDragEnd={props.onDragEnd} onDragLeave={props.onDragLeave}>
    <span className="drag-handle"><GripVertical size={14}/></span><div className="thumbnail-paper"><Thumbnail adapter={props.adapter} page={page}/></div><span className="page-number">{index+1}</span>
   </button>;})}
  </div>
 </div>;
}
