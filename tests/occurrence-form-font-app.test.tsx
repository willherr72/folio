import {act,cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {App} from "../src/App";
import {nativeAdapter} from "../src/editor/adapter";
import {fontApi,flushFontReleases} from "../src/editor/custom-fonts";
const run={objectIndex:1,objectPath:[1,0,0],text:"Café old",fontName:"ABCDEF+Subset",fontSize:16,bounds:{x:28,y:90,width:90,height:16},supported:true,isEmbedded:true,canSubstitute:true};
const id="d".repeat(64),info={id,name:"Full Serif",weight:400,italic:false,coverage:[[32,126],[232,233]] as [number,number][]};
vi.mock("../src/components/DocumentViewport",()=>({DocumentViewport:(props:any)=><div><output data-testid="sources">{props.pages.map((p:any)=>p.sourceId).join(",")}</output><button onClick={()=>props.onEditText(props.pages[0].id,run)}>Select shared text</button></div>}));
beforeEach(()=>{
 localStorage.clear();vi.stubGlobal("FontFace",class{load(){return Promise.resolve(this);}});Object.defineProperty(document,"fonts",{configurable:true,value:{add:vi.fn(),delete:vi.fn()}});
 vi.spyOn(fontApi,"listInstalled").mockResolvedValue([{id:"installed",name:info.name,supported:true}]);vi.spyOn(fontApi,"loadInstalled").mockResolvedValue(info);vi.spyOn(fontApi,"info").mockResolvedValue(info);vi.spyOn(fontApi,"bytes").mockResolvedValue(new Uint8Array([1,2]).buffer);vi.spyOn(fontApi,"release").mockResolvedValue();
 vi.spyOn(nativeAdapter,"openPdf").mockResolvedValue({id:"original",name:"Shared.pdf",pages:[{width:400,height:400}]});vi.spyOn(nativeAdapter,"renderPage").mockResolvedValue("data:image/png;base64,");vi.spyOn(nativeAdapter,"closeDocument").mockResolvedValue();vi.spyOn(nativeAdapter,"exportPdf").mockResolvedValue("Saved.pdf");
 vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,"replaceFormTextOccurrence").mockResolvedValue({document:{id:"derived",name:"Page.pdf",pages:[{width:400,height:400},{width:400,height:400}]},pageIndices:[0,1]});vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,"replaceText").mockResolvedValue({id:"wrong",name:"Page.pdf",pages:[{width:400,height:400}]});
});
afterEach(async()=>{cleanup();await act(async()=>{await Promise.resolve();await flushFontReleases();});vi.restoreAllMocks();vi.unstubAllGlobals();});
async function open(){render(<App initialDemo={false}/>);fireEvent.click(screen.getByRole("button",{name:"Open a PDF"}));await screen.findByRole("tab",{name:"Shared.pdf"});fireEvent.click(screen.getByRole("button",{name:"Duplicate"}));fireEvent.click(screen.getByRole("button",{name:"Select shared text"}));fireEvent.change(screen.getByRole("textbox",{name:"Replacement text"}),{target:{value:"Cafè edit"}});}
async function choose(){fireEvent.click(screen.getByRole("button",{name:"Choose substitute font…"}));fireEvent.click(await screen.findByRole("button",{name:"Full Serif"}));await waitFor(()=>expect(screen.getByRole("button",{name:"Apply font"})).toBeEnabled());fireEvent.click(screen.getByRole("button",{name:"Apply font"}));}
it("recovers from missing glyphs with an explicit occurrence font and one undo step",async()=>{
 vi.mocked(nativeAdapter.replaceFormTextOccurrence!).mockRejectedValueOnce(new Error("The subset lacks è."));await open();fireEvent.click(screen.getByRole("button",{name:"Apply changes"}));await screen.findByRole("alert");
 expect(screen.getByRole("textbox",{name:"Replacement text"})).toHaveValue("Cafè edit");await choose();expect(screen.queryByRole("alert")).not.toBeInTheDocument();expect(screen.queryByRole("button",{name:/Edit together/})).not.toBeInTheDocument();
 fireEvent.click(screen.getByRole("button",{name:"Apply changes"}));await waitFor(()=>expect(screen.getByTestId("sources")).toHaveTextContent("derived,derived"));
 expect(nativeAdapter.replaceFormTextOccurrence).toHaveBeenLastCalledWith("original",[0,0],0,[1,0,0],"Café old","Cafè edit",id);expect(nativeAdapter.replaceText).not.toHaveBeenCalled();await waitFor(()=>expect(fontApi.release).toHaveBeenCalledWith(id));
 fireEvent.click(screen.getByRole("button",{name:"Undo"}));expect(screen.getByTestId("sources")).toHaveTextContent("original,original");fireEvent.click(screen.getByRole("button",{name:"Redo"}));expect(screen.getByTestId("sources")).toHaveTextContent("derived,derived");
 fireEvent.click(screen.getByRole("button",{name:"Save a copy"}));await waitFor(()=>expect(nativeAdapter.exportPdf).toHaveBeenCalled());expect(vi.mocked(nativeAdapter.exportPdf).mock.calls[0][0].map(p=>p.sourceId)).toEqual(["derived","derived"]);
});
it("holds the chosen font through a late apply and closes its abandoned source",async()=>{
 let finish!:(value:any)=>void;vi.mocked(nativeAdapter.replaceFormTextOccurrence!).mockImplementation(()=>new Promise(resolve=>{finish=resolve;}));await open();await choose();fireEvent.click(screen.getByRole("button",{name:"Apply changes"}));
 await waitFor(()=>expect(nativeAdapter.replaceFormTextOccurrence).toHaveBeenCalled());cleanup();await act(async()=>{await Promise.resolve();await flushFontReleases();});expect(fontApi.release).not.toHaveBeenCalled();
 await act(async()=>finish({document:{id:"orphan",name:"Page.pdf",pages:[{width:400,height:400},{width:400,height:400}]},pageIndices:[0,1]}));await waitFor(()=>expect(nativeAdapter.closeDocument).toHaveBeenCalledWith("orphan"));await waitFor(()=>expect(fontApi.release).toHaveBeenCalledWith(id));
});
it("retains the chosen font and draft after a rejected substitute apply for retry",async()=>{
 vi.mocked(nativeAdapter.replaceFormTextOccurrence!).mockRejectedValueOnce(new Error("Replacement exceeds the available area."));await open();await choose();
 fireEvent.click(screen.getByRole("button",{name:"Apply changes"}));await screen.findByRole("alert");
 expect(screen.getByRole("textbox",{name:"Replacement text"})).toHaveValue("Cafè edit");expect(screen.getByRole("button",{name:"Use original font"})).toBeEnabled();expect(fontApi.release).not.toHaveBeenCalled();
 expect(screen.getByTestId("sources")).toHaveTextContent("original,original");fireEvent.change(screen.getByRole("textbox",{name:"Replacement text"}),{target:{value:"Cafè"}});fireEvent.click(screen.getByRole("button",{name:"Apply changes"}));
 await waitFor(()=>expect(screen.getByTestId("sources")).toHaveTextContent("derived,derived"));expect(nativeAdapter.replaceFormTextOccurrence).toHaveBeenLastCalledWith("original",[0,0],0,[1,0,0],"Café old","Cafè",id);await waitFor(()=>expect(fontApi.release).toHaveBeenCalledWith(id));
});
it("releases the chosen font after an abandoned apply rejects",async()=>{
 let reject!:(reason:Error)=>void;vi.mocked(nativeAdapter.replaceFormTextOccurrence!).mockImplementation(()=>new Promise((_,fail)=>{reject=fail;}));await open();await choose();fireEvent.click(screen.getByRole("button",{name:"Apply changes"}));
 await waitFor(()=>expect(nativeAdapter.replaceFormTextOccurrence).toHaveBeenCalled());cleanup();await act(async()=>{await Promise.resolve();await flushFontReleases();});expect(fontApi.release).not.toHaveBeenCalled();
 await act(async()=>reject(new Error("Replacement exceeds the available area.")));await waitFor(()=>expect(fontApi.release).toHaveBeenCalledWith(id));expect(nativeAdapter.closeDocument).not.toHaveBeenCalledWith("derived");
});
