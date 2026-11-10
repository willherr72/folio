import {act,cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {afterEach,expect,it,vi} from "vitest";
import {FormFillDialog} from "../src/components/FormFillDialog";
import type {FolioAdapter} from "../src/editor/adapter";
import type {FormField,PagePlan} from "../src/editor/types";
afterEach(()=>{cleanup();vi.restoreAllMocks();vi.unstubAllGlobals();});
const page:PagePlan={id:"p",sourceId:"s",pageIndex:0,width:300,height:400,rotation:0,overlays:[]};
const first={annotationIndex:0,name:"name",label:"Full name",kind:"text",value:{kind:"text" as const,text:"Original"},options:[],selectedIndices:[],required:false,readOnly:false,multiline:false,password:false,multiSelect:false,editableChoice:false,reason:null,visible:true,noRotate:false,bounds:{x:40,y:80,width:180,height:30}};
const second={...first,annotationIndex:1,name:"consent",label:"Consent",kind:"checkbox",value:{kind:"toggle" as const,checked:false},bounds:{x:40,y:160,width:18,height:18}};
function setup(fields:FormField[]=[first,second],overrides:Partial<FolioAdapter>={},current=page){
 const adapter:FolioAdapter={kind:"native",openPdf:vi.fn(),exportPdf:vi.fn(),engineStatus:vi.fn(),closeDocument:vi.fn(),renderPage:vi.fn().mockResolvedValue("data:image/png;base64,AA"),renderPagePlan:vi.fn().mockResolvedValue("data:image/png;base64,AA"),listFormFields:vi.fn().mockResolvedValue(fields),...overrides};
 const onApply=vi.fn().mockResolvedValue(undefined),onClose=vi.fn();const props={page:current,adapter,pageNumber:1,onApply,onClose};
 return {adapter,props,onApply,onClose,...render(<FormFillDialog {...props}/>)};
}
const target=(label:string)=>screen.getByRole("button",{name:`Select form field: ${label}`});
it("selects page fields with pointer and keyboard and focuses their value controls",async()=>{
 const {onApply}=setup();await screen.findByRole("button",{name:"Select form field: Consent"});
 fireEvent.click(target("Consent"));expect(screen.getByRole("checkbox",{name:"Consent"})).toHaveFocus();expect(target("Consent")).toHaveAttribute("aria-pressed","true");
 fireEvent.click(screen.getByRole("checkbox",{name:"Consent"}));fireEvent.click(screen.getByRole("button",{name:"Apply field"}));
 await waitFor(()=>expect(onApply).toHaveBeenCalledWith(expect.objectContaining({annotationIndex:1}),{kind:"toggle",checked:true}));
 fireEvent.click(screen.getByRole("button",{name:"Reset draft"}));fireEvent.keyDown(target("Full name"),{key:"Enter"});
 expect(screen.getByRole("textbox",{name:"Full name"})).toHaveFocus();
});
it("preserves a dirty same-field draft and prevents selecting another field until reset",async()=>{
 setup();const input=await screen.findByRole("textbox",{name:"Full name"});await screen.findByRole("button",{name:"Select form field: Consent"});
 fireEvent.change(input,{target:{value:"Unsaved name"}});expect(target("Consent")).toHaveAttribute("aria-disabled","true");expect(target("Consent")).toHaveAttribute("tabindex","-1");
 fireEvent.click(target("Consent"));fireEvent.keyDown(target("Consent"),{key:" "});expect(input).toHaveValue("Unsaved name");
 fireEvent.click(target("Full name"));expect(input).toHaveValue("Unsaved name");expect(input).toHaveFocus();
 fireEvent.click(screen.getByRole("button",{name:"Reset draft"}));fireEvent.keyDown(target("Consent"),{key:" "});expect(screen.getByRole("checkbox",{name:"Consent"})).toHaveFocus();
});
it("keeps visible readonly fields inspectable but excludes hidden, invalid and outside targets",async()=>{
 const readonly={...first,annotationIndex:2,label:"Read only",readOnly:true,reason:"This field is read-only."};
 setup([first,readonly,{...first,annotationIndex:3,label:"Hidden",visible:false},{...first,annotationIndex:4,label:"Invalid",bounds:{x:0,y:0,width:0,height:20}},{...first,annotationIndex:5,label:"Outside",bounds:{x:400,y:80,width:20,height:20}},{...first,annotationIndex:6,label:"Partial",bounds:{x:-10,y:80,width:30,height:20}}]);
 await screen.findByRole("button",{name:"Select form field: Read only"});expect(screen.queryByRole("button",{name:/Select form field: (Hidden|Invalid|Outside)/})).not.toBeInTheDocument();
 expect(target("Partial")).toHaveAttribute("x","0");expect(target("Partial")).toHaveAttribute("width","20");
 fireEvent.click(target("Read only"));expect(screen.getByRole("textbox",{name:"Read only"})).toBeDisabled();expect(screen.getByText("This field is read-only.")).toBeInTheDocument();
 expect(screen.getByRole("combobox",{name:"Field"})).toBeEnabled();
});
it.each([
 [0,40,80,180,30],[90,290,40,30,180],[180,80,290,180,30],[270,80,80,30,180],
])("maps source bounds through %i degree workspace rotation",async(rotation,x,y,width,height)=>{
 setup([first],{}, {...page,rotation:rotation as PagePlan["rotation"]});const box=await screen.findByRole("button",{name:"Select form field: Full name"});
 for(const [key,value] of Object.entries({x,y,width,height}))expect(box).toHaveAttribute(key,String(value));
});
it("keeps NoRotate bounds upright about the rotated anchor",async()=>{
 setup([{...first,noRotate:true,bounds:{x:40,y:80,width:50,height:20}}],{}, {...page,rotation:90});const box=await screen.findByRole("button",{name:"Select form field: Full name"});
 for(const [key,value] of Object.entries({x:320,y:40,width:50,height:20}))expect(box).toHaveAttribute(key,String(value));
});
it("hides obsolete targets and ignores late fields/raster after source and adapter replacement",async()=>{
 let oldFields!:(fields:FormField[])=>void,oldRaster!:(url:string)=>void;
 const revoke=vi.fn();vi.stubGlobal("URL",class extends URL {static revokeObjectURL=revoke;});
 const {props,rerender}=setup([first],{listFormFields:vi.fn().mockImplementation(()=>new Promise(resolve=>oldFields=resolve)),renderPagePlan:vi.fn().mockImplementation(()=>new Promise(resolve=>oldRaster=resolve))});
 const replacement={...second,label:"Current consent"},adapter={...props.adapter,listFormFields:vi.fn().mockResolvedValue([replacement]),renderPagePlan:vi.fn().mockResolvedValue("data:image/png;base64,NEW")};
 rerender(<FormFillDialog {...props} page={{...page,sourceId:"new"}} adapter={adapter}/>);
 await screen.findByRole("button",{name:"Select form field: Current consent"});await act(async()=>{oldFields([first]);oldRaster("blob:obsolete");});
 expect(screen.queryByRole("button",{name:"Select form field: Full name"})).not.toBeInTheDocument();expect(screen.getByAltText("Form page preview")).toHaveAttribute("src","data:image/png;base64,NEW");expect(revoke).toHaveBeenCalledWith("blob:obsolete");
});
it("disables every page target during native apply",async()=>{
 let done!:()=>void;const {props,rerender}=setup();rerender(<FormFillDialog {...props} onApply={()=>new Promise<void>(resolve=>done=resolve)}/>);
 const input=await screen.findByRole("textbox",{name:"Full name"});await screen.findByRole("button",{name:"Select form field: Consent"});
 fireEvent.change(input,{target:{value:"New"}});fireEvent.click(screen.getByRole("button",{name:"Apply field"}));
 expect(target("Full name")).toHaveAttribute("aria-disabled","true");expect(target("Consent")).toHaveAttribute("aria-disabled","true");
 fireEvent.click(target("Consent"));expect(input).toHaveValue("New");await act(async()=>done());
});
