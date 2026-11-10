import {useEffect,useLayoutEffect,useRef,useState} from "react";
import type {DocumentBookmark,EditorDocument} from "../editor/types";
import {bookmarkPage,branchEnd,moveBookmark,MAX_BOOKMARKS} from "../editor/bookmarks";
import "./bookmarks.css";
const EMPTY:DocumentBookmark[]=[];
export function BookmarkSidebar({document,disabled,readOnly=false,onNavigate,onAdd,onEdit,onMove,onRemove,onDiscardError}:{
 document:EditorDocument;disabled:boolean;readOnly?:boolean;onNavigate(pageId:string):void;onAdd(parentId?:string):void;onEdit(id:string):void;onMove(id:string,direction:-1|1):void;onRemove(id:string):void;onDiscardError():void;
}){
 const items=document.bookmarks??EMPTY;
 const [selected,setSelected]=useState<string|null>(null);
 const [collapsed,setCollapsed]=useState(()=>new Set(items.filter(b=>b.closed).map(b=>b.id)));
 const refs=useRef(new Map<string,HTMLDivElement>()),add=useRef<HTMLButtonElement>(null),pendingFocus=useRef<string|null>(null);
 const previousIds=useRef(new Set(items.map(b=>b.id)));
 useEffect(()=>{
   const added=items.find(b=>!previousIds.current.has(b.id));
   previousIds.current=new Set(items.map(b=>b.id));
   if(added){
     setSelected(added.id);let level=added.level;const ancestors:string[]=[];
     for(let i=items.indexOf(added)-1;i>=0&&level>0;i--){if(items[i].level<level){ancestors.push(items[i].id);level=items[i].level;}}
     setCollapsed(old=>new Set([...old].filter(id=>!ancestors.includes(id))));
   }else if(selected&&!items.some(b=>b.id===selected))setSelected(items[0]?.id??null);
 },[items,selected]);
 useLayoutEffect(()=>{if(pendingFocus.current!==null){(refs.current.get(pendingFocus.current)??add.current)?.focus();pendingFocus.current=null;}},[items,collapsed,selected]);
 const visible:DocumentBookmark[]=[];let hiddenLevel=Infinity;
 for(const b of items){if(b.level>hiddenLevel)continue;hiddenLevel=collapsed.has(b.id)?b.level:Infinity;visible.push(b);}
 const chosen=items.find(b=>b.id===selected),chosenIndex=chosen?items.indexOf(chosen):-1;
 const focus=(id:string)=>{setSelected(id);refs.current.get(id)?.focus();};
 const activate=(b:DocumentBookmark)=>{if(disabled)return;setSelected(b.id);const page=bookmarkPage(b);if(page)onNavigate(page);};
 const toggle=(b:DocumentBookmark)=>{if(disabled)return;setSelected(b.id);pendingFocus.current=b.id;setCollapsed(value=>{const next=new Set(value);if(next.has(b.id))next.delete(b.id);else next.add(b.id);return next;});};
 const key=(event:React.KeyboardEvent,b:DocumentBookmark)=>{
   if(disabled)return;const index=visible.indexOf(b),all=items.indexOf(b),hasChildren=items[all+1]?.level>b.level;
   const focusAt=(n:number)=>focus(visible[Math.max(0,Math.min(visible.length-1,n))].id);
   if(event.key==="ArrowDown")focusAt(index+1);else if(event.key==="ArrowUp")focusAt(index-1);
   else if(event.key==="Home")focusAt(0);else if(event.key==="End")focusAt(visible.length-1);
   else if(event.key==="ArrowRight"){if(hasChildren){if(collapsed.has(b.id))toggle(b);else focusAt(index+1);}}
   else if(event.key==="ArrowLeft"){if(hasChildren&&!collapsed.has(b.id))toggle(b);else {for(let i=all-1;i>=0;i--){if(items[i].level<b.level){focus(items[i].id);break;}}}}
   else if(event.key==="Enter"||event.key===" ")activate(b);else return;
   event.preventDefault();event.stopPropagation();
 };
 const remove=()=>{if(!chosen)return;const removed=new Set(items.slice(chosenIndex,branchEnd(items,chosenIndex)).map(b=>b.id));const survivors=visible.filter(b=>!removed.has(b.id));const fallback=survivors[Math.min(visible.indexOf(chosen),survivors.length-1)];pendingFocus.current=fallback?.id??"";setSelected(fallback?.id??null);onRemove(chosen.id);};
 return <div className="bookmarks-panel">
  {document.bookmarkError&&<div className="bookmark-warning"><p>{document.bookmarkError}</p><p>Saving is blocked until unreadable bookmarks are discarded. Page content is kept.</p><button className="button" disabled={disabled||readOnly} onClick={onDiscardError}>Discard unreadable bookmarks</button></div>}
  <div className="bookmark-tree" role="tree" aria-label="Document bookmarks">{visible.map((b,i)=>{
    const page=bookmarkPage(b),pageIndex=document.pages.findIndex(p=>p.id===page),index=items.indexOf(b),hasChildren=items[index+1]?.level>b.level;
    const label=`${b.title||"Untitled bookmark"} — ${pageIndex>=0?`Page ${pageIndex+1}`:b.action?"External action (preserved)":"Heading"}`;
    return <div ref={el=>{if(el)refs.current.set(b.id,el);else refs.current.delete(b.id);}} key={b.id} role="treeitem" aria-label={label} aria-level={b.level+1} aria-selected={selected===b.id} aria-expanded={hasChildren?!collapsed.has(b.id):undefined} aria-disabled={disabled} tabIndex={disabled?-1:(selected===b.id||(!selected&&i===0)?0:-1)} className={`bookmark-row ${selected===b.id?"selected":""}`} style={{paddingLeft:6+Math.min(b.level,8)*12}} onKeyDown={e=>key(e,b)} onClick={()=>activate(b)}>
      {hasChildren?<button tabIndex={-1} aria-label={`${collapsed.has(b.id)?"Expand":"Collapse"} ${b.title}`} disabled={disabled} onClick={e=>{e.stopPropagation();toggle(b);}}>{collapsed.has(b.id)?"▸":"▾"}</button>:<span className="bookmark-leaf">·</span>}
      <span className="bookmark-title" title={label} style={{fontWeight:b.bold?700:undefined,fontStyle:b.italic?"italic":undefined,color:b.color?`rgb(${b.color.map(n=>Math.round(n*255)).join(",")})`:undefined}}>{b.title||"Untitled bookmark"}</span><small>{pageIndex>=0?pageIndex+1:b.action?"↗":""}</small>
    </div>;
  })}</div>
  {!items.length&&!document.bookmarkError&&<p className="bookmark-empty">No bookmarks yet. Add one for a page you want to return to.</p>}
  <div className="bookmark-actions">
    <button ref={add} className="button" disabled={disabled||readOnly||items.length>=MAX_BOOKMARKS} onClick={()=>onAdd()}>Add bookmark…</button>
    <div className="bookmark-action-grid">
      <button className="button" aria-label="Add child bookmark" disabled={disabled||readOnly||!chosen||chosen.level>=31||items.length>=MAX_BOOKMARKS} onClick={()=>onAdd(chosen!.id)}>Add child</button>
      <button className="button" aria-label="Edit bookmark" disabled={disabled||readOnly||!chosen} onClick={()=>onEdit(chosen!.id)}>Edit…</button>
      <button className="button" aria-label="Move bookmark up" disabled={disabled||readOnly||!chosen||moveBookmark(document,chosen.id,-1)===document} onClick={()=>{pendingFocus.current=chosen!.id;onMove(chosen!.id,-1);}}>Move up</button>
      <button className="button" aria-label="Move bookmark down" disabled={disabled||readOnly||!chosen||moveBookmark(document,chosen.id,1)===document} onClick={()=>{pendingFocus.current=chosen!.id;onMove(chosen!.id,1);}}>Move down</button>
    </div>
    <button className="button" aria-label="Delete bookmark" disabled={disabled||readOnly||!chosen} onClick={remove}>Delete bookmark</button>
  </div>
 </div>;
}
