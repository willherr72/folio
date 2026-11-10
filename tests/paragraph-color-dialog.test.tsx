import {act,cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {afterEach,expect,it,vi} from "vitest";
import {TextParagraphDialog} from "../src/components/TextParagraphDialog";
import type {FolioAdapter} from "../src/editor/adapter";
import type {DocumentInfo,EditableTextRun} from "../src/editor/types";
afterEach(()=>{cleanup();vi.restoreAllMocks();});
const anchor:EditableTextRun={objectIndex:0,text:"First line.",fontName:"Helvetica",fontSize:12,bounds:{x:60,y:88,width:90,height:12},supported:true};
const selection={objectIndices:[0],text:anchor.text,fontName:"Helvetica",fontSize:12,bounds:{x:60,y:88,width:180,height:70},color:"#336699"};
async function setup(overrides:Partial<FolioAdapter>={}) {
 const adapter:FolioAdapter={kind:"native",openPdf:vi.fn(),exportPdf:vi.fn(),engineStatus:vi.fn(),closeDocument:vi.fn().mockResolvedValue(undefined),listTextRuns:vi.fn().mockResolvedValue({runs:[anchor,{...anchor,objectIndex:1,text:"Second"}]}),inspectTextParagraph:vi.fn().mockResolvedValue({...selection,objectIndices:[0,1],color:"#CC0000"}),suggestTextParagraph:vi.fn().mockResolvedValue(selection),previewTextParagraph:vi.fn().mockResolvedValue({id:"preview",name:"page.pdf",pages:[{width:400,height:500}]}),renderPage:vi.fn().mockResolvedValue("data:image/png;base64,AA"),...overrides};
 const view=render(<TextParagraphDialog adapter={adapter} sourceId="source" pageIndex={0} pageWidth={400} pageHeight={500} anchor={anchor} onApply={()=>true} onCancel={()=>{}}/>);
 await screen.findByRole("textbox",{name:"Paragraph text"});return {...view,adapter};
}
const click=(name:string)=>fireEvent.click(screen.getByRole("button",{name}));
const choose=(value:string)=>fireEvent.change(screen.getByLabelText("Text color"),{target:{value}});
async function preview(){click("Preview paragraph");await screen.findByRole("img",{name:"Paragraph PDF preview"});}
it("shows original color, sends only explicit changes and resets while releasing the preview",async()=>{
 const {adapter}=await setup();expect(screen.getByLabelText("Text color")).toHaveValue("#336699");
 await preview();expect(adapter.previewTextParagraph).toHaveBeenLastCalledWith("source",0,[0],selection.text,selection.text,selection.bounds);
 choose("#1a80ef");expect(adapter.closeDocument).toHaveBeenCalledWith("preview");expect(screen.getByRole("button",{name:"Apply paragraph"})).toBeDisabled();
 await preview();expect(adapter.previewTextParagraph).toHaveBeenLastCalledWith("source",0,[0],selection.text,selection.text,selection.bounds,undefined,undefined,undefined,undefined,"#1A80EF");
 click("Use original color");expect(screen.getByLabelText("Text color")).toHaveValue("#336699");await preview();
 expect(adapter.previewTextParagraph).toHaveBeenLastCalledWith("source",0,[0],selection.text,selection.text,selection.bounds);
});
it("releases a stale colored preview after resetting color",async()=>{
 let resolve!:(doc:DocumentInfo)=>void;
 const {adapter}=await setup({previewTextParagraph:vi.fn().mockImplementation(()=>new Promise(done=>resolve=done))});
 choose("#cc0000");click("Preview paragraph");click("Use original color");
 await act(async()=>resolve({id:"stale",name:"page.pdf",pages:[]}));
 await waitFor(()=>expect(adapter.closeDocument).toHaveBeenCalledWith("stale"));expect(screen.getByRole("button",{name:"Apply paragraph"})).toBeDisabled();
});
it("keeps color when adjustment is cancelled and resets it when membership changes",async()=>{
 await setup();choose("#112233");await preview();click("Adjust selection");click("Cancel adjustment");
 expect(screen.getByLabelText("Text color")).toHaveValue("#112233");expect(screen.getByRole("button",{name:"Apply paragraph"})).toBeEnabled();
 click("Adjust selection");fireEvent.click(screen.getByRole("checkbox",{name:"Second"}));click("Check selection");
 expect(await screen.findByLabelText("Text color")).toHaveValue("#cc0000");expect(screen.getByRole("button",{name:"Use original color"})).toBeDisabled();
});
it("does not silently recolor older payloads without color metadata",async()=>{
 const {adapter}=await setup({suggestTextParagraph:vi.fn().mockResolvedValue({...selection,color:undefined})});
 await preview();expect(adapter.previewTextParagraph).toHaveBeenLastCalledWith("source",0,[0],selection.text,selection.text,selection.bounds);
 choose("#ffffff");await preview();expect(vi.mocked(adapter.previewTextParagraph!).mock.calls.at(-1)?.at(-1)).toBe("#FFFFFF");
});
