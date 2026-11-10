import {act,cleanup,fireEvent,render,screen} from "@testing-library/react";
import {afterEach,expect,it,vi} from "vitest";
import {TextParagraphDialog} from "../src/components/TextParagraphDialog";
import type {FolioAdapter} from "../src/editor/adapter";
import type {EditableTextRun,TextParagraphSelection} from "../src/editor/types";
afterEach(()=>{cleanup();vi.restoreAllMocks();vi.unstubAllGlobals();});
const anchor:EditableTextRun={objectIndex:0,text:"First line.",fontName:"Helvetica",fontSize:12,bounds:{x:60,y:88,width:90,height:12},supported:true};
const next={...anchor,objectIndex:1,text:"Second line.",bounds:{...anchor.bounds,y:106}};
const selection:TextParagraphSelection={objectIndices:[0],text:anchor.text,fontName:"Helvetica",fontSize:12,bounds:{x:60,y:88,width:180,height:50}};
const suggested={...selection,objectIndices:[0,1],text:"First line. Second line."};
async function draft(overrides:Partial<FolioAdapter>={}) {
 const adapter:FolioAdapter={kind:"native",openPdf:vi.fn(),exportPdf:vi.fn(),engineStatus:vi.fn(),closeDocument:vi.fn().mockResolvedValue(undefined),
  listTextRuns:vi.fn().mockResolvedValue({runs:[anchor,next]}),suggestTextParagraph:vi.fn().mockResolvedValue(suggested),
  inspectTextParagraph:vi.fn().mockResolvedValue(selection),previewTextParagraph:vi.fn().mockResolvedValue({id:"preview",name:"page.pdf",pages:[{width:400,height:500}]}),
  renderPage:vi.fn().mockResolvedValue("data:image/png;base64,AA"),...overrides};
 const onApply=vi.fn().mockReturnValue(true),onCancel=vi.fn();
 const view=render(<TextParagraphDialog adapter={adapter} sourceId="source" pageIndex={0} pageWidth={400} pageHeight={500} anchor={anchor} onApply={onApply} onCancel={onCancel}/>);
 fireEvent.change(await screen.findByRole("textbox",{name:"Paragraph text"}),{target:{value:"My edited paragraph."}});
 fireEvent.change(screen.getByRole("spinbutton",{name:"Left (pt)"}),{target:{value:"100"}});
 fireEvent.click(screen.getByRole("button",{name:"Preview paragraph"}));await screen.findByRole("img",{name:"Paragraph PDF preview"});
 return {adapter,onApply,onCancel,...view};
}
const adjust=()=>fireEvent.click(screen.getByRole("button",{name:"Adjust selection"}));
async function check(){fireEvent.click(screen.getByRole("button",{name:"Check selection"}));await screen.findByRole("textbox",{name:"Paragraph text"});}

it("cancels piece changes without losing draft, area, preview or keyboard focus",async()=>{
 const {adapter}=await draft();adjust();
 expect(screen.getByRole("group",{name:/Paragraph pieces/})).toHaveFocus();
 expect(screen.getByRole("img",{name:"Original PDF page"})).toBeInTheDocument();
 expect(screen.getByRole("button",{name:"Edited"})).toBeDisabled();
 fireEvent.click(screen.getByRole("checkbox",{name:next.text}));
 expect(screen.getByRole("button",{name:"Apply paragraph"})).toBeDisabled();expect(screen.getByRole("button",{name:"Preview paragraph"})).toBeDisabled();
 expect(adapter.closeDocument).not.toHaveBeenCalled();fireEvent.click(screen.getByRole("button",{name:"Cancel adjustment"}));
 expect(screen.getByRole("textbox",{name:"Paragraph text"})).toHaveValue("My edited paragraph.");expect(screen.getByRole("textbox",{name:"Paragraph text"})).toHaveFocus();
 expect(screen.getByRole("spinbutton",{name:"Left (pt)"})).toHaveValue(100);expect(screen.getByRole("img",{name:"Paragraph PDF preview"})).toBeInTheDocument();
 expect(screen.getByRole("button",{name:"Apply paragraph"})).toBeEnabled();expect(adapter.closeDocument).not.toHaveBeenCalled();
});
it("keeps draft and candidate when the same selection is checked",async()=>{
 const {adapter}=await draft();adjust();await check();
 expect(screen.getByRole("textbox",{name:"Paragraph text"})).toHaveValue("My edited paragraph.");expect(screen.getByRole("spinbutton",{name:"Left (pt)"})).toHaveValue(100);
 expect(screen.getByRole("button",{name:"Apply paragraph"})).toBeEnabled();expect(adapter.inspectTextParagraph).not.toHaveBeenCalled();expect(adapter.closeDocument).not.toHaveBeenCalled();
});
it("commits a different validated selection and releases the replaced candidate once",async()=>{
 const {adapter,unmount}=await draft();adjust();expect(screen.getByText(/Checking a different selection replaces/)).toBeInTheDocument();
 fireEvent.click(screen.getByRole("checkbox",{name:next.text}));await check();
 expect(adapter.inspectTextParagraph).toHaveBeenCalledWith("source",0,[0]);expect(screen.getByRole("textbox",{name:"Paragraph text"})).toHaveValue(anchor.text);
 expect(screen.getByRole("spinbutton",{name:"Left (pt)"})).toHaveValue(60);expect(screen.getByRole("button",{name:"Apply paragraph"})).toBeDisabled();
 expect(adapter.closeDocument).toHaveBeenCalledOnce();expect(adapter.closeDocument).toHaveBeenCalledWith("preview");unmount();expect(adapter.closeDocument).toHaveBeenCalledOnce();
});
it("keeps the draft and candidate if checking a different selection fails",async()=>{
 const {adapter}=await draft({inspectTextParagraph:vi.fn().mockRejectedValue(new Error("Mixed formatting."))});adjust();
 fireEvent.click(screen.getByRole("checkbox",{name:next.text}));fireEvent.click(screen.getByRole("button",{name:"Check selection"}));
 expect(await screen.findByRole("alert")).toHaveTextContent("Mixed formatting.");expect(adapter.closeDocument).not.toHaveBeenCalled();
 fireEvent.click(screen.getByRole("button",{name:"Cancel adjustment"}));expect(screen.queryByRole("alert")).not.toBeInTheDocument();
 expect(screen.getByRole("textbox",{name:"Paragraph text"})).toHaveValue("My edited paragraph.");expect(screen.getByRole("button",{name:"Apply paragraph"})).toBeEnabled();
});
it("ignores a late check after adjustment cancellation and transfers the retained candidate once",async()=>{
 let finish!:(value:TextParagraphSelection)=>void;
 const {adapter,onApply,unmount}=await draft({inspectTextParagraph:vi.fn().mockImplementation(()=>new Promise(resolve=>finish=resolve))});adjust();
 fireEvent.click(screen.getByRole("checkbox",{name:next.text}));fireEvent.click(screen.getByRole("button",{name:"Check selection"}));
 expect(screen.getByRole("button",{name:"Check selection"})).toBeDisabled();fireEvent.click(screen.getByRole("button",{name:"Cancel adjustment"}));
 fireEvent.click(screen.getByRole("button",{name:"Apply paragraph"}));await act(async()=>finish(selection));
 expect(onApply).toHaveBeenCalledOnce();expect(onApply).toHaveBeenCalledWith(expect.objectContaining({id:"preview"}));
 expect(screen.getByRole("textbox",{name:"Paragraph text"})).toHaveValue("My edited paragraph.");unmount();expect(adapter.closeDocument).not.toHaveBeenCalled();
});
it("releases the retained candidate and URL exactly once on whole-dialog cancellation",async()=>{
 const revoke=vi.fn();vi.stubGlobal("URL",class extends URL {static revokeObjectURL=revoke;});
 const {adapter,onCancel,unmount}=await draft({renderPage:vi.fn().mockImplementation(id=>Promise.resolve(id==="preview"?"blob:draft":"data:image/png;base64,AA"))});
 adjust();expect(revoke).not.toHaveBeenCalled();fireEvent.click(screen.getByRole("button",{name:"Cancel"}));unmount();
 expect(onCancel).toHaveBeenCalledOnce();expect(adapter.closeDocument).toHaveBeenCalledOnce();expect(adapter.closeDocument).toHaveBeenCalledWith("preview");
 expect(revoke).toHaveBeenCalledOnce();expect(revoke).toHaveBeenCalledWith("blob:draft");
});
it.each(["success","rejection"])("ignores late adjustment %s after replacing the source and adapter",async outcome=>{
 const revoke=vi.fn();vi.stubGlobal("URL",class extends URL {static revokeObjectURL=revoke;});
 let resolve!:(value:TextParagraphSelection)=>void,reject!:(reason:Error)=>void;
 const {adapter,onApply,onCancel,rerender,unmount}=await draft({
  renderPage:vi.fn().mockImplementation(id=>Promise.resolve(id==="preview"?"blob:draft":"data:image/png;base64,AA")),
  inspectTextParagraph:vi.fn().mockImplementation(()=>new Promise((done,fail)=>{resolve=done;reject=fail;}))});
 adjust();fireEvent.click(screen.getByRole("checkbox",{name:next.text}));fireEvent.click(screen.getByRole("button",{name:"Check selection"}));
 expect(screen.getByRole("button",{name:"Check selection"})).toBeDisabled();
 const replacement={...anchor,text:"Replacement document."};
 const newAdapter:FolioAdapter={...adapter,closeDocument:vi.fn().mockResolvedValue(undefined),
  renderPage:vi.fn().mockResolvedValue("data:image/png;base64,AA"),listTextRuns:vi.fn().mockResolvedValue({runs:[replacement]}),
  suggestTextParagraph:vi.fn().mockResolvedValue({...selection,text:replacement.text}),
  previewTextParagraph:vi.fn().mockResolvedValue({id:"new-preview",name:"new.pdf",pages:[{width:400,height:500}]})};
 rerender(<TextParagraphDialog adapter={newAdapter} sourceId="new-source" pageIndex={0} pageWidth={400} pageHeight={500} anchor={replacement} onApply={onApply} onCancel={onCancel}/>);
 expect(await screen.findByRole("textbox",{name:"Paragraph text"})).toHaveValue(replacement.text);
 await act(async()=>{if(outcome==="success")resolve(selection);else reject(new Error("Old source failed."));});
 expect(screen.getByRole("textbox",{name:"Paragraph text"})).toHaveValue(replacement.text);expect(screen.queryByRole("alert")).not.toBeInTheDocument();
 expect(screen.getByRole("button",{name:"Apply paragraph"})).toBeDisabled();expect(screen.getByRole("button",{name:"Preview paragraph"})).toBeEnabled();
 expect(adapter.closeDocument).toHaveBeenCalledOnce();expect(adapter.closeDocument).toHaveBeenCalledWith("preview");expect(newAdapter.closeDocument).not.toHaveBeenCalled();
 expect(revoke).toHaveBeenCalledOnce();expect(revoke).toHaveBeenCalledWith("blob:draft");expect(onApply).not.toHaveBeenCalled();
 fireEvent.click(screen.getByRole("button",{name:"Preview paragraph"}));await screen.findByRole("img",{name:"Paragraph PDF preview"});
 expect(newAdapter.previewTextParagraph).toHaveBeenCalledWith("new-source",0,[0],replacement.text,replacement.text,selection.bounds);
 fireEvent.click(screen.getByRole("button",{name:"Apply paragraph"}));expect(onApply).toHaveBeenCalledOnce();expect(onApply).toHaveBeenCalledWith(expect.objectContaining({id:"new-preview"}));
 unmount();expect(adapter.closeDocument).toHaveBeenCalledOnce();expect(newAdapter.closeDocument).not.toHaveBeenCalled();expect(revoke).toHaveBeenCalledOnce();
});
