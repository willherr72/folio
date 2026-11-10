import {cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {App} from "../src/App";
import {nativeAdapter} from "../src/editor/adapter";
import {clearSearchTextCache} from "../src/editor/search";
import {clearRenderCache} from "../src/components/PageView";
vi.mock("../src/editor/useWorkspaceRecovery",()=>({useWorkspaceRecovery:()=>({ready:true,checkpoint:vi.fn().mockResolvedValue(undefined)})}));
vi.mock("../src/components/DocumentViewport",()=>({DocumentViewport:(p:any)=><><output data-testid="crop-state">{JSON.stringify(p.pages[0].crop??null)}</output><output data-testid="navigation">{JSON.stringify(p.navigationRequest)}</output></>}));
beforeEach(()=>{
  localStorage.clear();Object.defineProperty(window,"__TAURI_INTERNALS__",{value:{invoke:vi.fn().mockResolvedValue(0),transformCallback:()=>0},configurable:true});
  vi.spyOn(nativeAdapter,"openPdf").mockResolvedValue({id:"crop-source",name:"Crop.pdf",pages:[{width:300,height:400}]});
  vi.spyOn(nativeAdapter,"renderPage").mockResolvedValue("data:image/png;base64,AA");
  vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,"renderPagePlan").mockResolvedValue("data:image/png;base64,AA");
  vi.spyOn(nativeAdapter,"closeDocument").mockResolvedValue();vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,"startPdfSave").mockReturnValue({result:Promise.resolve("Cropped.pdf"),cancel:vi.fn().mockResolvedValue(false)});
});
afterEach(()=>{cleanup();clearSearchTextCache(["crop-source"]);clearRenderCache(["crop-source"]);vi.restoreAllMocks();delete (window as any).__TAURI_INTERNALS__;});
it("crops as one undo step and passes the crop to ordinary save",async()=>{
  render(<App initialDemo={false}/>);fireEvent.click(screen.getByRole("button",{name:"Open a PDF"}));await screen.findByRole("tab",{name:"Crop.pdf"});
  fireEvent.click(screen.getByRole("button",{name:"Crop page…"}));fireEvent.change(screen.getByLabelText("Left margin (pt)"),{target:{value:"20"}});
  fireEvent.click(screen.getByRole("button",{name:"Preview crop"}));await waitFor(()=>expect(screen.getByRole("button",{name:"Apply crop"})).toBeEnabled());
  fireEvent.click(screen.getByRole("button",{name:"Apply crop"}));await waitFor(()=>expect(screen.queryByRole("dialog")).toBeNull());
  expect(screen.getByTestId("crop-state")).toHaveTextContent('"x":20');
  fireEvent.click(screen.getByRole("button",{name:"Undo"}));expect(screen.getByTestId("crop-state")).toHaveTextContent("null");expect(screen.getByRole("button",{name:"Undo"})).toBeDisabled();
  fireEvent.click(screen.getByRole("button",{name:"Redo"}));expect(screen.getByTestId("crop-state")).toHaveTextContent('"x":20');
  fireEvent.click(screen.getByRole("button",{name:"Save a copy"}));await waitFor(()=>expect(nativeAdapter.startPdfSave).toHaveBeenCalled());
  expect(vi.mocked(nativeAdapter.startPdfSave!).mock.calls[0][0][0]).toMatchObject({sourceId:"crop-source",width:300,height:400,crop:{x:20,y:0,width:280,height:400}});
});

it("searches a partially visible phrase and navigates to its visible second line",async()=>{
 vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,"getPageText").mockResolvedValue({characters:[{text:"alpha ",x:20,y:20,width:40,height:12},{text:"beta",x:90,y:300,width:60,height:12}]});
 render(<App initialDemo={false}/>);fireEvent.click(screen.getByRole("button",{name:"Open a PDF"}));await screen.findByRole("tab",{name:"Crop.pdf"});
 fireEvent.click(screen.getByRole("button",{name:"Crop page…"}));
 for(const side of ["Left","Top"])fireEvent.change(screen.getByLabelText(`${side} margin (pt)`),{target:{value:"100"}});
 fireEvent.click(screen.getByRole("button",{name:"Preview crop"}));await waitFor(()=>expect(screen.getByRole("button",{name:"Apply crop"})).toBeEnabled());
 fireEvent.click(screen.getByRole("button",{name:"Apply crop"}));await waitFor(()=>expect(screen.queryByRole("dialog")).toBeNull());
 fireEvent.click(screen.getByRole("button",{name:"Find"}));fireEvent.change(screen.getByRole("searchbox"),{target:{value:"alpha beta"}});
 await screen.findByText("1 of 1");await waitFor(()=>expect(JSON.parse(screen.getByTestId("navigation").textContent!).rect).toEqual({x:100,y:300,width:50,height:12}));
});
