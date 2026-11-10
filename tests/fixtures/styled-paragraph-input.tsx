import {useEffect,useRef,useState} from "react";
import {createRoot} from "react-dom/client";
import {StyledParagraphInput} from "../../src/components/StyledParagraphInput";
import {createStyledEditor,editStyledEditor,undoStyledEditor,redoStyledEditor,styledEditorRequest,type StyledEditorState,type StyledEditorTransaction} from "../../src/editor/styled-paragraph-editor";
import {styledEditorFixture} from "./styled-editor-data";
import type {StyledParagraphSelection} from "../../src/editor/types";
declare global {interface Window {styledInputAcceptance:{get():StyledEditorState;reset(selection?:StyledParagraphSelection):void;edit(transaction:StyledEditorTransaction):void;request():ReturnType<typeof styledEditorRequest>;errors():string[]}}}
function Fixture(){
 const [value,setValue]=useState(()=>createStyledEditor(styledEditorFixture())),[errors,setErrors]=useState<string[]>([]);const current=useRef(value),messages=useRef(errors);current.current=value;messages.current=errors;
 useEffect(()=>{window.styledInputAcceptance={get:()=>current.current,reset:selection=>{setValue(createStyledEditor(selection??styledEditorFixture()));setErrors([]);},edit:transaction=>setValue(state=>editStyledEditor(state,transaction)),request:()=>styledEditorRequest(current.current),errors:()=>messages.current};},[]);
 return <main style={{maxWidth:760,margin:"2rem auto",padding:"1rem",fontFamily:"system-ui"}}><h1>Paragraph editing</h1><p>Select text to change its size or color. Typing preserves the other styles.</p><div style={{display:"flex",gap:8,marginBottom:16,flexWrap:"wrap"}}>
  <button onClick={()=>setValue(undoStyledEditor)}>Undo</button><button onClick={()=>setValue(redoStyledEditor)}>Redo</button>
  <button onClick={()=>setValue(state=>editStyledEditor(state,{type:"format",patch:{fontSize:16}}))}>16 pt</button><button onClick={()=>setValue(state=>editStyledEditor(state,{type:"format",patch:{color:"#A02233"}}))}>Red</button>
  <button onClick={()=>window.styledInputAcceptance.reset()}>Reset</button></div><StyledParagraphInput value={value} onChange={setValue} onError={message=>setErrors(old=>[...old,message])}/>
  {errors.length>0&&<p role="alert">{errors.at(-1)}</p>}<pre style={{whiteSpace:"pre-wrap",overflowWrap:"anywhere"}} aria-label="Draft state">{JSON.stringify(styledEditorRequest(value),null,2)}</pre></main>;
}
createRoot(document.getElementById("root")!).render(<Fixture/>);
