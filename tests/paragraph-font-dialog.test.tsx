import {act,cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {TextParagraphDialog} from "../src/components/TextParagraphDialog";
import {fontApi,flushFontReleases,type FontInfo} from "../src/editor/custom-fonts";
import type {FolioAdapter} from "../src/editor/adapter";
import type {DocumentInfo,EditableTextRun,TextParagraphSelection} from "../src/editor/types";

const chosen=vi.hoisted(()=>({id:"a".repeat(64),name:"Test Serif",weight:400,italic:false,coverage:[[32,255]]}));
vi.mock("../src/components/FontPicker",()=>({FontPicker:({onChoose,onClose}:{onChoose:(f:FontInfo)=>void;onClose:()=>void})=><div><button onClick={()=>onChoose(chosen as FontInfo)}>Choose test font</button><button onClick={onClose}>Cancel font picker</button></div>}));
beforeEach(()=>{vi.spyOn(fontApi,"release").mockResolvedValue(undefined);});
afterEach(async()=>{cleanup();await flushFontReleases();vi.restoreAllMocks();});
const anchor:EditableTextRun={objectIndex:0,text:"First line.",fontName:"Helvetica",fontSize:12,bounds:{x:60,y:88,width:90,height:12},supported:true};
const selection:TextParagraphSelection={objectIndices:[0],text:anchor.text,fontName:"Helvetica",fontSize:12,bounds:{x:60,y:88,width:180,height:70}};
function setup(overrides:Partial<FolioAdapter>={}) {
 const adapter:FolioAdapter={kind:"native",openPdf:vi.fn(),exportPdf:vi.fn(),engineStatus:vi.fn(),closeDocument:vi.fn().mockResolvedValue(undefined),listTextRuns:vi.fn().mockResolvedValue({runs:[anchor,{...anchor,objectIndex:1,text:"Second"}]}),inspectTextParagraph:vi.fn().mockResolvedValue({...selection,objectIndices:[1],text:"Second"}),suggestTextParagraph:vi.fn().mockResolvedValue(selection),previewTextParagraph:vi.fn().mockResolvedValue({id:"preview",name:"page.pdf",pages:[{width:400,height:500}]}),renderPage:vi.fn().mockResolvedValue("data:image/png;base64,AA"),...overrides};
 const props={adapter,sourceId:"source",pageIndex:0,pageWidth:400,pageHeight:500,anchor,onApply:vi.fn().mockReturnValue(true),onCancel:vi.fn()};
 return {...render(<TextParagraphDialog {...props}/>),adapter,props};
}
const click=(name:string)=>fireEvent.click(screen.getByRole("button",{name}));
async function ready(){await screen.findByRole("textbox",{name:"Paragraph text"});}
it("shows the embedded display name while retaining the raw font identity",async()=>{
 const labelled={...selection,fontName:`Folio${"a".repeat(64)}`,fontDisplayName:"DejaVu Serif"};
 setup({suggestTextParagraph:vi.fn().mockResolvedValue(labelled)});await ready();
 expect(screen.getByText(/DejaVu Serif.*12 pt/)).toBeInTheDocument();
 expect(screen.queryByText(new RegExp(labelled.fontName))).not.toBeInTheDocument();
});
async function choose(){click("Choose substitute font…");click("Choose test font");await ready();}
async function preview(){click("Preview paragraph");await screen.findByRole("img",{name:"Paragraph PDF preview"});}

it("retains a candidate when font picking is canceled and invalidates only on committed choice",async()=>{
 const {adapter}=setup();await ready();await preview();click("Choose substitute font…");click("Cancel font picker");
 expect(screen.getByRole("button",{name:"Apply paragraph"})).toBeEnabled();expect(adapter.closeDocument).not.toHaveBeenCalled();
 await choose();expect(adapter.closeDocument).toHaveBeenCalledWith("preview");expect(screen.getByRole("button",{name:"Apply paragraph"})).toBeDisabled();
 await preview();expect(adapter.previewTextParagraph).toHaveBeenLastCalledWith("source",0,[0],selection.text,selection.text,selection.bounds,undefined,undefined,undefined,chosen.id);
});
it("forwards justification together with substitute font, size and spacing",async()=>{
 const {adapter}=setup();await ready();await choose();fireEvent.change(screen.getByRole("combobox",{name:"Paragraph alignment"}),{target:{value:"justify"}});
 fireEvent.change(screen.getByRole("spinbutton",{name:"Font size (pt)"}),{target:{value:"16"}});fireEvent.change(screen.getByRole("spinbutton",{name:"Line spacing (× font size)"}),{target:{value:"1.5"}});await preview();
 expect(adapter.previewTextParagraph).toHaveBeenLastCalledWith("source",0,[0],selection.text,selection.text,selection.bounds,16,"justify",1.5,chosen.id);
});
it("combines color with a substitute font and keeps color when the font is reset",async()=>{
 const {adapter}=setup();await ready();await choose();
 fireEvent.change(screen.getByLabelText("Text color"),{target:{value:"#1a80ef"}});await preview();
 expect(adapter.previewTextParagraph).toHaveBeenLastCalledWith("source",0,[0],selection.text,selection.text,selection.bounds,undefined,undefined,undefined,chosen.id,"#1A80EF");
 click("Use original font");await preview();
 expect(adapter.previewTextParagraph).toHaveBeenLastCalledWith("source",0,[0],selection.text,selection.text,selection.bounds,undefined,undefined,undefined,undefined,"#1A80EF");
});
it("preserves all draft formatting through choosing and resetting the font",async()=>{
 const {adapter}=setup();await ready();fireEvent.change(screen.getByRole("textbox",{name:"Paragraph text"}),{target:{value:"Cafè draft"}});
 for(const [name,value] of [["Font size (pt)","16"],["Line spacing (× font size)","1.5"],["Left (pt)","70"]])fireEvent.change(screen.getByRole("spinbutton",{name}),{target:{value}});
 fireEvent.change(screen.getByRole("combobox",{name:"Paragraph alignment"}),{target:{value:"right"}});await choose();await preview();
 expect(adapter.previewTextParagraph).toHaveBeenLastCalledWith("source",0,[0],selection.text,"Cafè draft",{...selection.bounds,x:70},16,"right",1.5,chosen.id);
 click("Use original font");await preview();expect(adapter.previewTextParagraph).toHaveBeenLastCalledWith("source",0,[0],selection.text,"Cafè draft",{...selection.bounds,x:70},16,"right",1.5);
 await waitFor(()=>expect(fontApi.release).toHaveBeenCalledWith(chosen.id));
});
it("holds the selected font until a stale native preview settles after unmount",async()=>{
 let finish!:(doc:DocumentInfo)=>void;const {adapter,unmount}=setup({previewTextParagraph:vi.fn().mockImplementation(()=>new Promise(resolve=>finish=resolve))});
 await ready();await choose();click("Preview paragraph");unmount();await act(async()=>{});expect(fontApi.release).not.toHaveBeenCalled();
 await act(async()=>finish({id:"late",name:"page.pdf",pages:[]}));await waitFor(()=>expect(fontApi.release).toHaveBeenCalledTimes(1));expect(adapter.closeDocument).toHaveBeenCalledWith("late");
});
it("keeps operation leases across overlapping stale previews and a reset, including rejection",async()=>{
 const requests:{resolve:(doc:DocumentInfo)=>void;reject:(error:Error)=>void}[]=[];
 const {adapter,unmount}=setup({previewTextParagraph:vi.fn().mockImplementation(()=>new Promise((resolve,reject)=>requests.push({resolve,reject})))});
 await ready();await choose();click("Preview paragraph");
 fireEvent.change(screen.getByRole("textbox",{name:"Paragraph text"}),{target:{value:"Changed draft"}});click("Preview paragraph");expect(requests).toHaveLength(2);
 click("Use original font");await act(async()=>{});expect(fontApi.release).not.toHaveBeenCalled();
 await act(async()=>requests[0].resolve({id:"stale",name:"page.pdf",pages:[]}));expect(adapter.closeDocument).toHaveBeenCalledWith("stale");expect(fontApi.release).not.toHaveBeenCalled();
 await act(async()=>requests[1].reject(new Error("Canceled native preview")));await waitFor(()=>expect(fontApi.release).toHaveBeenCalledTimes(1));expect(screen.queryByRole("alert")).not.toBeInTheDocument();unmount();await flushFontReleases();expect(fontApi.release).toHaveBeenCalledTimes(1);
});
it("keeps the font for canceled adjustment but clears it for different pieces",async()=>{
 const {adapter}=setup();await ready();await choose();click("Adjust selection");click("Cancel adjustment");await preview();expect(vi.mocked(adapter.previewTextParagraph!).mock.calls.at(-1)?.[9]).toBe(chosen.id);
 click("Adjust selection");const boxes=screen.getAllByRole("checkbox");fireEvent.click(boxes[0]);fireEvent.click(boxes[1]);click("Check selection");await ready();await preview();
 expect(adapter.previewTextParagraph).toHaveBeenLastCalledWith("source",0,[1],"Second","Second",selection.bounds);await waitFor(()=>expect(fontApi.release).toHaveBeenCalledWith(chosen.id));
});
it("clears font choice when the document changes and rejects unsupported characters before native preview",async()=>{
 const {adapter,props,rerender}=setup();await ready();await choose();fireEvent.change(screen.getByRole("textbox",{name:"Paragraph text"}),{target:{value:"No Ω"}});
 expect(screen.getByRole("button",{name:"Preview paragraph"})).toBeDisabled();expect(screen.getByText(/cannot use.*U\+03A9/)).toBeVisible();expect(adapter.previewTextParagraph).not.toHaveBeenCalled();
 rerender(<TextParagraphDialog {...props} sourceId="other"/>);await ready();await preview();expect(adapter.previewTextParagraph).toHaveBeenLastCalledWith("other",0,[0],selection.text,selection.text,selection.bounds);await waitFor(()=>expect(fontApi.release).toHaveBeenCalledWith(chosen.id));
});
