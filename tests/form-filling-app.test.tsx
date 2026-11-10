import {act,cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {App} from "../src/App";
import {nativeAdapter} from "../src/editor/adapter";
import type {DocumentInfo,FormField} from "../src/editor/types";
vi.mock("../src/components/DocumentViewport",()=>({DocumentViewport:({pages}:{pages:{sourceId:string}[]})=><div data-testid="sources">{pages.map(p=>p.sourceId).join(",")}</div>}));
const field:FormField={annotationIndex:2,name:"person.name",label:"Full name",kind:"text",value:{kind:"text",text:"Original"},options:[],selectedIndices:[],readOnly:false,required:true,multiline:false,password:false,multiSelect:false,editableChoice:false,reason:null};
const info:DocumentInfo={id:"source",name:"Form.pdf",pages:[{width:300,height:400},{width:300,height:400}]};
beforeEach(()=>{
 localStorage.clear();vi.spyOn(nativeAdapter,"openPdf").mockResolvedValue(info);
 vi.spyOn(nativeAdapter,"renderPage").mockResolvedValue("data:image/png;base64,");vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,"renderPagePlan").mockResolvedValue("data:image/png;base64,");
 vi.spyOn(nativeAdapter,"exportPdf").mockResolvedValue("saved.pdf");vi.spyOn(nativeAdapter,"closeDocument").mockResolvedValue();
 vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,"listFormFields").mockImplementation(async(id)=>[{...field,value:{kind:"text",text:id==="filled"?"New name":"Original"}}]);
 vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,"fillFormField").mockResolvedValue({...info,id:"filled"});
});
afterEach(()=>{cleanup();vi.restoreAllMocks();});
async function open(){const view=render(<App initialDemo={false}/>);fireEvent.click(screen.getByRole("button",{name:"Open a PDF"}));await screen.findByRole("tab",{name:"Form.pdf"});fireEvent.click(screen.getByRole("button",{name:"Fill forms"}));await screen.findByRole("textbox",{name:/Full name/});return view;}
it("fills the source as one undoable operation, refreshes the fields, and saves all shared pages",async()=>{
 await open();fireEvent.change(screen.getByRole("textbox",{name:/Full name/}),{target:{value:"New name"}});
 expect(screen.getByRole("button",{name:"Save a copy"})).toBeDisabled();
 fireEvent.click(screen.getByRole("button",{name:"Apply field"}));
 await waitFor(()=>expect(screen.getByTestId("sources")).toHaveTextContent("filled,filled"));
 expect(nativeAdapter.fillFormField).toHaveBeenCalledWith("source",{pageIndex:0,annotationIndex:2,expectedValue:{kind:"text",text:"Original"},value:{kind:"text",text:"New name"}});
 await waitFor(()=>expect(screen.getByRole("textbox",{name:/Full name/})).toHaveValue("New name"));
 fireEvent.click(screen.getByRole("button",{name:"Done"}));fireEvent.click(screen.getByRole("button",{name:"Undo"}));expect(screen.getByTestId("sources")).toHaveTextContent("source,source");
 fireEvent.click(screen.getByRole("button",{name:"Redo"}));fireEvent.click(screen.getByRole("button",{name:"Save a copy"}));await waitFor(()=>expect(nativeAdapter.exportPdf).toHaveBeenCalled());
 expect(vi.mocked(nativeAdapter.exportPdf).mock.calls[0][0].map(p=>p.sourceId)).toEqual(["filled","filled"]);
});
it("releases a late candidate when the app unmounts",async()=>{
 let finish!:(info:DocumentInfo)=>void;vi.mocked(nativeAdapter.fillFormField!).mockReturnValue(new Promise(resolve=>{finish=resolve;}));
 const view=await open();fireEvent.change(screen.getByRole("textbox",{name:/Full name/}),{target:{value:"New name"}});fireEvent.click(screen.getByRole("button",{name:"Apply field"}));
 view.unmount();await act(async()=>{finish({...info,id:"late"});});await waitFor(()=>expect(nativeAdapter.closeDocument).toHaveBeenCalledWith("late"));
});
it("shows native failures without changing the document or discarding the draft",async()=>{
 vi.mocked(nativeAdapter.fillFormField!).mockRejectedValue(new Error("Field length exceeded"));await open();
 fireEvent.change(screen.getByRole("textbox",{name:/Full name/}),{target:{value:"Long name"}});fireEvent.click(screen.getByRole("button",{name:"Apply field"}));
 expect(await screen.findByRole("alert")).toHaveTextContent("Field length exceeded");expect(screen.getByRole("textbox",{name:/Full name/})).toHaveValue("Long name");expect(screen.getByTestId("sources")).toHaveTextContent("source,source");
});
