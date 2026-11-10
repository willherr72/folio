import {act,cleanup,fireEvent,render,screen,waitFor,within} from "@testing-library/react";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {App} from "../src/App";
import {nativeAdapter} from "../src/editor/adapter";
import {clearRenderCache} from "../src/components/PageView";
import type {DocumentInfo,FormField} from "../src/editor/types";

const field:FormField={annotationIndex:2,name:"person.name",label:"Full name",kind:"text",value:{kind:"text",text:"Original"},options:[],selectedIndices:[],readOnly:false,required:false,multiline:false,password:false,multiSelect:false,editableChoice:false,reason:null,visible:true,bounds:{x:30,y:50,width:180,height:25}};
const consent:FormField={...field,annotationIndex:4,name:"consent",label:"Consent",kind:"checkbox",value:{kind:"toggle",checked:false},bounds:{x:30,y:100,width:20,height:20}};
const info:DocumentInfo={id:"canvas-form",name:"Canvas form.pdf",pages:[{width:300,height:400},{width:300,height:400}]};
beforeEach(()=>{
 localStorage.clear();vi.spyOn(nativeAdapter,"openPdf").mockResolvedValue(info);
 vi.spyOn(nativeAdapter,"renderPage").mockResolvedValue("data:image/png;base64,");vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,"renderPagePlan").mockResolvedValue("data:image/png;base64,");
 vi.spyOn(nativeAdapter,"exportPdf").mockResolvedValue("saved.pdf");vi.spyOn(nativeAdapter,"closeDocument").mockResolvedValue();
 vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,"listFormFields").mockImplementation(async(id,page)=>[{...field,label:page===0?"Full name":"Second name",value:{kind:"text",text:id==="canvas-filled"?"New name":"Original"}},consent]);
 vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,"fillFormField").mockResolvedValue({...info,id:"canvas-filled"});
});
afterEach(()=>{cleanup();clearRenderCache(["canvas-form","canvas-filled"]);vi.restoreAllMocks();});
async function open(){render(<App initialDemo={false}/>);fireEvent.click(screen.getByRole("button",{name:"Open a PDF"}));await screen.findByRole("tab",{name:info.name});fireEvent.click(screen.getByRole("button",{name:"Fill forms"}));await screen.findByRole("textbox",{name:"Full name"});}

it("fills on the document canvas with a persistent properties editor instead of a default modal",async()=>{
 await open();expect(screen.queryByRole("dialog",{name:"Fill forms"})).not.toBeInTheDocument();
 const properties=screen.getByRole("complementary",{name:"Properties"});expect(within(properties).getByRole("textbox",{name:"Full name"})).toHaveValue("Original");
 const document=screen.getByRole("main",{name:"Document"});const target=await within(within(document).getByLabelText("Page 1 container")).findByRole("button",{name:"Select form field: Consent"});
 fireEvent.keyDown(target,{key:"Enter"});expect(within(properties).getByRole("checkbox",{name:"Consent"})).toHaveFocus();
 expect(screen.getByRole("button",{name:"Zoom in"})).toBeEnabled();expect(screen.getByRole("button",{name:"Next page"})).toBeEnabled();
});

it("retains a dirty field while navigating and zooming and blocks save or replacement selections",async()=>{
 await open();const input=screen.getByRole("textbox",{name:"Full name"});fireEvent.change(input,{target:{value:"Keep this draft"}});
 fireEvent.click(screen.getByRole("button",{name:"Zoom in"}));fireEvent.click(screen.getByRole("button",{name:"Next page"}));
 expect(screen.getByRole("button",{name:"Go to page"})).toHaveTextContent("Page 2 of 2");expect(input).toHaveValue("Keep this draft");
 expect(screen.getByRole("button",{name:"Save a copy"})).toBeDisabled();expect(screen.getByRole("button",{name:"Select"})).toBeDisabled();
 const second=await within(screen.getByRole("main",{name:"Document"})).findByRole("button",{name:"Select form field: Second name"});
 expect(second).toHaveAttribute("aria-disabled","true");fireEvent.click(second);expect(input).toHaveValue("Keep this draft");
 fireEvent.click(screen.getByRole("button",{name:"Reset draft"}));fireEvent.click(second);
 expect(await screen.findByRole("textbox",{name:"Second name"})).toHaveValue("Original");
});

it("applies the selected source page and refreshes shared occurrences before normal undo and save",async()=>{
 await open();fireEvent.click(screen.getByRole("button",{name:"Next page"}));
 const target=await within(screen.getByRole("main",{name:"Document"})).findByRole("button",{name:"Select form field: Second name"});fireEvent.click(target);
 const input=await screen.findByRole("textbox",{name:"Second name"});fireEvent.change(input,{target:{value:"New name"}});fireEvent.click(screen.getByRole("button",{name:"Apply field"}));
 await waitFor(()=>expect(nativeAdapter.fillFormField).toHaveBeenCalledWith("canvas-form",{pageIndex:1,annotationIndex:2,expectedValue:field.value,value:{kind:"text",text:"New name"}}));
 await waitFor(()=>expect(screen.getByRole("textbox",{name:"Second name"})).toHaveValue("New name"));
 fireEvent.click(screen.getByRole("button",{name:"Done"}));fireEvent.click(screen.getByRole("button",{name:"Undo"}));fireEvent.click(screen.getByRole("button",{name:"Redo"}));fireEvent.click(screen.getByRole("button",{name:"Save a copy"}));
 await waitFor(()=>expect(nativeAdapter.exportPdf).toHaveBeenCalled());expect(vi.mocked(nativeAdapter.exportPdf).mock.calls[0][0].map(p=>p.sourceId)).toEqual(["canvas-filled","canvas-filled"]);
});

it("rejects a form candidate already owned by another document without closing it",async()=>{
 vi.mocked(nativeAdapter.openPdf).mockResolvedValueOnce({...info,id:"foreign",name:"Other.pdf"}).mockResolvedValueOnce(info);
 vi.mocked(nativeAdapter.fillFormField!).mockResolvedValue({...info,id:"foreign"});
 render(<App initialDemo={false}/>);fireEvent.click(screen.getByRole("button",{name:"Open a PDF"}));await screen.findByRole("tab",{name:"Other.pdf"});fireEvent.click(screen.getByRole("button",{name:"Open"}));await screen.findByRole("tab",{name:info.name});
 fireEvent.click(screen.getByRole("button",{name:"Fill forms"}));fireEvent.change(await screen.findByRole("textbox",{name:"Full name"}),{target:{value:"New name"}});fireEvent.click(screen.getByRole("button",{name:"Apply field"}));
 expect(await screen.findByRole("alert")).toHaveTextContent("new source version");expect(screen.getByRole("textbox",{name:"Full name"})).toHaveValue("New name");
 expect(nativeAdapter.closeDocument).not.toHaveBeenCalledWith("foreign");
});

it("retains same-field drafts and preserves the selected field through the optional overview",async()=>{
 await open();const document=screen.getByRole("main",{name:"Document"});const owner=within(document).getByLabelText("Page 1 container");
 const target=await within(owner).findByRole("button",{name:"Select form field: Full name"});const input=screen.getByRole("textbox",{name:"Full name"});
 fireEvent.change(input,{target:{value:"Keep me"}});fireEvent.keyDown(target,{key:" "});expect(input).toHaveFocus();expect(input).toHaveValue("Keep me");
 expect(screen.getByRole("button",{name:"Full-page form overview"})).toBeDisabled();fireEvent.click(screen.getByRole("button",{name:"Reset draft"}));
 fireEvent.change(screen.getByRole("combobox",{name:"Field"}),{target:{value:"1"}});expect(screen.getByRole("checkbox",{name:"Consent"})).toBeInTheDocument();
 fireEvent.click(screen.getByRole("button",{name:"Full-page form overview"}));const overview=await screen.findByRole("dialog",{name:"Fill forms"});expect(await within(overview).findByRole("checkbox",{name:"Consent"})).toBeInTheDocument();
 fireEvent.click(within(overview).getByRole("button",{name:"Done"}));expect(await screen.findByRole("checkbox",{name:"Consent"})).toBeInTheDocument();expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
 fireEvent.click(screen.getByRole("button",{name:"Done"}));expect(screen.getByRole("button",{name:"Fill forms"})).toHaveFocus();
});

it("keeps a pending apply owned and retains the draft after native failure",async()=>{
 let reject!:(error:Error)=>void;
 vi.mocked(nativeAdapter.fillFormField!).mockImplementation(()=>new Promise((_resolve,fail)=>{reject=fail;}));
 await open();fireEvent.change(screen.getByRole("textbox",{name:"Full name"}),{target:{value:"Retry this"}});
 fireEvent.click(screen.getByRole("button",{name:"Apply field"}));
 expect(screen.getByRole("button",{name:"Discard draft and close"})).toBeDisabled();
 expect(screen.getByRole("button",{name:"Zoom in"})).toBeDisabled();
 expect(screen.getByRole("button",{name:"Next page"})).toBeDisabled();
 expect(screen.getByRole("button",{name:"Reset draft"})).toBeDisabled();
 const target=within(screen.getByLabelText("Page 1 container")).getByRole("button",{name:"Select form field: Consent"});
 expect(target).toHaveAttribute("aria-disabled","true");fireEvent.keyDown(target,{key:"Enter"});
 reject(new Error("Native field update failed"));
 expect(await screen.findByRole("alert")).toHaveTextContent("Native field update failed");
 expect(screen.getByRole("textbox",{name:"Full name"})).toHaveValue("Retry this");
 expect(screen.getByRole("button",{name:"Apply field"})).toBeEnabled();
 expect(screen.getByRole("button",{name:"Zoom in"})).toBeEnabled();
});

it.each([4,9])("selects another page's non-first field %s without selection feedback",async annotationIndex=>{
 vi.mocked(nativeAdapter.listFormFields!).mockImplementation(async(id,page)=>[
  {...field,label:page===0?"Full name":"Second name"},
  {...consent,annotationIndex:page===1?annotationIndex:4,value:{kind:"toggle",checked:id==="canvas-filled"}}
 ]);
 await open();fireEvent.click(screen.getByRole("button",{name:"Next page"}));
 fireEvent.click(await within(screen.getByLabelText("Page 2 container")).findByRole("button",{name:"Select form field: Consent"}));
 const checkbox=await screen.findByRole("checkbox",{name:"Consent"});expect(checkbox).toHaveFocus();
 fireEvent.click(checkbox);fireEvent.click(screen.getByRole("button",{name:"Apply field"}));
 await waitFor(()=>expect(nativeAdapter.fillFormField).toHaveBeenCalledWith("canvas-form",{pageIndex:1,annotationIndex,expectedValue:{kind:"toggle",checked:false},value:{kind:"toggle",checked:true}}));
 await waitFor(()=>expect(screen.getByRole("checkbox",{name:"Consent"})).toBeChecked());
});

it("reopens Properties on same-field selection without resetting the draft",async()=>{
 await open();const input=screen.getByRole("textbox",{name:"Full name"});fireEvent.change(input,{target:{value:"Keep collapsed draft"}});
 const toggle=screen.getByRole("button",{name:"Properties"});fireEvent.click(toggle);expect(toggle).toHaveAttribute("aria-expanded","false");
 fireEvent.keyDown(within(screen.getByLabelText("Page 1 container")).getByRole("button",{name:"Select form field: Full name"}),{key:"Enter"});
 expect(toggle).toHaveAttribute("aria-expanded","true");expect(input).toHaveValue("Keep collapsed draft");expect(input).toHaveFocus();
});

it("honors a newer canvas selection while that page's editor fields are loading",async()=>{
 let pageLoads=0,finish!:(fields:FormField[])=>void;
 const second=[{...field,label:"Second name"},{...consent,annotationIndex:9}];
 vi.mocked(nativeAdapter.listFormFields!).mockImplementation(async(_id,page)=>{
  if(page===1&&++pageLoads===2)return new Promise(resolve=>{finish=resolve;});
  return page===1?second:[field,consent];
 });
 await open();fireEvent.click(screen.getByRole("button",{name:"Next page"}));
 const pageTwo=within(screen.getByLabelText("Page 2 container"));
 fireEvent.click(await pageTwo.findByRole("button",{name:"Select form field: Second name"}));
 await waitFor(()=>expect(pageLoads).toBe(2));
 fireEvent.click(pageTwo.getByRole("button",{name:"Select form field: Consent"}));
 await act(async()=>finish(second));
 expect(screen.getByRole("checkbox",{name:"Consent"})).toHaveFocus();expect(screen.queryByRole("textbox",{name:"Second name"})).not.toBeInTheDocument();
});
