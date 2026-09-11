import {act,cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {App} from "../src/App";
import {nativeAdapter} from "../src/editor/adapter";
import {fontApi,flushFontReleases} from "../src/editor/custom-fonts";
const run={objectIndex:1,text:"Original",fontName:"ABCDEF+Subset",fontSize:12,bounds:{x:30,y:40,width:90,height:14},supported:true,isEmbedded:true,canSubstitute:true};
const id="f".repeat(64),info={id,name:"Full Serif",weight:400,italic:false,coverage:[[32,126],[233,233]] as [number,number][]};
vi.mock("../src/components/DocumentViewport",()=>({DocumentViewport:(props:any)=><div><output data-testid="source">{props.pages[0].sourceId}</output><button onClick={()=>props.onEditText(props.pages[0].id,run)}>Select embedded run</button></div>}));
beforeEach(()=>{
 localStorage.clear();vi.stubGlobal("FontFace",class{load(){return Promise.resolve(this);}});Object.defineProperty(document,"fonts",{configurable:true,value:{add:vi.fn(),delete:vi.fn()}});
 vi.spyOn(fontApi,"listInstalled").mockResolvedValue([{id:"installed",name:info.name,supported:true}]);vi.spyOn(fontApi,"loadInstalled").mockResolvedValue(info);vi.spyOn(fontApi,"info").mockResolvedValue(info);vi.spyOn(fontApi,"bytes").mockResolvedValue(new Uint8Array([1,2]).buffer);vi.spyOn(fontApi,"release").mockResolvedValue();
 vi.spyOn(nativeAdapter,"openPdf").mockResolvedValue({id:"original",name:"Subset.pdf",pages:[{width:300,height:400}]});vi.spyOn(nativeAdapter,"renderPage").mockResolvedValue("data:image/png;base64,");vi.spyOn(nativeAdapter,"closeDocument").mockResolvedValue();vi.spyOn(nativeAdapter,"exportPdf").mockResolvedValue("Saved.pdf");
 vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,"replaceText").mockResolvedValue({id:"derived",name:"Page.pdf",pages:[{width:300,height:400}]});
});
afterEach(async()=>{cleanup();await act(async()=>{await Promise.resolve();await flushFontReleases();});vi.restoreAllMocks();vi.unstubAllGlobals();});
async function open(){render(<App initialDemo={false}/>);fireEvent.click(screen.getByRole("button",{name:"Open a PDF"}));await screen.findByRole("tab",{name:"Subset.pdf"});fireEvent.click(screen.getByRole("button",{name:"Edit text"}));fireEvent.click(screen.getByRole("button",{name:"Select embedded run"}));fireEvent.change(screen.getByRole("textbox",{name:"Replacement text"}),{target:{value:"New café"}});}
async function substitute(){fireEvent.click(screen.getByRole("button",{name:"Choose substitute font…"}));fireEvent.click(await screen.findByRole("button",{name:"Full Serif"}));await waitFor(()=>expect(screen.getByRole("button",{name:"Apply font"})).toBeEnabled());fireEvent.click(screen.getByRole("button",{name:"Apply font"}));}
it("retries with an explicit font and keeps the resulting source through undo/redo/export",async()=>{
 vi.mocked(nativeAdapter.replaceText!).mockRejectedValueOnce(new Error("The subset lacks U+00E9. Choose a substitute font."));await open();fireEvent.click(screen.getByRole("button",{name:"Apply changes"}));await screen.findByRole("alert");expect(screen.getByRole("textbox",{name:"Replacement text"})).toHaveValue("New café");
 await substitute();expect(screen.queryByRole("alert")).not.toBeInTheDocument();expect(fontApi.release).not.toHaveBeenCalled();fireEvent.click(screen.getByRole("button",{name:"Apply changes"}));
 await waitFor(()=>expect(screen.getByTestId("source")).toHaveTextContent("derived"));expect(nativeAdapter.replaceText).toHaveBeenLastCalledWith("original",0,1,"Original","New café",id);await waitFor(()=>expect(fontApi.release).toHaveBeenCalledWith(id));
 fireEvent.click(screen.getByRole("button",{name:"Undo"}));expect(screen.getByTestId("source")).toHaveTextContent("original");fireEvent.click(screen.getByRole("button",{name:"Redo"}));expect(screen.getByTestId("source")).toHaveTextContent("derived");
 fireEvent.click(screen.getByRole("button",{name:"Save a copy"}));await waitFor(()=>expect(nativeAdapter.exportPdf).toHaveBeenCalled());expect(vi.mocked(nativeAdapter.exportPdf).mock.calls[0][0][0].sourceId).toBe("derived");
});
it("cancels a chosen substitute without editing the PDF or leaking its font",async()=>{
 await open();await substitute();fireEvent.click(screen.getByRole("button",{name:"Cancel"}));expect(nativeAdapter.replaceText).not.toHaveBeenCalled();expect(screen.getByTestId("source")).toHaveTextContent("original");await waitFor(()=>expect(fontApi.release).toHaveBeenCalledWith(id));
});
