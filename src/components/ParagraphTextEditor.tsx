import {StyledParagraphInput} from "./StyledParagraphInput";
import {useEffect,useLayoutEffect,useMemo,useRef,useState} from "react";
import {editStyledEditor,type StyledEditorState} from "../editor/styled-paragraph-editor";
import {styledText} from "../editor/styled-paragraph";
import {restoreStyledDomSelection} from "../editor/styled-paragraph-dom";
import {findParagraphMatches,nextParagraphMatch,paragraphFindText,type ParagraphMatch} from "../editor/paragraph-find";
interface Props {value:StyledEditorState;onChange(value:StyledEditorState):void;onError(message:string):void;onPendingInput?(pending:boolean):void;disabled?:boolean;busy?:boolean}
/** Search owns only the current immutable draft; PDF admission remains native. */
export function ParagraphTextEditor({value,onChange,onError,onPendingInput,disabled=false,busy=false}:Props){
 const [open,setOpen]=useState(false),[query,setQuery]=useState(""),[replacement,setReplacement]=useState("");
 const [matchCase,setMatchCase]=useState(false),[wholeWords,setWholeWords]=useState(false),[error,setError]=useState<string|null>(null);
 const host=useRef<HTMLDivElement>(null),input=useRef<HTMLDivElement>(null),find=useRef<HTMLTextAreaElement>(null),findFocus=useRef(false);
 const latest=useRef(value);latest.current=value;
 const text=styledText(value.present.draft),selection=value.present.selection;
 const result=useMemo(()=>{try{return {matches:findParagraphMatches(text,query,{matchCase,wholeWords}),error:null};}catch(error){return {matches:[] as readonly ParagraphMatch[],error:error instanceof Error?error.message:String(error)};}},[text,query,matchCase,wholeWords]);
 const matches=result.matches,index=matches.findIndex(match=>match.start===selection.start&&match.end===selection.end);
 const focusDraft=()=>requestAnimationFrame(()=>{const root=input.current?.querySelector<HTMLElement>('[role="textbox"]');if(root){const range=latest.current.present.selection;root.focus();restoreStyledDomSelection(root,range);}});
 const show=()=>{
  if(disabled)return;
  if(!open&&selection.start!==selection.end)setQuery(text.slice(selection.start,selection.end));
  setOpen(true);findFocus.current=true;if(find.current){find.current.focus();find.current.select();findFocus.current=false;}
 };
 const close=()=>{setOpen(false);focusDraft();};
 useLayoutEffect(()=>{if(open&&findFocus.current&&find.current){findFocus.current=false;find.current.focus();find.current.select();}},[open]);
 useEffect(()=>{setOpen(false);setQuery("");setReplacement("");setError(null);setMatchCase(false);setWholeWords(false);},[value.original]);
 useEffect(()=>setError(null),[query,replacement,matchCase,wholeWords,value.present.draft]);
 const navigate=(direction:1|-1)=>{
  if(disabled)return;
  const match=nextParagraphMatch(matches,selection,direction);if(match){onChange(editStyledEditor(value,{type:"select",...match}));focusDraft();}
 };
 const replace=(all:boolean)=>{
  if(disabled||busy||result.error||(!all&&index<0)||!matches.length)return;
  try{
   const insert=paragraphFindText(replacement),ranges=all?matches:[matches[index]];
   let next=editStyledEditor(value,{type:"replace-many",expectedText:text,ranges,text:insert});
   if(!all){const remaining=findParagraphMatches(styledText(next.present.draft),query,{matchCase,wholeWords});const match=nextParagraphMatch(remaining,next.present.selection,1);if(match)next=editStyledEditor(next,{type:"select",...match});}
   onChange(next);setError(null);focusDraft();
  }catch(error){setError(error instanceof Error?error.message:String(error));}
 };
 const keydown=(event:KeyboardEvent)=>{
  if(event.defaultPrevented||event.isComposing||event.keyCode===229||disabled)return;
  const key=event.key.toLowerCase();
  if((event.ctrlKey||event.metaKey)&&!event.altKey&&(key==="f"||key==="h")){event.preventDefault();event.stopPropagation();show();}
  else if(open&&event.key==="Escape"){event.preventDefault();event.stopPropagation();close();}
  else if(open&&((event.key==="F3"&&!event.ctrlKey&&!event.metaKey&&!event.altKey)||(event.key==="Enter"&&event.target===find.current))){event.preventDefault();event.stopPropagation();navigate(event.shiftKey?-1:1);}
 };
 useLayoutEffect(()=>{
  const scope=host.current?.closest<HTMLElement>('.paragraph-workspace-panel, [role="dialog"]')??host.current;if(!scope)return;
  // Formatting/position controls share this owner. Portals live outside its DOM
  // scope. Rebind at each commit so native events use render-current search state.
  scope.addEventListener("keydown",keydown,true);return()=>scope.removeEventListener("keydown",keydown,true);
 });
 const status=!query?"Enter text to find in this paragraph.":matches.length?index>=0?`${index+1} of ${matches.length} matches`:`${matches.length} ${matches.length===1?"match":"matches"}`:"No matches";
 return <div ref={host} className="paragraph-text-editor">
  <button className="button" disabled={disabled} aria-expanded={open} onClick={show}>Find and replace…</button>
  {open&&<section className="paragraph-find" aria-label="Find and replace in paragraph">
   <strong>Current paragraph</strong>
   <label>Find in paragraph<textarea ref={find} rows={1} value={query} disabled={disabled} onChange={event=>setQuery(event.target.value)}/></label>
   <div className="paragraph-find-options"><label><input type="checkbox" disabled={disabled} checked={matchCase} onChange={event=>setMatchCase(event.target.checked)}/>Match case</label><label><input type="checkbox" disabled={disabled} checked={wholeWords} onChange={event=>setWholeWords(event.target.checked)}/>Whole words</label></div>
   <div className="paragraph-selection-actions"><button className="button" disabled={disabled||!matches.length||!!result.error} onClick={()=>navigate(-1)}>Previous match</button><button className="button" disabled={disabled||!matches.length||!!result.error} onClick={()=>navigate(1)}>Next match</button></div>
   <label>Replace with in paragraph<textarea rows={2} value={replacement} disabled={disabled} onChange={event=>setReplacement(event.target.value)}/></label>
   <div className="paragraph-selection-actions"><button className="button" disabled={disabled||busy||index<0||!!result.error} onClick={()=>replace(false)}>Replace</button><button className="button" disabled={disabled||busy||!matches.length||!!result.error} onClick={()=>replace(true)}>Replace all</button><button className="button" onClick={close}>Close find</button></div>
   <p role="status">{status}</p>{(result.error||error)&&<p role="alert" className="existing-text-error">{result.error||error}</p>}
  </section>}
  <div ref={input}><StyledParagraphInput value={value} onChange={onChange} onError={onError} onPendingInput={onPendingInput} disabled={disabled}/></div>
 </div>;
}
