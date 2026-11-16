/** Internal mixed-paragraph draft model. Style IDs come from a checked native
 * palette; this model does not establish PDF/font admission or render HTML. */
export interface StyledRun { readonly text:string; readonly styleId:number }
export interface StyledParagraph { readonly runs:readonly StyledRun[]; readonly styleIds:readonly number[] }
const MAX_BYTES=4096,MAX_STYLES=64;
const encoder=new TextEncoder();
const segmenter=new Intl.Segmenter(undefined,{granularity:"grapheme"});
const fail=(message:string):never=>{throw new Error(message);};
const validId=(id:number)=>Number.isSafeInteger(id)&&id>=0;
function checkText(text:string){
  if(text.length>MAX_BYTES||encoder.encode(text).length>MAX_BYTES)fail("Use at most 4096 UTF-8 bytes for a paragraph.");
  for(const c of text){const value=c.codePointAt(0)!;if(value>=0xd800&&value<=0xdfff)fail("Use well-formed Unicode text.");}
  if(text.includes("\r"))fail("Use LF line breaks in the paragraph draft.");
}
function boundaries(text:string){return new Set([0,text.length,...Array.from(segmenter.segment(text),part=>part.index)]);}
function checkStyle(doc:StyledParagraph,styleId:number){if(!validId(styleId)||!doc.styleIds.includes(styleId))fail("Unknown paragraph style.");}
export function styledText(doc:StyledParagraph):string{return doc.runs.map(run=>run.text).join("");}

/** Copy and freeze: history snapshots never share mutable caller-owned runs. */
export function createStyledParagraph(runs:readonly StyledRun[],styleIds:readonly number[],emptyStyle=runs[0]?.styleId??styleIds[0]):StyledParagraph {
  if(!styleIds.length||styleIds.length>MAX_STYLES||new Set(styleIds).size!==styleIds.length||!styleIds.every(validId))fail("Use a unique paragraph palette of 1 to 64 styles.");
  if(!styleIds.includes(emptyStyle))fail("Unknown empty paragraph style.");
  if(runs.length>MAX_BYTES)fail("The paragraph has too many style runs.");
  const merged:{text:string;styleId:number}[]=[];let length=0;
  for(const run of runs){
    if(!validId(run.styleId)||!styleIds.includes(run.styleId))fail("Unknown paragraph style.");
    length+=run.text.length;if(length>MAX_BYTES)fail("Use at most 4096 UTF-8 bytes for a paragraph.");
    if(!run.text)continue;
    const last=merged.at(-1);if(last?.styleId===run.styleId)last.text+=run.text;else merged.push({...run});
  }
  const text=merged.map(run=>run.text).join("");checkText(text);
  const edges=boundaries(text);let offset=0;
  for(const run of merged){if(!edges.has(offset))fail("A style boundary must not split a grapheme.");offset+=run.text.length;}
  if(!merged.length)merged.push({text:"",styleId:emptyStyle});
  return Object.freeze({runs:Object.freeze(merged.map(run=>Object.freeze(run))),styleIds:Object.freeze([...styleIds])});
}
function checkEndpoints(text:string,edges:ReadonlySet<number>,start:number,end:number){
  if(!Number.isInteger(start)||!Number.isInteger(end)||start<0||end<start||end>text.length)fail("Select a valid paragraph range.");
  if(!edges.has(start)||!edges.has(end))fail("A paragraph range must not split a grapheme.");
}
function checkRange(doc:StyledParagraph,start:number,end:number){
  const text=styledText(doc);checkEndpoints(text,boundaries(text),start,end);
}
function sliceRuns(doc:StyledParagraph,start:number,end:number):StyledRun[]{
  const runs:StyledRun[]=[];let offset=0;
  for(const run of doc.runs){const limit=offset+run.text.length,from=Math.max(start,offset),to=Math.min(end,limit);if(from<to)runs.push({text:run.text.slice(from-offset,to-offset),styleId:run.styleId});offset=limit;}
  return runs;
}
function inheritedStyle(doc:StyledParagraph,start:number,end:number):number {
  const position=start===end?Math.max(0,start-1):start;let offset=0;
  for(const run of doc.runs){offset+=run.text.length;if(position<offset)return run.styleId;}
  return doc.runs.at(-1)!.styleId;
}
/** Null means mixed; a collapsed range reports the inherited typing style. */
export function styledRangeStyle(doc:StyledParagraph,start:number,end:number):number|null {
  checkRange(doc,start,end);if(start===end)return inheritedStyle(doc,start,end);
  const styles=new Set(sliceRuns(doc,start,end).map(run=>run.styleId));return styles.size===1?[...styles][0]:null;
}
/** Insertion may join surrounding graphemes. Snap the arithmetic post-edit caret
 * before querying styles or submitting another edit; never split the new cluster. */
export function snapStyledCaret(doc:StyledParagraph,offset:number,direction:"forward"|"backward"="forward"):number {
  const text=styledText(doc);
  if(!Number.isInteger(offset)||offset<0||offset>text.length)fail("Select a valid paragraph range.");
  const edges=[...boundaries(text)].sort((a,b)=>a-b);
  return direction==="forward"?edges.find(edge=>edge>=offset)!:edges.reverse().find(edge=>edge<=offset)!;
}
export function replaceStyledRange(doc:StyledParagraph,start:number,end:number,text:string,styleId?:number):StyledParagraph {
  checkRange(doc,start,end);const chosen=styleId??inheritedStyle(doc,start,end);checkStyle(doc,chosen);
  // A CRLF pair contracts to one LF. Bound raw work without rejecting a paste
  // whose normalized form fits; checkText and the constructor enforce byte limits.
  if(text.length>MAX_BYTES*2)fail("Use at most 4096 UTF-8 bytes for a paragraph.");
  text=text.replace(/\r\n?/g,"\n");checkText(text);
  return createStyledParagraph([...sliceRuns(doc,0,start),{text,styleId:chosen},...sliceRuns(doc,end,styledText(doc).length)],doc.styleIds,chosen);
}
/** Compose final runs from original coordinates; no partially edited snapshot. */
export function replaceStyledRanges(doc:StyledParagraph,ranges:readonly {start:number;end:number}[],text:string):StyledParagraph {
 if(ranges.length>MAX_BYTES)fail("Use at most 4096 paragraph matches.");
 let previous=0;const original=styledText(doc),edges=boundaries(original);
 for(const item of ranges){checkEndpoints(original,edges,item.start,item.end);if(item.start===item.end||item.start<previous)fail("Use ordered nonoverlapping paragraph matches.");previous=item.end;}
 if(text.length>MAX_BYTES*2)fail("Use at most 4096 UTF-8 bytes for a paragraph.");
 text=text.replace(/\r\n?/g,"\n");checkText(text);
 if(/[\u0000-\u0009\u000b-\u001f\u007f-\u009f]/.test(text))fail("Use plain text and line breaks.");
 const changes=ranges.filter(item=>original.slice(item.start,item.end)!==text);
 if(!changes.length)return doc;
 const insertedBytes=encoder.encode(text).length;
 const finalBytes=encoder.encode(original).length+changes.reduce((sum,item)=>sum+insertedBytes-encoder.encode(original.slice(item.start,item.end)).length,0);
 if(finalBytes>MAX_BYTES)fail("Use at most 4096 UTF-8 bytes for a paragraph.");
 const runs:StyledRun[]=[];
 const append=(run:StyledRun)=>{if(!run.text)return;const last=runs.at(-1);if(last?.styleId===run.styleId)runs[runs.length-1]={text:last.text+run.text,styleId:run.styleId};else runs.push(run);};
 let consumed=0,runIndex=0,runStart=0,emptyStyle=doc.runs[0].styleId;
 const advance=(to:number,copy:boolean)=>{
  while(consumed<to){const run=doc.runs[runIndex],end=runStart+run.text.length,limit=Math.min(end,to);if(copy)append({text:run.text.slice(consumed-runStart,limit-runStart),styleId:run.styleId});consumed=limit;if(consumed===end){runStart=end;runIndex++;}}
 };
 for(const [index,item]of changes.entries()){
  advance(item.start,true);const styleId=doc.runs[runIndex].styleId;if(index===0)emptyStyle=styleId;
  advance(item.end,false);append({text,styleId});
 }
 advance(original.length,true);
 return createStyledParagraph(runs,doc.styleIds,emptyStyle);
}
/** Nonempty selections change style. A caret's typing override belongs to the
 * editor; only a completely empty draft stores that style in its sole empty run. */
export function formatStyledRange(doc:StyledParagraph,start:number,end:number,styleId:number):StyledParagraph {
  checkRange(doc,start,end);checkStyle(doc,styleId);const text=styledText(doc);
  if(start===end)return text?doc:createStyledParagraph([],doc.styleIds,styleId);
  return createStyledParagraph([...sliceRuns(doc,0,start),{text:text.slice(start,end),styleId},...sliceRuns(doc,end,text.length)],doc.styleIds,styleId);
}
