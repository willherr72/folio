import {useState} from "react";
import {act,cleanup,fireEvent,render,screen} from "@testing-library/react";
import {afterEach,expect,it,vi} from "vitest";
import {StyledParagraphInput} from "../src/components/StyledParagraphInput";
import {createStyledEditor,editStyledEditor,styledEditorRequest,type StyledEditorState} from "../src/editor/styled-paragraph-editor";
import {styledText} from "../src/editor/styled-paragraph";
import {styledEditorFixture} from "./fixtures/styled-editor-data";
afterEach(cleanup);
function setup(){
 let latest=createStyledEditor(styledEditorFixture());const error=vi.fn();let replace!:(value:StyledEditorState)=>void;
 function Harness(){const [state,setState]=useState(latest);latest=state;replace=setState;return <StyledParagraphInput value={state} onChange={setState} onError={error}/>;}
 const view=render(<Harness/>);const root=screen.getByRole("textbox",{name:"Paragraph text"});root.focus();
 return {root,error,view,state:()=>latest,replace:(state:StyledEditorState)=>act(()=>replace(state))};
}
function point(root:HTMLElement,offset:number):[Node,number]{const walker=document.createTreeWalker(root,NodeFilter.SHOW_TEXT);let node;while((node=walker.nextNode())){if(offset<=node.textContent!.length)return [node,offset];offset-=node.textContent!.length;}return [root,root.childNodes.length];}
function select(root:HTMLElement,start:number,end=start){act(()=>{const a=point(root,start),b=point(root,end),range=document.createRange();range.setStart(...a);range.setEnd(...b);const s=getSelection()!;s.removeAllRanges();s.addRange(range);document.dispatchEvent(new Event("selectionchange"));});}
function input(root:HTMLElement,type:string,data:string|null=null,target?:[number,number]){
 const event=new InputEvent("beforeinput",{bubbles:true,cancelable:true,inputType:type,data});
 if(target){const a=point(root,target[0]),b=point(root,target[1]);Object.defineProperty(event,"getTargetRanges",{value:()=>[{startContainer:a[0],startOffset:a[1],endContainer:b[0],endOffset:b[1]}]});}
 act(()=>root.dispatchEvent(event));return event;
}
it('keeps browser selection awaiting selectionchange across unrelated parent renders',()=>{
 const state=createStyledEditor(styledEditorFixture());const changed=vi.fn();
 const view=render(<StyledParagraphInput value={state} onChange={value=>changed(value)} onError={()=>{}}/>);
 const root=screen.getByRole('textbox',{name:'Paragraph text'});root.focus();
 // Browser keyboard selection is immediate; selectionchange is queued separately.
 const range=document.createRange();range.setStart(...point(root,4));range.setEnd(...point(root,7));
 const selection=getSelection()!;selection.removeAllRanges();selection.addRange(range);
 view.rerender(<StyledParagraphInput value={state} onChange={value=>changed(value)} onError={()=>{}}/>);
 expect(selection.toString()).toBe('two');
 input(root,'deleteContentBackward');
 expect(root.textContent).toBe('one ');expect(changed.mock.calls.at(-1)?.[0].past).toHaveLength(1);
});
it('retains an authorized noncancelable input across an unrelated parent render',()=>{
 const state=createStyledEditor(styledEditorFixture());const changed=vi.fn(),error=vi.fn();
 const view=render(<StyledParagraphInput value={state} onChange={value=>changed(value)} onError={error}/>);
 const root=screen.getByRole('textbox',{name:'Paragraph text'});root.focus();
 const range=document.createRange();range.setStart(...point(root,4));range.setEnd(...point(root,7));
 const selection=getSelection()!;selection.removeAllRanges();selection.addRange(range);
 act(()=>root.dispatchEvent(new InputEvent('beforeinput',{bubbles:true,cancelable:false,inputType:'insertReplacementText',data:'new'})));
 view.rerender(<StyledParagraphInput value={state} onChange={value=>changed(value)} onError={error}/>);
 root.textContent='one new';fireEvent.input(root,{inputType:'insertReplacementText',data:'new'});
 expect(root.textContent).toBe('one new');expect(error).not.toHaveBeenCalled();
 expect(changed.mock.calls.at(-1)?.[0].past).toHaveLength(1);
});
it("uses actual target ranges and keeps unaffected styled text when typing and deleting",()=>{
 const h=setup();select(h.root,0);expect(input(h.root,"insertText","this",[4,7]).defaultPrevented).toBe(true);expect(styledText(h.state().present.draft)).toBe("one this");
 expect(styledEditorRequest(h.state()).runs.map(r=>r.text)).toEqual(["one ","this"]);input(h.root,"deleteContentBackward");expect(h.root.textContent).toBe("one thi");input(h.root,"insertParagraph");expect(h.root.textContent).toBe("one thi\n");expect(h.error).not.toHaveBeenCalled();
});
it("pastes plain text across spans and cuts a selection without accepting HTML",()=>{
 const h=setup();select(h.root,2,6);const getData=vi.fn((type:string)=>type==="text/plain"?"<b>x</b>\r\ny":"<b>wrong</b>");fireEvent.paste(h.root,{clipboardData:{getData}});
 expect(h.root.textContent).toBe("on<b>x</b>\nyo");expect(h.root.querySelector("b")).toBeNull();expect(getData).toHaveBeenCalledWith("text/plain");select(h.root,0,2);const setData=vi.fn();fireEvent.cut(h.root,{clipboardData:{setData}});expect(setData).toHaveBeenCalledWith("text/plain","on");expect(h.root.textContent).toBe("<b>x</b>\nyo");
});
it("keeps selected text when clipboard text or an insertion payload is unavailable",()=>{
 const h=setup();select(h.root,0,3);fireEvent.paste(h.root,{clipboardData:{getData:()=>""}});expect(h.root.textContent).toBe("one two");
 input(h.root,"insertText",null);expect(h.root.textContent).toBe("one two");expect(h.state().past).toHaveLength(0);
});
it("rejects an empty fallback payload and removes browser-created markup even when text matches",()=>{
 const h=setup();select(h.root,0,3);const event=new InputEvent("beforeinput",{bubbles:true,cancelable:true,inputType:"insertText",data:null});Object.defineProperty(event,"dataTransfer",{value:{getData:()=>""}});act(()=>h.root.dispatchEvent(event));expect(h.root.textContent).toBe("one two");
 h.root.innerHTML="<b>one two</b>";fireEvent.input(h.root,{inputType:"formatBold"});expect(h.root.querySelector("b")).toBeNull();expect(h.root.textContent).toBe("one two");expect(h.state().past).toHaveLength(0);
});
it("captures a noncancelable beforeinput transaction and applies it once after the browser mutation",()=>{
 const h=setup();select(h.root,4,7);const event=new InputEvent("beforeinput",{bubbles:true,cancelable:false,inputType:"insertReplacementText",data:"new"});act(()=>h.root.dispatchEvent(event));expect(styledText(h.state().present.draft)).toBe("one two");h.root.textContent="one new";fireEvent.input(h.root,{inputType:"insertReplacementText",data:"new"});expect(h.root.textContent).toBe("one new");expect(h.state().past).toHaveLength(1);expect(h.error).not.toHaveBeenCalled();
});
it("owns keyboard undo and redo and restores the model selection",()=>{
 const h=setup();select(h.root,4,7);input(h.root,"insertText","new");fireEvent.keyDown(h.root,{key:"z",ctrlKey:true});expect(h.root.textContent).toBe("one two");expect(h.state().present.selection).toMatchObject({start:4,end:7});fireEvent.keyDown(h.root,{key:"y",ctrlKey:true});expect(h.root.textContent).toBe("one new");
});
it("commits composition once, keeps provisional DOM, and tolerates a trailing input event",()=>{
 const h=setup();select(h.root,4,7);fireEvent.compositionStart(h.root);
 h.root.textContent="one e";fireEvent.input(h.root,{inputType:"insertCompositionText",data:"e",isComposing:true});expect(styledText(h.state().present.draft)).toBe("one two");expect(h.root.textContent).toBe("one e");
 h.root.textContent="one \u00e9";fireEvent.compositionEnd(h.root,{data:"\u00e9"});fireEvent.input(h.root,{inputType:"insertCompositionText",data:"\u00e9"});expect(h.root.textContent).toBe("one \u00e9");expect(h.state().past).toHaveLength(1);expect(styledEditorRequest(h.state()).runs).toEqual([{text:"one ",styleId:0},{text:"\u00e9",styleId:1}]);expect(h.error).not.toHaveBeenCalled();
});
it("cancels empty composition and discards a composition when the source is replaced",()=>{
 const h=setup();select(h.root,4,7);fireEvent.compositionStart(h.root);h.root.textContent="one transient";fireEvent.compositionEnd(h.root,{data:""});expect(h.root.textContent).toBe("one two");expect(h.state().past).toHaveLength(0);
 fireEvent.compositionStart(h.root);h.root.textContent="old composition";const original=styledEditorFixture();original.text="fresh";original.runs=[{text:"fresh",styleId:0}];h.replace(createStyledEditor(original));fireEvent.compositionEnd(h.root,{data:"stale"});expect(h.root.textContent).toBe("fresh");expect(h.state().past).toHaveLength(0);
});
it("restores the last good draft on invalid insertion or an unmapped browser mutation",()=>{
 const h=setup();select(h.root,1);input(h.root,"insertText","x".repeat(4097));expect(h.root.textContent).toBe("one two");expect(h.state().past).toHaveLength(0);expect(h.error).toHaveBeenCalled();
 h.error.mockClear();h.root.textContent="unauthorized mutation";fireEvent.input(h.root,{inputType:"insertText",data:"mutation"});expect(h.root.textContent).toBe("one two");expect(h.error).toHaveBeenCalled();
});
it("deletes whole graphemes and preserves an empty or trailing-newline input",()=>{
 const h=setup(),source=styledEditorFixture();source.text="a\u0301\n";source.runs=[{text:source.text,styleId:0}];h.replace(createStyledEditor(source));select(h.root,2);input(h.root,"deleteContentBackward");expect(h.root.textContent).toBe("\n");input(h.root,"deleteContentForward");expect(h.root.textContent).toBe("");input(h.root,"insertText","a");expect(h.root.textContent).toBe("a");expect(h.error).not.toHaveBeenCalled();
});

it("shows verified highlight cues through replacement, formatting, undo and dense remapping",()=>{
 const h=setup(),source=styledEditorFixture();source.styles[1]={...source.styles[1],backgroundColor:"#FFFF00"};h.replace(createStyledEditor(source));
 const highlighted=()=>h.root.querySelector<HTMLElement>('[data-highlighted="true"]')!;
 expect(highlighted()?.textContent).toBe("two");expect(highlighted().style.backgroundColor).toBe("rgb(255, 255, 0)");expect(highlighted().title).toContain("Highlighted");
 expect(h.root.querySelector<HTMLElement>('span[data-style-id="0"]')!.style.backgroundColor).toBe("");
 select(h.root,4,7);input(h.root,"insertText","new");expect(highlighted().textContent).toBe("new");
 select(h.root,4,7);h.replace(editStyledEditor(h.state(),{type:"format",patch:{fontSize:18,color:"#123456"}}));expect(highlighted().style.backgroundColor).toBe("rgb(255, 255, 0)");expect(highlighted().style.fontSize).toBe("18pt");
 fireEvent.keyDown(h.root,{key:"z",ctrlKey:true});expect(highlighted().textContent).toBe("new");fireEvent.keyDown(h.root,{key:"z",ctrlKey:true});expect(highlighted().textContent).toBe("two");fireEvent.keyDown(h.root,{key:"y",ctrlKey:true});expect(highlighted().textContent).toBe("new");
 select(h.root,0,4);input(h.root,"deleteContentBackward");expect(h.root.textContent).toBe("new");expect(highlighted().textContent).toBe("new");
 expect(styledEditorRequest(h.state()).styleChanges).toEqual([{styleId:0,sourceStyleId:1}]);expect(JSON.stringify(styledEditorRequest(h.state()))).not.toContain("backgroundColor");expect(h.error).not.toHaveBeenCalled();
});
