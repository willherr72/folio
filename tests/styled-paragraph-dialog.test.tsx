import {act,cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {StyledParagraphDialog} from "../src/components/StyledParagraphDialog";
import {fontApi,flushFontReleases,type FontInfo} from "../src/editor/custom-fonts";
import type {FolioAdapter} from "../src/editor/adapter";
import type {DocumentInfo,EditableTextRun,StyledParagraphSelection} from "../src/editor/types";
import {styledEditorFixture} from "./fixtures/styled-editor-data";
import {flushSync} from "react-dom";
const font=vi.hoisted(()=>({id:"a".repeat(64),name:"Test Serif",weight:400,italic:false,coverage:[[32,255]]}));
vi.mock("../src/components/FontPicker",()=>({FontPicker:({text,onChoose,onClose}:{text:string;onChoose:(f:FontInfo)=>void;onClose:()=>void})=><div><p data-testid="font-sample">{text}</p><button onClick={()=>onChoose(font as FontInfo)}>Choose test font</button><button onClick={onClose}>Cancel font picker</button></div>}));
beforeEach(()=>{vi.spyOn(fontApi,"release").mockResolvedValue(undefined);vi.stubGlobal("URL",class extends URL {static revokeObjectURL=vi.fn();});});
afterEach(async()=>{cleanup();await flushFontReleases();vi.restoreAllMocks();vi.unstubAllGlobals();});
const selection=styledEditorFixture();
const anchor:EditableTextRun={objectIndex:0,text:"one ",fontName:"Courier",fontSize:12,bounds:{x:20,y:30,width:30,height:12},supported:true};
const candidate:DocumentInfo={id:"preview",name:"page.pdf",pages:[{width:400,height:500}]};
function setup(overrides:Partial<FolioAdapter>={}){
 const adapter:FolioAdapter={kind:"native",openPdf:vi.fn(),exportPdf:vi.fn(),engineStatus:vi.fn(),closeDocument:vi.fn().mockResolvedValue(undefined),listTextRuns:vi.fn().mockResolvedValue({runs:[anchor,{...anchor,objectIndex:1,text:"two"},{...anchor,objectIndex:2,text:"other"}]}),suggestStyledParagraph:vi.fn().mockResolvedValue(selection),inspectStyledParagraph:vi.fn().mockResolvedValue(selection),previewStyledParagraph:vi.fn().mockResolvedValue(candidate),renderPage:vi.fn().mockImplementation(id=>Promise.resolve(id==="source"?"data:image/png;base64,AA":"blob:"+id)),...overrides};
 const props={adapter,sourceId:"source",pageIndex:0,pageWidth:400,pageHeight:500,anchor,onApply:vi.fn().mockReturnValue(true),onCancel:vi.fn()};
 return {...render(<StyledParagraphDialog {...props}/>),adapter,props};
}
const click=(name:string)=>fireEvent.click(screen.getByRole("button",{name}));
async function ready(){return screen.findByRole("textbox",{name:"Paragraph text"});}
it("shows compact bounds without changing untouched exact requests or history",async()=>{
 const bounds={x:143.99998474121094,y:30.123456789,width:200.123456789,height:50.987654321};
 const h=setup({suggestStyledParagraph:vi.fn().mockResolvedValue({...selection,bounds})});await ready();
 const left=screen.getByRole("spinbutton",{name:"Left (pt)"});expect(left).toHaveValue(144);fireEvent.focus(left);expect(left).toHaveValue(bounds.x);fireEvent.blur(left);expect(left).toHaveValue(144);
 expect(screen.getByRole("button",{name:"Undo"})).toBeDisabled();await preview();expect(h.adapter.previewStyledParagraph).toHaveBeenLastCalledWith("source",0,expect.objectContaining({bounds}));
 fireEvent.focus(left);fireEvent.change(left,{target:{value:"144.123456789"}});fireEvent.blur(left);expect(left).toHaveValue(144.123);await preview();expect(h.adapter.previewStyledParagraph).toHaveBeenLastCalledWith("source",0,expect.objectContaining({bounds:{...bounds,x:144.123456789}}));
 click("Undo");await preview();expect(h.adapter.previewStyledParagraph).toHaveBeenLastCalledWith("source",0,expect.objectContaining({bounds}));
 click("Redo");click("Reset position");await preview();expect(h.adapter.previewStyledParagraph).toHaveBeenLastCalledWith("source",0,expect.objectContaining({bounds}));
});
function select(start:number,end=start){const root=screen.getByRole("textbox",{name:"Paragraph text"});act(()=>{root.focus();const point=(offset:number):[Node,number]=>{const walker=document.createTreeWalker(root,NodeFilter.SHOW_TEXT);let n;while((n=walker.nextNode())){if(offset<=n.textContent!.length)return [n,offset];offset-=n.textContent!.length;}return [root,root.childNodes.length];};const range=document.createRange();range.setStart(...point(start));range.setEnd(...point(end));getSelection()!.removeAllRanges();getSelection()!.addRange(range);document.dispatchEvent(new Event("selectionchange"));});}
function type(text:string){act(()=>screen.getByRole("textbox",{name:"Paragraph text"}).dispatchEvent(new InputEvent("beforeinput",{bubbles:true,cancelable:true,inputType:"insertText",data:text})));}

it("fits the complete current draft as one undoable area change and requires a new preview",async()=>{
 const fitted={...selection.bounds,width:240,height:80};const h=setup({fitStyledParagraph:vi.fn().mockResolvedValue(fitted)});await ready();select(4,7);type("longer");fireEvent.change(screen.getByRole("spinbutton",{name:"Font size (pt)"}),{target:{value:"16"}});await preview();click("Fit area to text");
 await waitFor(()=>expect(screen.getByRole("spinbutton",{name:"Area width (pt)"})).toHaveValue(240));expect(h.adapter.fitStyledParagraph).toHaveBeenCalledWith("source",0,expect.objectContaining({expectedText:"one two",runs:expect.arrayContaining([expect.objectContaining({text:"longer"})]),bounds:selection.bounds}));expect(screen.getByRole("button",{name:"Apply paragraph"})).toBeDisabled();expect(h.adapter.closeDocument).toHaveBeenCalledWith("preview");expect(await ready()).toHaveTextContent("one longer");click("Undo");expect(screen.getByRole("spinbutton",{name:"Area width (pt)"})).toHaveValue(selection.bounds.width);expect(await ready()).toHaveTextContent("one longer");click("Redo");expect(screen.getByRole("spinbutton",{name:"Area width (pt)"})).toHaveValue(240);
});
it("keeps a valid preview and history when fitting returns the current area",async()=>{
 const h=setup({fitStyledParagraph:vi.fn().mockResolvedValue(selection.bounds)});await ready();await preview();click("Fit area to text");await screen.findByText("The current area already fits the text.");expect(screen.getByRole("button",{name:"Apply paragraph"})).toBeEnabled();expect(screen.getByRole("button",{name:"Undo"})).toBeDisabled();expect(h.adapter.closeDocument).not.toHaveBeenCalled();
});
it.each(["refused","invalid"])("preserves the draft, area and preview when fitting is %s",async kind=>{
 const fit=kind==="refused"?vi.fn().mockRejectedValue(new Error("No room.")):vi.fn().mockResolvedValue({...selection.bounds,width:999});const h=setup({fitStyledParagraph:fit});await ready();await preview();click("Fit area to text");await screen.findByRole("alert");expect(screen.getByRole("spinbutton",{name:"Area width (pt)"})).toHaveValue(selection.bounds.width);expect(await ready()).toHaveTextContent("one two");expect(screen.getByRole("button",{name:"Apply paragraph"})).toBeEnabled();expect(h.adapter.closeDocument).not.toHaveBeenCalled();
});
it("ignores a fit response after draft edits and disables fitting for incomplete fields",async()=>{
 let finish!:(b:typeof selection.bounds)=>void;setup({fitStyledParagraph:vi.fn().mockImplementation(()=>new Promise(resolve=>finish=resolve))});await ready();click("Fit area to text");expect(screen.getByRole("button",{name:"Fit area to text"})).toBeDisabled();select(4,7);type("new");await act(async()=>finish({...selection.bounds,width:240}));expect(screen.getByRole("spinbutton",{name:"Area width (pt)"})).toHaveValue(selection.bounds.width);expect(await ready()).toHaveTextContent("one new");fireEvent.change(screen.getByRole("spinbutton",{name:"Area width (pt)"}),{target:{value:""}});expect(screen.getByRole("button",{name:"Fit area to text"})).toBeDisabled();
});
it("holds a fitting request's font lease through unmount until the native result returns",async()=>{
 let finish!:(b:typeof selection.bounds)=>void;const h=setup({fitStyledParagraph:vi.fn().mockImplementation(()=>new Promise(resolve=>finish=resolve))});await ready();select(4,7);await choose();click("Fit area to text");h.unmount();await flushFontReleases();expect(fontApi.release).not.toHaveBeenCalled();await act(async()=>finish({...selection.bounds,width:240}));await flushFontReleases();expect(fontApi.release).toHaveBeenCalledTimes(1);
});
it("applies fitting to the current text selection and ignores results from an old source",async()=>{
 let finish!:(b:typeof selection.bounds)=>void;const h=setup({fitStyledParagraph:vi.fn().mockImplementation(()=>new Promise(resolve=>finish=resolve))});await ready();select(4,7);click("Fit area to text");select(0,3);await act(async()=>finish({...selection.bounds,width:240}));type("new");expect(await ready()).toHaveTextContent("new two");expect(screen.getByRole("spinbutton",{name:"Area width (pt)"})).toHaveValue(240);
 click("Fit area to text");h.rerender(<StyledParagraphDialog {...h.props} sourceId="other"/>);await ready();await act(async()=>finish({...selection.bounds,width:300}));expect(await ready()).toHaveTextContent("one two");expect(screen.getByRole("spinbutton",{name:"Area width (pt)"})).toHaveValue(selection.bounds.width);
});
async function preview(){click("Preview paragraph");await screen.findByRole("img",{name:"Paragraph PDF preview"});}
it("keeps the baseline when Fit adds headroom and translates it only for position edits",async()=>{
 const baselineAnchor={x:20,y:42};
 const fitted={...selection.bounds,y:25,height:85};
 const h=setup({suggestStyledParagraph:vi.fn().mockResolvedValue({...selection,baselineAnchor}),fitStyledParagraph:vi.fn().mockResolvedValue(fitted)});
 await ready();click("Fit area to text");
 await waitFor(()=>expect(screen.getByRole("spinbutton",{name:"Top (pt)"})).toHaveValue(25));
 await preview();expect(h.adapter.previewStyledParagraph).toHaveBeenLastCalledWith("source",0,expect.objectContaining({bounds:fitted,baselineAnchor}));
 fireEvent.change(screen.getByRole("spinbutton",{name:"Top (pt)"}),{target:{value:"35"}});
 await preview();expect(h.adapter.previewStyledParagraph).toHaveBeenLastCalledWith("source",0,expect.objectContaining({baselineAnchor:{x:20,y:52}}));
 click("Undo");await preview();expect(h.adapter.previewStyledParagraph).toHaveBeenLastCalledWith("source",0,expect.objectContaining({bounds:fitted,baselineAnchor}));
 click("Redo");await preview();expect(h.adapter.previewStyledParagraph).toHaveBeenLastCalledWith("source",0,expect.objectContaining({baselineAnchor:{x:20,y:52}}));
 click("Reset position");await preview();expect(h.adapter.previewStyledParagraph).toHaveBeenLastCalledWith("source",0,expect.objectContaining({bounds:fitted,baselineAnchor}));
 click("Undo");await preview();expect(h.adapter.previewStyledParagraph).toHaveBeenLastCalledWith("source",0,expect.objectContaining({baselineAnchor:{x:20,y:52}}));
 click("Redo");await preview();expect(h.adapter.previewStyledParagraph).toHaveBeenLastCalledWith("source",0,expect.objectContaining({bounds:fitted,baselineAnchor}));
 click("Reset paragraph");await ready();await preview();expect(h.adapter.previewStyledParagraph).toHaveBeenLastCalledWith("source",0,expect.objectContaining({bounds:selection.bounds,baselineAnchor}));
});
async function choose(){click("Choose substitute font…");click("Choose test font");await ready();}
it("edits a selected range while keeping original source identity and mixed properties",async()=>{
 const {adapter}=setup();await ready();select(0,7);expect(screen.getByRole("spinbutton",{name:"Font size (pt)"})).toHaveAttribute("placeholder","Mixed");
 fireEvent.change(screen.getByRole("spinbutton",{name:"Font size (pt)"}),{target:{value:"16"}});select(4,7);type("this");await preview();
 expect(adapter.previewStyledParagraph).toHaveBeenLastCalledWith("source",0,expect.objectContaining({objectIndices:[0,1],expectedText:"one two",runs:[{text:"one ",styleId:0},{text:"this",styleId:1}],styleChanges:[{styleId:0,sourceStyleId:0,fontSize:16},{styleId:1,sourceStyleId:1,fontSize:16}],alignment:"right",lineSpacing:1.6}));
});
it("keeps edits after native rejection and preserves a candidate through canceled or failed adjustment",async()=>{
 const {adapter}=setup({previewStyledParagraph:vi.fn().mockRejectedValueOnce(new Error("Does not fit.")).mockResolvedValue(candidate),inspectStyledParagraph:vi.fn().mockRejectedValue(new Error("Overlap."))});await ready();select(4,7);type("this");click("Preview paragraph");expect(await screen.findByRole("alert")).toHaveTextContent("Does not fit.");expect(await ready()).toHaveTextContent("one this");await preview();
 click("Adjust selection");fireEvent.click(screen.getByRole("checkbox",{name:"other"}));click("Check selection");expect(await screen.findByRole("alert")).toHaveTextContent("Overlap.");click("Cancel adjustment");expect(await ready()).toHaveTextContent("one this");expect(screen.getByRole("button",{name:"Apply paragraph"})).toBeEnabled();expect(adapter.closeDocument).not.toHaveBeenCalled();
});
it("reclaims out-of-order preview results and transfers the current document only on acceptance",async()=>{
 const pending:((doc:DocumentInfo)=>void)[]=[];const {adapter,props,unmount}=setup({previewStyledParagraph:vi.fn().mockImplementation(()=>new Promise(resolve=>pending.push(resolve)))});await ready();click("Preview paragraph");select(7);type("!");click("Preview paragraph");expect(pending).toHaveLength(2);
 await act(async()=>pending[1]({...candidate,id:"new"}));await screen.findByRole("img",{name:"Paragraph PDF preview"});await act(async()=>pending[0]({...candidate,id:"old"}));expect(adapter.closeDocument).toHaveBeenCalledTimes(1);expect(adapter.closeDocument).toHaveBeenCalledWith("old");
 props.onApply.mockReturnValueOnce(false);click("Apply paragraph");expect(screen.getByRole("button",{name:"Apply paragraph"})).toBeEnabled();click("Apply paragraph");unmount();expect(adapter.closeDocument).toHaveBeenCalledTimes(1);expect(adapter.closeDocument).toHaveBeenCalledWith("old");expect(URL.revokeObjectURL).toHaveBeenCalledTimes(1);expect(URL.revokeObjectURL).toHaveBeenCalledWith("blob:new");
});
it("reclaims a rendered candidate after cancellation and releases failures exactly once",async()=>{
 let finish!:(url:string)=>void;const {adapter,unmount}=setup({renderPage:vi.fn().mockImplementation(id=>id==="source"?Promise.resolve("data:image/png;base64,AA"):new Promise(resolve=>finish=resolve))});await ready();click("Preview paragraph");await waitFor(()=>expect(adapter.renderPage).toHaveBeenCalledWith("preview",0,1600));click("Cancel");unmount();await act(async()=>finish("blob:late"));expect(adapter.closeDocument).toHaveBeenCalledTimes(1);expect(adapter.closeDocument).toHaveBeenCalledWith("preview");expect(URL.revokeObjectURL).toHaveBeenCalledTimes(1);expect(URL.revokeObjectURL).toHaveBeenCalledWith("blob:late");
});
it("keeps font leases for undo, redo and outstanding requests after resetting the draft",async()=>{
 let finish!:(doc:DocumentInfo)=>void;const {adapter,unmount}=setup({previewStyledParagraph:vi.fn().mockImplementation(()=>new Promise(resolve=>finish=resolve))});await ready();select(4,7);click("Choose substitute font…");expect(screen.getByTestId("font-sample")).toHaveTextContent(/^two$/);click("Choose test font");await ready();click("Undo");expect(fontApi.release).not.toHaveBeenCalled();click("Redo");click("Preview paragraph");click("Reset paragraph");expect(fontApi.release).not.toHaveBeenCalled();unmount();expect(fontApi.release).not.toHaveBeenCalled();await act(async()=>finish(candidate));await flushFontReleases();expect(fontApi.release).toHaveBeenCalledTimes(1);expect(fontApi.release).toHaveBeenCalledWith(font.id);expect(adapter.closeDocument).toHaveBeenCalledTimes(1);expect(adapter.closeDocument).toHaveBeenCalledWith("preview");
});
it("checks font coverage only where a substitute is used and keeps the draft on unsupported insertion",async()=>{
 const {adapter}=setup();await ready();select(4,7);await choose();select(0);type("Ω");await preview();expect(adapter.previewStyledParagraph).toHaveBeenCalledOnce();select(6);type("Ω");expect(screen.getByRole("button",{name:"Preview paragraph"})).toBeDisabled();expect(screen.getByRole("alert")).toHaveTextContent("U+03A9");
});
it("shows selection failures without hidden draft font warnings and restores them on cancel",async()=>{
 const h=setup({fitStyledParagraph:vi.fn(),inspectStyledParagraph:vi.fn().mockRejectedValue(new Error("Overlap."))});await ready();select(4,7);await choose();select(5);type("Ω");
 expect(screen.getByRole("alert")).toHaveTextContent("U+03A9");expect(screen.getByRole("button",{name:"Fit area to text"})).toBeDisabled();
 click("Adjust selection");expect(screen.queryByRole("alert")).toBeNull();expect(screen.queryByRole("button",{name:"Fit area to text"})).toBeNull();expect(screen.queryByText(/Try Fit area to text/)).toBeNull();
 fireEvent.click(screen.getByRole("checkbox",{name:"other"}));click("Check selection");await waitFor(()=>expect(screen.getByRole("alert")).toHaveTextContent("Overlap."));expect(screen.queryByText(/U\+03A9/)).toBeNull();
 click("Cancel adjustment");expect(await ready()).toHaveTextContent("one tΩwo");expect(screen.getByRole("alert")).toHaveTextContent("U+03A9");expect(screen.queryByText("Overlap.")).toBeNull();expect(screen.getByRole("button",{name:"Fit area to text"})).toBeDisabled();expect(h.adapter.fitStyledParagraph).not.toHaveBeenCalled();
});
it("ignores late inspection when source changes and cleans old candidate with its original adapter",async()=>{
 let finish!:(s:StyledParagraphSelection)=>void;const {adapter,props,rerender,unmount}=setup({inspectStyledParagraph:vi.fn().mockImplementation(()=>new Promise(resolve=>finish=resolve))});await ready();await preview();click("Adjust selection");fireEvent.click(screen.getByRole("checkbox",{name:"other"}));click("Check selection");const other={...adapter,closeDocument:vi.fn().mockResolvedValue(undefined)};rerender(<StyledParagraphDialog {...props} adapter={other} sourceId="other"/>);await ready();await act(async()=>finish({...selection,text:"stale",runs:[{text:"stale",styleId:0}]}));expect(await ready()).toHaveTextContent("one two");expect(adapter.closeDocument).toHaveBeenCalledTimes(1);expect(adapter.closeDocument).toHaveBeenCalledWith("preview");expect(other.closeDocument).not.toHaveBeenCalled();unmount();
});
it("renders previews with existing overlays and prevents invalid geometry from applying an old candidate",async()=>{
 const {adapter,props,rerender}=setup({renderPagePlan:vi.fn().mockResolvedValue("blob:composed")});rerender(<StyledParagraphDialog {...props} overlays={[{type:"text",id:"note",text:"Note",x:1,y:1,fontSize:12,color:"#000000"}]}/>);await ready();await preview();expect(adapter.renderPagePlan).toHaveBeenCalledWith(expect.objectContaining({sourceId:"preview",pageIndex:0}),1600);fireEvent.change(screen.getByRole("spinbutton",{name:"Area width (pt)"}),{target:{value:"999"}});expect(screen.getByRole("button",{name:"Preview paragraph"})).toBeDisabled();expect(screen.getByRole("button",{name:"Apply paragraph"})).toBeDisabled();
});
it("closes a native result when raster rendering fails and keeps the edited text",async()=>{
 const {adapter,unmount}=setup({renderPage:vi.fn().mockImplementation(id=>id==="source"?Promise.resolve("data:image/png;base64,AA"):Promise.reject(new Error("Raster failed.")))});await ready();select(4,7);type("draft");click("Preview paragraph");expect(await screen.findByRole("alert")).toHaveTextContent("Raster failed.");expect(await ready()).toHaveTextContent("one draft");expect(adapter.closeDocument).toHaveBeenCalledTimes(1);unmount();expect(adapter.closeDocument).toHaveBeenCalledTimes(1);
});
it("ignores late checked selections after cancel adjustment and preserves a valid saved selection over 64 pieces",async()=>{
 let finish!:(s:StyledParagraphSelection)=>void;const saved={...selection,objectIndices:Array.from({length:200},(_,i)=>i)};
 const {adapter}=setup({suggestStyledParagraph:vi.fn().mockResolvedValue(saved),inspectStyledParagraph:vi.fn().mockImplementation(()=>new Promise(resolve=>finish=resolve))});await ready();await preview();click("Adjust selection");click("Check selection");expect(adapter.inspectStyledParagraph).not.toHaveBeenCalled();expect(screen.getByRole("button",{name:"Apply paragraph"})).toBeEnabled();
 // A changed smaller selection can be checked, but its late answer cannot replace a canceled draft.
 cleanup();const h=setup({inspectStyledParagraph:vi.fn().mockImplementation(()=>new Promise(resolve=>finish=resolve))});await ready();await preview();click("Adjust selection");fireEvent.click(screen.getByRole("checkbox",{name:"other"}));click("Check selection");click("Cancel adjustment");await act(async()=>finish({...selection,text:"late",runs:[{text:"late",styleId:0}]}));expect(await ready()).toHaveTextContent("one two");expect(h.adapter.closeDocument).not.toHaveBeenCalled();
});
it("does not close an accepted document when apply synchronously unmounts the dialog",async()=>{
 const h=setup();await ready();await preview();h.props.onApply.mockImplementation(()=>{flushSync(()=>h.unmount());return true;});click("Apply paragraph");expect(h.adapter.closeDocument).not.toHaveBeenCalled();expect(URL.revokeObjectURL).toHaveBeenCalledTimes(1);
});
it("preserves a preview when the font picker is canceled, and holds history-only fonts until eviction",async()=>{
 const h=setup();await ready();select(4,7);await choose();await preview();click("Choose substitute font…");click("Cancel font picker");expect(screen.getByRole("button",{name:"Apply paragraph"})).toBeEnabled();click("Use original font");expect(fontApi.release).not.toHaveBeenCalled();
 for(let i=0;i<101;i++)fireEvent.change(screen.getByRole("spinbutton",{name:"Left (pt)"}),{target:{value:String(21+i%2)}});
 await flushFontReleases();expect(fontApi.release).toHaveBeenCalledTimes(1);h.unmount();await flushFontReleases();expect(fontApi.release).toHaveBeenCalledTimes(1);
});
it.each([false,true])("resets empty coordinate drafts without clearing other fields or adding a no-op undo step (moved: %s)",async moved=>{
 const {adapter}=setup();await ready();select(4,7);type("draft");
 if(moved)fireEvent.change(screen.getByRole("spinbutton",{name:"Top (pt)"}),{target:{value:"60"}});
 const left=screen.getByRole("spinbutton",{name:"Left (pt)"}),width=screen.getByRole("spinbutton",{name:"Area width (pt)"});
 fireEvent.change(left,{target:{value:""}});fireEvent.change(width,{target:{value:""}});click("Reset position");
 expect(left).toHaveValue(20);expect(screen.getByRole("spinbutton",{name:"Top (pt)"})).toHaveValue(30);expect(width).toHaveValue(null);expect(await ready()).toHaveTextContent("one draft");expect(screen.getByRole("button",{name:"Preview paragraph"})).toBeDisabled();
 fireEvent.change(width,{target:{value:"200"}});await preview();expect(adapter.previewStyledParagraph).toHaveBeenLastCalledWith("source",0,expect.objectContaining({bounds:selection.bounds}));
 click("Undo");if(moved){expect(screen.getByRole("spinbutton",{name:"Top (pt)"})).toHaveValue(60);expect(await ready()).toHaveTextContent("one draft");}else expect(await ready()).toHaveTextContent("one two");
});
it("resets an empty font-size draft when its format is already original",async()=>{
 setup();await ready();select(4,7);type("draft");select(0);
 const size=screen.getByRole("spinbutton",{name:"Font size (pt)"});fireEvent.change(size,{target:{value:""}});click("Reset font size");expect(size).toHaveValue(12);expect(screen.getByRole("button",{name:"Preview paragraph"})).toBeEnabled();expect(await ready()).toHaveTextContent("one draft");
});
