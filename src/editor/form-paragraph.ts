import type {FolioAdapter} from "./adapter";
import type {EditableTextRun} from "./types";

export function supportsFormParagraph(adapter:FolioAdapter):boolean {
 return !!adapter.listFormParagraphRuns&&!!adapter.inspectFormStyledParagraph&&!!adapter.suggestFormStyledParagraph
  &&!!adapter.fitFormStyledParagraph&&!!adapter.previewFormStyledParagraphOccurrence;
}
const copyRun=(run:EditableTextRun):EditableTextRun=>({...run,bounds:{...run.bounds}});
/** Keep original occurrence authority; projected source IDs never enter the workspace. */
export async function prepareFormParagraph(adapter:FolioAdapter,sourceId:string,pageIndex:number,anchor:EditableTextRun):Promise<{adapter:FolioAdapter;anchor:EditableTextRun}> {
 if(!supportsFormParagraph(adapter))throw new Error("Reusable-object paragraph editing is unavailable in this adapter.");
 const path=[...(anchor.objectPath??[])],expected=anchor.sourceText??anchor.text;
 if(path.length<2||path.length>13||path.some(index=>!Number.isSafeInteger(index)||index<0))throw new Error("The reusable text selection is invalid.");
 const result=await adapter.listFormParagraphRuns!(sourceId,pageIndex,[...path],expected);
 if(!result||!Number.isSafeInteger(result.anchorIndex)||!Array.isArray(result.runs)||!result.runs.length||result.runs.length>5000)throw new Error("The paragraph mapping is invalid.");
 let previous=-1;
 for(const run of result.runs){
  const b=run?.bounds;
  if(!run||!Number.isSafeInteger(run.objectIndex)||run.objectIndex<=previous||run.objectPath!==undefined
   ||typeof run.text!=="string"||!b||![b.x,b.y,b.width,b.height].every(Number.isFinite)||b.width<0||b.height<=0)throw new Error("The paragraph mapping is invalid.");
  previous=run.objectIndex;
 }
 const mapped=result.runs.find(run=>run.objectIndex===result.anchorIndex);
 if(!mapped||(mapped.sourceText??mapped.text)!==expected)throw new Error("The paragraph text changed before preparation.");
 const runs=result.runs.map(copyRun);
 const check=(source:string,page:number)=>{if(source!==sourceId||page!==pageIndex)throw new Error("The paragraph source selection changed.");};
 const bound:FolioAdapter={...adapter,
  mapParagraphDraft:adapter.mapFormParagraphDraft?async(source,page,indices,text,index,expected,kind)=>{check(source,page);if(kind!=="styled")throw new Error("Reusable paragraphs require styled editing.");return adapter.mapFormParagraphDraft!(source,page,[...path],indices,text,index,expected);}:undefined,
  listOccurrenceTextRuns:async(source,page)=>{check(source,page);return {runs:runs.map(copyRun)};},
  inspectOccurrenceStyledParagraph:async(source,page,indices)=>{check(source,page);return adapter.inspectFormStyledParagraph!(source,page,[...path],indices);},
  suggestOccurrenceStyledParagraph:async(source,page,index,text)=>{check(source,page);return adapter.suggestFormStyledParagraph!(source,page,[...path],index,text);},
  fitOccurrenceStyledParagraph:async(source,page,request)=>{check(source,page);return adapter.fitFormStyledParagraph!(source,page,[...path],request);},
  previewStyledParagraphOccurrence:async(source,occurrences,selected,request)=>{
   check(source,Number.isSafeInteger(selected)&&selected>=0?occurrences[selected]:NaN);
   return adapter.previewFormStyledParagraphOccurrence!(source,[...occurrences],selected,[...path],request);
  },
 };
 return {adapter:bound,anchor:copyRun(mapped)};
}
