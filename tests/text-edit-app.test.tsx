import {act,cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {App} from "../src/App";
import {nativeAdapter} from "../src/editor/adapter";
import {clearSearchTextCache} from "../src/editor/search";
const run={objectIndex:2,text:"Original",fontName:"Helvetica",fontSize:12,bounds:{x:30,y:40,width:90,height:14},supported:true};
const suggestStyledParagraph=nativeAdapter.suggestStyledParagraph;
// Keep explicit coverage for adapters that still return isolated text pages.
const replaceTextOccurrence=nativeAdapter.replaceTextOccurrence;
const listOccurrenceTextRuns=nativeAdapter.listOccurrenceTextRuns;
vi.mock("../src/components/DocumentViewport",()=>({DocumentViewport:(props:any)=><div><output data-testid="sources">{props.pages.map((page:any)=>page.sourceId).join(",")}</output><button onClick={()=>props.onEditText?.(props.pages[0].id,run)}>Choose existing run</button></div>}));
beforeEach(()=>{
 localStorage.clear();
 nativeAdapter.replaceTextOccurrence=undefined;
 nativeAdapter.listOccurrenceTextRuns=undefined;
 vi.spyOn(nativeAdapter,"openPdf").mockResolvedValue({id:"text-old",name:"Editable.pdf",pages:[{width:300,height:400,overlays:[{type:"comment",id:"note",x:20,y:25,text:"Keep note",color:"#ffff00"}]}]});
 vi.spyOn(nativeAdapter,"renderPage").mockResolvedValue("data:image/png;base64,");
 vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,"getPageText").mockImplementation(async source=>({characters:Array.from(source==="text-old"?"Original":"New",(text,i)=>({text,x:i*6,y:30,width:6,height:12}))}));
 vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,"replaceText").mockResolvedValue({id:"text-new",name:"Page.pdf",pages:[{width:300,height:400}]});
 vi.spyOn(nativeAdapter,"closeDocument").mockResolvedValue();
 vi.spyOn(nativeAdapter,"exportPdf").mockResolvedValue("Edited.pdf");
});
afterEach(()=>{cleanup();clearSearchTextCache(["text-old","text-new"]);vi.restoreAllMocks();nativeAdapter.suggestStyledParagraph=suggestStyledParagraph;nativeAdapter.replaceTextOccurrence=replaceTextOccurrence;nativeAdapter.listOccurrenceTextRuns=listOccurrenceTextRuns;});
async function open(){render(<App initialDemo={false}/>);fireEvent.click(screen.getByRole("button",{name:"Open a PDF"}));await screen.findByRole("tab",{name:"Editable.pdf"});fireEvent.click(screen.getByRole("button",{name:"Edit text"}));}
async function edit(){fireEvent.click(screen.getByRole("button",{name:"Choose existing run"}));const input=await screen.findByRole("textbox",{name:"Replacement text"});fireEvent.change(input,{target:{value:"New"}});fireEvent.click(screen.getByRole("button",{name:"Apply changes"}));}
it("keeps Edit text active after Apply and through Undo/Redo for the next existing-text edit",async()=>{
 await open();await edit();await waitFor(()=>expect(screen.getByTestId("sources")).toHaveTextContent("text-new"));
 expect(screen.getByRole("button",{name:"Edit text"})).toHaveAttribute("aria-pressed","true");
 fireEvent.click(screen.getByRole("button",{name:"Undo"}));expect(screen.getByTestId("sources")).toHaveTextContent("text-old");
 expect(screen.getByRole("button",{name:"Edit text"})).toHaveAttribute("aria-pressed","true");
 fireEvent.click(screen.getByRole("button",{name:"Redo"}));expect(screen.getByTestId("sources")).toHaveTextContent("text-new");
 expect(screen.getByRole("button",{name:"Edit text"})).toHaveAttribute("aria-pressed","true");
 fireEvent.click(screen.getByRole("button",{name:"Select"}));expect(screen.getByRole("button",{name:"Select"})).toHaveClass("active");
 expect(screen.getByRole("button",{name:"Edit text"})).toHaveAttribute("aria-pressed","false");
});
it.each(["uniform","styled"])("applies the %s paragraph as one undo step and retains its source for redo",async mode=>{
 vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,"renderPagePlan").mockResolvedValue("data:image/png;base64,COMPOSED");
 vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,"listTextRuns").mockResolvedValue({runs:[run]});
 vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,"inspectTextParagraph").mockResolvedValue({objectIndices:[2],text:"Original",fontName:"Helvetica",fontSize:12,bounds:{x:30,y:40,width:180,height:50}});
 vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,"previewTextParagraph").mockResolvedValue({id:"text-new",name:"Page.pdf",pages:[{width:300,height:400}]});
 if(mode==="uniform")nativeAdapter.suggestStyledParagraph=undefined;
 else {
  vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,"suggestStyledParagraph").mockResolvedValue({objectIndices:[2],text:"Original",runs:[{text:"Original",styleId:0}],styles:[{styleId:0,objectIndices:[2],fontName:"Helvetica",fontDisplayName:null,fontSize:12,color:"#000000",isEmbedded:false}],bounds:{x:30,y:40,width:180,height:50},alignment:null,lineSpacing:null});
  vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,"previewStyledParagraph").mockResolvedValue({id:"text-new",name:"Page.pdf",pages:[{width:300,height:400}]});
 }
 await open();fireEvent.click(screen.getByRole("button",{name:"Choose existing run"}));fireEvent.click(screen.getByRole("button",{name:"Edit paragraph…"}));
 if(mode==="uniform"){
  await waitFor(()=>expect(screen.getByRole("button",{name:"Check selection"})).toBeEnabled());fireEvent.click(screen.getByRole("button",{name:"Check selection"}));
  fireEvent.change(await screen.findByRole("textbox",{name:"Paragraph text"}),{target:{value:"New paragraph"}});
 }else {
  const input=await screen.findByRole("textbox",{name:"Paragraph text"});input.focus();fireEvent.keyDown(input,{key:"a",ctrlKey:true});act(()=>input.dispatchEvent(new InputEvent("beforeinput",{bubbles:true,cancelable:true,inputType:"insertText",data:"New paragraph"})));
 }
 expect(screen.getByRole("button",{name:"Save a copy"})).toBeDisabled();
 fireEvent.click(screen.getByRole("button",{name:"Preview paragraph"}));await waitFor(()=>expect(screen.getByRole("button",{name:"Apply paragraph"})).toBeEnabled());
 if(mode==="styled"){expect(nativeAdapter.previewTextParagraph).not.toHaveBeenCalled();expect(nativeAdapter.previewStyledParagraph).toHaveBeenCalledWith("text-old",0,expect.objectContaining({expectedText:"Original",runs:[{text:"New paragraph",styleId:0}]}));}
 expect(screen.getByTestId("sources")).toHaveTextContent("text-old");
 fireEvent.click(screen.getByRole("button",{name:"Apply paragraph"}));
 await waitFor(()=>expect(screen.getByTestId("sources")).toHaveTextContent("text-new"));
 expect(nativeAdapter.closeDocument).not.toHaveBeenCalledWith("text-new");
 fireEvent.click(screen.getByRole("button",{name:"Undo"}));expect(screen.getByTestId("sources")).toHaveTextContent("text-old");expect(screen.getByRole("button",{name:"Undo"})).toBeDisabled();
 fireEvent.click(screen.getByRole("button",{name:"Redo"}));expect(screen.getByTestId("sources")).toHaveTextContent("text-new");
 fireEvent.click(screen.getByRole("button",{name:"Save a copy"}));await waitFor(()=>expect(nativeAdapter.exportPdf).toHaveBeenCalled());
 expect(vi.mocked(nativeAdapter.exportPdf).mock.calls[0][0][0].sourceId).toBe("text-new");
 expect(vi.mocked(nativeAdapter.exportPdf).mock.calls[0][0][0].overlays).toEqual([expect.objectContaining({id:"note",text:"Keep note"})]);
 expect(nativeAdapter.renderPagePlan).toHaveBeenCalledWith(expect.objectContaining({sourceId:"text-old",rotation:0,overlays:[expect.objectContaining({id:"note"})]}),600);
 expect(nativeAdapter.renderPagePlan).toHaveBeenCalledWith(expect.objectContaining({sourceId:"text-new",rotation:0,overlays:[expect.objectContaining({id:"note"})]}),600);
});
async function groupEdit(){
 vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,"listTextRuns").mockResolvedValue({runs:[run,{...run,objectIndex:3,text:" suffix"}]});
 vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,"inspectTextGroup").mockResolvedValue({objectIndices:[2,3],text:"Original suffix",fontName:"Helvetica",fontSize:12});
 await open();fireEvent.click(screen.getByRole("button",{name:"Choose existing run"}));
 fireEvent.click(screen.getByRole("button",{name:"Edit together…"}));
 fireEvent.click(await screen.findByRole("checkbox",{name:"suffix"}));fireEvent.click(screen.getByRole("button",{name:"Check selection"}));
 const input=await screen.findByRole("textbox",{name:"Replacement text"});fireEvent.change(input,{target:{value:"New"}});
 return input;
}
it("applies an explicit group as one source change and one undo while blocking workspace actions",async()=>{
 vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,"replaceTextGroup").mockResolvedValue({id:"text-new",name:"Page.pdf",pages:[{width:300,height:400}]});
 await groupEdit();
 expect(screen.getByRole("button",{name:"Settings"})).toBeDisabled();expect(screen.getByRole("tab",{name:"Editable.pdf"})).toBeDisabled();expect(screen.getByRole("button",{name:"Close Editable.pdf"})).toBeDisabled();expect(screen.getByRole("button",{name:"Open"})).toBeDisabled();
 expect(screen.getByRole("button",{name:"Save a copy"})).toBeDisabled();expect(screen.getByRole("button",{name:"Rotate"})).toBeDisabled();
 fireEvent.keyDown(window,{key:"o",ctrlKey:true});expect(nativeAdapter.openPdf).toHaveBeenCalledTimes(1);
 fireEvent.click(screen.getByRole("button",{name:"Apply changes"}));await waitFor(()=>expect(screen.getByTestId("sources")).toHaveTextContent("text-new"));
 expect(nativeAdapter.replaceTextGroup).toHaveBeenCalledWith("text-old",0,[2,3],"Original suffix","New");expect(nativeAdapter.replaceText).not.toHaveBeenCalled();
 expect(screen.getByRole("button",{name:"Edit text"})).toHaveAttribute("aria-pressed","true");
 fireEvent.click(screen.getByRole("button",{name:"Undo"}));expect(screen.getByTestId("sources")).toHaveTextContent("text-old");expect(screen.getByRole("button",{name:"Undo"})).toBeDisabled();
});
it("does not reopen a cancelled group when nearby text finishes loading",async()=>{
 let resolve!:(value:any)=>void;
 vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,"listTextRuns").mockImplementation(()=>new Promise(done=>{resolve=done;}));
 await open();fireEvent.click(screen.getByRole("button",{name:"Choose existing run"}));fireEvent.click(screen.getByRole("button",{name:"Edit together…"}));
 expect(screen.getByText("Loading nearby text…")).toHaveAttribute("role","status");fireEvent.click(screen.getByRole("button",{name:"Cancel"}));
 await act(async()=>resolve({runs:[run,{...run,objectIndex:3,text:" suffix"}]}));
 expect(screen.queryByRole("dialog")).not.toBeInTheDocument();expect(screen.getByTestId("sources")).toHaveTextContent("text-old");expect(nativeAdapter.replaceText).not.toHaveBeenCalled();
});
it("keeps a rejected group draft and releases a late group source after unmount",async()=>{
 const replace=vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,"replaceTextGroup").mockRejectedValueOnce(new Error("Group text does not fit"));
 const input=await groupEdit();fireEvent.click(screen.getByRole("button",{name:"Apply changes"}));await screen.findByText("Group text does not fit");expect(input).toHaveValue("New");
 let resolve!:(value:any)=>void;replace.mockImplementation(()=>new Promise(done=>{resolve=done;}));
 fireEvent.click(screen.getByRole("button",{name:"Apply changes"}));expect(screen.getByRole("button",{name:"Cancel"})).toBeDisabled();
 cleanup();await act(async()=>resolve({id:"text-new",name:"Page.pdf",pages:[{width:300,height:400}]}));
 await waitFor(()=>expect(nativeAdapter.closeDocument).toHaveBeenCalledWith("text-new"));
});
it("uses changed source bytes for save/search/undo and releases every owned version on close",async()=>{
 await open();fireEvent.click(screen.getByRole("button",{name:"Rotate"}));fireEvent.click(screen.getByRole("button",{name:"Duplicate"}));
 await edit();await waitFor(()=>expect(screen.getByTestId("sources")).toHaveTextContent("text-new,text-old"));
 expect(nativeAdapter.replaceText).toHaveBeenCalledWith("text-old",0,2,"Original","New");
 fireEvent.click(screen.getByRole("button",{name:"Save a copy"}));await waitFor(()=>expect(nativeAdapter.exportPdf).toHaveBeenCalled());
 const pages=vi.mocked(nativeAdapter.exportPdf).mock.calls[0][0];expect(pages[0]).toMatchObject({sourceId:"text-new",rotation:90,overlays:[{id:"note",text:"Keep note"}]});expect(pages[1].sourceId).toBe("text-old");
 fireEvent.click(screen.getByRole("button",{name:"Find"}));fireEvent.change(screen.getByRole("searchbox"),{target:{value:"New"}});await screen.findByText("1 of 1");
 fireEvent.click(screen.getByRole("button",{name:"Undo"}));await waitFor(()=>expect(screen.getByTestId("sources")).toHaveTextContent("text-old,text-old"));await screen.findByText("No matches");
 fireEvent.click(screen.getByRole("button",{name:"Redo"}));await screen.findByText("1 of 1");
 fireEvent.click(screen.getByRole("button",{name:"Close Editable.pdf"}));await waitFor(()=>expect(nativeAdapter.closeDocument).toHaveBeenCalledWith("text-old"));expect(nativeAdapter.closeDocument).toHaveBeenCalledWith("text-new");
});
it("keeps typed replacement after native rejection and blocks workspace changes during apply",async()=>{
 let reject!:(error:Error)=>void;vi.mocked(nativeAdapter.replaceText!).mockImplementation(()=>new Promise((_resolve,fail)=>{reject=fail;}));
 await open();await edit();expect(screen.getByRole("button",{name:"Undo"})).toBeDisabled();expect(screen.getByRole("button",{name:"Cancel"})).toBeDisabled();
 fireEvent.keyDown(screen.getByRole("dialog"),{key:"Escape"});expect(screen.getByRole("dialog")).toBeInTheDocument();
 await act(async()=>reject(new Error("Replacement does not fit")));await screen.findByText(/Replacement does not fit/);expect(screen.getByRole("textbox",{name:"Replacement text"})).toHaveValue("New");
 fireEvent.click(screen.getByRole("button",{name:"Cancel"}));expect(screen.getByTestId("sources")).toHaveTextContent("text-old");expect(screen.queryByLabelText("Unsaved changes")).not.toBeInTheDocument();
});
it("releases a late derived source if the workspace unmounts while native editing finishes",async()=>{
 let resolve!:(value:any)=>void;vi.mocked(nativeAdapter.replaceText!).mockImplementation(()=>new Promise(done=>{resolve=done;}));await open();await edit();cleanup();await act(async()=>resolve({id:"text-new",name:"Page.pdf",pages:[{width:300,height:400}]}));await waitFor(()=>expect(nativeAdapter.closeDocument).toHaveBeenCalledWith("text-new"));
});
