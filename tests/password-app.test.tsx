import {act,cleanup,fireEvent,render,screen,waitFor,within} from "@testing-library/react";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {App} from "../src/App";
import {nativeAdapter} from "../src/editor/adapter";
import type {DocumentInfo} from "../src/editor/types";
const reader:DocumentInfo={id:"reader",name:"Protected.pdf",pages:[{width:300,height:400}],protection:{documentId:"original",revision:4,owner:false,permissions:{print:true,printHighQuality:false,copyContent:false,editContent:false,annotate:true,fillForms:true,assemble:false}}};
const owner:DocumentInfo={...reader,id:"owner",protection:{...reader.protection!,owner:true,permissions:{print:true,printHighQuality:true,copyContent:true,editContent:true,annotate:true,fillForms:true,assemble:true}}};
vi.mock("../src/components/DocumentViewport",()=>({DocumentViewport:(props:any)=><output data-testid="source">{props.pages[0].sourceId}</output>}));
beforeEach(()=>{
 localStorage.clear();vi.spyOn(nativeAdapter,"openPdf").mockResolvedValue(reader);
 vi.spyOn(nativeAdapter,"renderPage").mockResolvedValue("data:image/png;base64,");
 vi.spyOn(nativeAdapter,"closeDocument").mockResolvedValue();
});
afterEach(()=>{cleanup();vi.restoreAllMocks();});
it("requests the owner password, retries, and keeps unlock undoable without unsaved PDF edits",async()=>{
 const unlock=vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,"unlockSources").mockResolvedValueOnce(null).mockResolvedValueOnce([{sourceId:"reader",document:owner}]);
 render(<App initialDemo={false}/>);fireEvent.click(screen.getByRole("button",{name:"Open a PDF"}));await screen.findByRole("tab",{name:"Protected.pdf"});
 fireEvent.click(screen.getByRole("button",{name:"Unlock editing"}));const dialog=await screen.findByRole("dialog",{name:"Unlock editing"});
 const input=within(dialog).getByLabelText("Owner password");fireEvent.change(input,{target:{value:"reader"}});fireEvent.submit(input.closest("form")!);
 await within(dialog).findByRole("alert");expect(screen.getByTestId("source")).toHaveTextContent("reader");
 fireEvent.change(input,{target:{value:"owner"}});fireEvent.submit(input.closest("form")!);
 await waitFor(()=>expect(screen.getByTestId("source")).toHaveTextContent("owner"));
 expect(unlock).toHaveBeenLastCalledWith(["reader"],"owner");
 expect(screen.queryByText("Unsaved edits")).not.toBeInTheDocument();
 fireEvent.click(screen.getByRole("button",{name:"Undo"}));expect(screen.getByTestId("source")).toHaveTextContent("reader");
 expect(screen.getByRole("button",{name:"Unlock editing"})).toBeEnabled();
 fireEvent.click(screen.getByRole("button",{name:"Redo"}));expect(screen.getByTestId("source")).toHaveTextContent("owner");
 expect(nativeAdapter.closeDocument).not.toHaveBeenCalled();
});
it("opens with the password workflow and releases a late owner source after unmount",async()=>{
 vi.mocked(nativeAdapter.openPdf).mockImplementation(async options=>{
  expect(options).toBeDefined();const answer=await options!.requestPassword({status:"passwordRequired",challengeId:"token",name:"Protected.pdf",incorrect:false});expect(answer).toBe("reader");return reader;
 });
 let done!:(value:any)=>void;vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,"unlockSources").mockImplementation(()=>new Promise(resolve=>{done=resolve;}));
 const view=render(<App initialDemo={false}/>);fireEvent.click(screen.getByRole("button",{name:"Open a PDF"}));
 const input=await screen.findByLabelText("PDF password");fireEvent.change(input,{target:{value:"reader"}});fireEvent.submit(input.closest("form")!);
 await screen.findByRole("tab",{name:"Protected.pdf"});fireEvent.click(screen.getByRole("button",{name:"Unlock editing"}));
 const ownerInput=await screen.findByLabelText("Owner password");fireEvent.change(ownerInput,{target:{value:"owner"}});fireEvent.submit(ownerInput.closest("form")!);
 await waitFor(()=>expect(nativeAdapter.unlockSources).toHaveBeenCalled());view.unmount();
 await act(async()=>done([{sourceId:"reader",document:owner}]));
 expect(nativeAdapter.closeDocument).toHaveBeenCalledWith("owner");
});


it("disables both existing-text tools when the source denies content edits",async()=>{
 render(<App initialDemo={false}/>);fireEvent.click(screen.getByRole("button",{name:"Open a PDF"}));await screen.findByRole("tab",{name:"Protected.pdf"});
 expect(screen.getByRole("button",{name:"Edit text"})).toBeDisabled();
 expect(screen.getByRole("button",{name:"Edit paragraphs"})).toBeDisabled();
});
