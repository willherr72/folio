import type {FolioAdapter} from "./adapter";
import type {DocumentInfo,EditorDocument,PageContentReplacement,PagePlan,StyledParagraphLayoutRequest} from "./types";
import {pageReplacementRequest,replacementPages} from "./page-replacement";

export interface ParagraphOccurrence {
 document:EditorDocument;page:PagePlan;
 /** Live workspace ownership, including every tab's retained undo/redo sources. */
 isSourceRetained(id:string):boolean;
}
export interface ParagraphPreview {
 document:DocumentInfo;pageIndex:number;replacement?:PageContentReplacement;
 retain():()=>Promise<void>;
 release():Promise<void>;
}
type Kind="uniform"|"styled";
export function supportsOccurrenceParagraph(adapter:FolioAdapter,kind:Kind):boolean {
 return !!adapter.listOccurrenceTextRuns&&(kind==="styled"
  ?!!adapter.inspectOccurrenceStyledParagraph&&!!adapter.suggestOccurrenceStyledParagraph&&!!adapter.previewStyledParagraphOccurrence
  :!!adapter.inspectOccurrenceTextParagraph&&!!adapter.previewTextParagraphOccurrence);
}
export function paragraphEditorKind(adapter:FolioAdapter):Kind|null {
 if(supportsOccurrenceParagraph(adapter,"styled"))return "styled";
 if(supportsOccurrenceParagraph(adapter,"uniform"))return "uniform";
 if(adapter.listTextRuns&&adapter.inspectStyledParagraph&&adapter.suggestStyledParagraph&&adapter.previewStyledParagraph)return "styled";
 if(adapter.listTextRuns&&adapter.inspectTextParagraph&&adapter.previewTextParagraph)return "uniform";
 return null;
}
/** Adapt inspection only; complete-source preview identity remains explicit. */
export function paragraphReadAdapter(adapter:FolioAdapter,occurrence:ParagraphOccurrence|undefined,kind:Kind):FolioAdapter {
 if(!occurrence||!supportsOccurrenceParagraph(adapter,kind))return adapter;
 return {...adapter,listTextRuns:adapter.listOccurrenceTextRuns,
  ...(kind==="styled"?{inspectStyledParagraph:adapter.inspectOccurrenceStyledParagraph,suggestStyledParagraph:adapter.suggestOccurrenceStyledParagraph,fitStyledParagraph:adapter.fitOccurrenceStyledParagraph}
   :{inspectTextParagraph:adapter.inspectOccurrenceTextParagraph,suggestTextParagraph:adapter.suggestOccurrenceTextParagraph})};
}
function retained(occurrence:ParagraphOccurrence|undefined,id:string):boolean {
 return !!occurrence&&(occurrence.document.pages.some(page=>page.sourceId===id)||occurrence.isSourceRetained(id));
}
async function own(adapter:FolioAdapter,occurrence:ParagraphOccurrence|undefined,document:DocumentInfo,replacement?:PageContentReplacement):Promise<ParagraphPreview> {
 const id=document?.id;
 if(typeof id!=="string"||!id||retained(occurrence,id))throw new Error("Paragraph preview must return a new source version.");
 let released=false,closed=false,readers=0;
 const close=async()=>{if(!released||readers||closed)return;closed=true;if(!retained(occurrence,id))await adapter.closeDocument(id).catch(()=>{});};
 const release=async()=>{released=true;await close();};
 const retain=()=>{if(released)throw new Error("Paragraph preview was released.");readers++;let done=false;return async()=>{if(done)return;done=true;readers--;await close();};};
 try {
  let pageIndex=0;
  if(replacement){
   if(!occurrence)throw new Error("The paragraph source selection is unavailable.");
   const pages=replacementPages(occurrence.document,occurrence.page,replacement);
   pageIndex=pages.find(page=>page.id===occurrence.page.id)!.pageIndex;
  }else if(occurrence){
   const size=document.pages?.[0];
   if(document.pages?.length!==1||!size||!Number.isFinite(size.width)||!Number.isFinite(size.height)||Math.abs(size.width-occurrence.page.width)>0.01||Math.abs(size.height-occurrence.page.height)>0.01)throw new Error("Paragraph preview changed page dimensions.");
  }
  return {document,pageIndex,replacement,release,retain};
 }catch(error){await release();throw error;}
}
export async function previewStyledParagraph(adapter:FolioAdapter,occurrence:ParagraphOccurrence|undefined,sourceId:string,pageIndex:number,request:StyledParagraphLayoutRequest):Promise<ParagraphPreview>{
 if(occurrence&&supportsOccurrenceParagraph(adapter,"styled")){
  const bound=pageReplacementRequest(occurrence.document,occurrence.page);
  const result=await adapter.previewStyledParagraphOccurrence!(bound.sourceId,bound.occurrences,bound.selectedOccurrence,request);
  return own(adapter,occurrence,result?.document,result);
 }
 return own(adapter,occurrence,await adapter.previewStyledParagraph!(sourceId,pageIndex,request));
}
export async function previewTextParagraph(adapter:FolioAdapter,occurrence:ParagraphOccurrence|undefined,...args:Parameters<NonNullable<FolioAdapter["previewTextParagraph"]>>):Promise<ParagraphPreview>{
 if(occurrence&&supportsOccurrenceParagraph(adapter,"uniform")){
  const bound=pageReplacementRequest(occurrence.document,occurrence.page);
  const [, , ...rest]=args;
  const result=await adapter.previewTextParagraphOccurrence!(bound.sourceId,bound.occurrences,bound.selectedOccurrence,...rest);
  return own(adapter,occurrence,result?.document,result);
 }
 return own(adapter,occurrence,await adapter.previewTextParagraph!(...args));
}
