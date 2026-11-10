import {useLayoutEffect,useRef} from "react";
import {editStyledEditor,undoStyledEditor,redoStyledEditor,type StyledEditorState,type StyledEditorTransaction,type StyledEditorRange} from "../editor/styled-paragraph-editor";
import {snapStyledCaret,styledText} from "../editor/styled-paragraph";
import {restoreStyledDomSelection,styledDomOffset,styledDomSelection} from "../editor/styled-paragraph-dom";
import {fontFamily} from "../editor/custom-fonts";
import "./styled-paragraph-input.css";

interface Props {value:StyledEditorState;onChange(state:StyledEditorState):void;onError(message:string):void;disabled?:boolean}
type Composition={source:StyledEditorState;base:StyledEditorState;range:StyledEditorRange};
/** Native beforeinput gives explicit ranges; browser HTML is never our model.
 * Composition's uncancelable DOM edits remain provisional until compositionend.
 * See https://www.w3.org/TR/input-events-2/#input-event-order-during-composition */
export function StyledParagraphInput({value,onChange,onError,disabled=false}:Props){
 const element=useRef<HTMLDivElement>(null);const state=useRef(value);const callbacks=useRef({onChange,onError,disabled});
 const painted=useRef(false);
 const composition=useRef<Composition|null>(null);const discardedComposition=useRef(false);
 const pending=useRef<{base:StyledEditorState;transaction:StyledEditorTransaction}|null>(null);
 const typing=useRef<{key:string;end:number;at:number}|null>(null);const typingSequence=useRef(0);
 const paint=()=>{
  const root=element.current;if(!root)return;const current=state.current;const doc=root.ownerDocument;
  const nodes=current.present.draft.runs.map(run=>{
   const style=current.present.styles.find(s=>s.id===run.styleId)!;const source=current.original.styles[style.sourceStyleId];const span=doc.createElement("span");
   span.textContent=run.text;span.dataset.styleId=String(run.styleId);span.style.fontSize=`${style.fontSize??source.fontSize}pt`;span.style.color=style.color??source.color;
   span.style.fontFamily=style.fontId?`"${fontFamily(style.fontId)}", sans-serif`:"inherit";span.title=source.fontDisplayName??source.fontName;
   // Background ownership follows the immutable inspected source style, including
   // derived formatting and dense IDs. It is never an editable request property.
   if(source.backgroundColor){span.style.backgroundColor=source.backgroundColor;span.dataset.highlighted="true";span.title+=" — Highlighted";}
   return span;
  });
  // A text-free terminal break lets a caret occupy an empty final line. It adds
  // no model character or clipboard text and is ignored by DOM Range.toString().
  const end=doc.createElement("br");end.dataset.paragraphEnd="true";root.replaceChildren(...nodes,end);painted.current=true;
  if(doc.activeElement===root)restoreStyledDomSelection(root,current.present.selection);
 };
 const publish=(next:StyledEditorState)=>{if(next!==state.current){callbacks.current.onChange(next);state.current=next;}paint();};
 const reject=(error:unknown)=>{pending.current=null;paint();callbacks.current.onError(error instanceof Error?error.message:String(error));};
 useLayoutEffect(()=>{
  callbacks.current={onChange,onError,disabled};
  // Native selectionchange is queued after the browser moves its caret. A
  // callback-only parent render must not repaint that selection from old state,
  // or discard a beforeinput transaction awaiting its native input event.
  if(painted.current&&value===state.current&&!disabled)return;
  if((value!==state.current||disabled)&&composition.current){composition.current=null;discardedComposition.current=true;}
  if(value!==state.current||disabled)typing.current=null;
  state.current=value;pending.current=null;if(!composition.current)paint();
 },[value,onChange,onError,disabled]);
 useLayoutEffect(()=>{
  const root=element.current!;const doc=root.ownerDocument;
  const perform=(transaction:StyledEditorTransaction,base=state.current)=>{try{publish(editStyledEditor(base,transaction));}catch(error){reject(error);}};
  const selected=()=>styledDomSelection(root)??state.current.present.selection;
  const target=(event:InputEvent):StyledEditorRange=>{
   const ranges=event.getTargetRanges?.()??[];if(ranges.length>1)throw new Error("Select one paragraph range at a time.");
   if(ranges.length){const r=ranges[0];return {start:styledDomOffset(root,r.startContainer,r.startOffset),end:styledDomOffset(root,r.endContainer,r.endOffset),direction:"forward"};}
   return selected();
  };
  const before=(raw:Event)=>{
   const event=raw as InputEvent;
   if(callbacks.current.disabled||discardedComposition.current){event.preventDefault();return;}
   if(composition.current||event.isComposing)return;
   try{
    if(event.inputType==="historyUndo"||event.inputType==="historyRedo"){
     event.preventDefault();publish(event.inputType==="historyUndo"?undoStyledEditor(state.current):redoStyledEditor(state.current));return;
    }
    const selection=target(event);let {start,end}=selection;let text:string;
    switch(event.inputType){
     case "insertText":case "insertReplacementText":{
      const data=event.data??event.dataTransfer?.getData("text/plain");if(data===undefined||(event.data===null&&data===""))throw new Error("The browser did not provide the replacement text.");text=data;break;
     }
     case "insertLineBreak":case "insertParagraph":text="\n";break;
     case "insertFromPaste":text=event.dataTransfer?.getData("text/plain")??event.data??"";if(!text){event.preventDefault();return;}break;
     case "deleteContentBackward":case "deleteContentForward":
      if(start===end){if(event.inputType==="deleteContentBackward")start=snapStyledCaret(state.current.present.draft,Math.max(0,start-1),"backward");else end=snapStyledCaret(state.current.present.draft,Math.min(styledText(state.current.present.draft).length,end+1));}text="";break;
     case "deleteWordBackward":case "deleteWordForward":case "deleteSoftLineBackward":case "deleteSoftLineForward":case "deleteHardLineBackward":case "deleteHardLineForward":case "deleteByCut":
      if(start===end)throw new Error("The browser did not provide the text range to delete.");text="";break;
     case "insertCompositionText":return;
     default:throw new Error("Use the paragraph controls for this formatting change.");
    }
    let historyGroup:string|undefined;
    if(event.inputType==="insertText"){
     const previous=typing.current,now=performance.now();historyGroup=previous&&start===end&&start===previous.end&&now-previous.at<=1000?previous.key:`typing-${++typingSequence.current}`;
     typing.current={key:historyGroup,end:start+text.length,at:now};
    }else typing.current=null;
    const transaction:StyledEditorTransaction={type:"replace",start,end,text,...(historyGroup?{historyGroup}:{})};
    if(event.cancelable){event.preventDefault();perform(transaction);}
    else pending.current={base:state.current,transaction};
   }catch(error){event.preventDefault();reject(error);}
  };
  const input=()=>{
   if(composition.current)return;
   if(discardedComposition.current){paint();return;}
   const captured=pending.current;pending.current=null;
   if(captured&&captured.base===state.current){perform(captured.transaction,captured.base);return;}
   if(root.textContent!==styledText(state.current.present.draft))reject(new Error("This browser edit could not be mapped to the paragraph. The draft was kept."));
   else paint(); // Text equality does not authorize browser-created HTML/styles.
  };
  const selectionChange=()=>{
   if(callbacks.current.disabled||composition.current||discardedComposition.current||doc.activeElement!==root)return;
   const next=styledDomSelection(root);if(!next)return;
   try{const changed=editStyledEditor(state.current,{type:"select",...next});if(changed!==state.current){callbacks.current.onChange(changed);state.current=changed;}}
   catch{/* A transient browser selection inside a cluster is validated on edit. */}
  };
  const focus=()=>{if(!composition.current&&!callbacks.current.disabled)restoreStyledDomSelection(root,state.current.present.selection);};
  const paste=(event:ClipboardEvent)=>{
   event.preventDefault();if(callbacks.current.disabled||composition.current)return;
   try{const text=event.clipboardData?.getData("text/plain");if(!text)return;const {start,end}=selected();perform({type:"replace",start,end,text});}catch(error){reject(error);}
  };
  const cut=(event:ClipboardEvent)=>{
   event.preventDefault();if(callbacks.current.disabled||composition.current)return;
   try{const {start,end}=selected();if(start===end)return;const next=editStyledEditor(state.current,{type:"replace",start,end,text:""});
    if(!event.clipboardData)throw new Error("The selected text could not be copied.");
    event.clipboardData.setData("text/plain",styledText(state.current.present.draft).slice(start,end));publish(next);
   }catch(error){reject(error);}
  };
  const keydown=(event:KeyboardEvent)=>{
   if(callbacks.current.disabled||composition.current||event.isComposing)return;
   if((event.ctrlKey||event.metaKey)&&!event.altKey){const key=event.key.toLowerCase();
    if(key==="z"||key==="y"){event.preventDefault();publish(key==="y"||event.shiftKey?redoStyledEditor(state.current):undoStyledEditor(state.current));}
    else if(key==="a"){event.preventDefault();perform({type:"select",start:0,end:styledText(state.current.present.draft).length});}
   }
  };
  const compositionStart=()=>{
   if(callbacks.current.disabled)return;
   try{const range=selected(),source=state.current,base=editStyledEditor(source,{type:"select",...range});composition.current={source,base,range};discardedComposition.current=false;pending.current=null;typing.current=null;}
   catch(error){discardedComposition.current=true;reject(error);}
  };
  const compositionEnd=(event:CompositionEvent)=>{
   const captured=composition.current;composition.current=null;discardedComposition.current=false;
   if(!captured||captured.source!==state.current||callbacks.current.disabled){paint();return;}
   if(!event.data){publish(captured.base);return;}
   perform({type:"replace",start:captured.range.start,end:captured.range.end,text:event.data},captured.base);
  };
  const drop=(event:DragEvent)=>{event.preventDefault();if(!composition.current&&!callbacks.current.disabled)reject(new Error("Paste text to insert it into the paragraph."));};
  root.addEventListener("beforeinput",before);root.addEventListener("input",input);root.addEventListener("paste",paste);root.addEventListener("cut",cut);root.addEventListener("keydown",keydown);
  root.addEventListener("compositionstart",compositionStart);root.addEventListener("compositionend",compositionEnd);root.addEventListener("drop",drop);root.addEventListener("focus",focus);doc.addEventListener("selectionchange",selectionChange);
  return ()=>{composition.current=null;pending.current=null;root.removeEventListener("beforeinput",before);root.removeEventListener("input",input);root.removeEventListener("paste",paste);root.removeEventListener("cut",cut);root.removeEventListener("keydown",keydown);root.removeEventListener("compositionstart",compositionStart);root.removeEventListener("compositionend",compositionEnd);root.removeEventListener("drop",drop);root.removeEventListener("focus",focus);doc.removeEventListener("selectionchange",selectionChange);};
 },[]);
 return <div ref={element} role="textbox" aria-label="Paragraph text" aria-multiline="true" aria-disabled={disabled} className="styled-paragraph-input" contentEditable={!disabled} suppressContentEditableWarning spellCheck={false} tabIndex={0}/>;
}
