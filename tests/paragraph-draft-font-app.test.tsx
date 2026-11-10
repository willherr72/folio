import {act,cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {App} from "../src/App";
import {nativeAdapter} from "../src/editor/adapter";
import {fontApi,flushFontReleases} from "../src/editor/custom-fonts";
import {styledEditorFixture} from "./fixtures/styled-editor-data";
const selection=styledEditorFixture(),size={width:300,height:400};
const run={objectIndex:0,text:"one",fontName:"Courier",fontSize:12,bounds:selection.bounds,supported:true,canSubstitute:true};
const info={id:"d".repeat(64),name:"Full Serif",weight:400,italic:false,coverage:[[32,126]] as [number,number][]};
vi.mock("../src/editor/useWorkspaceRecovery",()=>({useWorkspaceRecovery:()=>({ready:true,checkpoint:vi.fn().mockResolvedValue(undefined)})}));
vi.mock("../src/components/DocumentViewport",()=>({DocumentViewport:(props:any)=><button onClick={()=>props.onEditText(props.selectedPageId,run)}>Choose text</button>}));
beforeEach(()=>{
 localStorage.clear();vi.stubGlobal("FontFace",class{load(){return Promise.resolve(this);}});Object.defineProperty(document,"fonts",{configurable:true,value:{add:vi.fn(),delete:vi.fn()}});
 Object.defineProperty(window,"__TAURI_INTERNALS__",{configurable:true,value:{invoke:vi.fn().mockResolvedValue(0),transformCallback:()=>0}});
 vi.spyOn(fontApi,"listInstalled").mockResolvedValue([{id:"installed",name:info.name,supported:true}]);vi.spyOn(fontApi,"loadInstalled").mockResolvedValue(info);vi.spyOn(fontApi,"info").mockResolvedValue(info);vi.spyOn(fontApi,"bytes").mockResolvedValue(new Uint8Array([1,2]).buffer);vi.spyOn(fontApi,"release").mockResolvedValue();
 vi.spyOn(nativeAdapter,"openPdf").mockResolvedValue({id:"source",name:"Source.pdf",pages:[size]});vi.spyOn(nativeAdapter,"renderPage").mockResolvedValue("data:image/png;base64,AA");vi.spyOn(nativeAdapter,"closeDocument").mockResolvedValue();
 vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,"listOccurrenceTextRuns").mockResolvedValue({runs:[run]});vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,"suggestOccurrenceStyledParagraph").mockResolvedValue(selection);
 vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,"previewStyledParagraphOccurrence").mockResolvedValue({document:{id:"edited",name:"Edited.pdf",pages:[size]},pageIndices:[0]});
});
afterEach(async()=>{cleanup();await act(async()=>{await Promise.resolve();await flushFontReleases();});vi.restoreAllMocks();vi.unstubAllGlobals();delete (window as any).__TAURI_INTERNALS__;});
async function open(){render(<App initialDemo={false}/>);fireEvent.click(screen.getByRole("button",{name:"Open a PDF"}));await screen.findByRole("tab",{name:"Source.pdf"});fireEvent.click(screen.getByRole("button",{name:"Choose text"}));await screen.findByRole("textbox",{name:"Replacement text"});}
async function choose(){fireEvent.click(screen.getByRole("button",{name:"Choose substitute font…"}));fireEvent.click(await screen.findByRole("button",{name:info.name}));await waitFor(()=>expect(screen.getByRole("button",{name:"Apply font"})).toBeEnabled());fireEvent.click(screen.getByRole("button",{name:"Apply font"}));}
const mapping={sourceId:"source",pageIndex:0,objectIndices:[0,1],anchorIndex:0,paragraphText:"one two",sourceText:"one",sourceStart:0,sourceEnd:3,paragraphStart:0,paragraphEnd:3};
it("retains a chosen font across handoff, local history and Cancel",async()=>{
 const previous=nativeAdapter.mapParagraphDraft;nativeAdapter.mapParagraphDraft=vi.fn().mockResolvedValue(mapping);
 try{
  await open();fireEvent.change(screen.getByRole("textbox",{name:"Replacement text"}),{target:{value:"one this"}});await choose();
  fireEvent.click(screen.getByRole("button",{name:"Edit paragraph…"}));expect(await screen.findByRole("textbox",{name:"Paragraph text"})).toHaveTextContent("one this two");
  await act(async()=>{await Promise.resolve();await flushFontReleases();});expect(fontApi.release).not.toHaveBeenCalledWith(info.id);
  fireEvent.click(screen.getByRole("button",{name:"Preview paragraph"}));await waitFor(()=>expect(nativeAdapter.previewStyledParagraphOccurrence).toHaveBeenCalled());
  const request=vi.mocked(nativeAdapter.previewStyledParagraphOccurrence!).mock.calls[0][3];
  expect(request.runs.at(-1)).toEqual({text:"two",styleId:1});expect(request.styleChanges).toEqual([expect.objectContaining({sourceStyleId:0,fontId:info.id})]);
  fireEvent.click(screen.getAllByRole("button",{name:"Undo"}).find(b=>!b.hasAttribute("disabled"))!);expect(screen.getByRole("textbox",{name:"Paragraph text"})).toHaveTextContent("one two");
  await act(async()=>{await flushFontReleases();});expect(fontApi.release).not.toHaveBeenCalledWith(info.id);
  fireEvent.click(screen.getAllByRole("button",{name:"Redo"}).find(b=>!b.hasAttribute("disabled"))!);expect(screen.getByRole("textbox",{name:"Paragraph text"})).toHaveTextContent("one this two");
  fireEvent.click(screen.getByRole("button",{name:"Cancel"}));await waitFor(()=>expect(fontApi.release).toHaveBeenCalledWith(info.id));
 }finally{nativeAdapter.mapParagraphDraft=previous;}
});
it("holds the font until an abandoned preparation settles, then releases it",async()=>{
 let resolve!:(value:typeof mapping)=>void;const previous=nativeAdapter.mapParagraphDraft;nativeAdapter.mapParagraphDraft=vi.fn().mockImplementation(()=>new Promise(done=>{resolve=done;}));
 try{
  await open();await choose();fireEvent.click(screen.getByRole("button",{name:"Edit paragraph…"}));await waitFor(()=>expect(nativeAdapter.mapParagraphDraft).toHaveBeenCalled());
  cleanup();await act(async()=>{await Promise.resolve();await flushFontReleases();});expect(fontApi.release).not.toHaveBeenCalledWith(info.id);
  await act(async()=>resolve(mapping));await waitFor(()=>expect(fontApi.release).toHaveBeenCalledWith(info.id));expect(nativeAdapter.previewStyledParagraphOccurrence).not.toHaveBeenCalled();
 }finally{nativeAdapter.mapParagraphDraft=previous;}
});
it("keeps the selected font and input when changed handoff requires an unavailable styled editor",async()=>{
 const keys=["inspectOccurrenceStyledParagraph","suggestOccurrenceStyledParagraph","previewStyledParagraphOccurrence","inspectStyledParagraph","suggestStyledParagraph","previewStyledParagraph"] as const;
 const previous=Object.fromEntries(keys.map(key=>[key,nativeAdapter[key]]));for(const key of keys)(nativeAdapter as any)[key]=undefined;
 vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,"suggestOccurrenceTextParagraph").mockResolvedValue({objectIndices:[0],text:"one",fontName:"Courier",fontSize:12,bounds:selection.bounds});
 const mapPrevious=nativeAdapter.mapParagraphDraft;nativeAdapter.mapParagraphDraft=vi.fn().mockResolvedValue({...mapping,objectIndices:[0],paragraphText:"one"});
 try{
  await open();fireEvent.change(screen.getByRole("textbox",{name:"Replacement text"}),{target:{value:"one this"}});await choose();
  fireEvent.click(screen.getByRole("button",{name:"Edit paragraph…"}));
  await waitFor(()=>expect(screen.queryByRole("textbox",{name:"Replacement text"})).toHaveValue("one this"));
  expect(await screen.findByRole("alert")).toHaveTextContent("styled paragraph editor");expect(screen.getByText(info.name)).toBeInTheDocument();
  await act(async()=>{await flushFontReleases();});expect(fontApi.release).not.toHaveBeenCalledWith(info.id);
  expect(nativeAdapter.mapParagraphDraft).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button",{name:"Cancel"}));await waitFor(()=>expect(fontApi.release).toHaveBeenCalledWith(info.id));
 }finally{Object.assign(nativeAdapter,previous);nativeAdapter.mapParagraphDraft=mapPrevious;}
});
