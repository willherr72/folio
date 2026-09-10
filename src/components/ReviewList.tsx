import { Highlighter, MessageSquare } from "lucide-react";
import type { Overlay, PagePlan } from "../editor/types";
import "./review.css";
export function ReviewList({pages,selectedPageId,selectedOverlayId,disabled,onSelect}:{
 pages:PagePlan[];selectedPageId:string|null;selectedOverlayId:string|null;disabled:boolean;onSelect(pageId:string,overlay:Overlay):void;
}){
 const entries=pages.flatMap((page,index)=>page.overlays.filter(overlay=>overlay.type==="comment"||overlay.type==="highlight").map(overlay=>({page,index,overlay})));
 return <section className="property-section review-list" aria-label="Document review"><h3>Review <span>{entries.length}</span></h3>
   {!entries.length && <p>Highlights and comments appear here. Select an item to jump to its page.</p>}
   {entries.map(({page,index,overlay})=>{
     const title=overlay.type==="comment" ? overlay.text.trim()||"Empty comment" : overlay.type==="highlight" ? overlay.text?.trim()||"Highlight" : "";
     return <button key={page.id+":"+overlay.id} className={page.id===selectedPageId&&overlay.id===selectedOverlayId ? "review-entry active" : "review-entry"} disabled={disabled} aria-pressed={page.id===selectedPageId&&overlay.id===selectedOverlayId} aria-label={`Page ${index+1}: ${title}`} onClick={()=>onSelect(page.id,overlay)}>
       {overlay.type==="highlight" ? <Highlighter size={15}/> : <MessageSquare size={15}/>}<span><small>Page {index+1} · {overlay.type==="highlight" ? "Highlight" : "Comment"}</small><span>{title}</span></span>
     </button>;
   })}
 </section>;
}
