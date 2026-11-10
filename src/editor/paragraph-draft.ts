/** Source-bound transfer into an uncommitted paragraph editor. */
import type {FontInfo} from "./custom-fonts";
import type {EditableTextRun,StyledParagraphSelection} from "./types";
import type {FolioAdapter} from "./adapter";
import {paragraphEditorKind} from "./paragraph-transaction";
import {createStyledEditor,editStyledEditor,type StyledEditorRange,type StyledEditorState} from "./styled-paragraph-editor";
import {snapStyledCaret,styledRangeStyle} from "./styled-paragraph";
export interface ExistingTextDraft {
 readonly originalText:string;readonly replacement:string;
 readonly selection:StyledEditorRange;readonly font:FontInfo|null;
}
export interface ParagraphDraftMapping {
 sourceId:string;pageIndex:number;objectIndices:number[];anchorIndex:number;
 paragraphText:string;sourceText:string;
 sourceStart:number;sourceEnd:number;paragraphStart:number;paragraphEnd:number;
}
export interface PreparedParagraphDraft {readonly kind:"styled";readonly editor:StyledEditorState;readonly font:FontInfo|null}
type MappingOwner=Pick<ParagraphDraftMapping,"sourceId"|"pageIndex"|"objectIndices"|"anchorIndex"|"paragraphText"|"sourceText">;
function fail():never{throw new Error("The paragraph draft could not be matched to the selected source text. Your replacement is still available.");}
const utf8=new TextEncoder(),segmenter=new Intl.Segmenter(undefined,{granularity:"grapheme"});
const edges=(s:string)=>new Set([0,s.length,...Array.from(segmenter.segment(s),p=>p.index)]);
const bounded=(s:unknown):s is string=>typeof s==="string"&&s.length<=4096&&utf8.encode(s).length<=4096;
export function validateParagraphMapping(value:ParagraphDraftMapping,owner:MappingOwner):ParagraphDraftMapping {
 if(!value||value.sourceId!==owner.sourceId||value.pageIndex!==owner.pageIndex||value.anchorIndex!==owner.anchorIndex
  ||value.paragraphText!==owner.paragraphText||value.sourceText!==owner.sourceText
  ||!Array.isArray(value.objectIndices)||!value.objectIndices.length||value.objectIndices.length>4096
  ||JSON.stringify(value.objectIndices)!==JSON.stringify(owner.objectIndices)
  ||value.objectIndices.some((id,i,ids)=>!Number.isSafeInteger(id)||id<0||(i>0&&id<=ids[i-1]))
  ||!value.objectIndices.includes(value.anchorIndex)||!bounded(value.sourceText)||!bounded(value.paragraphText))fail();
 for(const [s,start,end] of [[value.sourceText,value.sourceStart,value.sourceEnd],[value.paragraphText,value.paragraphStart,value.paragraphEnd]] as const){
  if(!Number.isSafeInteger(start)||!Number.isSafeInteger(end)||start<0||end<=start||end>s.length||!edges(s).has(start)||!edges(s).has(end))fail();
 }
 if(value.sourceText.slice(value.sourceStart,value.sourceEnd)!==value.paragraphText.slice(value.paragraphStart,value.paragraphEnd)
  ||value.sourceText.slice(0,value.sourceStart).trim()||value.sourceText.slice(value.sourceEnd).trim())fail();
 return Object.freeze({...value,objectIndices:Object.freeze([...value.objectIndices])}) as ParagraphDraftMapping;
}
interface Change {start:number;end:number;text:string}
/** Bounded grapheme LCS. Equal text keeps styling; ties consume source first. */
function difference(source:string,replacement:string){
 const a=Array.from(segmenter.segment(source)),b=Array.from(segmenter.segment(replacement));
 const stride=b.length+1,table=new Uint16Array((a.length+1)*stride);
 for(let i=a.length-1;i>=0;i--)for(let j=b.length-1;j>=0;j--){table[i*stride+j]=a[i].segment===b[j].segment?table[(i+1)*stride+j+1]+1:Math.max(table[(i+1)*stride+j],table[i*stride+j+1]);}
 const changes:Change[]=[];
 let i=0,j=0,pending:Change|null=null;
 const flush=()=>{if(pending){changes.push(pending);pending=null;}};
 while(i<a.length||j<b.length){
  if(i<a.length&&j<b.length&&a[i].segment===b[j].segment){flush();i++;j++;}
  else{
   pending??={start:a[i]?.index??source.length,end:a[i]?.index??source.length,text:""};
   if(i<a.length&&(j===b.length||table[(i+1)*stride+j]>=table[i*stride+j+1])){pending.end=a[i].index+a[i].segment.length;i++;}
   else{pending.text+=b[j].segment;j++;}
  }
 }
 flush();return {changes};
}
export function transferStyledDraft(selection:StyledParagraphSelection,mapping:ParagraphDraftMapping,draft:ExistingTextDraft):StyledEditorState {
 const map=validateParagraphMapping(mapping,{...mapping,objectIndices:selection.objectIndices,paragraphText:selection.text,sourceText:draft.originalText});
 if(!bounded(draft.originalText)||typeof draft.replacement!=="string"||draft.replacement.length>1000||/[\u0000-\u001f\u007f-\u009f]/.test(draft.replacement)||!draft.replacement.trim())fail();
 for(const c of draft.replacement){const cp=c.codePointAt(0)!;if(cp>=0xd800&&cp<=0xdfff)fail();}
 const input=draft.selection;
 if(!input||!Number.isSafeInteger(input.start)||!Number.isSafeInteger(input.end)||input.start<0||input.end<input.start||input.end>draft.replacement.length||!["forward","backward"].includes(input.direction))fail();
 const initial=createStyledEditor(selection);let editor=initial;
 // Anchor unchanged trimmed padding at the edges before diffing. Otherwise
 // a trailing source space could match the separator before a newly typed word.
 const prefix=draft.originalText.slice(0,map.sourceStart),suffix=draft.originalText.slice(map.sourceEnd);
 let leading=0,trailing=0;
 while(leading<prefix.length&&leading<draft.replacement.length&&prefix[leading]===draft.replacement[leading])leading++;
 while(trailing<suffix.length&&trailing<draft.replacement.length-leading&&suffix[suffix.length-1-trailing]===draft.replacement[draft.replacement.length-1-trailing])trailing++;
 if(!edges(draft.replacement).has(leading)||!edges(draft.replacement).has(draft.replacement.length-trailing))fail();
 const replacement=draft.replacement.slice(leading,draft.replacement.length-trailing);
 const {changes}=difference(draft.originalText.slice(map.sourceStart,map.sourceEnd),replacement);
 const project=(at:number)=>map.paragraphStart+at;
 for(const change of [...changes].reverse()){
  const start=project(change.start),end=project(change.end);
  const hint=start===map.paragraphStart&&start===end
   ?styledRangeStyle(editor.present.draft,start,snapStyledCaret(editor.present.draft,start+1))
   :styledRangeStyle(editor.present.draft,start,end===start?start:snapStyledCaret(editor.present.draft,start+1));
  editor=editStyledEditor(editor,{type:"replace",start,end,text:change.text,...(hint!==null?{styleId:hint}:{})});
 }
 const targetOffset=(at:number)=>Math.max(0,Math.min(replacement.length,at-leading));
 const pieceLength=replacement.length;
 if(draft.font){editor=editStyledEditor(editor,{type:"select",start:map.paragraphStart,end:map.paragraphStart+pieceLength});editor=editStyledEditor(editor,{type:"format",patch:{fontId:draft.font.id}});}
 editor=editStyledEditor(editor,{type:"select",start:snapStyledCaret(editor.present.draft,map.paragraphStart+targetOffset(input.start),"backward"),end:snapStyledCaret(editor.present.draft,map.paragraphStart+targetOffset(input.end)),direction:input.direction});
 return Object.freeze({...editor,past:Object.freeze(changes.length||draft.font?[initial.present]:[]),future:Object.freeze([]),historyGroup:null});
}
export async function prepareParagraphDraft(adapter:FolioAdapter,sourceId:string,pageIndex:number,anchor:EditableTextRun,draft:ExistingTextDraft):Promise<PreparedParagraphDraft>{
 if(draft.originalText!==(anchor.sourceText??anchor.text))fail();
 const kind=paragraphEditorKind(adapter);
 if(kind!=="styled")throw new Error("Keeping a replacement requires the styled paragraph editor. Your replacement is still available.");
 const suggest=adapter.suggestOccurrenceStyledParagraph??adapter.suggestStyledParagraph;
 if(!suggest)fail();
 const result=await suggest(sourceId,pageIndex,anchor.objectIndex,anchor.text);
 // Take ownership before awaiting the second provider; adapter-owned objects
 // may be reused or mutated after their own promise settles.
 const selection=JSON.parse(JSON.stringify(result)) as StyledParagraphSelection;
 createStyledEditor(selection);
 if(!selection.objectIndices.includes(anchor.objectIndex))fail();
 let map:ParagraphDraftMapping;
 if(adapter.mapParagraphDraft){map=await adapter.mapParagraphDraft(sourceId,pageIndex,[...selection.objectIndices],selection.text,anchor.objectIndex,draft.originalText,kind);}
 else {
  if(selection.objectIndices.length!==1)fail();
  const start=draft.originalText.length-draft.originalText.trimStart().length,end=draft.originalText.trimEnd().length;
  if(draft.originalText.slice(start,end)!==selection.text)fail();
  map={sourceId,pageIndex,objectIndices:[...selection.objectIndices],anchorIndex:anchor.objectIndex,paragraphText:selection.text,sourceText:draft.originalText,sourceStart:start,sourceEnd:end,paragraphStart:0,paragraphEnd:selection.text.length};
 }
 map=validateParagraphMapping(map,{sourceId,pageIndex,objectIndices:selection.objectIndices,anchorIndex:anchor.objectIndex,paragraphText:selection.text,sourceText:draft.originalText});
 const editor=transferStyledDraft(selection,map,draft);
 return Object.freeze({kind,editor,font:draft.font});
}
