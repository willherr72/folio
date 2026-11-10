import { Highlighter, MessageSquare } from "lucide-react";
import type { Overlay, PagePlan, NativeComment } from "../editor/types";
import "./review.css";
export function ReviewList({pages,selectedPageId,selectedOverlayId,disabled,onSelect,nativeNotes=new Map(),nativeErrors=[],nativeLoading=false,onSelectNative}:{
 nativeNotes?:Map<string,NativeComment[]>;nativeErrors?:string[];nativeLoading?:boolean;onSelectNative?(pageId:string,note:NativeComment):void;
 pages:PagePlan[];selectedPageId:string|null;selectedOverlayId:string|null;disabled:boolean;onSelect(pageId:string,overlay:Overlay):void;
}){
 const nativeEntries=pages.flatMap((page,index)=>(nativeNotes.get(page.sourceId)??[]).filter(note=>note.pageIndex===page.pageIndex).map(note=>({page,index,note})));
 const entries=pages.flatMap((page,index)=>page.overlays.filter(overlay=>overlay.type==="comment"||overlay.type==="highlight").map(overlay=>({page,index,overlay})));
 const ordered=[...nativeEntries.map(entry=>({...entry,kind:"native" as const})),...entries.map(entry=>({...entry,kind:"overlay" as const}))].sort((a,b)=>a.index-b.index);
 return <section className="property-section review-list" aria-label="Document review"><h3>Review <span>{entries.length+nativeEntries.length}</span></h3>
   {!entries.length && !nativeEntries.length && !nativeLoading && !nativeErrors.length && <p>Highlights and comments appear here. Select an item to jump to its page.</p>}
   {nativeLoading&&<p role="status">Loading PDF comments…</p>}
   {nativeErrors.map((error,index)=><p key={index} className="error-note">PDF comments could not be loaded: {error}</p>)}
   {ordered.map(entry=>{
     const {page,index}=entry;
     if(entry.kind==="native"){
       const {note}=entry;
       return <button key={`native:${page.id}:${note.annotationIndex}`} className="review-entry" disabled={disabled} aria-label={`Page ${index+1}: ${note.text.trim()||"Empty comment"}`} onClick={()=>onSelectNative?.(page.id,note)}><MessageSquare size={15}/><span><small>Page {index+1} · PDF comment{note.author?` · ${note.author}`:""}</small><span>{note.text.trim()||"Empty comment"}</span></span></button>;
     }
     const {overlay}=entry;
     const title=overlay.type==="comment" ? overlay.text.trim()||"Empty comment" : overlay.type==="highlight" ? overlay.text?.trim()||"Highlight" : "";
     return <button key={page.id+":"+overlay.id} className={page.id===selectedPageId&&overlay.id===selectedOverlayId ? "review-entry active" : "review-entry"} disabled={disabled} aria-pressed={page.id===selectedPageId&&overlay.id===selectedOverlayId} aria-label={`Page ${index+1}: ${title}`} onClick={()=>onSelect(page.id,overlay)}>
       {overlay.type==="highlight" ? <Highlighter size={15}/> : <MessageSquare size={15}/>}<span><small>Page {index+1} · {overlay.type==="highlight" ? "Highlight" : "Comment"}</small><span>{title}</span></span>
     </button>;
   })}
 </section>;
}
