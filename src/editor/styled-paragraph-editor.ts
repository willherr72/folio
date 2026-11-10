/** Immutable editing transactions. Native inspection/publication remains the
 * authority for fonts, geometry, source identity and PDF admission. */
import {createStyledParagraph,replaceStyledRange,snapStyledCaret,styledRangeStyle,styledText,type StyledParagraph,type StyledRun} from "./styled-paragraph";
import type {AnnotationRect,ParagraphBaselineAnchor,ParagraphAlignment,StyledParagraphLayoutRequest,StyledParagraphSelection,StyledParagraphStyleChange} from "./types";

export interface StyledEditorRange {readonly start:number;readonly end:number;readonly direction:"forward"|"backward"}
export interface StyledEditorStyle {readonly id:number;readonly sourceStyleId:number;readonly fontSize?:number;readonly color?:string;readonly fontId?:string}
export interface StyledEditorSnapshot {
 readonly draft:StyledParagraph;readonly selection:StyledEditorRange;readonly typingStyleId:number|null;
 readonly styles:readonly StyledEditorStyle[];readonly bounds:Readonly<AnnotationRect>;
 readonly alignment:ParagraphAlignment;readonly lineSpacing:number;
 readonly baselineAnchor?:Readonly<ParagraphBaselineAnchor>;
}
export interface StyledEditorState {
 readonly original:Readonly<StyledParagraphSelection>;readonly present:StyledEditorSnapshot;
 readonly past:readonly StyledEditorSnapshot[];readonly future:readonly StyledEditorSnapshot[];
 /** Monotonic across undo/redo; retained snapshots never alias a reused ID. */
 readonly nextStyleId:number;
 readonly historyGroup:string|null;
}
export interface StyledEditorFormat {fontSize?:number|null;color?:string|null;fontId?:string|null}
export type StyledEditorTransaction=
 | {type:"select";start:number;end:number;direction?:"forward"|"backward"}
 | {type:"replace";text:string;start?:number;end?:number;styleId?:number;historyGroup?:string}
 | {type:"format";patch:StyledEditorFormat}
 | {type:"area";bounds:AnnotationRect;intent?:"fit"|"move"|"resize"}
 | {type:"reset-position"}
 | {type:"paragraph";alignment?:ParagraphAlignment;lineSpacing?:number};
const fail=(message:string):never=>{throw new Error(message);};
const alignments:readonly string[]=["left","center","right","justify"];
function freeze<T>(value:T):T {
 if(value===null||typeof value!=="object")return value;
 const copy=Array.isArray(value)?value.map(freeze):Object.fromEntries(Object.entries(value).map(([key,v])=>[key,freeze(v)]));
 return Object.freeze(copy) as T;
}
function area(bounds:Readonly<AnnotationRect>):AnnotationRect {
 if(![bounds.x,bounds.y,bounds.width,bounds.height].every(Number.isFinite)||bounds.x<0||bounds.y<0||bounds.width<=0||bounds.height<=0)fail("Use a valid paragraph area.");
 return {...bounds};
}
function paragraph(alignment:ParagraphAlignment,lineSpacing:number){
 if(!alignments.includes(alignment)||!Number.isFinite(lineSpacing)||lineSpacing<0.6||lineSpacing>3)fail("Use valid paragraph alignment and line spacing from 0.6 to 3.");
 return {alignment,lineSpacing};
}
function range(draft:StyledParagraph,start:number,end:number,direction:StyledEditorRange["direction"]="forward"):StyledEditorRange {
 styledRangeStyle(draft,start,end);if(direction!=="forward"&&direction!=="backward")fail("Use a valid selection direction.");
 return {start,end,direction};
}
function snapshot(base:Omit<StyledEditorSnapshot,"draft"|"styles">,runs:readonly StyledRun[],definitions:readonly StyledEditorStyle[]):StyledEditorSnapshot {
 if(base.baselineAnchor&&![base.baselineAnchor.x,base.baselineAnchor.y].every(Number.isFinite))fail("Use a finite paragraph baseline.");
 const needed=new Set(runs.map(r=>r.styleId));if(base.typingStyleId!==null)needed.add(base.typingStyleId);
 const styles=definitions.filter(s=>needed.has(s.id));
 if(styles.length!==needed.size||styles.length>64)fail("Use at most 64 paragraph styles.");
 const draft=createStyledParagraph(runs,styles.map(s=>s.id));
 return freeze({...base,bounds:area(base.bounds),...paragraph(base.alignment,base.lineSpacing),selection:range(draft,base.selection.start,base.selection.end,base.selection.direction),draft,styles});
}
export function createStyledEditor(selection:StyledParagraphSelection):StyledEditorState {
 if(!selection.objectIndices.length||selection.objectIndices.length>4096||selection.objectIndices.some((id,i,ids)=>!Number.isSafeInteger(id)||id<0||(i>0&&id<=ids[i-1])))fail("Use a checked paragraph selection.");
 if(!selection.styles.length||selection.styles.length>64||selection.styles.some((style,i)=>style.styleId!==i||!Number.isFinite(style.fontSize)||style.fontSize<1||style.fontSize>300||!/^#[0-9a-f]{6}$/i.test(style.color)||(style.backgroundColor!=null&&(typeof style.backgroundColor!=="string"||!/^#[0-9a-f]{6}$/i.test(style.backgroundColor)))))fail("Use a checked paragraph palette.");
 const draft=createStyledParagraph(selection.runs,selection.styles.map(s=>s.styleId));
 if(styledText(draft)!==selection.text)fail("The checked paragraph text does not match its runs.");
 const styles=selection.styles.map(s=>({id:s.styleId,sourceStyleId:s.styleId}));
 const present=snapshot({selection:{start:0,end:0,direction:"forward"},typingStyleId:null,bounds:selection.bounds,alignment:selection.alignment??"left",lineSpacing:selection.lineSpacing??1.2,...(selection.baselineAnchor?{baselineAnchor:selection.baselineAnchor}:{})},draft.runs,styles);
 return freeze({original:selection,present,past:[],future:[],nextStyleId:styles.length,historyGroup:null});
}
function commit(state:StyledEditorState,present:StyledEditorSnapshot,nextStyleId=state.nextStyleId,historyGroup:string|null=null):StyledEditorState {
 if(JSON.stringify(present)===JSON.stringify(state.present))return state;
 // Snapshots already own frozen data. Reuse them rather than copying the entire
 // history on every keypress; only the new shallow history arrays need freezing.
 const merge=historyGroup!==null&&historyGroup===state.historyGroup&&state.past.length>0&&state.future.length===0;
 return Object.freeze({...state,present,nextStyleId,historyGroup,past:merge?state.past:Object.freeze([...state.past,state.present].slice(-100)),future:Object.freeze([])});
}
function patchStyle(state:StyledEditorState,style:StyledEditorStyle,patch:StyledEditorFormat):Omit<StyledEditorStyle,"id"> {
 if(Object.keys(patch).some(key=>!["fontSize","color","fontId"].includes(key)))fail("Unknown paragraph format property.");
 const {id:_,...result}=style;const original=state.original.styles[style.sourceStyleId];
 if(patch.fontSize!==undefined){
  if(patch.fontSize===null)delete result.fontSize;
  else {if(!Number.isFinite(patch.fontSize)||patch.fontSize<1||patch.fontSize>300)fail("Use a font size from 1 to 300 pt.");result.fontSize=patch.fontSize;}
 }
 if(patch.color!==undefined){
  if(patch.color===null)delete result.color;
  else {if(!/^#[0-9a-f]{6}$/i.test(patch.color))fail("Use a valid text color.");result.color=patch.color.toUpperCase();}
 }
 if(patch.fontId!==undefined){
  if(patch.fontId===null)delete result.fontId;
  else {if(!/^[a-f0-9]{64}$/.test(patch.fontId))fail("Use a registered paragraph font.");result.fontId=patch.fontId;}
 }
 if(result.fontSize===original.fontSize)delete result.fontSize;
 if(result.color===original.color.toUpperCase())delete result.color;
 return result;
}
const key=(s:Omit<StyledEditorStyle,"id">)=>`${s.sourceStyleId}/${s.fontSize??""}/${s.color??""}/${s.fontId??""}`;
export function editStyledEditor(state:StyledEditorState,transaction:StyledEditorTransaction):StyledEditorState {
 const current=state.present;
 if(transaction.type==="select"){
  const selection=range(current.draft,transaction.start,transaction.end,transaction.direction);
  if(JSON.stringify(selection)===JSON.stringify(current.selection))return state;
  const present=snapshot({...current,selection,typingStyleId:null},current.draft.runs,current.styles);
  return Object.freeze({...state,present,historyGroup:null});
 }
 if(transaction.type==="replace"){
  if(transaction.historyGroup!==undefined&&(typeof transaction.historyGroup!=="string"||!transaction.historyGroup.length||transaction.historyGroup.length>128))fail("Use a valid paragraph undo group.");
  if((transaction.start===undefined)!==(transaction.end===undefined))fail("Provide both paragraph range endpoints.");
  const start=transaction.start??current.selection.start,end=transaction.end??current.selection.end;
  const typing=start===end&&start===current.selection.start&&end===current.selection.end?current.typingStyleId:null;
  const draft=replaceStyledRange(current.draft,start,end,transaction.text,transaction.styleId??typing??undefined);
  const offset=snapStyledCaret(draft,start+transaction.text.replace(/\r\n?/g,"\n").length);
  return commit(state,snapshot({...current,selection:{start:offset,end:offset,direction:"forward"},typingStyleId:typing},draft.runs,current.styles),state.nextStyleId,transaction.historyGroup??null);
 }
 if(transaction.type==="reset-position"){
  const original=state.original.baselineAnchor,anchor=current.baselineAnchor;
  // Fit/resize may have changed the frame around an unmoved baseline. Undo
  // the positioning delta while retaining that headroom and the current draft.
  const position=original&&anchor?{x:current.bounds.x+original.x-anchor.x,y:current.bounds.y+original.y-anchor.y}:{x:state.original.bounds.x,y:state.original.bounds.y};
  return commit(state,snapshot({...current,bounds:{...current.bounds,...position},...(original?{baselineAnchor:original}:{})},current.draft.runs,current.styles));
 }
 if(transaction.type==="area"){
  const bounds=area(transaction.bounds);
  if(transaction.intent!==undefined&&!["fit","move","resize"].includes(transaction.intent))fail("Use a valid paragraph area operation.");
  const move=transaction.intent==="move"||(transaction.intent===undefined&&bounds.width===current.bounds.width&&bounds.height===current.bounds.height);
  const baselineAnchor=current.baselineAnchor&&move?{x:current.baselineAnchor.x+bounds.x-current.bounds.x,y:current.baselineAnchor.y+bounds.y-current.bounds.y}:current.baselineAnchor;
  return commit(state,snapshot({...current,bounds,...(baselineAnchor?{baselineAnchor}:{})},current.draft.runs,current.styles));
 }
 if(transaction.type==="paragraph")return commit(state,snapshot({...current,alignment:transaction.alignment??current.alignment,lineSpacing:transaction.lineSpacing??current.lineSpacing},current.draft.runs,current.styles));
 const {start,end}=current.selection;let nextStyleId=state.nextStyleId;const definitions=[...current.styles];
 const derive=(id:number)=>{
  const old=definitions.find(s=>s.id===id)??fail("Unknown paragraph style.");const next=patchStyle(state,old,transaction.patch);const found=definitions.find(s=>key(s)===key(next));
  if(found)return found.id;
  if(!Number.isSafeInteger(nextStyleId+1))fail("The paragraph style identifier limit was reached.");
  const created={...next,id:nextStyleId++};definitions.push(created);return created.id;
 };
 if(start===end){
  const id=derive(current.typingStyleId??styledRangeStyle(current.draft,start,end)!);
  const runs=styledText(current.draft)?current.draft.runs:[{text:"",styleId:id}];
  return commit(state,snapshot({...current,typingStyleId:id},runs,definitions),nextStyleId);
 }
 const runs:StyledRun[]=[];let offset=0;
 for(const run of current.draft.runs){
  const limit=offset+run.text.length,from=Math.max(start,offset),to=Math.min(end,limit);
  if(from<to){
   if(from>offset)runs.push({text:run.text.slice(0,from-offset),styleId:run.styleId});
   runs.push({text:run.text.slice(from-offset,to-offset),styleId:derive(run.styleId)});
   if(to<limit)runs.push({text:run.text.slice(to-offset),styleId:run.styleId});
  }else runs.push(run);
  offset=limit;
 }
 return commit(state,snapshot({...current,typingStyleId:null},runs,definitions),nextStyleId);
}
export function undoStyledEditor(state:StyledEditorState):StyledEditorState {
 const present=state.past.at(-1);if(!present)return state;
 return Object.freeze({...state,present,historyGroup:null,past:Object.freeze(state.past.slice(0,-1)),future:Object.freeze([state.present,...state.future].slice(0,100))});
}
export function redoStyledEditor(state:StyledEditorState):StyledEditorState {
 const present=state.future[0];if(!present)return state;
 return Object.freeze({...state,present,historyGroup:null,past:Object.freeze([...state.past,state.present].slice(-100)),future:Object.freeze(state.future.slice(1))});
}
export function styledEditorRequest(state:StyledEditorState):StyledParagraphLayoutRequest {
 const current=state.present;const used=new Set(current.draft.runs.map(run=>run.styleId));
 // A caret override belongs to future typing. Do not embed or change a PDF
 // style merely because a user chose a font before inserting any text.
 const styles=current.styles.filter(style=>used.has(style.id));const ids=new Map(styles.map((style,index)=>[style.id,index]));
 // Dense targets can reuse any original palette slot. All definitions still
 // resolve from the immutable original palette, so remapping cannot form chains.
 const changes:StyledParagraphStyleChange[]=styles.flatMap((style,styleId)=>{
  const {id:_,...format}=style;
  return styleId===style.sourceStyleId&&Object.keys(format).length===1?[]:[{styleId,...format}];
 });
 return {objectIndices:[...state.original.objectIndices],expectedText:state.original.text,runs:current.draft.runs.map(run=>({text:run.text,styleId:ids.get(run.styleId)!})),
  ...(changes.length?{styleChanges:changes}:{}),...(current.baselineAnchor?{baselineAnchor:{...current.baselineAnchor}}:{}),bounds:{...current.bounds},alignment:current.alignment,lineSpacing:current.lineSpacing};
}
export function styledEditorFontIds(state:StyledEditorState):ReadonlySet<string> {
 return new Set([state.present,...state.past,...state.future].flatMap(s=>s.styles.flatMap(style=>style.fontId?[style.fontId]:[])));
}
