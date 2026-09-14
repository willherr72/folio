import {act,cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {App} from "../src/App";
import {nativeAdapter} from "../src/editor/adapter";
const run={objectIndex:2,objectPath:[2,0,1],text:"Shared words",fontName:"Helvetica",fontSize:12,bounds:{x:30,y:50,width:80,height:15},supported:true};
vi.mock("../src/components/DocumentViewport",()=>({DocumentViewport:(props:any)=><div><output data-testid="sources">{props.pages.map((page:any)=>page.sourceId).join(",")}</output><button onClick={()=>props.onEditText?.(props.pages[0].id,run)}>Choose form occurrence</button></div>}));
beforeEach(()=>{localStorage.clear();vi.spyOn(nativeAdapter,"openPdf").mockResolvedValue({id:"form-original",name:"Forms.pdf",pages:[{width:300,height:400}]});vi.spyOn(nativeAdapter,"closeDocument").mockResolvedValue();vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,"replaceText").mockResolvedValue({id:"wrong-command",name:"Page.pdf",pages:[{width:300,height:400}]});});
afterEach(()=>{cleanup();vi.restoreAllMocks();});
async function open(){render(<App initialDemo={false}/>);fireEvent.click(screen.getByRole("button",{name:"Open a PDF"}));await screen.findByRole("tab",{name:"Forms.pdf"});fireEvent.click(screen.getByRole("button",{name:"Choose form occurrence"}));const input=await screen.findByRole("textbox",{name:"Replacement text"});fireEvent.change(input,{target:{value:"Changed words"}});return input;}
it("routes the selected occurrence into one undo step and leaves duplicated pages on the original source",async()=>{
 const replace=vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,"replaceFormText").mockResolvedValue({id:"form-edited",name:"Forms.pdf",pages:[{width:300,height:400}]});
 render(<App initialDemo={false}/>);fireEvent.click(screen.getByRole("button",{name:"Open a PDF"}));await screen.findByRole("tab",{name:"Forms.pdf"});fireEvent.click(screen.getByRole("button",{name:"Duplicate"}));fireEvent.click(screen.getByRole("button",{name:"Choose form occurrence"}));
 fireEvent.change(await screen.findByRole("textbox",{name:"Replacement text"}),{target:{value:"Changed words"}});fireEvent.click(screen.getByRole("button",{name:"Apply changes"}));
 await waitFor(()=>expect(screen.getByTestId("sources")).toHaveTextContent("form-edited,form-original"));
 expect(replace).toHaveBeenCalledWith("form-original",0,[2,0,1],"Shared words","Changed words");expect(nativeAdapter.replaceText).not.toHaveBeenCalled();
 fireEvent.click(screen.getByRole("button",{name:"Undo"}));expect(screen.getByTestId("sources")).toHaveTextContent("form-original,form-original");
 fireEvent.click(screen.getByRole("button",{name:"Redo"}));expect(screen.getByTestId("sources")).toHaveTextContent("form-edited,form-original");
});
it("retains rejected drafts and closes a derived form source that arrives after unmount",async()=>{
 const replace=vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,"replaceFormText").mockRejectedValueOnce(new Error("Replacement exceeds the form boundary"));
 const input=await open();fireEvent.click(screen.getByRole("button",{name:"Apply changes"}));await screen.findByText("Replacement exceeds the form boundary");expect(input).toHaveValue("Changed words");
 let resolve!:(value:any)=>void;replace.mockImplementation(()=>new Promise(done=>{resolve=done;}));fireEvent.click(screen.getByRole("button",{name:"Apply changes"}));expect(screen.getByRole("button",{name:"Cancel"})).toBeDisabled();
 cleanup();await act(async()=>resolve({id:"orphan-form",name:"Forms.pdf",pages:[{width:300,height:400}]}));await waitFor(()=>expect(nativeAdapter.closeDocument).toHaveBeenCalledWith("orphan-form"));
});
