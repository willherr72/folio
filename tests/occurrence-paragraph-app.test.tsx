import {styledEditorFixture} from "./fixtures/styled-editor-data";
import {act,cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {App} from "../src/App";
import {nativeAdapter} from "../src/editor/adapter";
import {clearSearchTextCache} from "../src/editor/search";
const size={width:300,height:400};
const run={objectIndex:0,text:"Old content",fontName:"Helvetica",fontSize:18,bounds:{x:40,y:145,width:100,height:20},supported:true};
vi.mock("../src/editor/useWorkspaceRecovery",()=>({useWorkspaceRecovery:()=>({ready:true,checkpoint:vi.fn().mockResolvedValue(undefined)})}));
vi.mock("../src/components/DocumentViewport",()=>({DocumentViewport:(props:any)=><div><output data-testid="plans">{JSON.stringify(props.pages)}</output><button onClick={()=>props.onEditText(props.selectedPageId,run)}>Choose text</button><button onClick={()=>{const page=props.pages.find((p:any)=>p.id!==props.selectedPageId);props.onSelectPage(page.id);props.onEditText(page.id,run);}}>Activate other page text</button></div>}));
const selection=styledEditorFixture();
const candidate=()=>({document:{id:"edited",name:"Edited.pdf",pages:[size,size,size]},pageIndices:[0,2,1]});
beforeEach(()=>{
 localStorage.clear();Object.defineProperty(window,"__TAURI_INTERNALS__",{value:{invoke:vi.fn().mockResolvedValue(0),transformCallback:()=>0},configurable:true});
 vi.spyOn(nativeAdapter,"openPdf").mockResolvedValue({id:"source",name:"Shared.pdf",pages:[{...size,overlays:[{id:"note",type:"comment",x:20,y:25,text:"Keep note",color:"#ffff00"}]},size]});
 vi.spyOn(nativeAdapter,"renderPage").mockResolvedValue("data:image/png;base64,AA");
 vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,"renderPagePlan").mockResolvedValue("data:image/png;base64,AA");
 vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,"listOccurrenceTextRuns").mockResolvedValue({runs:[run]});
 vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,"suggestOccurrenceStyledParagraph").mockResolvedValue(selection);
 vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,"inspectOccurrenceStyledParagraph").mockResolvedValue(selection);
 vi.spyOn(nativeAdapter,"closeDocument").mockResolvedValue();
 vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,"startPdfSave").mockReturnValue({result:Promise.resolve("Edited.pdf"),cancel:vi.fn().mockResolvedValue(false)});
 vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,"previewStyledParagraphOccurrence").mockResolvedValue(candidate());
 vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,"previewStyledParagraph").mockRejectedValue(new Error("legacy isolation must not run"));
});
afterEach(()=>{cleanup();clearSearchTextCache(["source","edited","borrowed"]);vi.restoreAllMocks();delete (window as any).__TAURI_INTERNALS__;});
const plans=()=>JSON.parse(screen.getByTestId("plans").textContent!);
async function open(){render(<App initialDemo={false}/>);fireEvent.click(screen.getByRole("button",{name:"Open a PDF"}));await screen.findByRole("tab",{name:"Shared.pdf"});fireEvent.click(screen.getByRole("button",{name:"Rotate"}));fireEvent.click(screen.getByRole("button",{name:"Duplicate"}));}
async function edit(){fireEvent.click(screen.getByRole("button",{name:"Choose text"}));fireEvent.click(await screen.findByRole("button",{name:"Edit paragraph…"}));await screen.findByRole("textbox",{name:"Paragraph text"});fireEvent.click(screen.getByRole("button",{name:"Preview paragraph"}));}
async function apply(){await waitFor(()=>expect(screen.getByRole("button",{name:"Apply paragraph"})).toBeEnabled());fireEvent.click(screen.getByRole("button",{name:"Apply paragraph"}));}

it("keeps a typed source-piece replacement when entering paragraph editing",async()=>{
 const checked={...selection,text:"Old content two",runs:[{text:"Old content ",styleId:0},{text:"two",styleId:1}]};
 vi.mocked(nativeAdapter.suggestOccurrenceStyledParagraph!).mockResolvedValue(checked);
 const mapping=vi.fn().mockResolvedValue({sourceId:"source",pageIndex:0,objectIndices:[0,1],anchorIndex:0,paragraphText:checked.text,sourceText:run.text,sourceStart:0,sourceEnd:11,paragraphStart:0,paragraphEnd:11});
 const previous=(nativeAdapter as any).mapParagraphDraft;(nativeAdapter as any).mapParagraphDraft=mapping;
 try{
  await open();const before=plans();fireEvent.click(screen.getByRole("button",{name:"Choose text"}));
  fireEvent.change(await screen.findByRole("textbox",{name:"Replacement text"}),{target:{value:"Old this content"}});
  fireEvent.click(screen.getByRole("button",{name:"Edit paragraph…"}));
  expect(await screen.findByRole("textbox",{name:"Paragraph text"})).toHaveTextContent("Old this content two");
  expect(plans()).toEqual(before);
  fireEvent.click(screen.getAllByRole("button",{name:"Undo"}).find(button=>!button.hasAttribute("disabled"))!);
  expect(screen.getByRole("textbox",{name:"Paragraph text"})).toHaveTextContent("Old content two");
  fireEvent.click(screen.getAllByRole("button",{name:"Redo"}).find(button=>!button.hasAttribute("disabled"))!);
  expect(screen.getByRole("textbox",{name:"Paragraph text"})).toHaveTextContent("Old this content two");
  fireEvent.click(screen.getByRole("button",{name:"Preview paragraph"}));
  await waitFor(()=>expect(nativeAdapter.previewStyledParagraphOccurrence).toHaveBeenCalledWith("source",[0,0,1],1,expect.objectContaining({expectedText:checked.text,runs:[{text:"Old this content ",styleId:0},{text:"two",styleId:1}]})));
 }finally{if(previous===undefined)delete (nativeAdapter as any).mapParagraphDraft;else (nativeAdapter as any).mapParagraphDraft=previous;}
});

it.each(["reject","wrong-owner"])("retains original typed input after paragraph mapping %s",async mode=>{
 const checked={...selection,text:"Old content two",runs:[{text:"Old content ",styleId:0},{text:"two",styleId:1}]};
 vi.mocked(nativeAdapter.suggestOccurrenceStyledParagraph!).mockResolvedValue(checked);
 const mapping=vi.fn().mockImplementation(async()=>{if(mode==="reject")throw new Error("Selected paragraph cannot be mapped.");return {sourceId:"other",pageIndex:0,objectIndices:[0,1],anchorIndex:0,paragraphText:checked.text,sourceText:run.text,sourceStart:0,sourceEnd:11,paragraphStart:0,paragraphEnd:11};});
 const previous=nativeAdapter.mapParagraphDraft;nativeAdapter.mapParagraphDraft=mapping;
 try{
  await open();const before=plans();fireEvent.click(screen.getByRole("button",{name:"Choose text"}));
  fireEvent.change(await screen.findByRole("textbox",{name:"Replacement text"}),{target:{value:"Old this content"}});
  fireEvent.click(screen.getByRole("button",{name:"Edit paragraph…"}));await screen.findByRole("alert");
  expect(screen.getByRole("textbox",{name:"Replacement text"})).toHaveValue("Old this content");
  expect(screen.queryByRole("textbox",{name:"Paragraph text"})).toBeNull();expect(plans()).toEqual(before);
  expect(nativeAdapter.previewStyledParagraphOccurrence).not.toHaveBeenCalled();
 }finally{nativeAdapter.mapParagraphDraft=previous;}
});

it("keeps typed input in the original dialog when a legacy editor cannot retain its history",async()=>{
 const keys=["inspectOccurrenceStyledParagraph","suggestOccurrenceStyledParagraph","previewStyledParagraphOccurrence","inspectStyledParagraph","suggestStyledParagraph","previewStyledParagraph"] as const;
 const previous=Object.fromEntries(keys.map(key=>[key,nativeAdapter[key]]));
 for(const key of keys)(nativeAdapter as any)[key]=undefined;
 const uniform={objectIndices:[0,1],text:"Old content two",fontName:"Courier",fontSize:12,bounds:selection.bounds};
 vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,"suggestOccurrenceTextParagraph").mockResolvedValue(uniform);
 const mapPrevious=nativeAdapter.mapParagraphDraft;
 nativeAdapter.mapParagraphDraft=vi.fn().mockResolvedValue({sourceId:"source",pageIndex:0,objectIndices:[0,1],anchorIndex:0,paragraphText:uniform.text,sourceText:run.text,sourceStart:0,sourceEnd:11,paragraphStart:0,paragraphEnd:11});
 try{
  await open();const before=plans();fireEvent.click(screen.getByRole("button",{name:"Choose text"}));
  fireEvent.change(await screen.findByRole("textbox",{name:"Replacement text"}),{target:{value:"Old this content"}});
  fireEvent.click(screen.getByRole("button",{name:"Edit paragraph…"}));
  await waitFor(()=>expect(screen.queryByRole("textbox",{name:"Replacement text"})).toHaveValue("Old this content"));
  expect(await screen.findByRole("alert")).toHaveTextContent("styled paragraph editor");
  expect(screen.queryByRole("textbox",{name:"Paragraph text"})).toBeNull();expect(plans()).toEqual(before);
  expect(nativeAdapter.suggestOccurrenceTextParagraph).not.toHaveBeenCalled();expect(nativeAdapter.mapParagraphDraft).not.toHaveBeenCalled();
 }finally{Object.assign(nativeAdapter,previous);nativeAdapter.mapParagraphDraft=mapPrevious;}
});

it("keeps unchanged entry into the legacy uniform paragraph editor",async()=>{
 const keys=["inspectOccurrenceStyledParagraph","suggestOccurrenceStyledParagraph","previewStyledParagraphOccurrence","inspectStyledParagraph","suggestStyledParagraph","previewStyledParagraph"] as const;
 const previous=Object.fromEntries(keys.map(key=>[key,nativeAdapter[key]]));for(const key of keys)(nativeAdapter as any)[key]=undefined;
 const uniform={objectIndices:[0,1],text:"Old content two",fontName:"Courier",fontSize:12,bounds:selection.bounds};
 vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,"suggestOccurrenceTextParagraph").mockResolvedValue(uniform);
 try{
  await open();fireEvent.click(screen.getByRole("button",{name:"Choose text"}));
  fireEvent.click(await screen.findByRole("button",{name:"Edit paragraph…"}));
  expect(await screen.findByRole("textbox",{name:"Paragraph text"})).toHaveValue(uniform.text);
  expect(screen.queryByRole("textbox",{name:"Replacement text"})).toBeNull();
  expect(nativeAdapter.suggestOccurrenceTextParagraph).toHaveBeenCalledWith("source",0,0,run.text);
 }finally{Object.assign(nativeAdapter,previous);}
});

it("remaps all linked pages in one undo step while preserving workspace state and saved sources",async()=>{
 await open();const before=plans();await edit();await apply();await waitFor(()=>expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
 expect(nativeAdapter.previewStyledParagraphOccurrence).toHaveBeenCalledWith("source",[0,0,1],1,expect.objectContaining({objectIndices:selection.objectIndices}));expect(nativeAdapter.previewStyledParagraph).not.toHaveBeenCalled();
 const after=plans();expect(after.map((p:any)=>[p.sourceId,p.pageIndex])).toEqual([["edited",0],["edited",2],["edited",1]]);
 expect(after.map((p:any)=>[p.id,p.rotation,p.overlays])).toEqual(before.map((p:any)=>[p.id,p.rotation,p.overlays]));
 fireEvent.click(screen.getByRole("button",{name:"Undo"}));expect(plans()).toEqual(before);
 fireEvent.click(screen.getByRole("button",{name:"Redo"}));expect(plans()).toEqual(after);
 fireEvent.click(screen.getByRole("button",{name:"Save a copy"}));await waitFor(()=>expect(nativeAdapter.startPdfSave).toHaveBeenCalled());expect(vi.mocked(nativeAdapter.startPdfSave!).mock.calls[0][0]).toEqual(after);
 expect(nativeAdapter.closeDocument).not.toHaveBeenCalledWith("source");expect(nativeAdapter.closeDocument).not.toHaveBeenCalledWith("edited");
});
it.each(["source","borrowed"])("rejects borrowed result %s without adopting or closing it",async id=>{
 await open();
 if(id==="borrowed"){
  vi.mocked(nativeAdapter.openPdf).mockResolvedValue({id:"borrowed",name:"Other.pdf",pages:[size]});fireEvent.click(screen.getByRole("button",{name:"Open PDF in new tab"}));await screen.findByRole("tab",{name:"Other.pdf"});fireEvent.click(screen.getByRole("tab",{name:"Shared.pdf"}));
 }
 const before=plans();vi.mocked(nativeAdapter.previewStyledParagraphOccurrence!).mockResolvedValue({...candidate(),document:{...candidate().document,id}});
 await edit();await screen.findByRole("alert");expect(plans()).toEqual(before);expect(nativeAdapter.closeDocument).not.toHaveBeenCalledWith(id);expect(screen.getByRole("textbox",{name:"Paragraph text"})).toHaveTextContent("one two");
});
it("disposes an invalid mapping and keeps the draft and original document",async()=>{
 await open();const before=plans();vi.mocked(nativeAdapter.previewStyledParagraphOccurrence!).mockResolvedValue({...candidate(),pageIndices:[2,0,1]});
 await edit();await screen.findByRole("alert");expect(plans()).toEqual(before);await waitFor(()=>expect(nativeAdapter.closeDocument).toHaveBeenCalledWith("edited"));
});
it("rejects a result owned only by undo history",async()=>{
 await open();await edit();await apply();await waitFor(()=>expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
 fireEvent.click(screen.getByRole("button",{name:"Undo"}));const before=plans();
 await edit();await screen.findByRole("alert");expect(plans()).toEqual(before);expect(nativeAdapter.closeDocument).not.toHaveBeenCalledWith("edited");
});
it("keeps the draft after native refusal without retrying through isolated editing",async()=>{
 await open();const before=plans();vi.mocked(nativeAdapter.previewStyledParagraphOccurrence!).mockRejectedValueOnce(new Error("The replacement would overlap neighboring text."));
 await edit();await screen.findByText("The replacement would overlap neighboring text.");expect(plans()).toEqual(before);
 expect(screen.getByRole("textbox",{name:"Paragraph text"})).toHaveTextContent("one two");expect(nativeAdapter.previewStyledParagraph).not.toHaveBeenCalled();
});
it("releases a fresh result after unmount while keeping Apply controls blocked",async()=>{
 let resolve!:(value:ReturnType<typeof candidate>)=>void;vi.mocked(nativeAdapter.previewStyledParagraphOccurrence!).mockImplementation(()=>new Promise(done=>{resolve=done;}));
 await open();await edit();expect(screen.getByRole("button",{name:"Apply paragraph"})).toBeDisabled();expect(screen.getAllByRole("button",{name:"Undo"})[0]).toBeDisabled();
 cleanup();await act(async()=>resolve(candidate()));await waitFor(()=>expect(nativeAdapter.closeDocument).toHaveBeenCalledWith("edited"));
});

it("keeps paragraph drafts across reading navigation while blocking other edits",async()=>{
 await open();await edit();await waitFor(()=>expect(screen.getByRole("button",{name:"Apply paragraph"})).toBeEnabled());
 expect(screen.queryByRole("dialog")).toBeNull();expect(screen.getByRole("button",{name:"Zoom in"})).toBeEnabled();
 expect(screen.getByRole("button",{name:"Save a copy"})).toBeDisabled();expect(screen.getByRole("button",{name:"Rotate"})).toBeDisabled();
 fireEvent.click(screen.getByRole("button",{name:"Next page"}));fireEvent.click(screen.getByRole("button",{name:"Zoom in"}));
 expect(screen.getByRole("textbox",{name:"Paragraph text"})).toHaveTextContent("one two");expect(nativeAdapter.suggestOccurrenceStyledParagraph).toHaveBeenCalledTimes(1);
 fireEvent.click(screen.getByRole("button",{name:"Locate paragraph"}));await apply();expect(plans().map((p:any)=>p.sourceId)).toEqual(["edited","edited","edited"]);
});

it("keeps one document viewport through paragraph preview, cancel, apply and history",async()=>{
 await open();const viewport=screen.getByTestId("plans").parentElement;
 await edit();await waitFor(()=>expect(screen.getByRole("button",{name:"Apply paragraph"})).toBeEnabled());
 expect(screen.getByTestId("plans").parentElement).toBe(viewport);
 fireEvent.click(screen.getByRole("button",{name:"Cancel"}));
 await waitFor(()=>expect(screen.queryByRole("textbox",{name:"Paragraph text"})).toBeNull());
 expect(screen.getByTestId("plans").parentElement).toBe(viewport);
 await waitFor(()=>expect(nativeAdapter.closeDocument).toHaveBeenCalledWith("edited"));
 await edit();await apply();
 expect(screen.getByTestId("plans").parentElement).toBe(viewport);
 expect(plans().every((page:any)=>page.sourceId==="edited")).toBe(true);
 fireEvent.click(screen.getByRole("button",{name:"Undo"}));
 expect(screen.getByTestId("plans").parentElement).toBe(viewport);
 expect(plans().every((page:any)=>page.sourceId==="source")).toBe(true);
 fireEvent.click(screen.getByRole("button",{name:"Redo"}));
 expect(screen.getByTestId("plans").parentElement).toBe(viewport);
 expect(plans().every((page:any)=>page.sourceId==="edited")).toBe(true);
});


it("enters paragraphs directly and keeps cancel, focus and document history faithful",async()=>{
 await open();const before=plans(),viewport=screen.getByTestId("plans").parentElement;
 const tool=screen.getByRole("button",{name:"Edit paragraphs"});fireEvent.click(tool);
 fireEvent.click(screen.getByRole("button",{name:"Choose text"}));
 await screen.findByRole("textbox",{name:"Paragraph text"});
 expect(screen.queryByRole("dialog",{name:"Edit existing text"})).toBeNull();
 expect(screen.getByTestId("plans").parentElement).toBe(viewport);
 expect(nativeAdapter.suggestOccurrenceStyledParagraph).toHaveBeenCalledWith("source",0,run.objectIndex,run.text);
 fireEvent.click(screen.getByRole("button",{name:"Preview paragraph"}));await waitFor(()=>expect(screen.getByRole("button",{name:"Apply paragraph"})).toBeEnabled());
 fireEvent.click(screen.getByRole("button",{name:"Cancel"}));
 await waitFor(()=>expect(screen.queryByRole("textbox",{name:"Paragraph text"})).toBeNull());
 expect(plans()).toEqual(before);expect(tool).toHaveFocus();
 await waitFor(()=>expect(nativeAdapter.closeDocument).toHaveBeenCalledWith("edited"));
 fireEvent.click(screen.getByRole("button",{name:"Choose text"}));await screen.findByRole("textbox",{name:"Paragraph text"});
 fireEvent.click(screen.getByRole("button",{name:"Preview paragraph"}));await apply();
 expect(plans().map((p:any)=>p.sourceId)).toEqual(["edited","edited","edited"]);expect(tool).toHaveFocus();
 fireEvent.click(screen.getByRole("button",{name:"Undo"}));expect(plans()).toEqual(before);
 fireEvent.click(screen.getByRole("button",{name:"Redo"}));expect(plans().map((p:any)=>p.sourceId)).toEqual(["edited","edited","edited"]);
});
it("returns explicitly to the individual-text tool after direct paragraph editing",async()=>{
 await open();fireEvent.click(screen.getByRole("button",{name:"Edit paragraphs"}));
 fireEvent.click(screen.getByRole("button",{name:"Choose text"}));await screen.findByRole("textbox",{name:"Paragraph text"});
 fireEvent.click(screen.getByRole("button",{name:"Cancel"}));
 fireEvent.click(screen.getByRole("button",{name:"Edit text"}));fireEvent.click(screen.getByRole("button",{name:"Choose text"}));
 expect(await screen.findByRole("dialog",{name:"Edit existing text"})).toBeInTheDocument();
 expect(screen.queryByRole("textbox",{name:"Paragraph text"})).toBeNull();
});
it("refuses direct annotated paragraph entry without a compositor and preserves the source",async()=>{
 await open();const before=plans(),render=nativeAdapter.renderPagePlan;nativeAdapter.renderPagePlan=undefined;
 try{
  fireEvent.click(screen.getByRole("button",{name:"Edit paragraphs"}));fireEvent.click(screen.getByRole("button",{name:"Choose text"}));
  expect(await screen.findByRole("alert")).toHaveTextContent("Annotated paragraph previews are unavailable");
  expect(plans()).toEqual(before);expect(screen.queryByRole("dialog",{name:"Edit existing text"})).toBeNull();
  expect(nativeAdapter.suggestOccurrenceStyledParagraph).not.toHaveBeenCalled();
 }finally{nativeAdapter.renderPagePlan=render;}
});


it("opens the modal paragraph route after activating text on another visible page",async()=>{
 await open();fireEvent.click(screen.getByRole("button",{name:"Edit text"}));
 fireEvent.click(screen.getByRole("button",{name:"Activate other page text"}));
 expect(await screen.findByRole("dialog",{name:"Edit existing text"})).toBeInTheDocument();
 fireEvent.click(screen.getByRole("button",{name:"Edit paragraph…"}));
 expect(await screen.findByRole("textbox",{name:"Paragraph text"})).toHaveTextContent("one two");
});
