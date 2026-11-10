import {act,cleanup,render,screen} from "@testing-library/react";
import {afterEach,expect,it,vi} from "vitest";
import {FormPageTargets} from "../src/components/FormPageTargets";
import {CanvasFormLayer} from "../src/components/CanvasFormLayer";
import {nativeAdapter} from "../src/editor/adapter";
import {toolAllowed} from "../src/editor/protection";
import type {FormField,PagePlan} from "../src/editor/types";
afterEach(cleanup);
const page:PagePlan={id:"p",sourceId:"s",pageIndex:0,width:300,height:400,rotation:0,overlays:[],crop:{x:50,y:50,width:100,height:100}};
const field:FormField={annotationIndex:0,name:"inside",label:"Inside",kind:"text",value:{kind:"text",text:""},options:[],selectedIndices:[],readOnly:false,required:false,multiline:false,password:false,multiSelect:false,editableChoice:false,reason:null,visible:true,bounds:{x:40,y:60,width:40,height:20}};
it("clips canvas form targets to the crop and removes off-crop keyboard targets",()=>{
 render(<svg><FormPageTargets page={page} fields={[field,{...field,annotationIndex:1,label:"Outside",bounds:{x:180,y:60,width:40,height:20}}]} selected={0} disabled={false} dirty={false} onSelect={vi.fn()} embedded clipToCrop/></svg>);
 const inside=screen.getByRole("button",{name:"Select form field: Inside"});expect(inside).toHaveAttribute("x","50");expect(inside).toHaveAttribute("width","30");expect(screen.queryByRole("button",{name:"Select form field: Outside"})).not.toBeInTheDocument();
});

it.each([0,90,180,270] as const)("keeps cropped targets within the visible area at rotation %i",rotation=>{
 const current={...page,rotation};render(<svg><FormPageTargets page={current} fields={[field]} selected={0} disabled={false} dirty={false} onSelect={vi.fn()} embedded clipToCrop/></svg>);
 const box=screen.getByRole("button",{name:"Select form field: Inside"});
 const expected=rotation===0?[50,60,30,20]:rotation===90?[320,50,20,30]:rotation===180?[220,320,30,20]:[60,220,20,30];
 for(const [i,key] of ["x","y","width","height"].entries())expect(box).toHaveAttribute(key,String(expected[i]));
});

it("retains NoRotate geometry while clipping an upright target",()=>{
 render(<svg><FormPageTargets page={{...page,rotation:90}} fields={[{...field,noRotate:true}]} selected={0} disabled={false} dirty={false} onSelect={vi.fn()} embedded clipToCrop/></svg>);
 const box=screen.getByRole("button",{name:"Select form field: Inside"});
 for(const [key,value] of Object.entries({x:340,y:50,width:10,height:10}))expect(box).toHaveAttribute(key,String(value));
});

it("ignores obsolete asynchronous fields after the source changes",async()=>{
 let finish!:(fields:FormField[])=>void;const adapter={...nativeAdapter,listFormFields:vi.fn().mockImplementationOnce(()=>new Promise<FormField[]>(resolve=>{finish=resolve;})).mockResolvedValue([{...field,label:"Current"}])};
 const props={adapter,page,disabled:false,dirty:false,onSelect:vi.fn()};const view=render(<svg><CanvasFormLayer {...props}/></svg>);
 view.rerender(<svg><CanvasFormLayer {...props} page={{...page,sourceId:"new"}}/></svg>);
 await screen.findByRole("button",{name:"Select form field: Current"});await act(async()=>finish([field]));
 expect(screen.queryByRole("button",{name:"Select form field: Inside"})).not.toBeInTheDocument();
});

it("requires fill permission independently from annotation and existing-text rights",async()=>{
 const protection={documentId:"protected",revision:6,owner:false,permissions:{fillForms:true,annotate:false,editContent:false,copyContent:false,assemble:false,print:false,printHighQuality:false}};
 const allowed={...page,protection};expect(toolAllowed(allowed,"form")).toBe(true);expect(toolAllowed(allowed,"edit")).toBe(false);
 const adapter={...nativeAdapter,listFormFields:vi.fn().mockResolvedValue([field])};const props={adapter,page:allowed,disabled:false,dirty:false,onSelect:vi.fn()};
 const view=render(<svg><CanvasFormLayer {...props}/></svg>);await screen.findByRole("button",{name:"Select form field: Inside"});
 view.rerender(<svg><CanvasFormLayer {...props} page={{...allowed,protection:{...protection,permissions:{...protection.permissions,fillForms:false}}}}/></svg>);
 expect(screen.queryByRole("button",{name:"Select form field: Inside"})).not.toBeInTheDocument();expect(adapter.listFormFields).toHaveBeenCalledTimes(1);
});
