import {cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {afterEach,expect,it,vi} from "vitest";
import {FormFillDialog} from "../src/components/FormFillDialog";
import type {FormField,PagePlan} from "../src/editor/types";
import {nativeAdapter} from "../src/editor/adapter";
afterEach(cleanup);
const page:PagePlan={id:"p",sourceId:"s",pageIndex:0,width:300,height:400,rotation:0,overlays:[]};
const base:FormField={annotationIndex:0,name:"field",label:"Field label",kind:"text",value:{kind:"text",text:""},options:[],selectedIndices:[],required:false,readOnly:false,multiline:false,password:false,multiSelect:false,editableChoice:false,reason:null};
function setup(fields:FormField[]){const apply=vi.fn().mockResolvedValue(undefined),close=vi.fn();const adapter={...nativeAdapter,listFormFields:vi.fn().mockResolvedValue(fields),renderPagePlan:vi.fn().mockResolvedValue("data:image/png;base64,")};render(<FormFillDialog page={page} adapter={adapter} pageNumber={1} onApply={apply} onClose={close}/>);return{apply,close};}
it("edits a checkbox and prevents a draft from being lost through field selection",async()=>{
 const {apply}=setup([{...base,kind:"checkbox",value:{kind:"toggle",checked:false}},{...base,annotationIndex:1}]);
 const check=await screen.findByRole("checkbox",{name:"Field label"});fireEvent.click(check);expect(screen.getByRole("combobox",{name:"Field"})).toBeDisabled();
 fireEvent.click(screen.getByRole("button",{name:"Apply field"}));await waitFor(()=>expect(apply).toHaveBeenCalledWith(expect.anything(),{kind:"toggle",checked:true}));
});
it("submits option indices for multi-selection instead of confusing duplicate display labels",async()=>{
 const {apply}=setup([{...base,kind:"list",multiSelect:true,options:["Same","Same","Third"],value:{kind:"choice",indices:[]}}]);
 const list=await screen.findByRole("listbox",{name:"Field label"});const options=list.querySelectorAll("option");options[1].selected=true;options[2].selected=true;fireEvent.change(list);
 fireEvent.click(screen.getByRole("button",{name:"Apply field"}));await waitFor(()=>expect(apply).toHaveBeenCalledWith(expect.anything(),{kind:"choice",indices:[1,2]}));
});
it("explains readonly controls and retains the radio export label",async()=>{
 setup([{...base,kind:"text",readOnly:true,reason:"This field is read-only."},{...base,annotationIndex:1,kind:"radio",options:["Right"],value:{kind:"toggle",checked:false}}]);
 expect(await screen.findByRole("textbox",{name:"Field label"})).toBeDisabled();expect(screen.getByText("This field is read-only.")).toBeInTheDocument();
 fireEvent.change(screen.getByRole("combobox",{name:"Field"}),{target:{value:"1"}});expect(screen.getByRole("radio",{name:"Field label — Right"})).toBeEnabled();
});
it("retains multiline drafts and resets them without applying",async()=>{
 const {apply}=setup([{...base,multiline:true,value:{kind:"text",text:"Before"}}]);const input=await screen.findByRole("textbox",{name:"Field label"});
 expect(input.tagName).toBe("TEXTAREA");fireEvent.change(input,{target:{value:"One\nTwo"}});expect(screen.getByRole("button",{name:"Discard draft and close"})).toBeInTheDocument();
 fireEvent.click(screen.getByRole("button",{name:"Reset draft"}));expect(input).toHaveValue("Before");expect(apply).not.toHaveBeenCalled();
});
it("offers index-based editable-combo choices separately from custom text",async()=>{
 const {apply}=setup([{...base,kind:"combo",editableChoice:true,options:["Alpha","Beta","Beta"],value:{kind:"text",text:"a"}}]);
 const choice=await screen.findByRole("combobox",{name:"Field label choice"});fireEvent.change(choice,{target:{value:"2"}});
 fireEvent.click(screen.getByRole("button",{name:"Apply field"}));await waitFor(()=>expect(apply).toHaveBeenCalledWith(expect.anything(),{kind:"choice",indices:[2]}));
});
