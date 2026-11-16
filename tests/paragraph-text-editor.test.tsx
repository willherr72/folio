import {useState} from "react";
import {act,cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {afterEach,expect,it,vi} from "vitest";
import {ParagraphTextEditor} from "../src/components/ParagraphTextEditor";
import {createStyledEditor,editStyledEditor,type StyledEditorState} from "../src/editor/styled-paragraph-editor";
import {styledText} from "../src/editor/styled-paragraph";
import {styledEditorFixture} from "./fixtures/styled-editor-data";
import {Modal} from "../src/components/Modal";
afterEach(cleanup);
function setup(text="one two one",busy=false,disabled=false){
 const source=styledEditorFixture();source.text=text;source.runs=[{text,styleId:0}];let latest=createStyledEditor(source);let update!:(state:StyledEditorState)=>void;const outer=vi.fn();
 function Harness(){const [state,setState]=useState(latest);latest=state;update=setState;return <div className="paragraph-workspace-panel" onKeyDown={outer}><ParagraphTextEditor value={state} onChange={setState} onError={()=>{}} busy={busy} disabled={disabled}/><input aria-label="Sibling font size" defaultValue="18"/><select aria-label="Sibling alignment"><option>Left</option><option>Right</option></select><button>Sibling paragraph control</button></div>;}
 const view=render(<Harness/>);const root=screen.getByRole("textbox",{name:"Paragraph text"});root.focus();return {root,view,state:()=>latest,update:(state:StyledEditorState)=>act(()=>update(state)),outer};
}
const open=()=>fireEvent.click(screen.getByRole("button",{name:"Find and replace…"}));
const query=(value:string)=>fireEvent.change(screen.getByRole("textbox",{name:"Find in paragraph"}),{target:{value}});
const replacement=(value:string)=>fireEvent.change(screen.getByRole("textbox",{name:"Replace with in paragraph"}),{target:{value}});
const click=(name:string)=>fireEvent.click(screen.getByRole("button",{name}));
it("opens a clearly scoped panel with keyboard and selected text, without history",()=>{
 const h=setup();h.update(editStyledEditor(h.state(),{type:"select",start:4,end:7}));fireEvent.keyDown(h.root,{key:"f",ctrlKey:true});
 expect(screen.getByText("Current paragraph")).toBeInTheDocument();expect(screen.getByRole("textbox",{name:"Find in paragraph"})).toHaveValue("two");expect(h.state().past).toHaveLength(0);expect(h.outer).not.toHaveBeenCalled();
});
it("opens Find from a sibling numeric field with the current paragraph selection",()=>{
 const h=setup();h.update(editStyledEditor(h.state(),{type:"select",start:4,end:7}));const before=h.state();const field=screen.getByRole("textbox",{name:"Sibling font size"});field.focus();
 expect(fireEvent.keyDown(field,{key:"f",ctrlKey:true})).toBe(false);
 expect(screen.getByRole("textbox",{name:"Find in paragraph"})).toHaveValue("two");expect(screen.getByRole("textbox",{name:"Find in paragraph"})).toHaveFocus();expect(h.state()).toBe(before);expect(h.outer).not.toHaveBeenCalled();
});
it("opens Replace from a sibling alignment control without changing text or history",()=>{
 const h=setup();const before=h.state();const field=screen.getByRole("combobox",{name:"Sibling alignment"});field.focus();
 expect(fireEvent.keyDown(field,{key:"h",ctrlKey:true})).toBe(false);expect(screen.getByRole("textbox",{name:"Replace with in paragraph"})).toBeInTheDocument();expect(h.state()).toBe(before);expect(field).toHaveValue("Left");expect(h.outer).not.toHaveBeenCalled();
});
it("navigates once from sibling controls with render-current query and selection",()=>{
 const h=setup();open();query("one");const sibling=screen.getByRole("button",{name:"Sibling paragraph control"});sibling.focus();
 expect(fireEvent.keyDown(sibling,{key:"F3"})).toBe(false);expect(h.state().present.selection).toMatchObject({start:0,end:3});
 sibling.focus();fireEvent.keyDown(sibling,{key:"F3"});expect(h.state().present.selection).toMatchObject({start:8,end:11});
 sibling.focus();fireEvent.keyDown(sibling,{key:"F3",shiftKey:true});expect(h.state().present.selection).toMatchObject({start:0,end:3});
 query("two");const alignment=screen.getByRole("combobox",{name:"Sibling alignment"});alignment.focus();fireEvent.keyDown(alignment,{key:"F3"});expect(h.state().present.selection).toMatchObject({start:4,end:7});expect(h.state().past).toHaveLength(0);expect(h.outer).not.toHaveBeenCalled();
});
it("leaves modified F3, composition and already prevented keys with their owner",()=>{
 const h=setup();open();query("one");const before=h.state();
 for(const modifier of [{ctrlKey:true},{metaKey:true},{altKey:true}])expect(fireEvent.keyDown(h.root,{key:"F3",...modifier})).toBe(true);
 const field=screen.getByRole("textbox",{name:"Sibling font size"});field.focus();fireEvent.keyDown(field,{key:"F3",isComposing:true});fireEvent.keyDown(field,{key:"F3",keyCode:229});
 const event=new KeyboardEvent("keydown",{key:"F3",bubbles:true,cancelable:true});event.preventDefault();act(()=>field.dispatchEvent(event));expect(h.state()).toBe(before);
});
it("does not open Find during disabled gestures from sibling controls",()=>{
 setup(undefined,false,true);const field=screen.getByRole("textbox",{name:"Sibling font size"});field.focus();expect(fireEvent.keyDown(field,{key:"f",ctrlKey:true})).toBe(true);expect(screen.queryByRole("textbox",{name:"Find in paragraph"})).toBeNull();
});
it("removes the scope listener when the editor unmounts but its workspace stays",()=>{
 const h=setup();open();h.view.rerender(<div className="paragraph-workspace-panel" onKeyDown={h.outer}><button>Remaining sibling</button></div>);h.outer.mockClear();
 expect(fireEvent.keyDown(screen.getByRole("button",{name:"Remaining sibling"}),{key:"f",ctrlKey:true})).toBe(true);expect(h.outer).toHaveBeenCalledTimes(1);
});
it("uses a modal scope for shortcuts from sibling fields",()=>{
 const source=createStyledEditor(styledEditorFixture()),changed=vi.fn();render(<Modal title="Paragraph owner" onClose={()=>{}}><ParagraphTextEditor value={source} onChange={changed} onError={()=>{}}/><input aria-label="Modal paragraph size"/></Modal>);
 const field=screen.getByRole("textbox",{name:"Modal paragraph size"});field.focus();fireEvent.keyDown(field,{key:"h",metaKey:true});expect(screen.getByRole("textbox",{name:"Find in paragraph"})).toHaveFocus();expect(changed).not.toHaveBeenCalled();
});
it("falls back to its own host when no paragraph workspace or dialog exists",()=>{
 const source=createStyledEditor(styledEditorFixture());render(<ParagraphTextEditor value={source} onChange={()=>{}} onError={()=>{}}/>);const input=screen.getByRole("textbox",{name:"Paragraph text"});input.focus();fireEvent.keyDown(input,{key:"f",ctrlKey:true});expect(screen.getByRole("textbox",{name:"Find in paragraph"})).toHaveFocus();
});
it("keeps nested portal Find shortcuts and navigation outside the paragraph owner",()=>{
 const source=createStyledEditor(styledEditorFixture()),changed=vi.fn();
 const element=(nested:boolean)=><div className="paragraph-workspace-panel"><ParagraphTextEditor value={source} onChange={changed} onError={()=>{}}/>{nested&&<Modal title="Nested font choice" onClose={()=>{}}><input aria-label="Nested font query"/></Modal>}</div>;
 const view=render(element(false));open();query("one");view.rerender(element(true));const nested=screen.getByRole("textbox",{name:"Nested font query"});nested.focus();
 for(const key of [{key:"f",ctrlKey:true},{key:"h",metaKey:true},{key:"F3"},{key:"F3",shiftKey:true}])expect(fireEvent.keyDown(nested,key)).toBe(true);
 expect(nested).toHaveFocus();expect(changed).not.toHaveBeenCalled();expect(document.querySelector<HTMLTextAreaElement>('.paragraph-find textarea')?.value).toBe("one");
});
it("selects next/previous matches and wraps without changing text or history",async()=>{
 const h=setup();open();query("one");click("Next match");await waitFor(()=>expect(h.root).toHaveFocus());expect(h.state().present.selection).toMatchObject({start:0,end:3});
 fireEvent.keyDown(h.root,{key:"F3"});expect(h.state().present.selection).toMatchObject({start:8,end:11});fireEvent.keyDown(h.root,{key:"F3",shiftKey:true});expect(h.state().present.selection).toMatchObject({start:0,end:3});expect(h.state().past).toHaveLength(0);
});
it("uses case and Unicode whole-word options",()=>{
 setup("ONE one ones");open();query("one");expect(screen.getByRole("status")).toHaveTextContent("3 matches");fireEvent.click(screen.getByRole("checkbox",{name:"Match case"}));expect(screen.getByRole("status")).toHaveTextContent("2 matches");fireEvent.click(screen.getByRole("checkbox",{name:"Whole words"}));expect(screen.getByRole("status")).toHaveTextContent("1 match");
});
it("replaces only an exact selected match and moves to the next one",()=>{
 const h=setup();open();query("one");replacement("new");expect(screen.getByRole("button",{name:"Replace"})).toBeDisabled();click("Next match");click("Replace");expect(styledText(h.state().present.draft)).toBe("new two one");expect(h.state().present.selection).toMatchObject({start:8,end:11});expect(h.state().past).toHaveLength(1);
});
it("advances after an identical Replace without adding history",()=>{
 const h=setup();open();query("one");replacement("one");click("Next match");click("Replace");expect(h.state().present.selection).toMatchObject({start:8,end:11});expect(h.state().past).toHaveLength(0);
});
it("replaces all literally in one Undo step and recomputes after Undo/Redo",async()=>{
 const h=setup();open();query("one");replacement("$&");click("Replace all");expect(styledText(h.state().present.draft)).toBe("$& two $&");expect(h.state().past).toHaveLength(1);await waitFor(()=>expect(h.root).toHaveFocus());fireEvent.keyDown(h.root,{key:"z",ctrlKey:true});expect(styledText(h.state().present.draft)).toBe("one two one");expect(screen.getByRole("status")).toHaveTextContent("2 matches");fireEvent.keyDown(h.root,{key:"y",ctrlKey:true});expect(styledText(h.state().present.draft)).toBe("$& two $&");
});
it("closes Find on Escape before forwarding a later Escape to the workspace",async()=>{
 const h=setup();open();const find=screen.getByRole("textbox",{name:"Find in paragraph"});fireEvent.keyDown(find,{key:"Escape"});expect(screen.queryByRole("textbox",{name:"Find in paragraph"})).toBeNull();expect(h.outer).not.toHaveBeenCalled();await waitFor(()=>expect(h.root).toHaveFocus());fireEvent.keyDown(h.root,{key:"Escape"});expect(h.outer).toHaveBeenCalledTimes(1);
});
it("owns first Escape from a sibling paragraph control without losing edits or history",async()=>{
 const h=setup();open();query("one");replacement("new");click("Replace all");const changed=h.state();const sibling=screen.getByRole("button",{name:"Sibling paragraph control"});sibling.focus();fireEvent.keyDown(sibling,{key:"Escape",isComposing:true});expect(screen.getByRole("textbox",{name:"Find in paragraph"})).toBeInTheDocument();h.outer.mockClear();fireEvent.keyDown(sibling,{key:"Escape"});expect(screen.queryByRole("textbox",{name:"Find in paragraph"})).toBeNull();expect(h.outer).not.toHaveBeenCalled();expect(h.state()).toBe(changed);await waitFor(()=>expect(h.root).toHaveFocus());fireEvent.keyDown(h.root,{key:"Escape"});expect(h.outer).toHaveBeenCalledTimes(1);
});
it("leaves an active nested modal's Escape ownership alone",()=>{
 const state=createStyledEditor(styledEditorFixture()),close=vi.fn();
 const element=(picker:boolean)=><div className="paragraph-workspace-panel"><ParagraphTextEditor value={state} onChange={()=>{}} onError={()=>{}}/>{picker&&<Modal title="Nested font choice" onClose={close}><input aria-label="Nested font query"/></Modal>}</div>;
 const view=render(element(false));open();view.rerender(element(true));const nested=screen.getByRole("textbox",{name:"Nested font query"});nested.focus();fireEvent.keyDown(nested,{key:"Escape"});expect(close).toHaveBeenCalledTimes(1);expect(document.querySelector('.paragraph-find')).not.toBeNull();
});
it("does not intercept composition and disables replacements while busy",()=>{
 const h=setup(undefined,true);fireEvent.keyDown(h.root,{key:"f",ctrlKey:true,isComposing:true});expect(screen.queryByRole("textbox",{name:"Find in paragraph"})).toBeNull();open();query("one");click("Next match");expect(screen.getByRole("button",{name:"Replace"})).toBeDisabled();expect(screen.getByRole("button",{name:"Replace all"})).toBeDisabled();expect(h.state().past).toHaveLength(0);
});
it("recomputes against external edits and clears state for a new original",()=>{
 const h=setup();open();query("one");h.update(editStyledEditor(h.state(),{type:"replace",start:0,end:3,text:"new"}));expect(screen.getByRole("status")).toHaveTextContent("1 match");
 const fresh=styledEditorFixture();h.update(createStyledEditor(fresh));expect(screen.queryByRole("textbox",{name:"Find in paragraph"})).toBeNull();open();expect(screen.getByRole("textbox",{name:"Find in paragraph"})).toHaveValue("");
});
it("retains the full draft and history on invalid or overflowing replacement",()=>{
 const h=setup();open();query("one");replacement("x".repeat(4096));click("Replace all");expect(screen.getByRole("alert")).toHaveTextContent("4096");expect(styledText(h.state().present.draft)).toBe("one two one");expect(h.state().past).toHaveLength(0);replacement("\t");click("Replace all");expect(screen.getByRole("alert")).toHaveTextContent("plain text");expect(h.state().past).toHaveLength(0);
});
it("handles Enter/Shift+Enter in Find and keeps literal blank-line replacements",()=>{
 const h=setup();open();query("one");const field=screen.getByRole("textbox",{name:"Find in paragraph"});fireEvent.keyDown(field,{key:"Enter"});expect(h.state().present.selection).toMatchObject({start:0,end:3});fireEvent.keyDown(field,{key:"Enter",shiftKey:true});expect(h.state().present.selection).toMatchObject({start:8,end:11});replacement("first\r\n\r\nlast");click("Replace all");expect(styledText(h.state().present.draft)).toBe("first\n\nlast two first\n\nlast");
});
