import {act,cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {App} from "../src/App";
import {nativeAdapter} from "../src/editor/adapter";
import {clearSearchTextCache} from "../src/editor/search";
const run={objectIndex:2,text:"Original",fontName:"Helvetica",fontSize:12,bounds:{x:30,y:40,width:90,height:14},supported:true};
vi.mock("../src/components/DocumentViewport",()=>({DocumentViewport:(props:any)=><div><output data-testid="sources">{props.pages.map((page:any)=>page.sourceId).join(",")}</output><button onClick={()=>props.onEditText?.(props.pages[0].id,run)}>Choose existing run</button></div>}));
beforeEach(()=>{
 localStorage.clear();
 vi.spyOn(nativeAdapter,"openPdf").mockResolvedValue({id:"text-old",name:"Editable.pdf",pages:[{width:300,height:400,overlays:[{type:"comment",id:"note",x:20,y:25,text:"Keep note",color:"#ffff00"}]}]});
 vi.spyOn(nativeAdapter,"renderPage").mockResolvedValue("data:image/png;base64,");
 vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,"getPageText").mockImplementation(async source=>({characters:Array.from(source==="text-old"?"Original":"New",(text,i)=>({text,x:i*6,y:30,width:6,height:12}))}));
 vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,"replaceText").mockResolvedValue({id:"text-new",name:"Page.pdf",pages:[{width:300,height:400}]});
 vi.spyOn(nativeAdapter,"closeDocument").mockResolvedValue();
 vi.spyOn(nativeAdapter,"exportPdf").mockResolvedValue("Edited.pdf");
});
afterEach(()=>{cleanup();clearSearchTextCache(["text-old","text-new"]);vi.restoreAllMocks();});
async function open(){render(<App initialDemo={false}/>);fireEvent.click(screen.getByRole("button",{name:"Open a PDF"}));await screen.findByRole("tab",{name:"Editable.pdf"});fireEvent.click(screen.getByRole("button",{name:"Edit text"}));}
async function edit(){fireEvent.click(screen.getByRole("button",{name:"Choose existing run"}));const input=await screen.findByRole("textbox",{name:"Replacement text"});fireEvent.change(input,{target:{value:"New"}});fireEvent.click(screen.getByRole("button",{name:"Apply changes"}));}
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
