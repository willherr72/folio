import {act,cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {StyledParagraphDialog} from "../src/components/StyledParagraphDialog";
import {fontApi,flushFontReleases,type FontInfo} from "../src/editor/custom-fonts";
import {styledEditorFixture} from "./fixtures/styled-editor-data";
import type {CanvasParagraphView,ParagraphCanvasPresentation} from "../src/editor/paragraph-canvas";
import type {FolioAdapter} from "../src/editor/adapter";
import type {AnnotationRect,DocumentInfo,EditableTextRun} from "../src/editor/types";
const font=vi.hoisted(()=>({id:"a".repeat(64),name:"Test Serif",weight:400,italic:false,coverage:[[32,255]]}));
vi.mock("../src/components/FontPicker",()=>({FontPicker:({onChoose}:{onChoose:(f:FontInfo)=>void})=><button onClick={()=>onChoose(font as FontInfo)}>Choose test font</button>}));
const original={...styledEditorFixture(),baselineAnchor:{x:20,y:42}};
const fitted={...original.bounds,y:25,width:240,height:105};
const anchor:EditableTextRun={objectIndex:0,text:"one ",fontName:"Courier",fontSize:12,bounds:{x:20,y:30,width:30,height:12},supported:true};
const doc:DocumentInfo={id:"candidate",name:"page.pdf",pages:[{width:400,height:500}]};
beforeEach(()=>{vi.spyOn(fontApi,"release").mockResolvedValue(undefined);vi.stubGlobal("URL",class extends URL {static revokeObjectURL=vi.fn();});});
afterEach(async()=>{cleanup();await flushFontReleases();vi.restoreAllMocks();vi.unstubAllGlobals();});
function setup(overrides:Partial<FolioAdapter>={},crop?:AnnotationRect,canvas?:ParagraphCanvasPresentation){
 const adapter:FolioAdapter={kind:"native",openPdf:vi.fn(),exportPdf:vi.fn(),engineStatus:vi.fn(),closeDocument:vi.fn().mockResolvedValue(undefined),listTextRuns:vi.fn().mockResolvedValue({runs:[anchor]}),suggestStyledParagraph:vi.fn().mockResolvedValue(original),inspectStyledParagraph:vi.fn().mockResolvedValue(original),fitStyledParagraph:vi.fn().mockResolvedValue(fitted),previewStyledParagraph:vi.fn().mockResolvedValue(doc),renderPage:vi.fn().mockResolvedValue("data:image/png;base64,AA"),...overrides};
 const props={adapter,sourceId:"source",pageIndex:0,pageWidth:400,pageHeight:500,crop,canvas,anchor,onApply:vi.fn().mockReturnValue(true),onCancel:vi.fn()};
 return {...render(<StyledParagraphDialog {...props}/>),adapter,props};
}
const click=(name:string)=>fireEvent.click(screen.getByRole("button",{name}));
const field=(name:string)=>screen.getByRole("spinbutton",{name});
const input=()=>screen.getByRole("textbox",{name:"Paragraph text"});
const ready=()=>screen.findByRole("textbox",{name:"Paragraph text"});
function select(start:number,end=start){act(()=>{const root=input();root.focus();const point=(offset:number):[Node,number]=>{const walk=document.createTreeWalker(root,NodeFilter.SHOW_TEXT);let n;while((n=walk.nextNode())){if(offset<=n.textContent!.length)return[n,offset];offset-=n.textContent!.length;}return[root,root.childNodes.length];};const r=document.createRange();r.setStart(...point(start));r.setEnd(...point(end));getSelection()!.removeAllRanges();getSelection()!.addRange(r);document.dispatchEvent(new Event("selectionchange"));});}
function type(text:string){act(()=>input().dispatchEvent(new InputEvent("beforeinput",{bubbles:true,cancelable:true,inputType:"insertText",data:text})));}
async function edit(){await ready();select(4,7);type("longer");}
async function preview(){click("Preview paragraph");await waitFor(()=>expect(screen.getByRole("button",{name:"Apply paragraph"})).toBeEnabled());}
it("fits edited text before preview with unchanged fonts and baseline",async()=>{
 let finish!:(b:AnnotationRect)=>void;const h=setup({fitStyledParagraph:vi.fn().mockImplementation(()=>new Promise(resolve=>finish=resolve))});await edit();click("Preview paragraph");
 expect(h.adapter.fitStyledParagraph).toHaveBeenCalledWith("source",0,expect.objectContaining({expectedText:"one two",bounds:original.bounds,baselineAnchor:original.baselineAnchor}));expect(h.adapter.previewStyledParagraph).not.toHaveBeenCalled();
 await act(async()=>finish(fitted));await waitFor(()=>expect(h.adapter.previewStyledParagraph).toHaveBeenCalled());expect(h.adapter.previewStyledParagraph).toHaveBeenLastCalledWith("source",0,expect.objectContaining({bounds:fitted,baselineAnchor:original.baselineAnchor,runs:[{text:"one ",styleId:0},{text:"longer",styleId:1}]}));expect(field("Area width (pt)")).toHaveValue(240);expect(field("Top (pt)")).toHaveValue(25);
});
it("merges automatic geometry with the edit and restores it exactly through Undo and Redo",async()=>{
 const h=setup();await edit();await preview();expect(field("Area width (pt)")).toHaveValue(240);click("Undo");expect(input()).toHaveTextContent("one two");expect(field("Area width (pt)")).toHaveValue(200);await preview();click("Redo");expect(input()).toHaveTextContent("one longer");expect(field("Area width (pt)")).toHaveValue(240);await preview();expect(h.adapter.fitStyledParagraph).toHaveBeenCalledTimes(1);
});
it("does not fit untouched previews, selection or caret-only formatting",async()=>{
 const h=setup();await ready();await preview();select(4);fireEvent.change(field("Font size (pt)"),{target:{value:"16"}});await preview();expect(h.adapter.fitStyledParagraph).not.toHaveBeenCalled();expect(h.adapter.previewStyledParagraph).toHaveBeenLastCalledWith("source",0,expect.objectContaining({bounds:original.bounds,baselineAnchor:original.baselineAnchor}));
});
it("offers default fitting with an explicit fixed-area preference and invalidates on re-enable",async()=>{
 const h=setup();await edit();const toggle=screen.getByRole("checkbox",{name:"Automatically fit edited text"});expect(toggle).toBeChecked();fireEvent.click(toggle);await preview();expect(h.adapter.fitStyledParagraph).not.toHaveBeenCalled();expect(field("Area width (pt)")).toHaveValue(200);fireEvent.click(toggle);expect(screen.getByRole("button",{name:"Apply paragraph"})).toBeDisabled();await preview();expect(h.adapter.fitStyledParagraph).toHaveBeenCalledTimes(1);expect(field("Area width (pt)")).toHaveValue(240);
});
it("preserves a manual area until another content edit",async()=>{
 const h=setup();await edit();fireEvent.change(field("Area width (pt)"),{target:{value:"210"}});await preview();expect(h.adapter.fitStyledParagraph).not.toHaveBeenCalled();expect(h.adapter.previewStyledParagraph).toHaveBeenLastCalledWith("source",0,expect.objectContaining({bounds:{...original.bounds,width:210}}));select(10);type("!");await preview();expect(h.adapter.fitStyledParagraph).toHaveBeenCalledTimes(1);
});
it("does not refit keyboard history navigation",async()=>{
 const h=setup();await edit();await preview();fireEvent.keyDown(input(),{key:"z",ctrlKey:true});expect(input()).toHaveTextContent("one two");expect(field("Area width (pt)")).toHaveValue(200);await preview();fireEvent.keyDown(input(),{key:"z",ctrlKey:true,shiftKey:true});expect(input()).toHaveTextContent("one longer");expect(field("Area width (pt)")).toHaveValue(240);await preview();expect(h.adapter.fitStyledParagraph).toHaveBeenCalledTimes(1);
});
it.each(["refused","invalid","crop"])("retains the edited draft and native preview path for a %s fit proposal",async kind=>{
 const fit=kind==="refused"?vi.fn().mockRejectedValue(new Error("No safe area.")):vi.fn().mockResolvedValue(kind==="invalid"?{...fitted,width:999}:fitted);
 const h=setup({fitStyledParagraph:fit},kind==="crop"?{x:0,y:0,width:220,height:500}:undefined);await edit();await preview();expect(h.adapter.fitStyledParagraph).toHaveBeenCalledTimes(1);expect(input()).toHaveTextContent("one longer");expect(field("Area width (pt)")).toHaveValue(200);expect(h.adapter.previewStyledParagraph).toHaveBeenLastCalledWith("source",0,expect.objectContaining({bounds:original.bounds}));expect(screen.getByText(/Your edited text is preserved/)).toBeVisible();
});
it("discards a delayed fit after another edit and fits the newer draft on demand",async()=>{
 let finish!:(b:AnnotationRect)=>void;const fit=vi.fn().mockResolvedValue(fitted).mockImplementationOnce(()=>new Promise(resolve=>finish=resolve));const h=setup({fitStyledParagraph:fit});await edit();click("Preview paragraph");expect(fit).toHaveBeenCalledTimes(1);select(4,10);type("new");await act(async()=>finish(fitted));expect(h.adapter.previewStyledParagraph).not.toHaveBeenCalled();expect(input()).toHaveTextContent("one new");expect(field("Area width (pt)")).toHaveValue(200);await preview();expect(fit).toHaveBeenCalledTimes(2);expect(h.adapter.previewStyledParagraph).toHaveBeenLastCalledWith("source",0,expect.objectContaining({runs:[{text:"one ",styleId:0},{text:"new",styleId:1}]}));
});
it("preserves current caret selection while adopting a delayed fit",async()=>{
 let finish!:(b:AnnotationRect)=>void;setup({fitStyledParagraph:vi.fn().mockImplementation(()=>new Promise(resolve=>finish=resolve))});await edit();click("Preview paragraph");select(0,3);await act(async()=>finish(fitted));await waitFor(()=>expect(field("Area width (pt)")).toHaveValue(240));type("new");expect(input()).toHaveTextContent("new longer");
});
it("ignores a fit from a previous source owner",async()=>{
 let finish!:(b:AnnotationRect)=>void;const h=setup({fitStyledParagraph:vi.fn().mockImplementation(()=>new Promise(resolve=>finish=resolve))});await edit();click("Preview paragraph");expect(h.adapter.fitStyledParagraph).toHaveBeenCalledTimes(1);h.rerender(<StyledParagraphDialog {...h.props} sourceId="other"/>);await ready();await act(async()=>finish(fitted));expect(input()).toHaveTextContent("one two");expect(field("Area width (pt)")).toHaveValue(200);expect(h.adapter.previewStyledParagraph).not.toHaveBeenCalled();
});
it("holds substitute fonts through a pending fit after unmount",async()=>{
 let finish!:(b:AnnotationRect)=>void;const h=setup({fitStyledParagraph:vi.fn().mockImplementation(()=>new Promise(resolve=>finish=resolve))});await ready();select(4,7);click("Choose substitute font…");click("Choose test font");click("Preview paragraph");expect(h.adapter.fitStyledParagraph).toHaveBeenCalledTimes(1);h.unmount();await flushFontReleases();expect(fontApi.release).not.toHaveBeenCalled();await act(async()=>finish(fitted));await flushFontReleases();expect(fontApi.release).toHaveBeenCalledTimes(1);expect(h.adapter.previewStyledParagraph).not.toHaveBeenCalled();
});
it("owns the fitted rectangle before awaiting the PDF candidate",async()=>{
 const owned={...fitted};let finish!:(d:DocumentInfo)=>void;const h=setup({fitStyledParagraph:vi.fn().mockResolvedValue(owned),previewStyledParagraph:vi.fn().mockImplementation(()=>new Promise(resolve=>finish=resolve))});await edit();click("Preview paragraph");await waitFor(()=>expect(h.adapter.previewStyledParagraph).toHaveBeenCalled());owned.width=999;await act(async()=>finish(doc));expect(field("Area width (pt)")).toHaveValue(240);expect(h.adapter.previewStyledParagraph).toHaveBeenLastCalledWith("source",0,expect.objectContaining({bounds:fitted}));
});
it("retains existing preview support when fitting is unavailable",async()=>{
 const h=setup({fitStyledParagraph:undefined});await edit();expect(screen.queryByRole("checkbox",{name:"Automatically fit edited text"})).toBeNull();await preview();expect(h.adapter.previewStyledParagraph).toHaveBeenCalledWith("source",0,expect.objectContaining({bounds:original.bounds}));
});

it("keeps Apply disabled until the fitted canvas candidate raster is ready",async()=>{
 let view!:CanvasParagraphView,finish!:(url:string)=>void;
 const canvas={pageId:"p",sessionKey:"s",zoom:100,onLocate:vi.fn(),onModalChange:vi.fn(),renderCanvas:(next:CanvasParagraphView)=>{view=next;return <div data-testid="canvas"/>;}};
 const h=setup({renderPage:vi.fn().mockImplementation(id=>id==="source"?Promise.resolve("data:image/png;base64,AA"):new Promise(resolve=>finish=resolve))},undefined,canvas);
 await edit();click("Preview paragraph");await waitFor(()=>expect(h.adapter.renderPage).toHaveBeenCalledWith("candidate",0,600));expect(view.bounds).toEqual(fitted);expect(screen.getByRole("button",{name:"Apply paragraph"})).toBeDisabled();
 await act(async()=>finish("data:image/png;base64,AA"));await waitFor(()=>expect(screen.getByRole("button",{name:"Apply paragraph"})).toBeEnabled());expect(h.adapter.fitStyledParagraph).toHaveBeenCalledTimes(1);
});
it("discards an automatic fit when the paragraph is reset",async()=>{
 let finish!:(b:AnnotationRect)=>void;const h=setup({fitStyledParagraph:vi.fn().mockImplementation(()=>new Promise(resolve=>finish=resolve))});await edit();click("Preview paragraph");expect(h.adapter.fitStyledParagraph).toHaveBeenCalledOnce();click("Reset paragraph");await act(async()=>finish(fitted));expect(input()).toHaveTextContent("one two");expect(field("Area width (pt)")).toHaveValue(200);expect(h.adapter.previewStyledParagraph).not.toHaveBeenCalled();
});
it("preserves the draft and reports native preview refusal after fitting also fails",async()=>{
 setup({fitStyledParagraph:vi.fn().mockRejectedValue(new Error("No safe area.")),previewStyledParagraph:vi.fn().mockRejectedValue(new Error("Text does not fit the checked area."))});await edit();click("Preview paragraph");await waitFor(()=>expect(screen.getByRole("alert")).toHaveTextContent("Text does not fit the checked area."));expect(input()).toHaveTextContent("one longer");expect(field("Area width (pt)")).toHaveValue(200);expect(screen.getByText(/Your edited text is preserved/)).toBeVisible();expect(screen.getByRole("button",{name:"Apply paragraph"})).toBeDisabled();
});

it("keeps composition entered while an automatic fit reply is pending",async()=>{
 let finish!:(b:AnnotationRect)=>void;const h=setup({fitStyledParagraph:vi.fn().mockImplementation(()=>new Promise(resolve=>finish=resolve))});await edit();click("Preview paragraph");expect(h.adapter.fitStyledParagraph).toHaveBeenCalledOnce();select(10);const root=input();
 act(()=>{fireEvent.compositionStart(root);root.textContent="one longeré";root.dispatchEvent(new InputEvent("input",{bubbles:true,inputType:"insertCompositionText",data:"é",isComposing:true}));});
 await act(async()=>finish(fitted));fireEvent.compositionEnd(root,{data:"é"});expect(input()).toHaveTextContent("one longeré");expect(h.adapter.previewStyledParagraph).not.toHaveBeenCalled();expect(field("Area width (pt)")).toHaveValue(200);
});
it("keeps a noncancelable browser edit pending across automatic fit settlement",async()=>{
 let finish!:(b:AnnotationRect)=>void;const h=setup({fitStyledParagraph:vi.fn().mockImplementation(()=>new Promise(resolve=>finish=resolve))});await edit();click("Preview paragraph");expect(h.adapter.fitStyledParagraph).toHaveBeenCalledOnce();select(10);const root=input();
 act(()=>{root.dispatchEvent(new InputEvent("beforeinput",{bubbles:true,cancelable:false,inputType:"insertText",data:"!"}));root.textContent="one longer!";const range=document.createRange();range.selectNodeContents(root);range.collapse(true);getSelection()!.removeAllRanges();getSelection()!.addRange(range);document.dispatchEvent(new Event("selectionchange"));});await act(async()=>finish(fitted));act(()=>root.dispatchEvent(new InputEvent("input",{bubbles:true,inputType:"insertText",data:"!"})));
 expect(input()).toHaveTextContent("one longer!");expect(h.adapter.previewStyledParagraph).not.toHaveBeenCalled();expect(field("Area width (pt)")).toHaveValue(200);
});
it("keeps fitted geometry when Undo removes only future-typing formatting",async()=>{
 setup();await edit();select(10);fireEvent.change(field("Font size (pt)"),{target:{value:"24"}});await preview();click("Undo");expect(input()).toHaveTextContent("one longer");expect(field("Font size (pt)")).toHaveValue(20);expect(field("Area width (pt)")).toHaveValue(240);await preview();
 click("Undo");expect(input()).toHaveTextContent("one two");expect(field("Area width (pt)")).toHaveValue(200);click("Redo");expect(input()).toHaveTextContent("one longer");expect(field("Area width (pt)")).toHaveValue(240);click("Redo");expect(field("Font size (pt)")).toHaveValue(24);expect(field("Area width (pt)")).toHaveValue(240);
});
it("binds fitted geometry to the text edit across caret formatting during the native wait",async()=>{
 let finish!:(b:AnnotationRect)=>void;setup({fitStyledParagraph:vi.fn().mockImplementation(()=>new Promise(resolve=>finish=resolve))});await edit();click("Preview paragraph");select(10);fireEvent.change(field("Font size (pt)"),{target:{value:"24"}});await act(async()=>finish(fitted));await waitFor(()=>expect(field("Area width (pt)")).toHaveValue(240));click("Undo");expect(input()).toHaveTextContent("one longer");expect(field("Area width (pt)")).toHaveValue(240);click("Undo");expect(input()).toHaveTextContent("one two");expect(field("Area width (pt)")).toHaveValue(200);
});
it("cancels a pending fit for request-equivalent history navigation",async()=>{
 let finish!:(b:AnnotationRect)=>void;const h=setup({fitStyledParagraph:vi.fn().mockImplementation(()=>new Promise(resolve=>finish=resolve))});await edit();select(10);fireEvent.change(field("Font size (pt)"),{target:{value:"24"}});click("Preview paragraph");expect(h.adapter.fitStyledParagraph).toHaveBeenCalledOnce();click("Undo");await act(async()=>finish(fitted));expect(input()).toHaveTextContent("one longer");expect(field("Area width (pt)")).toHaveValue(200);expect(h.adapter.previewStyledParagraph).not.toHaveBeenCalled();click("Redo");expect(field("Area width (pt)")).toHaveValue(200);
});

it("reenables an unchanged preview after composition is canceled",async()=>{
 const h=setup();await ready();select(4);const root=input();act(()=>{fireEvent.compositionStart(root);root.textContent="one provisional two";});expect(screen.getByRole("button",{name:"Preview paragraph"})).toBeDisabled();fireEvent.compositionEnd(root,{data:""});expect(input()).toHaveTextContent("one two");await preview();expect(h.adapter.fitStyledParagraph).not.toHaveBeenCalled();
});
it("keeps intentional reset and late composition separate from the new draft",async()=>{
 let finish!:(b:AnnotationRect)=>void;const h=setup({fitStyledParagraph:vi.fn().mockImplementation(()=>new Promise(resolve=>finish=resolve))});await edit();click("Preview paragraph");select(10);const root=input();act(()=>{fireEvent.compositionStart(root);root.textContent="one longeré";});click("Reset paragraph");await act(async()=>finish(fitted));fireEvent.compositionEnd(root,{data:"é"});expect(input()).toHaveTextContent("one two");expect(field("Area width (pt)")).toHaveValue(200);await preview();expect(h.adapter.fitStyledParagraph).toHaveBeenCalledTimes(1);
});
