import {commit} from "./model";
import {pageAllows} from "./protection";
import type {EditorDocument,PageContentReplacement,PagePlan} from "./types";
import type {DocumentSession} from "./workspace";

export function pageReplacementRequest(document:EditorDocument,page:PagePlan) {
 if(!document.pages.includes(page))throw new Error("The page changed. Reopen replacement and try again.");
 const pages=document.pages.filter(value=>value.sourceId===page.sourceId);
 const occurrences=pages.map(value=>value.pageIndex),selectedOccurrence=pages.indexOf(page);
 if(!occurrences.length||occurrences.length>100_000||occurrences.some(index=>!Number.isSafeInteger(index)||index<0))throw new Error("Invalid source page selection.");
 if(!pageAllows(page,"editContent"))throw new Error("This PDF does not permit replacing page content.");
 if(occurrences.filter(index=>index===page.pageIndex).length>1&&!pageAllows(page,"assemble"))throw new Error("Replacing a duplicated page also requires permission to arrange pages.");
 return {sourceId:page.sourceId,occurrences,selectedOccurrence};
}

/** Build preview and committed pages from the same validated native mapping. */
export function replacementPages(document:EditorDocument,page:PagePlan,result:PageContentReplacement):PagePlan[] {
 const {occurrences,selectedOccurrence}=pageReplacementRequest(document,page);
 const info=result.document;
 if(!info.id||document.pages.some(value=>value.sourceId===info.id))throw new Error("Replacement must return a new source version.");
 if(!Array.isArray(result.pageIndices)||result.pageIndices.length!==occurrences.length)throw new Error("Replacement omitted a page occurrence.");
 const repeated=occurrences.filter(index=>index===page.pageIndex).length>1;
 const first=occurrences.indexOf(page.pageIndex),append=info.pages.length-1;
 if(repeated&&occurrences.some(index=>index>=append))throw new Error("Replacement omitted an original page.");
 const expected=occurrences.map((index,i)=>!repeated||index!==page.pageIndex?index:
  (i===selectedOccurrence)===(first===selectedOccurrence)?page.pageIndex:append);
 if(result.pageIndices.some((index,i)=>!Number.isSafeInteger(index)||index<0||index!==expected[i]))throw new Error("Replacement changed the wrong page occurrence.");
 const before=page.protection,after=info.protection;
 if(!!before!==!!after||(before&&after&&(before.documentId!==after.documentId||before.owner!==after.owner||before.revision!==after.revision||Object.entries(before.permissions).some(([key,value])=>after.permissions[key as keyof typeof after.permissions]!==value))))throw new Error("Replacement changed the document’s protection.");
 let occurrence=0;
 return document.pages.map(value=>{
  if(value.sourceId!==page.sourceId)return value;
  const pageIndex=result.pageIndices[occurrence++],size=info.pages[pageIndex];
  if(!size||!Number.isFinite(size.width)||!Number.isFinite(size.height)||size.width<=0||size.height<=0||Math.abs(size.width-value.width)>0.01||Math.abs(size.height-value.height)>0.01)throw new Error("Replacement changed page dimensions or omitted a page.");
  return {...value,sourceId:info.id,pageIndex,contentSourceId:undefined,protection:info.protection};
 });
}

export function commitPageReplacement(session:DocumentSession,expected:EditorDocument,page:PagePlan,result:PageContentReplacement):DocumentSession {
 if(session.history.present.pages!==expected.pages)throw new Error("The document changed. Reopen replacement and try again.");
 if(session.sourceIds.includes(result.document.id))throw new Error("Replacement must return a new source version.");
 const pages=replacementPages(expected,page,result);
 return {...session,sourceIds:[...session.sourceIds,result.document.id],history:commit(session.history,current=>({...current,pages,selectedOverlayId:null}))};
}
