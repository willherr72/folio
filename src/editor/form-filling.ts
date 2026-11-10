import {commit} from "./model";
import type {DocumentInfo,EditorDocument} from "./types";
import type {DocumentSession} from "./workspace";

/** Replace an entire immutable source so repeated/shared fields keep one identity. */
export function commitFormReplacement(session:DocumentSession,expected:EditorDocument,sourceId:string,result:DocumentInfo):DocumentSession {
 if(session.history.present.pages!==expected.pages)throw new Error("The document changed while filling. Reopen the form and try again.");
 if(!result.id||session.sourceIds.includes(result.id))throw new Error("The filled form must have a new source version.");
 const pages=expected.pages.filter(page=>page.sourceId===sourceId);
 if(!pages.length)throw new Error("The form source is no longer in this document.");
 for(const page of pages){
  const size=result.pages[page.pageIndex];
  if(!size||!Number.isFinite(size.width)||!Number.isFinite(size.height)||Math.abs(size.width-page.width)>0.01||Math.abs(size.height-page.height)>0.01)throw new Error("The filled form changed page dimensions or omitted a page.");
 }
 return {...session,sourceIds:[...session.sourceIds,result.id],history:commit(session.history,current=>({...current,selectedOverlayId:null,
  pages:current.pages.map(page=>page.sourceId===sourceId?{...page,sourceId:result.id,contentSourceId:undefined,protection:result.protection}:page),
 }))};
}
