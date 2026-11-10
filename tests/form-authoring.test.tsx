import {act,cleanup,fireEvent,render,waitFor} from "@testing-library/react";
import {afterEach,describe,it,expect,vi} from "vitest";
import {FormAuthorDialog} from "../src/components/FormAuthorDialog";
import {fieldSourceBounds,initialFieldBounds,validateFieldDraft} from "../src/editor/form-authoring";
import type {EditorDocument,PagePlan,PageContentReplacement} from "../src/editor/types";
import type {FolioAdapter} from "../src/editor/adapter";
const page:PagePlan={id:"p",sourceId:"source",pageIndex:0,width:300,height:400,rotation:90,overlays:[],crop:{x:10,y:20,width:250,height:350}};
const document={id:"d",name:"Document",pages:[page],selectedPageId:"p",selectedOverlayId:null} as EditorDocument;
const result:PageContentReplacement={document:{id:"candidate",name:"Created",pages:[{width:300,height:400,overlays:[]}]},pageIndices:[0]};
afterEach(()=>{cleanup();vi.restoreAllMocks();});
function setup(){return {adapter:{renderPagePlan:vi.fn().mockResolvedValue("data:image/png;base64,AA"),closeDocument:vi.fn().mockResolvedValue(undefined),createFormField:vi.fn().mockResolvedValue(result)} as unknown as FolioAdapter,document,page,sourceIds:["source","other-tab"],onApply:vi.fn().mockReturnValue(true),onCancel:vi.fn()};}
describe("form creation",()=>{
 it("converts current-view rectangles back into source coordinates and respects crop",()=>{
  expect(fieldSourceBounds(page,{x:40,y:50,width:120,height:30})).toEqual({x:50,y:240,width:30,height:120});
  const area=initialFieldBounds(page);expect(validateFieldDraft(page,{name:"answer",label:"",required:false,fontSize:12,rotation:90,bounds:fieldSourceBounds(page,area),field:{kind:"text",multiline:false}})).toBeNull();
 });
 it("renders the candidate before Apply, invalidates it on change, and transfers ownership once",async()=>{
  const props=setup(),v=render(<FormAuthorDialog {...props}/>);
  expect(v.getByRole("button",{name:"Apply field"})).toBeDisabled();
  fireEvent.change(v.getByLabelText("Field name"),{target:{value:"answer"}});
  fireEvent.click(v.getByRole("button",{name:"Preview field"}));
  await waitFor(()=>expect(v.getByRole("button",{name:"Apply field"})).toBeEnabled());
  expect(props.adapter.createFormField).toHaveBeenCalledWith("source",[0],0,expect.objectContaining({name:"answer",rotation:90}));
  fireEvent.change(v.getByLabelText("Accessible label"),{target:{value:"Answer"}});
  expect(v.getByRole("button",{name:"Apply field"})).toBeDisabled();
  await waitFor(()=>expect(props.adapter.closeDocument).toHaveBeenCalledWith("candidate"));
  fireEvent.click(v.getByRole("button",{name:"Preview field"}));
  await waitFor(()=>expect(v.getByRole("button",{name:"Apply field"})).toBeEnabled());
  fireEvent.click(v.getByRole("button",{name:"Apply field"}));fireEvent.click(v.getByRole("button",{name:"Apply field"}));
  expect(props.onApply).toHaveBeenCalledOnce();v.unmount();
  expect(props.adapter.closeDocument).toHaveBeenCalledTimes(1);
 });
 it("disposes late completion after cancel",async()=>{
  const props=setup();let done!:(v:PageContentReplacement)=>void;
  vi.mocked(props.adapter.createFormField!).mockImplementation(()=>new Promise(resolve=>{done=resolve;}));
  const v=render(<FormAuthorDialog {...props}/>);fireEvent.click(v.getByRole("button",{name:"Preview field"}));
  fireEvent.click(v.getByRole("button",{name:"Cancel"}));v.unmount();
  await act(async()=>done(result));expect(props.adapter.closeDocument).toHaveBeenCalledWith("candidate");expect(props.onApply).not.toHaveBeenCalled();
 });
 it("never adopts or closes a source owned by another tab",async()=>{
  const props=setup();vi.mocked(props.adapter.createFormField!).mockResolvedValue({...result,document:{...result.document,id:"other-tab"}});
  const v=render(<FormAuthorDialog {...props}/>);fireEvent.click(v.getByRole("button",{name:"Preview field"}));
  expect(await v.findByRole("alert")).toHaveTextContent("new source");v.unmount();expect(props.adapter.closeDocument).not.toHaveBeenCalled();
 });
 it("draft changes during preparation discard the stale result",async()=>{
  const props=setup();let done!:(v:PageContentReplacement)=>void;
  vi.mocked(props.adapter.createFormField!).mockImplementation(()=>new Promise(resolve=>{done=resolve;}));
  const v=render(<FormAuthorDialog {...props}/>);fireEvent.click(v.getByRole("button",{name:"Preview field"}));
  fireEvent.change(v.getByLabelText("Field name"),{target:{value:"New draft"}});await act(async()=>done(result));
  expect(props.adapter.closeDocument).toHaveBeenCalledWith("candidate");expect(v.getByRole("button",{name:"Apply field"})).toBeDisabled();expect(props.onApply).not.toHaveBeenCalled();
 });
 it("a failed native raster cannot authorize Apply",async()=>{
  const props=setup();vi.mocked(props.adapter.renderPagePlan!).mockImplementation(async p=>{if(p.sourceId==="candidate")throw new Error("Native render failed");return "data:image/png;base64,AA";});
  const v=render(<FormAuthorDialog {...props}/>);fireEvent.click(v.getByRole("button",{name:"Preview field"}));
  expect(await v.findByRole("alert")).toHaveTextContent("Native render failed");expect(v.getByRole("button",{name:"Apply field"})).toBeDisabled();v.unmount();await waitFor(()=>expect(props.adapter.closeDocument).toHaveBeenCalledWith("candidate"));
 });
 it("supports numeric and keyboard placement while preventing hidden or invalid fields",()=>{
  const props=setup(),v=render(<FormAuthorDialog {...props}/>);
  const left=v.getByLabelText("Left (pt)") as HTMLInputElement;const before=Number(left.value);
  fireEvent.keyDown(v.getByRole("button",{name:"Move field"}),{key:"ArrowRight"});expect(Number(left.value)).toBe(before+1);
  fireEvent.change(left,{target:{value:""}});expect(v.getByRole("button",{name:"Preview field"})).toBeDisabled();
  fireEvent.change(left,{target:{value:"999"}});expect(v.getByRole("button",{name:"Preview field"})).toBeDisabled();
  fireEvent.change(left,{target:{value:String(before)}});
  fireEvent.change(v.getByLabelText("Width (pt)"),{target:{value:"-10"}});expect(v.getByRole("button",{name:"Preview field"})).toBeDisabled();
 });
});
