import {act,cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {App} from "../src/App";
import {nativeAdapter} from "../src/editor/adapter";
import {clearSearchTextCache} from "../src/editor/search";
vi.mock("../src/components/DocumentViewport",()=>({DocumentViewport:()=>null}));
beforeEach(()=>{
 localStorage.clear();
 vi.spyOn(nativeAdapter,"openPdf").mockResolvedValue({id:"source",name:"Pages.pdf",pages:[{width:300,height:400,overlays:[{type:"comment",id:"note",x:20,y:20,text:"Keep",color:"#ffff00"}]},{width:400,height:500},{width:500,height:600}]});
 vi.spyOn(nativeAdapter,"renderPage").mockResolvedValue("data:image/png;base64,");
 vi.spyOn(nativeAdapter,"exportPdf").mockResolvedValue("Extract.pdf");
 vi.spyOn(nativeAdapter,"closeDocument").mockResolvedValue();
});
afterEach(()=>{cleanup();clearSearchTextCache(["source"]);vi.restoreAllMocks();});
async function open(){render(<App initialDemo={false}/>);fireEvent.click(screen.getByRole("button",{name:"Open a PDF"}));await screen.findByRole("tab",{name:"Pages.pdf"});}
function extract(){fireEvent.click(screen.getByRole("button",{name:"Save options"}));fireEvent.click(screen.getByRole("menuitem",{name:"Extract pages…"}));}
it("extracts the current edited arrangement without marking the document saved or adding history",async()=>{
 await open();fireEvent.click(screen.getByRole("button",{name:"Rotate"}));fireEvent.click(screen.getByRole("button",{name:"Move down"}));
 extract();expect(screen.getByRole("button",{name:"Save a copy"})).toBeDisabled();
 fireEvent.change(screen.getByRole("combobox",{name:"Pages"}),{target:{value:"range"}});
 fireEvent.change(screen.getByRole("textbox",{name:"Page range"}),{target:{value:"3,1-2"}});
 fireEvent.click(screen.getByRole("button",{name:"Save extracted PDF…"}));await waitFor(()=>expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
 const exported=vi.mocked(nativeAdapter.exportPdf).mock.calls[0][0];expect(exported.map(p=>p.pageIndex)).toEqual([1,0,2]);
 expect(exported[1]).toMatchObject({sourceId:"source",rotation:90,overlays:[{id:"note",text:"Keep"}]});
 expect(screen.getByLabelText("Unsaved changes")).toBeInTheDocument();
 fireEvent.click(screen.getByRole("button",{name:"Undo"}));fireEvent.click(screen.getByRole("button",{name:"Undo"}));
 expect(screen.getByRole("button",{name:"Undo"})).toBeDisabled();expect(screen.queryByLabelText("Unsaved changes")).not.toBeInTheDocument();
 expect(nativeAdapter.closeDocument).not.toHaveBeenCalled();
});
it("keeps a cancelled range and failed export available for retry",async()=>{
 vi.mocked(nativeAdapter.exportPdf).mockResolvedValueOnce(null).mockRejectedValueOnce(new Error("Disk full"));
 await open();extract();fireEvent.change(screen.getByRole("combobox",{name:"Pages"}),{target:{value:"range"}});
 const range=screen.getByRole("textbox",{name:"Page range"});fireEvent.change(range,{target:{value:"2-3"}});
 fireEvent.click(screen.getByRole("button",{name:"Save extracted PDF…"}));await waitFor(()=>expect(screen.getByRole("button",{name:"Save extracted PDF…"})).toBeEnabled());
 expect(range).toHaveValue("2-3");fireEvent.click(screen.getByRole("button",{name:"Save extracted PDF…"}));
 expect(await screen.findByRole("alert")).toHaveTextContent("Disk full");expect(range).toHaveValue("2-3");
 fireEvent.click(screen.getByRole("button",{name:"Save extracted PDF…"}));await waitFor(()=>expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
 expect(nativeAdapter.exportPdf).toHaveBeenCalledTimes(3);
});
it("blocks page mutation, save and close while native export is pending",async()=>{
 let done!:(value:string|null)=>void;vi.mocked(nativeAdapter.exportPdf).mockImplementation(()=>new Promise(resolve=>done=resolve));
 await open();fireEvent.click(screen.getByRole("button",{name:"Extract pages…"}));
 fireEvent.click(screen.getByRole("button",{name:"Save extracted PDF…"}));
 expect(screen.getByRole("button",{name:"Rotate"})).toBeDisabled();expect(screen.getByRole("button",{name:"Close Pages.pdf"})).toBeDisabled();
 fireEvent.keyDown(screen.getByRole("dialog"),{key:"Escape"});expect(screen.getByRole("dialog")).toBeInTheDocument();
 fireEvent.keyDown(window,{key:"s",ctrlKey:true});expect(nativeAdapter.exportPdf).toHaveBeenCalledTimes(1);
 await act(async()=>done("Extract.pdf"));expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
});

it("freezes Current page and defers late search navigation throughout extraction",async()=>{
 const resolve:((value:any)=>void)[]=[];
 vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,"getPageText").mockImplementation(()=>new Promise(done=>resolve.push(done)));
 await open();fireEvent.click(screen.getByRole("button",{name:"Find"}));
 fireEvent.change(screen.getByRole("searchbox"),{target:{value:"Needle"}});
 await waitFor(()=>expect(resolve.length).toBeGreaterThan(0));
 extract();
 await act(async()=>resolve[0]({characters:[]}));
 await waitFor(()=>expect(resolve.length).toBeGreaterThan(1));
 await act(async()=>resolve[1]({characters:Array.from("Needle",(text,i)=>({text,x:i*6,y:30,width:6,height:12}))}));
 await waitFor(()=>expect(resolve.length).toBeGreaterThan(2));
 await act(async()=>resolve[2]({characters:[]}));
 await screen.findByText("1 of 1");
 expect(screen.getByText("Page 1 of 3")).toBeInTheDocument();
 fireEvent.click(screen.getByRole("button",{name:"Save extracted PDF…"}));
 await waitFor(()=>expect(nativeAdapter.exportPdf).toHaveBeenCalled());
 expect(vi.mocked(nativeAdapter.exportPdf).mock.calls[0][0][0].pageIndex).toBe(0);
});
