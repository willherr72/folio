import {act,cleanup,fireEvent,render,waitFor} from "@testing-library/react";
import {afterEach,expect,it,vi} from "vitest";
import {ExistingImageLayer} from "../src/components/ExistingImageLayer";
import {createDemoAdapter} from "../src/editor/adapter";
import type {ExistingImage,PagePlan} from "../src/editor/types";
const image:ExistingImage={objectPath:[2],selectionId:"a".repeat(64),corners:[{x:0,y:0},{x:100,y:0},{x:100,y:100},{x:0,y:100}],bounds:{x:0,y:0,width:100,height:100},pixelWidth:20,pixelHeight:10};
const page:PagePlan={id:"p",sourceId:"source",pageIndex:0,width:400,height:300,rotation:90,crop:{x:20,y:20,width:60,height:60},overlays:[]};
afterEach(cleanup);
it("shows a clipped keyboard target and ignores stale source responses",async()=>{
 const onEditImage=vi.fn();const adapter={...createDemoAdapter(),listExistingImages:vi.fn(async()=>({images:[image]}))};const v=render(<svg><ExistingImageLayer adapter={adapter} page={page} onEditImage={onEditImage}/></svg>);
 const target=await v.findByRole("button",{name:/Edit image 1/});expect(target.tagName).toBe("polygon");expect(target.getAttribute("points")).toContain("20,20");fireEvent.keyDown(target,{key:"Enter"});expect(onEditImage).toHaveBeenCalledWith(image);
 v.rerender(<svg><ExistingImageLayer adapter={adapter} page={{...page,sourceId:"new"}} disabled onEditImage={onEditImage}/></svg>);await waitFor(()=>expect(adapter.listExistingImages).toHaveBeenCalledWith("new",0));expect(v.getByRole("button")).toHaveAttribute("tabindex","-1");
});
it("does not create focus targets for completely cropped-out images",async()=>{
 const adapter={...createDemoAdapter(),listExistingImages:vi.fn(async()=>({images:[image]}))};const v=render(<svg><ExistingImageLayer adapter={adapter} page={{...page,crop:{x:200,y:200,width:50,height:50}}}/></svg>);await v.findByText(/No editable images/);expect(v.queryByRole("button")).toBeNull();
});
it("does not replace new discovery with a deferred old-source response",async()=>{
 let old!:(value:{images:ExistingImage[]})=>void;const next={...image,selectionId:"b".repeat(64),pixelWidth:80};
 const adapter={...createDemoAdapter(),listExistingImages:vi.fn((source:string)=>source==="source"?new Promise<{images:ExistingImage[]}>(resolve=>{old=resolve;}):Promise.resolve({images:[next]}))};
 const v=render(<svg><ExistingImageLayer adapter={adapter} page={page}/></svg>);v.rerender(<svg><ExistingImageLayer adapter={adapter} page={{...page,sourceId:"next"}}/></svg>);await v.findByRole("button",{name:/80 by 10/});await act(async()=>old({images:[image]}));expect(v.getByRole("button")).toHaveAccessibleName(/80 by 10/);
});
