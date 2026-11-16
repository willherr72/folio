import {act,cleanup,fireEvent,render,screen,waitFor,within} from "@testing-library/react";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {App} from "../src/App";
import {nativeAdapter} from "../src/editor/adapter";
import * as fonts from "../src/editor/custom-fonts";
import type {DocumentInfo} from "../src/editor/types";

vi.mock("../src/components/DocumentViewport",()=>({DocumentViewport:({pages,selectedPageId}:any)=><output data-testid="document-state">{JSON.stringify({pages:pages.map((p:any)=>({id:p.id,sourceId:p.sourceId})),selectedPageId})}</output>}));
const existing:DocumentInfo={id:"existing",name:"Existing.pdf",pages:[{width:300,height:400}]};
const late:DocumentInfo={id:"late-open",name:"Late.pdf",pages:[{width:300,height:400}]};
function pending<T>(){let resolve!:(value:T)=>void,reject!:(error:unknown)=>void;const promise=new Promise<T>((yes,no)=>{resolve=yes;reject=no;});return {promise,resolve,reject};}
beforeEach(()=>{
 localStorage.clear();vi.spyOn(nativeAdapter,"openPdf").mockResolvedValue(existing);
 vi.spyOn(nativeAdapter,"renderPage").mockResolvedValue("data:image/png;base64,");vi.spyOn(nativeAdapter,"closeDocument").mockResolvedValue();
 vi.spyOn(nativeAdapter,"engineStatus").mockResolvedValue("test");
});
afterEach(()=>{cleanup();vi.restoreAllMocks();});
const openButton=()=>screen.getByRole("button",{name:"Open"});
async function workspace(){const view=render(<App initialDemo={false}/>);fireEvent.click(screen.getByRole("button",{name:"Open a PDF"}));await screen.findByRole("tab",{name:"Existing.pdf"});return view;}

it("cancels an empty-workspace open before its adapter replies and restores focus",async()=>{
 const work=pending<DocumentInfo|null>();vi.mocked(nativeAdapter.openPdf).mockReturnValueOnce(work.promise);
 render(<App initialDemo={false}/>);const open=screen.getByRole("button",{name:"Open a PDF"});fireEvent.click(open);
 await waitFor(()=>expect(nativeAdapter.openPdf).toHaveBeenCalledOnce());
 try {
  fireEvent.click(await screen.findByRole("button",{name:"Cancel opening"}));
  await waitFor(()=>expect(open).toBeEnabled());expect(open).toHaveFocus();
  expect(screen.queryByRole("tab")).not.toBeInTheDocument();
 }finally{await act(async()=>work.resolve(late));}
 await waitFor(()=>expect(nativeAdapter.closeDocument).toHaveBeenCalledWith("late-open"));
 expect(screen.queryByRole("tab",{name:"Late.pdf"})).not.toBeInTheDocument();
});

it("preserves dirty pages, selection and Undo/Redo while cancelling another open",async()=>{
 await workspace();fireEvent.click(screen.getByRole("button",{name:"Duplicate"}));
 const before=screen.getByTestId("document-state").textContent;
 expect(within(screen.getByRole("tab",{name:"Existing.pdf"})).getByLabelText("Unsaved changes")).toBeInTheDocument();
 const work=pending<DocumentInfo|null>();vi.mocked(nativeAdapter.openPdf).mockReturnValueOnce(work.promise);fireEvent.click(openButton());
 try {
  fireEvent.click(await screen.findByRole("button",{name:"Cancel opening"}));await waitFor(()=>expect(openButton()).toBeEnabled());
  expect(openButton()).toHaveFocus();expect(screen.getByTestId("document-state").textContent).toBe(before);
  fireEvent.click(screen.getByRole("button",{name:"Undo"}));expect(screen.getByTestId("document-state").textContent).not.toBe(before);
  fireEvent.click(screen.getByRole("button",{name:"Redo"}));expect(screen.getByTestId("document-state").textContent).toBe(before);
 }finally{await act(async()=>work.resolve(late));}
 await waitFor(()=>expect(nativeAdapter.closeDocument).toHaveBeenCalledWith("late-open"));
 expect(screen.getAllByRole("tab")).toHaveLength(1);expect(nativeAdapter.closeDocument).not.toHaveBeenCalledWith("existing");
});

it("cancels preparation with Escape and never launches a picker after it finishes",async()=>{
 const preparation=pending<void>();vi.spyOn(fonts,"drainFontAcquisitions").mockReturnValueOnce(preparation.promise);
 render(<App initialDemo={false}/>);const open=screen.getByRole("button",{name:"Open a PDF"});fireEvent.click(open);
 try {
  await screen.findByRole("button",{name:"Cancel opening"});fireEvent.keyDown(window,{key:"Escape"});
  await waitFor(()=>expect(open).toBeEnabled());expect(open).toHaveFocus();expect(nativeAdapter.openPdf).not.toHaveBeenCalled();
 }finally{await act(async()=>preparation.resolve());}
 expect(nativeAdapter.openPdf).not.toHaveBeenCalled();
});

it("does not add pages from a cancelled Add PDF result",async()=>{
 await workspace();const before=screen.getByTestId("document-state").textContent,work=pending<DocumentInfo|null>();
 vi.mocked(nativeAdapter.openPdf).mockReturnValueOnce(work.promise);fireEvent.click(screen.getByRole("button",{name:"Add PDF"}));
 try {fireEvent.click(await screen.findByRole("button",{name:"Cancel opening"}));await waitFor(()=>expect(openButton()).toBeEnabled());}
 finally{await act(async()=>work.resolve(late));}
 await waitFor(()=>expect(nativeAdapter.closeDocument).toHaveBeenCalledWith("late-open"));
 expect(screen.getByTestId("document-state").textContent).toBe(before);expect(screen.getAllByRole("tab")).toHaveLength(1);
});

it("does not let a late cancelled rejection replace a newer open",async()=>{
 const work=pending<DocumentInfo|null>();vi.mocked(nativeAdapter.openPdf).mockReturnValueOnce(work.promise);
 render(<App initialDemo={false}/>);fireEvent.click(screen.getByRole("button",{name:"Open a PDF"}));
 try {
  fireEvent.click(await screen.findByRole("button",{name:"Cancel opening"}));
  await waitFor(()=>expect(screen.getByRole("button",{name:"Open a PDF"})).toBeEnabled());
  fireEvent.click(screen.getByRole("button",{name:"Open a PDF"}));await screen.findByRole("tab",{name:"Existing.pdf"});
 }finally{await act(async()=>work.reject(new Error("late cancelled error")));}
 expect(screen.queryByText(/late cancelled error/)).not.toBeInTheDocument();expect(openButton()).toBeEnabled();
});

it("leaves Escape to the password modal and has no duplicate opening cancel control",async()=>{
 vi.mocked(nativeAdapter.openPdf).mockImplementationOnce(async options=>{
  await options!.requestPassword({status:"passwordRequired",challengeId:"challenge",name:"Locked.pdf",incorrect:false});return null;
 });
 render(<App initialDemo={false}/>);fireEvent.click(screen.getByRole("button",{name:"Open a PDF"}));
 const dialog=await screen.findByRole("dialog",{name:"Open protected PDF"});
 expect(screen.queryByRole("button",{name:"Cancel opening"})).not.toBeInTheDocument();fireEvent.keyDown(dialog,{key:"Escape"});
 await waitFor(()=>expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
 expect(screen.getByRole("button",{name:"Open a PDF"})).toBeEnabled();
});

it("keeps normal picker cancellation and normal open errors",async()=>{
 vi.mocked(nativeAdapter.openPdf).mockResolvedValueOnce(null).mockRejectedValueOnce(new Error("read failed"));
 render(<App initialDemo={false}/>);const open=screen.getByRole("button",{name:"Open a PDF"});fireEvent.click(open);
 await waitFor(()=>expect(nativeAdapter.openPdf).toHaveBeenCalledTimes(1));await waitFor(()=>expect(open).toBeEnabled());
 expect(screen.queryByRole("button",{name:"Cancel opening"})).not.toBeInTheDocument();
 fireEvent.click(open);expect(await screen.findByRole("alert")).toHaveTextContent("read failed");expect(open).toBeEnabled();
});

it("disposes a source arriving after unmount",async()=>{
 const work=pending<DocumentInfo|null>();vi.mocked(nativeAdapter.openPdf).mockReturnValueOnce(work.promise);
 const view=render(<App initialDemo={false}/>);fireEvent.click(screen.getByRole("button",{name:"Open a PDF"}));
 await waitFor(()=>expect(nativeAdapter.openPdf).toHaveBeenCalledOnce());view.unmount();
 await act(async()=>work.resolve(late));await waitFor(()=>expect(nativeAdapter.closeDocument).toHaveBeenCalledWith("late-open"));
});
