import {cloneOverlay,commit,planDigest} from "./model";
import {filterBookmarks} from "./bookmarks";
import {allowsWorkspaceEdit,canArrangePages} from "./protection";
import type {EditorDocument,History,Rotation} from "./types";
import type {DocumentSession} from "./workspace";

export interface OrganizerSelection{ids:string[];anchorId:string;focusId:string}
export interface OrganizerFrame{document:EditorDocument;selection:OrganizerSelection}
export type OrganizerAction={type:"rotate";direction:-1|1}|{type:"duplicate";ids:string[]}|{type:"delete"}|{type:"move";boundary:number};
export function createOrganizer(document:EditorDocument):OrganizerFrame{
 const id=document.pages.find(p=>p.id===document.selectedPageId)?.id??document.pages[0]?.id;
 if(!id||document.pages.length>10000)throw Error("Organizing requires 1 to 10,000 pages.");
 return {document,selection:{ids:[id],anchorId:id,focusId:id}};
}
export function selectOrganizer(frame:OrganizerFrame,id:string,options:{toggle?:boolean;range?:boolean;focusOnly?:boolean}):OrganizerFrame{
 const pages=frame.document.pages,at=pages.findIndex(p=>p.id===id);if(at<0)return frame;
 if(options.focusOnly)return {...frame,selection:{...frame.selection,focusId:id}};
 let selected=new Set(options.toggle?frame.selection.ids:[]),anchor=id;
 if(options.range){
  anchor=pages.some(p=>p.id===frame.selection.anchorId)?frame.selection.anchorId:id;
  const start=pages.findIndex(p=>p.id===anchor);for(let i=Math.min(at,start);i<=Math.max(at,start);i++)selected.add(pages[i].id);
 }else if(options.toggle&&selected.has(id))selected.delete(id);else selected.add(id);
 return {...frame,selection:{ids:pages.filter(p=>selected.has(p.id)).map(p=>p.id),anchorId:anchor,focusId:id}};
}
export function selectAllOrganizer(frame:OrganizerFrame):OrganizerFrame{return {...frame,selection:{...frame.selection,ids:frame.document.pages.map(p=>p.id),anchorId:frame.document.pages[0].id}};}
export function organizerKeyTarget(frame:OrganizerFrame,key:string,columns:number):string|null{
 const pages=frame.document.pages,index=Math.max(0,pages.findIndex(p=>p.id===frame.selection.focusId)),cols=Number.isFinite(columns)?Math.max(1,Math.floor(columns)):1;
 const target=key==="Home"?0:key==="End"?pages.length-1:key==="ArrowLeft"?index-1:key==="ArrowRight"?index+1:key==="ArrowUp"?index-cols:key==="ArrowDown"?index+cols:null;
 return target===null?null:pages[Math.max(0,Math.min(pages.length-1,target))]?.id??null;
}
export function organizePages(frame:OrganizerFrame,action:OrganizerAction):OrganizerFrame{
 const document=frame.document,original=document.pages,ids=new Set(frame.selection.ids),known=new Set(original.map(p=>p.id));
 if(!canArrangePages(original))throw Error("These pages do not permit document assembly.");
 if([...ids].some(id=>!known.has(id)))throw Error("The selected pages changed. Select them again.");
 if(!ids.size)return frame;
 const selected=original.filter(p=>ids.has(p.id));let pages=original,selection=frame.selection;
 if(action.type==="rotate")pages=original.map(p=>ids.has(p.id)?{...p,rotation:((p.rotation+action.direction*90+360)%360) as Rotation}:p);
 else if(action.type==="duplicate"){
  if(original.length+selected.length>10000)throw Error("A document can contain at most 10,000 pages.");
  if(action.ids.length!==selected.length||new Set(action.ids).size!==action.ids.length||action.ids.some(id=>!id||known.has(id)))throw Error("Copies require unique new page IDs.");
  const copies=selected.map((p,i)=>({...p,id:action.ids[i],overlays:p.overlays.map(cloneOverlay)}));
  const at=original.findIndex(p=>p.id===selected.at(-1)!.id)+1;pages=[...original.slice(0,at),...copies,...original.slice(at)];
  selection={ids:action.ids,anchorId:action.ids[0],focusId:action.ids[0]};
 }else if(action.type==="delete"){
  if(ids.size===original.length)throw Error("Keep at least one page in the document.");
  pages=original.filter(p=>!ids.has(p.id));const at=original.findIndex(p=>ids.has(p.id));
  const focus=pages.some(p=>p.id===selection.focusId)?selection.focusId:pages[Math.min(at,pages.length-1)].id;
  selection={ids:[focus],anchorId:focus,focusId:focus};
 }else{
  const at=action.boundary;if(!Number.isInteger(at)||at<0||at>original.length)throw Error("Choose a valid insertion page.");
  const destination=original.slice(0,at).filter(p=>!ids.has(p.id)).length,rest=original.filter(p=>!ids.has(p.id));
  pages=[...rest.slice(0,destination),...selected,...rest.slice(destination)];
  if(pages.every((p,i)=>p===original[i]))return frame;
 }
 const retained=new Set(selection.ids);
 return {document:{...document,pages,selectedPageId:selection.focusId,selectedOverlayId:null,...(action.type==="delete"&&document.bookmarks?{bookmarks:filterBookmarks(document.bookmarks,pages)}:{})},selection:{...selection,ids:pages.filter(p=>retained.has(p.id)).map(p=>p.id)}};
}
export function commitOrganizer(history:History<OrganizerFrame>,action:OrganizerAction):History<OrganizerFrame>{
 const next=organizePages(history.present,action);if(next===history.present)return history;
 return {past:[...history.past,history.present].slice(-100),present:next,future:[]};
}
/** Parent source ownership stays intact; only the frozen document can authorize Apply. */
export function applyOrganizerDocument(session:DocumentSession,expected:EditorDocument,draft:EditorDocument):DocumentSession{
 const current=session.history.present;
 if(current.pages!==expected.pages||current.bookmarks!==expected.bookmarks||current.bookmarkError!==expected.bookmarkError)throw Error("The document changed while organizing. Close this workspace and try again.");
 if(!draft.pages.length||draft.pages.length>10000||new Set(draft.pages.map(p=>p.id)).size!==draft.pages.length||!draft.pages.some(p=>p.id===draft.selectedPageId))throw Error("The organized page selection is invalid.");
 const originals=new Map(expected.pages.map(p=>[p.id,p])),sources=new Map<string,Set<number>>();
 for(const page of expected.pages){let indexes=sources.get(page.sourceId);if(!indexes){indexes=new Set();sources.set(page.sourceId,indexes);}indexes.add(page.pageIndex);}
 for(const page of draft.pages){const before=originals.get(page.id);if(!page.id||!sources.get(page.sourceId)?.has(page.pageIndex)||(before&&(before.sourceId!==page.sourceId||before.pageIndex!==page.pageIndex)))throw Error("An organized page has an unexpected source identity.");}
 if(!canArrangePages(current.pages)||!allowsWorkspaceEdit(current,draft))throw Error("This document does not permit page assembly.");
 if(planDigest(current)===planDigest(draft))return session;
 return {...session,history:commit(session.history,()=>draft)};
}
