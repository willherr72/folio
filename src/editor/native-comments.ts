import {toDisplayPoint,fromDisplayPoint,type Point} from "./geometry";
import {useEffect,useState} from "react";
import {commit} from "./model";
import type {FolioAdapter} from "./adapter";
import type {DocumentInfo,EditorDocument,NativeComment,PagePlan,AnnotationRect} from "./types";
import type {DocumentSession} from "./workspace";

export function useNativeComments(adapter:FolioAdapter,pages:PagePlan[]){
 const key=JSON.stringify([...new Set(pages.map(page=>page.sourceId))]);
 const [state,setState]=useState<{adapter:FolioAdapter;key:string;notes:Map<string,NativeComment[]>;errors:string[];loading:boolean}|null>(null);
 useEffect(()=>{
  let cancelled=false;const ids=JSON.parse(key) as string[];
  if(!adapter.listNativeComments||!ids.length){setState({adapter,key,notes:new Map(),errors:[],loading:false});return;}
  setState({adapter,key,notes:new Map(),errors:[],loading:true});
  void Promise.all(ids.map(async id=>{
   try{return {id,notes:await adapter.listNativeComments!(id)};}
   catch(error){return {id,error:error instanceof Error?error.message:String(error)};}
  })).then(results=>{if(cancelled)return;setState({adapter,key,notes:new Map(results.filter(r=>r.notes).map(r=>[r.id,r.notes!])),errors:results.flatMap(r=>r.error?[r.error]:[]),loading:false});});
  return()=>{cancelled=true;};
 },[adapter,key]);
 const current=state?.adapter===adapter&&state.key===key?state:null;
 return {notes:current?.notes??new Map<string,NativeComment[]>(),errors:current?.errors??[],loading:!!adapter.listNativeComments&&(!current||current.loading)};
}

export function commitNativeCommentReplacement(session:DocumentSession,expected:EditorDocument,sourceId:string,result:DocumentInfo):DocumentSession {
 if(session.history.present.pages!==expected.pages)throw new Error("The document changed while editing. Reopen the comment and try again.");
 if(!result.id||session.sourceIds.includes(result.id))throw new Error("The edited comment must have a new source version.");
 const pages=expected.pages.filter(page=>page.sourceId===sourceId);
 if(!pages.length)throw new Error("The comment source is no longer in this document.");
 for(const page of pages){const size=result.pages[page.pageIndex];
  if(!size||!Number.isFinite(size.width)||!Number.isFinite(size.height)||Math.abs(size.width-page.width)>0.01||Math.abs(size.height-page.height)>0.01)throw new Error("Editing the comment changed or omitted a page.");
 }
 return {...session,sourceIds:[...session.sourceIds,result.id],history:commit(session.history,current=>({...current,selectedOverlayId:null,
  pages:current.pages.map(page=>page.sourceId===sourceId?{...page,sourceId:result.id,contentSourceId:undefined,protection:result.protection}:page),
 }))};
}

function mapBounds(bounds:AnnotationRect,map:(point:Point)=>Point):AnnotationRect {
 const corners=[{x:bounds.x,y:bounds.y},{x:bounds.x+bounds.width,y:bounds.y},{x:bounds.x,y:bounds.y+bounds.height},{x:bounds.x+bounds.width,y:bounds.y+bounds.height}].map(map);
 const x=Math.min(...corners.map(p=>p.x)),y=Math.min(...corners.map(p=>p.y));
 return {x,y,width:Math.max(...corners.map(p=>p.x))-x,height:Math.max(...corners.map(p=>p.y))-y};
}
export function displayNativeCommentBounds(bounds:AnnotationRect,page:PagePlan):AnnotationRect {
 return mapBounds(bounds,point=>toDisplayPoint(point,page.width,page.height,page.rotation));
}
export function nativeCommentPosition(displayed:AnnotationRect,page:PagePlan):Point {
 const bounds=mapBounds(displayed,point=>fromDisplayPoint(point,page.width,page.height,page.rotation));return {x:bounds.x,y:bounds.y};
}

/** Keep NoRotate's final upright display box in canonical SVG interaction space. */
export function nativeCommentPageHitBounds(note:NativeComment,page:PagePlan):AnnotationRect {
 const hit=note.hitBounds??note.bounds;if(!note.noRotate||!page.rotation)return hit;
 const anchor=toDisplayPoint(hit,page.width,page.height,page.rotation);
 return mapBounds({...hit,...anchor},point=>fromDisplayPoint(point,page.width,page.height,page.rotation));
}
