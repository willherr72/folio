/** Native-measured geometry for a transferred, still-uncommitted draft. */
import type {FolioAdapter} from "./adapter";
import type {AnnotationRect} from "./types";
import {validParagraphBounds} from "./paragraph-preview";
import {editStyledEditor,styledEditorRequest,type StyledEditorState} from "./styled-paragraph-editor";

export interface ParagraphDraftFitContext {
 readonly width:number;readonly height:number;readonly crop?:Readonly<AnnotationRect>;
 readonly isCurrent?:()=>boolean;
}
export interface ParagraphDraftFitFeedback {
 readonly kind:"fitted"|"unchanged"|"unavailable";readonly message:string;
}
export interface FittedParagraphDraft {readonly editor:StyledEditorState;readonly fitFeedback?:ParagraphDraftFitFeedback}

export function ownParagraphFitContext(context:ParagraphDraftFitContext):ParagraphDraftFitContext {
 return Object.freeze({...context,...(context.crop?{crop:Object.freeze({...context.crop})}:{})});
}
function inside(bounds:AnnotationRect,context:ParagraphDraftFitContext):boolean {
 if(!validParagraphBounds(bounds,context.width,context.height))return false;
 const crop=context.crop;
 return !crop||(validParagraphBounds(crop,context.width,context.height)&&bounds.x>=crop.x&&bounds.y>=crop.y
  &&bounds.x+bounds.width<=crop.x+crop.width&&bounds.y+bounds.height<=crop.y+crop.height);
}
/** A native proposal is part of the current edit, not an extra history step. */
export function applyAutomaticParagraphFit(editor:StyledEditorState,bounds:AnnotationRect):StyledEditorState {
 const fitted=editStyledEditor(editor,{type:"area",bounds,intent:"fit"});
 if(fitted===editor)return editor;
 // Caret-only formatting has its own Undo step but the same PDF request.
 // Bind the proposal to that whole trailing equivalence chain, so undoing a
 // future-typing style cannot restore the edited text's pre-fit rectangle.
 const request=JSON.stringify(styledEditorRequest(editor));let start=editor.past.length;
 while(start>0&&JSON.stringify(styledEditorRequest({...editor,present:editor.past[start-1]}))===request)start--;
 const past=start===editor.past.length?editor.past:Object.freeze(editor.past.map((entry,index)=>index<start?entry
  :editStyledEditor({...editor,present:entry},{type:"area",bounds,intent:"fit"}).present));
 return Object.freeze({...fitted,past,future:editor.future,historyGroup:editor.historyGroup});
}
export function fitTransferredParagraphDraft(adapter:FolioAdapter,sourceId:string,pageIndex:number,editor:StyledEditorState,context:ParagraphDraftFitContext):Promise<FittedParagraphDraft> {
 return fitAutomaticParagraphDraft(adapter,sourceId,pageIndex,editor,context,"transferred");
}
export function fitEditedParagraphDraft(adapter:FolioAdapter,sourceId:string,pageIndex:number,editor:StyledEditorState,context:ParagraphDraftFitContext):Promise<FittedParagraphDraft> {
 return fitAutomaticParagraphDraft(adapter,sourceId,pageIndex,editor,context,"edited");
}
async function fitAutomaticParagraphDraft(adapter:FolioAdapter,sourceId:string,pageIndex:number,editor:StyledEditorState,context:ParagraphDraftFitContext,kind:"transferred"|"edited"):Promise<FittedParagraphDraft> {
 const fit=adapter.fitOccurrenceStyledParagraph??adapter.fitStyledParagraph;
 if(!fit||!editor.past.length||context.isCurrent?.()===false)return Object.freeze({editor});
 const owned=ownParagraphFitContext(context);
 try{
  if(![owned.width,owned.height].every(value=>Number.isFinite(value)&&value>0)
   ||(owned.crop&&!validParagraphBounds(owned.crop,owned.width,owned.height)))throw new Error("The visible page geometry could not be verified.");
  const result=await fit(sourceId,pageIndex,styledEditorRequest(editor));
  if(owned.isCurrent?.()===false)return Object.freeze({editor});
  const bounds={x:result?.x,y:result?.y,width:result?.width,height:result?.height};
  if(!inside(bounds,owned))throw new Error("The fitted area would extend outside the visible page.");
  const next=applyAutomaticParagraphFit(editor,bounds);
  return Object.freeze({editor:next,fitFeedback:Object.freeze({kind:next===editor?"unchanged":"fitted",
   message:kind==="transferred"?(next===editor?"The existing area fits the transferred text. Preview before applying.":"Area fitted to the transferred text. Preview before applying.")
    :(next===editor?"The current area fits the edited text.":"Area fitted to the edited text. Preview the paragraph before applying.")})});
 }catch(error){
  if(owned.isCurrent?.()===false)return Object.freeze({editor});
  const detail=error instanceof Error?error.message:typeof error==="string"?error:"Automatic paragraph fitting is unavailable.";
  return Object.freeze({editor,fitFeedback:Object.freeze({kind:"unavailable",message:`${detail} Your ${kind} text is preserved; adjust the text, font, or area, then use Fit area to text.`})});
 }
}
