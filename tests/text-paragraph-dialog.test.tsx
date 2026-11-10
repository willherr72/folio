import {cleanup, fireEvent, render, screen, waitFor, act} from "@testing-library/react";
import {afterEach,describe,it,expect,vi} from "vitest";
import {TextParagraphDialog} from "../src/components/TextParagraphDialog";
import type {FolioAdapter} from "../src/editor/adapter";
import type {DocumentInfo,EditableTextRun,TextParagraphSelection,Overlay} from "../src/editor/types";

afterEach(()=>{cleanup();vi.restoreAllMocks();vi.unstubAllGlobals();});
const anchor:EditableTextRun={objectIndex:0,text:"First line.",fontName:"Helvetica",fontSize:12,bounds:{x:60,y:88,width:90,height:12},supported:true};
const selection:TextParagraphSelection={objectIndices:[0],text:anchor.text,fontName:"Helvetica",fontSize:12,bounds:{x:60,y:88,width:180,height:50}};
function setup(overrides:Partial<FolioAdapter>={},overlays:Overlay[]=[]) {
    const adapter:FolioAdapter={kind:"native",openPdf:vi.fn(),exportPdf:vi.fn(),engineStatus:vi.fn(),closeDocument:vi.fn().mockResolvedValue(undefined),
        listTextRuns:vi.fn().mockResolvedValue({runs:[anchor]}),inspectTextParagraph:vi.fn().mockResolvedValue(selection),
        previewTextParagraph:vi.fn().mockResolvedValue({id:"preview",name:"page.pdf",pages:[{width:400,height:500}]}),
        renderPage:vi.fn().mockResolvedValue("data:image/png;base64,AA"),...overrides};
    const onApply=vi.fn().mockReturnValue(true); const onCancel=vi.fn();
    const view=render(<TextParagraphDialog adapter={adapter} sourceId="source" pageIndex={0} pageWidth={400} pageHeight={500} overlays={overlays} anchor={anchor} onApply={onApply} onCancel={onCancel}/>);
    return {adapter,onApply,onCancel,...view};
}
async function check() { await waitFor(()=>expect(screen.getByRole("button",{name:"Check selection"})).toBeEnabled()); fireEvent.click(screen.getByRole("button",{name:"Check selection"})); await screen.findByRole("textbox",{name:"Paragraph text"}); }
it("keeps full source bounds, size and inferred spacing through compact display and focus",async()=>{
 const bounds={x:143.99998474121094,y:30.123456789,width:200.123456789,height:50.987654321};
 const precise={...selection,bounds,fontSize:12.123456789,lineSpacing:1.2200012345};
 const {adapter}=setup({suggestTextParagraph:vi.fn().mockResolvedValue(precise)});await screen.findByRole("textbox",{name:"Paragraph text"});
 for(const [label,exact,compact] of [["Left (pt)",bounds.x,144],["Font size (pt)",precise.fontSize,12.123],["Line spacing (\u00d7 font size)",precise.lineSpacing,1.22]] as const){const field=screen.getByRole("spinbutton",{name:label});expect(field).toHaveValue(compact);fireEvent.focus(field);expect(field).toHaveValue(exact);fireEvent.blur(field);expect(field).toHaveValue(compact);}
 fireEvent.click(screen.getByRole("button",{name:"Preview paragraph"}));await waitFor(()=>expect(adapter.previewTextParagraph).toHaveBeenLastCalledWith("source",0,[0],selection.text,selection.text,bounds));
 const left=screen.getByRole("spinbutton",{name:"Left (pt)"});fireEvent.focus(left);fireEvent.change(left,{target:{value:"145.123456789"}});fireEvent.blur(left);fireEvent.click(screen.getByRole("button",{name:"Preview paragraph"}));await waitFor(()=>expect(adapter.previewTextParagraph).toHaveBeenLastCalledWith("source",0,[0],selection.text,selection.text,{...bounds,x:145.123456789}));
 fireEvent.click(screen.getByRole("button",{name:"Reset position"}));fireEvent.click(screen.getByRole("button",{name:"Preview paragraph"}));await waitFor(()=>expect(adapter.previewTextParagraph).toHaveBeenLastCalledWith("source",0,[0],selection.text,selection.text,bounds));
});

describe("paragraph justification",()=>{
 it("offers justification, invalidates a prior candidate and retains it through canceled adjustment",async()=>{
  const {adapter}=setup();await check();fireEvent.click(screen.getByRole("button",{name:"Preview paragraph"}));await screen.findByRole("img",{name:"Paragraph PDF preview"});
  expect(screen.getByRole("option",{name:"Justify"})).toBeInTheDocument();fireEvent.change(screen.getByRole("combobox",{name:"Paragraph alignment"}),{target:{value:"justify"}});
  expect(screen.getByRole("button",{name:"Apply paragraph"})).toBeDisabled();expect(adapter.closeDocument).toHaveBeenCalledWith("preview");
  fireEvent.click(screen.getByRole("button",{name:"Adjust selection"}));fireEvent.click(screen.getByRole("button",{name:"Cancel adjustment"}));expect(screen.getByRole("combobox",{name:"Paragraph alignment"})).toHaveValue("justify");
  expect(screen.getByText(/Last lines and lines ending at a newline keep natural spacing/)).toBeVisible();
  fireEvent.click(screen.getByRole("button",{name:"Preview paragraph"}));await waitFor(()=>expect(adapter.previewTextParagraph).toHaveBeenLastCalledWith("source",0,[0],selection.text,selection.text,selection.bounds,undefined,"justify"));
 });
 it("uses inferred justification without forcing a new formatting override",async()=>{
  const {adapter}=setup({suggestTextParagraph:vi.fn().mockResolvedValue({...selection,alignment:"justify"})});await screen.findByRole("textbox",{name:"Paragraph text"});expect(screen.getByRole("combobox",{name:"Paragraph alignment"})).toHaveValue("justify");
  fireEvent.click(screen.getByRole("button",{name:"Preview paragraph"}));await waitFor(()=>expect(adapter.previewTextParagraph).toHaveBeenCalledWith("source",0,[0],selection.text,selection.text,selection.bounds));
 });
});

describe("paragraph line spacing",()=>{
 it("rounds inferred spacing for display while preserving exact defaults and explicit edits",async()=>{
  const {adapter}=setup({suggestTextParagraph:vi.fn().mockResolvedValue({...selection,lineSpacing:1.2200012})});await screen.findByRole("textbox",{name:"Paragraph text"});
  const spacing=screen.getByRole("spinbutton",{name:"Line spacing (× font size)"});expect(spacing).toHaveValue(1.22);expect(screen.getByRole("button",{name:"Reset line spacing"})).toBeDisabled();
  fireEvent.click(screen.getByRole("button",{name:"Preview paragraph"}));await screen.findByRole("img",{name:"Paragraph PDF preview"});
  expect(adapter.previewTextParagraph).toHaveBeenLastCalledWith("source",0,[0],selection.text,selection.text,selection.bounds);
  fireEvent.change(spacing,{target:{value:"1.5"}});fireEvent.change(spacing,{target:{value:"1.22"}});
  fireEvent.click(screen.getByRole("button",{name:"Preview paragraph"}));await screen.findByRole("img",{name:"Paragraph PDF preview"});
  expect(adapter.previewTextParagraph).toHaveBeenLastCalledWith("source",0,[0],selection.text,selection.text,selection.bounds,undefined,undefined,1.22);
  fireEvent.click(screen.getByRole("button",{name:"Reset line spacing"}));expect(spacing).toHaveValue(1.22);
  fireEvent.click(screen.getByRole("button",{name:"Preview paragraph"}));await waitFor(()=>expect(adapter.previewTextParagraph).toHaveBeenLastCalledWith("source",0,[0],selection.text,selection.text,selection.bounds));
 });
 it("initializes inferred spacing, forwards changed formatting, invalidates and resets only spacing",async()=>{
  const {adapter}=setup({suggestTextParagraph:vi.fn().mockResolvedValue({...selection,lineSpacing:1.5})});
  await screen.findByRole("textbox",{name:"Paragraph text"});const spacing=screen.getByRole("spinbutton",{name:"Line spacing (× font size)"});expect(spacing).toHaveValue(1.5);
  fireEvent.change(spacing,{target:{value:"2"}});fireEvent.change(screen.getByRole("spinbutton",{name:"Font size (pt)"}),{target:{value:"16"}});fireEvent.change(screen.getByRole("combobox",{name:"Paragraph alignment"}),{target:{value:"right"}});
  fireEvent.click(screen.getByRole("button",{name:"Preview paragraph"}));await screen.findByRole("img",{name:"Paragraph PDF preview"});
  expect(adapter.previewTextParagraph).toHaveBeenLastCalledWith("source",0,[0],selection.text,selection.text,selection.bounds,16,"right",2);
  fireEvent.change(spacing,{target:{value:"1"}});expect(adapter.closeDocument).toHaveBeenCalledWith("preview");expect(screen.getByRole("button",{name:"Apply paragraph"})).toBeDisabled();
  fireEvent.click(screen.getByRole("button",{name:"Reset line spacing"}));expect(spacing).toHaveValue(1.5);expect(screen.getByRole("spinbutton",{name:"Font size (pt)"})).toHaveValue(16);
  fireEvent.click(screen.getByRole("button",{name:"Preview paragraph"}));await waitFor(()=>expect(adapter.previewTextParagraph).toHaveBeenLastCalledWith("source",0,[0],selection.text,selection.text,selection.bounds,16,"right"));
 });
 it.each(["","0.5","3.1","-1"])("retains invalid spacing %s and prevents preview",async value=>{
  const {adapter}=setup();await check();const spacing=screen.getByRole("spinbutton",{name:"Line spacing (× font size)"});expect(spacing).toHaveValue(1.2);fireEvent.change(spacing,{target:{value}});
  expect(screen.getByRole("button",{name:"Preview paragraph"})).toBeDisabled();expect(screen.getByText("Use line spacing from 0.6 to 3 times the font size.")).toBeInTheDocument();expect(adapter.previewTextParagraph).not.toHaveBeenCalled();
 });
 it("retains spacing on overflow and canceled adjustment, resets for a newly checked selection",async()=>{
  const next={...anchor,objectIndex:1,text:"Other line."};
  const {adapter}=setup({listTextRuns:vi.fn().mockResolvedValue({runs:[anchor,next]}),inspectTextParagraph:vi.fn().mockResolvedValue({...selection,objectIndices:[1],text:next.text}),suggestTextParagraph:vi.fn().mockResolvedValue({...selection,lineSpacing:1.5}),previewTextParagraph:vi.fn().mockRejectedValue(new Error("Text does not fit."))});await screen.findByRole("textbox",{name:"Paragraph text"});
  fireEvent.change(screen.getByRole("spinbutton",{name:"Line spacing (× font size)"}),{target:{value:"3"}});fireEvent.click(screen.getByRole("button",{name:"Preview paragraph"}));await screen.findByRole("alert");
  fireEvent.click(screen.getByRole("button",{name:"Adjust selection"}));fireEvent.click(screen.getByRole("button",{name:"Cancel adjustment"}));expect(screen.getByRole("spinbutton",{name:"Line spacing (× font size)"})).toHaveValue(3);
  fireEvent.click(screen.getByRole("button",{name:"Preview paragraph"}));await screen.findByRole("alert");expect(adapter.previewTextParagraph).toHaveBeenLastCalledWith("source",0,[0],selection.text,selection.text,selection.bounds,undefined,undefined,3);
  fireEvent.click(screen.getByRole("button",{name:"Adjust selection"}));await check();expect(screen.getByRole("spinbutton",{name:"Line spacing (× font size)"})).toHaveValue(3);expect(adapter.inspectTextParagraph).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button",{name:"Preview paragraph"}));await screen.findByRole("alert");expect(adapter.previewTextParagraph).toHaveBeenLastCalledWith("source",0,[0],selection.text,selection.text,selection.bounds,undefined,undefined,3);
  fireEvent.click(screen.getByRole("button",{name:"Adjust selection"}));fireEvent.click(screen.getByRole("checkbox",{name:anchor.text}));fireEvent.click(screen.getByRole("checkbox",{name:next.text}));await check();expect(screen.getByRole("spinbutton",{name:"Line spacing (× font size)"})).toHaveValue(1.2);expect(screen.getByRole("textbox",{name:"Paragraph text"})).toHaveValue(next.text);expect(adapter.inspectTextParagraph).toHaveBeenCalledWith("source",0,[1]);
  fireEvent.click(screen.getByRole("button",{name:"Preview paragraph"}));await screen.findByRole("alert");expect(adapter.previewTextParagraph).toHaveBeenLastCalledWith("source",0,[1],next.text,next.text,selection.bounds);
 });
 it("releases a late source when spacing changes during preview",async()=>{
  let finish!:(value:DocumentInfo)=>void;const {adapter}=setup({previewTextParagraph:vi.fn().mockImplementation(()=>new Promise(resolve=>finish=resolve))});await check();
  fireEvent.click(screen.getByRole("button",{name:"Preview paragraph"}));fireEvent.change(screen.getByRole("spinbutton",{name:"Line spacing (× font size)"}),{target:{value:"2"}});
  await act(async()=>finish({id:"late",name:"page.pdf",pages:[{width:400,height:500}]}));expect(adapter.closeDocument).toHaveBeenCalledWith("late");expect(screen.getByRole("button",{name:"Apply paragraph"})).toBeDisabled();
 });
});

describe("paragraph alignment",()=>{
 it("initializes from inferred alignment and forwards an explicit changed alignment with size",async()=>{
  const {adapter}=setup({suggestTextParagraph:vi.fn().mockResolvedValue({...selection,alignment:"center"})});await screen.findByRole("textbox",{name:"Paragraph text"});const align=screen.getByRole("combobox",{name:"Paragraph alignment"});expect(align).toHaveValue("center");
  fireEvent.change(align,{target:{value:"right"}});fireEvent.change(screen.getByRole("spinbutton",{name:"Font size (pt)"}),{target:{value:"18"}});fireEvent.click(screen.getByRole("button",{name:"Preview paragraph"}));await screen.findByRole("img",{name:"Paragraph PDF preview"});
  expect(adapter.previewTextParagraph).toHaveBeenCalledWith("source",0,[0],selection.text,selection.text,selection.bounds,18,"right");
  fireEvent.change(align,{target:{value:"left"}});expect(adapter.closeDocument).toHaveBeenCalledWith("preview");expect(screen.getByRole("button",{name:"Apply paragraph"})).toBeDisabled();
  fireEvent.click(screen.getByRole("button",{name:"Adjust selection"}));fireEvent.click(screen.getByRole("button",{name:"Cancel adjustment"}));expect(screen.getByRole("combobox",{name:"Paragraph alignment"})).toHaveValue("left");
 });
 it("defaults unknown alignment to left and restores a new selection's alignment",async()=>{
  const next={...anchor,objectIndex:1,text:"Second line."};setup({listTextRuns:vi.fn().mockResolvedValue({runs:[anchor,next]}),suggestTextParagraph:vi.fn().mockResolvedValue(selection),inspectTextParagraph:vi.fn().mockResolvedValue({...selection,objectIndices:[1],text:next.text,alignment:"right"})});
  await screen.findByRole("textbox",{name:"Paragraph text"});expect(screen.getByRole("combobox",{name:"Paragraph alignment"})).toHaveValue("left");fireEvent.change(screen.getByRole("combobox",{name:"Paragraph alignment"}),{target:{value:"center"}});fireEvent.click(screen.getByRole("button",{name:"Adjust selection"}));fireEvent.click(screen.getByRole("checkbox",{name:anchor.text}));fireEvent.click(screen.getByRole("checkbox",{name:next.text}));await check();expect(screen.getByRole("combobox",{name:"Paragraph alignment"})).toHaveValue("right");
 });
 it("releases source and late raster after alignment changes",async()=>{
  let finish!:(url:string)=>void;const revoke=vi.fn();vi.stubGlobal("URL",class extends URL {static revokeObjectURL=revoke;});const raster=vi.fn().mockImplementation(source=>source==="preview"?new Promise(resolve=>finish=resolve):Promise.resolve("data:original"));
  const {adapter,unmount}=setup({renderPage:raster});await check();fireEvent.click(screen.getByRole("button",{name:"Preview paragraph"}));await waitFor(()=>expect(raster).toHaveBeenCalledWith("preview",0,1600));fireEvent.change(screen.getByRole("combobox",{name:"Paragraph alignment"}),{target:{value:"center"}});await act(async()=>finish("blob:late-align"));unmount();expect(adapter.closeDocument).toHaveBeenCalledOnce();expect(adapter.closeDocument).toHaveBeenCalledWith("preview");expect(revoke).toHaveBeenCalledOnce();expect(revoke).toHaveBeenCalledWith("blob:late-align");
 });
});

describe("paragraph font size",()=>{
 it("previews chosen size, invalidates the candidate on resize, and resets to original",async()=>{
  const {adapter}=setup();await check();const size=screen.getByRole("spinbutton",{name:"Font size (pt)"});expect(size).toHaveValue(12);
  fireEvent.change(size,{target:{value:"18"}});fireEvent.click(screen.getByRole("button",{name:"Preview paragraph"}));await screen.findByRole("img",{name:"Paragraph PDF preview"});
  expect(adapter.previewTextParagraph).toHaveBeenLastCalledWith("source",0,[0],"First line.","First line.",selection.bounds,18);
  fireEvent.change(size,{target:{value:"10"}});expect(adapter.closeDocument).toHaveBeenCalledWith("preview");expect(screen.getByRole("button",{name:"Apply paragraph"})).toBeDisabled();
  fireEvent.click(screen.getByRole("button",{name:"Reset font size"}));expect(size).toHaveValue(12);
  fireEvent.click(screen.getByRole("button",{name:"Preview paragraph"}));await waitFor(()=>expect(adapter.previewTextParagraph).toHaveBeenLastCalledWith("source",0,[0],"First line.","First line.",selection.bounds));
 });
 it.each(["","0","301","-1"])("rejects invalid font size %s before preview",async(value)=>{
  const {adapter}=setup();await check();fireEvent.change(screen.getByRole("spinbutton",{name:"Font size (pt)"}),{target:{value}});
  expect(screen.getByRole("button",{name:"Preview paragraph"})).toBeDisabled();expect(screen.getByText("Use a font size from 1 to 300 pt.")).toBeInTheDocument();expect(adapter.previewTextParagraph).not.toHaveBeenCalled();
 });
 it("preserves chosen size and text after overflow and canceled selection adjustment",async()=>{
  setup({previewTextParagraph:vi.fn().mockRejectedValue(new Error("Text does not fit."))});await check();const size=screen.getByRole("spinbutton",{name:"Font size (pt)"});
  fireEvent.change(size,{target:{value:"24"}});fireEvent.change(screen.getByRole("textbox",{name:"Paragraph text"}),{target:{value:"My draft"}});fireEvent.click(screen.getByRole("button",{name:"Preview paragraph"}));await screen.findByRole("alert");
  fireEvent.click(screen.getByRole("button",{name:"Adjust selection"}));fireEvent.click(screen.getByRole("button",{name:"Cancel adjustment"}));expect(screen.getByRole("spinbutton",{name:"Font size (pt)"})).toHaveValue(24);expect(screen.getByRole("textbox",{name:"Paragraph text"})).toHaveValue("My draft");
 });
 it("releases a late preview after the font size changes",async()=>{
  let finish!:(value:DocumentInfo)=>void;const {adapter}=setup({previewTextParagraph:vi.fn().mockImplementation(()=>new Promise(resolve=>finish=resolve))});await check();
  fireEvent.click(screen.getByRole("button",{name:"Preview paragraph"}));fireEvent.change(screen.getByRole("spinbutton",{name:"Font size (pt)"}),{target:{value:"18"}});
  await act(async()=>finish({id:"late",name:"page.pdf",pages:[{width:400,height:500}]}));expect(adapter.closeDocument).toHaveBeenCalledWith("late");expect(screen.getByRole("button",{name:"Apply paragraph"})).toBeDisabled();
 });
 it("releases both candidate source and late raster after a size edit",async()=>{
  let finish!:(url:string)=>void;const revoke=vi.fn();vi.stubGlobal("URL",class extends URL {static revokeObjectURL=revoke;});
  const raster=vi.fn().mockImplementation(source=>source==="preview"?new Promise(resolve=>finish=resolve):Promise.resolve("data:original"));
  const {adapter,unmount}=setup({renderPage:raster});await check();fireEvent.click(screen.getByRole("button",{name:"Preview paragraph"}));await waitFor(()=>expect(raster).toHaveBeenCalledWith("preview",0,1600));
  fireEvent.change(screen.getByRole("spinbutton",{name:"Font size (pt)"}),{target:{value:"18"}});await act(async()=>finish("blob:late-size"));unmount();
  expect(adapter.closeDocument).toHaveBeenCalledOnce();expect(adapter.closeDocument).toHaveBeenCalledWith("preview");expect(revoke).toHaveBeenCalledOnce();expect(revoke).toHaveBeenCalledWith("blob:late-size");
 });
 it("uses the new selection's original size when checking different pieces",async()=>{
  const next={...anchor,objectIndex:1,text:"Other line."};setup({listTextRuns:vi.fn().mockResolvedValue({runs:[anchor,next]}),suggestTextParagraph:vi.fn().mockResolvedValue(selection),inspectTextParagraph:vi.fn().mockResolvedValue({...selection,objectIndices:[1],text:next.text,fontSize:16})});
  await screen.findByRole("textbox",{name:"Paragraph text"});fireEvent.change(screen.getByRole("spinbutton",{name:"Font size (pt)"}),{target:{value:"24"}});fireEvent.click(screen.getByRole("button",{name:"Adjust selection"}));
  fireEvent.click(screen.getByRole("checkbox",{name:anchor.text}));fireEvent.click(screen.getByRole("checkbox",{name:next.text}));await check();
  expect(screen.getByRole("spinbutton",{name:"Font size (pt)"})).toHaveValue(16);expect(screen.getByRole("textbox",{name:"Paragraph text"})).toHaveValue(next.text);
 });
});

describe("suggested paragraph selection",()=>{
 const next={...anchor,objectIndex:1,text:"Second line.",bounds:{...anchor.bounds,y:106}};
 const suggestion={...selection,objectIndices:[0,1],text:"First line. Second line."};
 it("opens with the validated paragraph and still requires Preview before Apply",async()=>{
  const suggestTextParagraph=vi.fn().mockResolvedValue(suggestion);
  const {adapter,onApply}=setup({listTextRuns:vi.fn().mockResolvedValue({runs:[anchor,next]}),suggestTextParagraph});
  expect(await screen.findByRole("textbox",{name:"Paragraph text"})).toHaveValue(suggestion.text);
  expect(suggestTextParagraph).toHaveBeenCalledWith("source",0,0,"First line.");
  expect(screen.queryAllByRole("checkbox")).toHaveLength(0);
  expect(screen.queryByRole("button",{name:"Check selection"})).not.toBeInTheDocument();
  expect(screen.getByText(/Suggested paragraph/)).toBeInTheDocument();
  expect(screen.getByRole("button",{name:"Apply paragraph"})).toBeDisabled();
  expect(adapter.inspectTextParagraph).not.toHaveBeenCalled();expect(adapter.previewTextParagraph).not.toHaveBeenCalled();expect(onApply).not.toHaveBeenCalled();
  fireEvent.change(screen.getByRole("textbox",{name:"Paragraph text"}),{target:{value:"Edited paragraph."}});
  fireEvent.click(screen.getByRole("button",{name:"Preview paragraph"}));
  await screen.findByRole("img",{name:"Paragraph PDF preview"});
  expect(adapter.previewTextParagraph).toHaveBeenCalledWith("source",0,[0,1],suggestion.text,"Edited paragraph.",suggestion.bounds);
 });
 it("lets the user adjust a suggestion and validates their corrected selection",async()=>{
  const {adapter}=setup({listTextRuns:vi.fn().mockResolvedValue({runs:[anchor,next]}),suggestTextParagraph:vi.fn().mockResolvedValue(suggestion)});
  await screen.findByRole("textbox",{name:"Paragraph text"});
  fireEvent.click(screen.getByRole("button",{name:"Adjust selection"}));
  expect(screen.queryByRole("textbox",{name:"Paragraph text"})).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("checkbox",{name:"Second line."}));await check();
  expect(adapter.inspectTextParagraph).toHaveBeenCalledWith("source",0,[0]);
  expect(screen.getByRole("textbox",{name:"Paragraph text"})).toHaveValue(anchor.text);
 });
 it("falls back to manual pieces when native suggestion cannot establish a uniform paragraph",async()=>{
  setup({suggestTextParagraph:vi.fn().mockRejectedValue(new Error("Mixed formatting."))});
  expect(await screen.findByRole("alert")).toHaveTextContent("Mixed formatting.");
  expect(screen.getByText(/Select the paragraph pieces manually/)).toBeInTheDocument();
  await check();expect(screen.getByRole("textbox",{name:"Paragraph text"})).toHaveValue(anchor.text);
 });
 it("ignores a suggestion that completes after Cancel even before the parent unmounts",async()=>{
  let finish!:(value:TextParagraphSelection)=>void;
  const {onCancel}=setup({suggestTextParagraph:vi.fn().mockImplementation(()=>new Promise(resolve=>finish=resolve))});
  fireEvent.click(screen.getByRole("button",{name:"Cancel"}));
  await act(async()=>finish(suggestion));
  expect(onCancel).toHaveBeenCalledOnce();expect(screen.queryByRole("textbox",{name:"Paragraph text"})).not.toBeInTheDocument();
 });
 it("cannot apply an old page suggestion after the dialog switches sources",async()=>{
  let finish!:(value:TextParagraphSelection)=>void;
  const suggestTextParagraph=vi.fn().mockImplementationOnce(()=>new Promise(resolve=>finish=resolve)).mockResolvedValueOnce({...selection,text:"Current page"});
  const {adapter,onApply,onCancel,rerender}=setup({suggestTextParagraph});
  rerender(<TextParagraphDialog adapter={adapter} sourceId="new-source" pageIndex={0} pageWidth={400} pageHeight={500} anchor={anchor} onApply={onApply} onCancel={onCancel}/>);
  expect(await screen.findByRole("textbox",{name:"Paragraph text"})).toHaveValue("Current page");
  await act(async()=>finish(suggestion));
  expect(screen.getByRole("textbox",{name:"Paragraph text"})).toHaveValue("Current page");expect(onApply).not.toHaveBeenCalled();
 });
});
describe("paragraph preview ownership",()=>{
    it("requires a native preview and transfers exactly that source on Apply",async()=>{
        const {adapter,onApply,unmount}=setup(); await check();
        fireEvent.change(screen.getByRole("textbox",{name:"Paragraph text"}),{target:{value:"First this line."}});
        expect(screen.getByRole("button",{name:"Apply paragraph"})).toBeDisabled();
        fireEvent.click(screen.getByRole("button",{name:"Preview paragraph"}));
        await screen.findByRole("img",{name:"Paragraph PDF preview"});
        expect(adapter.previewTextParagraph).toHaveBeenCalledWith("source",0,[0],"First line.","First this line.",selection.bounds);
        fireEvent.click(screen.getByRole("button",{name:"Apply paragraph"}));
        expect(onApply).toHaveBeenCalledWith(expect.objectContaining({id:"preview"}));
        unmount(); expect(adapter.closeDocument).not.toHaveBeenCalled();
    });
    it("invalidates previews on numeric drafts and retains text on overflow",async()=>{
        const {adapter}=setup(); await check(); fireEvent.click(screen.getByRole("button",{name:"Preview paragraph"}));
        await screen.findByRole("img",{name:"Paragraph PDF preview"});
        fireEvent.change(screen.getByRole("spinbutton",{name:"Area width (pt)"}),{target:{value:""}});
        expect(adapter.closeDocument).toHaveBeenCalledWith("preview");
        expect(screen.getByRole("button",{name:"Apply paragraph"})).toBeDisabled();
        fireEvent.change(screen.getByRole("spinbutton",{name:"Area width (pt)"}),{target:{value:"10"}});
        vi.mocked(adapter.previewTextParagraph!).mockRejectedValueOnce(new Error("The text does not fit."));
        fireEvent.click(screen.getByRole("button",{name:"Preview paragraph"}));
        expect(await screen.findByRole("alert")).toHaveTextContent("does not fit");
        expect(screen.getByRole("textbox",{name:"Paragraph text"})).toHaveValue("First line.");
    });
    it("closes a candidate that arrives after cancellation/unmount",async()=>{
        let resolve!:(v:DocumentInfo)=>void;
        const {adapter,unmount}=setup({previewTextParagraph:vi.fn(()=>new Promise<DocumentInfo>(r=>resolve=r))});
        await check(); fireEvent.click(screen.getByRole("button",{name:"Preview paragraph"})); unmount();
        await act(async()=>resolve({id:"late",name:"page.pdf",pages:[{width:400,height:500}]}));
        expect(adapter.closeDocument).toHaveBeenCalledWith("late"); expect(adapter.renderPage).not.toHaveBeenCalledWith("late",expect.anything(),expect.anything());
    });
    it("closes a candidate after render failure",async()=>{
        const {adapter}=setup({renderPage:vi.fn().mockImplementation(async id=>{if(id==="preview")throw new Error("Render failed");return "data:image/png;base64,AA";})});
        await check(); fireEvent.click(screen.getByRole("button",{name:"Preview paragraph"}));
        expect(await screen.findByRole("alert")).toHaveTextContent("Render failed");
        expect(adapter.closeDocument).toHaveBeenCalledWith("preview");
    });
});

it("selects from the page and preserves the candidate while comparing and zooming",async()=>{
    const {adapter}=setup();
    const target=await screen.findByRole("button",{name:"Select text piece: First line."});
    fireEvent.click(target);expect(screen.getByRole("checkbox")).not.toBeChecked();
    fireEvent.click(target);expect(screen.getByRole("checkbox")).toBeChecked();
    await check();expect(screen.queryByRole("button",{name:"Select text piece: First line."})).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button",{name:"Preview paragraph"}));await screen.findByRole("img",{name:"Paragraph PDF preview"});
    fireEvent.click(screen.getByRole("button",{name:"Original"}));await screen.findByRole("img",{name:"Original PDF page"});
    fireEvent.change(screen.getByRole("combobox",{name:"Preview zoom"}),{target:{value:"200"}});
    fireEvent.click(screen.getByRole("button",{name:"Edited"}));
    expect(screen.getByRole("button",{name:"Apply paragraph"})).toBeEnabled();
    expect(adapter.previewTextParagraph).toHaveBeenCalledTimes(1);expect(adapter.closeDocument).not.toHaveBeenCalled();
});

it("retains the draft and releases a candidate whose annotated preview fails",async()=>{
 const composition=vi.fn().mockImplementation(async plan=>{if(plan.sourceId==="preview")throw new Error("Annotation preview failed");return "data:image/png;base64,ORIGINAL";});
 const {adapter}=setup({renderPagePlan:composition},[{type:"comment",id:"note",x:10,y:10,text:"Keep",color:"#FFFF00"}]);
 await check();fireEvent.click(screen.getByRole("button",{name:"Preview paragraph"}));
 expect(await screen.findByRole("alert")).toHaveTextContent("Annotation preview failed");
 expect(screen.getByRole("button",{name:"Apply paragraph"})).toBeDisabled();
 expect(screen.getByRole("textbox",{name:"Paragraph text"})).toHaveValue("First line.");
 expect(adapter.closeDocument).toHaveBeenCalledWith("preview");expect(adapter.renderPage).not.toHaveBeenCalled();
});
it("explains the selection limit and restores page targets when a selected checkbox is cleared",async()=>{
 const runs=Array.from({length:65},(_,index)=>({...anchor,objectIndex:index,text:`Piece ${index}`}));
 setup({listTextRuns:vi.fn().mockResolvedValue({runs})});
 await screen.findByRole("button",{name:"Select text piece: Piece 64"});
 const checkboxes=screen.getAllByRole("checkbox");checkboxes.slice(1,64).forEach(box=>fireEvent.click(box));
 expect(screen.getByRole("status")).toHaveTextContent("64 pieces selected");
 expect(screen.getByRole("status")).toHaveTextContent("Deselect a piece");
 expect(checkboxes[64]).toBeDisabled();
 const target=screen.getByRole("button",{name:"Select text piece: Piece 64"});
 expect(target).toHaveAttribute("aria-disabled","true");
 fireEvent.click(checkboxes[0]);expect(checkboxes[64]).toBeEnabled();
 expect(target).toHaveAttribute("aria-disabled","false");expect(screen.queryByRole("status")).not.toBeInTheDocument();
 fireEvent.keyDown(target,{key:"Enter"});expect(checkboxes[64]).toBeChecked();
});
it("releases the late annotated raster and existing candidate exactly once after cancel",async()=>{
 let finish!:(url:string)=>void;
 const composition=vi.fn().mockImplementation(plan=>plan.sourceId==="preview"?new Promise<string>(resolve=>finish=resolve):Promise.resolve("data:image/png;base64,ORIGINAL"));
 const revoke=vi.fn();vi.stubGlobal("URL",class extends URL {static revokeObjectURL=revoke;});
 const {adapter,onApply,onCancel,unmount}=setup({renderPagePlan:composition},[{type:"comment",id:"note",x:10,y:10,text:"Keep",color:"#FFFF00"}]);
 await check();fireEvent.click(screen.getByRole("button",{name:"Preview paragraph"}));
 await waitFor(()=>expect(composition).toHaveBeenCalledWith(expect.objectContaining({sourceId:"preview",overlays:[expect.objectContaining({id:"note"})]}),1600));
 fireEvent.click(screen.getByRole("button",{name:"Cancel"}));expect(onCancel).toHaveBeenCalledOnce();unmount();
 await act(async()=>finish("blob:late-composite"));
 expect(onApply).not.toHaveBeenCalled();expect(adapter.closeDocument).toHaveBeenCalledOnce();expect(adapter.closeDocument).toHaveBeenCalledWith("preview");
 expect(revoke).toHaveBeenCalledOnce();expect(revoke).toHaveBeenCalledWith("blob:late-composite");
});
it("moves a paragraph numerically, invalidates its preview and resets only its position",async()=>{
 const {adapter}=setup();await check();
 const left=screen.getByRole("spinbutton",{name:"Left (pt)"}),top=screen.getByRole("spinbutton",{name:"Top (pt)"});
 expect(left).toHaveValue(60);expect(top).toHaveValue(88);expect(screen.getByRole("button",{name:"Reset position"})).toBeDisabled();
 fireEvent.change(screen.getByRole("textbox",{name:"Paragraph text"}),{target:{value:"Moved words"}});
 fireEvent.click(screen.getByRole("button",{name:"Preview paragraph"}));await screen.findByRole("img",{name:"Paragraph PDF preview"});
 fireEvent.change(left,{target:{value:"100"}});expect(adapter.closeDocument).toHaveBeenCalledOnce();expect(screen.getByRole("button",{name:"Apply paragraph"})).toBeDisabled();
 fireEvent.change(top,{target:{value:"120"}});fireEvent.change(screen.getByRole("spinbutton",{name:"Area width (pt)"}),{target:{value:"200"}});
 fireEvent.click(screen.getByRole("button",{name:"Preview paragraph"}));await screen.findByRole("img",{name:"Paragraph PDF preview"});
 expect(adapter.previewTextParagraph).toHaveBeenLastCalledWith("source",0,[0],"First line.","Moved words",{x:100,y:120,width:200,height:50});
 fireEvent.click(screen.getByRole("button",{name:"Reset position"}));expect(left).toHaveValue(60);expect(top).toHaveValue(88);
 expect(screen.getByRole("spinbutton",{name:"Area width (pt)"})).toHaveValue(200);expect(screen.getByRole("textbox",{name:"Paragraph text"})).toHaveValue("Moved words");
 expect(adapter.closeDocument).toHaveBeenCalledTimes(2);expect(screen.getByRole("button",{name:"Apply paragraph"})).toBeDisabled();
});
it("retains invalid position drafts and requires the entire area to fit before preview",async()=>{
 const {adapter}=setup();await check();const left=screen.getByRole("spinbutton",{name:"Left (pt)"});
 for(const value of ["","-1","399"]){fireEvent.change(left,{target:{value}});expect(screen.getByRole("button",{name:"Preview paragraph"})).toBeDisabled();}
 expect(left).toHaveValue(399);expect(screen.getByText("Keep the entire paragraph area inside the page.")).toBeInTheDocument();
 expect(adapter.previewTextParagraph).not.toHaveBeenCalled();fireEvent.change(left,{target:{value:"220"}});
 expect(screen.getByRole("button",{name:"Preview paragraph"})).toBeEnabled();
 fireEvent.change(screen.getByRole("spinbutton",{name:"Top (pt)"}),{target:{value:"451"}});expect(screen.getByRole("button",{name:"Preview paragraph"})).toBeDisabled();
 fireEvent.click(screen.getByRole("button",{name:"Reset position"}));expect(screen.getByRole("button",{name:"Preview paragraph"})).toBeEnabled();
});
