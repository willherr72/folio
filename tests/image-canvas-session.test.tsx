import {act,cleanup,fireEvent,render,waitFor} from "@testing-library/react";
import {afterEach,expect,it,vi} from "vitest";
import {ImageReplacementDialog} from "../src/components/ImageReplacementDialog";
import {createDemoAdapter} from "../src/editor/adapter";
import {frameMeasurements,type CanvasImageView} from "../src/editor/existing-image-canvas";
import {imageApi} from "../src/editor/image-resources";
import {cropFrame,fullCrop} from "../src/editor/existing-image-crop";
import type {EditorDocument,ExistingImage,PageContentReplacement,PagePlan} from "../src/editor/types";
const page:PagePlan={id:"p",sourceId:"source",pageIndex:0,width:400,height:300,rotation:90,crop:{x:20,y:30,width:300,height:200},overlays:[]};
const document:EditorDocument={name:"Pictures.pdf",pages:[page],selectedPageId:"p",selectedOverlayId:null};
const image:ExistingImage={objectPath:[2],selectionId:"a".repeat(64),corners:[{x:50,y:150},{x:150,y:150},{x:175,y:100},{x:75,y:100}],bounds:{x:50,y:100,width:125,height:50},pixelWidth:20,pixelHeight:10};
const result=(id="candidate"):PageContentReplacement=>({document:{id,name:"Edited.pdf",pages:[{width:400,height:300}]},pageIndices:[0]});
afterEach(()=>{cleanup();vi.restoreAllMocks();});
function setup(){let canvas!:CanvasImageView;const adapter={...createDemoAdapter(),kind:"native" as const,previewImageEditOccurrence:vi.fn(async()=>result()),previewImageReplacementOccurrence:vi.fn(async()=>result()),renderPagePlan:vi.fn(async(_plan:PagePlan,_width:number)=>"data:image/png;base64,AA"),closeDocument:vi.fn(async(_id:string)=>{})};const imported=vi.spyOn(imageApi,"importImage");
 const props={adapter,page,document,image,sourceIds:["source"],isSourceRetained:(id:string)=>id==="other-tab",onApply:vi.fn(()=>true),onCancel:vi.fn(),canvasZoom:150,onLocate:vi.fn(),renderCanvas:(view:CanvasImageView)=>{canvas=view;return <div data-testid="canvas-raster">{view.raster.url??"loading"}</div>;}};
 return {props,imported,get canvas(){return canvas;}};
}
it("shares full geometry across crop and position, with independent Reset and Restore",async()=>{
 const s=setup(),v=render(<ImageReplacementDialog {...s.props}/>);
 fireEvent.change(v.getByLabelText("Image action"),{target:{value:"crop"}});
 expect(s.canvas.mode).toBe("crop");
 const crop={left:.25,top:.1,right:.9,bottom:.8};
 act(()=>s.canvas.onCropChange!(crop));
 expect(s.canvas.fullFrame).toEqual(image.corners);
 expect(s.canvas.frame).toEqual(cropFrame(image.corners,crop));
 await waitFor(()=>expect(s.props.adapter.previewImageEditOccurrence).toHaveBeenCalledWith("source",[0],0,[2],image.selectionId,{kind:"adjust",transform:{deltaX:0,deltaY:0,scaleX:1,scaleY:1,rotationDegrees:0},crop}));
 fireEvent.change(v.getByLabelText("Image action"),{target:{value:"transform"}});
 const prior=frameMeasurements(s.canvas.frame,page);
 act(()=>s.canvas.onChange({...prior,centerX:prior.centerX+10}));
 await waitFor(()=>expect(v.getByRole("button",{name:"Apply changes"})).toBeEnabled());
 expect(s.props.adapter.previewImageEditOccurrence).toHaveBeenLastCalledWith("source",[0],0,[2],image.selectionId,expect.objectContaining({kind:"adjust",crop,transform:expect.objectContaining({deltaY:-10})}));
 fireEvent.change(v.getByLabelText("Image action"),{target:{value:"crop"}});
 fireEvent.click(v.getByRole("button",{name:"Restore full image"}));
 expect(s.canvas.crop).toEqual(fullCrop);
 expect(frameMeasurements(s.canvas.fullFrame!,page).centerX).toBeCloseTo(frameMeasurements(image.corners,page).centerX+10,8);
 fireEvent.change(v.getByLabelText("Image action"),{target:{value:"transform"}});
 fireEvent.click(v.getByRole("button",{name:"Reset position"}));
 expect(s.canvas.fullFrame).toEqual(image.corners);
});
it("supports precise crop edges and keeps invalid percentages unappliable when collapsed",async()=>{
 const s=setup(),v=render(<ImageReplacementDialog {...s.props}/>);
 fireEvent.change(v.getByLabelText("Image action"),{target:{value:"crop"}});fireEvent.click(v.getByRole("button",{name:"Precision and help"}));
 fireEvent.change(v.getByLabelText("Keep from left (%)"),{target:{value:"25"}});
 expect(s.canvas.crop).toEqual({...fullCrop,left:.25});expect(s.canvas.fullFrame).toEqual(image.corners);
 fireEvent.change(v.getByLabelText("Keep to right (%)"),{target:{value:""}});fireEvent.click(v.getByRole("button",{name:"Precision and help"}));
 expect(v.getByRole("button",{name:"Preview changes"})).toBeDisabled();
 fireEvent.click(v.getByRole("button",{name:"Restore full image"}));expect(s.canvas.crop).toEqual(fullCrop);expect(v.queryByRole("alert")).toBeNull();
});
it("admits final source-valid scales after shrinking and rebasing a crop",async()=>{
 const s=setup(),v=render(<ImageReplacementDialog {...s.props}/>);
 fireEvent.click(v.getByRole("button",{name:"Precision and help"}));
 const original=frameMeasurements(image.corners,page);
 fireEvent.change(v.getByLabelText("Width (pt)"),{target:{value:String(original.width*.001)}});
 fireEvent.change(v.getByLabelText("Image action"),{target:{value:"crop"}});
 fireEvent.change(v.getByLabelText("Keep from left (%)"),{target:{value:"50"}});
 fireEvent.change(v.getByLabelText("Width (pt)"),{target:{value:String(original.width)}});
 expect(v.queryByRole("alert")).toBeNull();
 await waitFor(()=>expect(v.getByRole("button",{name:"Apply changes"})).toBeEnabled());
 expect(s.props.adapter.previewImageEditOccurrence).toHaveBeenLastCalledWith("source",[0],0,[2],image.selectionId,expect.objectContaining({transform:expect.objectContaining({scaleX:expect.closeTo(2,11),scaleY:expect.closeTo(2,11)})}));
 fireEvent.change(v.getByLabelText("Keep from left (%)"),{target:{value:"25"}});
 fireEvent.change(v.getByLabelText("Width (pt)"),{target:{value:String(original.width*.001*.75)}});
 expect(v.queryByRole("alert")).toBeNull();
 fireEvent.change(v.getByLabelText("Width (pt)"),{target:{value:String(original.width*1001)}});
 expect(v.getByRole("button",{name:"Preview changes"})).toBeDisabled();
});
it("retains invalid hidden precision fields across modes and rejects obsolete crop previews",async()=>{
 const s=setup();let done!:(v:PageContentReplacement)=>void;s.props.adapter.previewImageEditOccurrence.mockImplementationOnce(()=>new Promise(resolve=>{done=resolve;}));
 const v=render(<ImageReplacementDialog {...s.props}/>);
 const details=v.getByRole("button",{name:"Precision and help"});expect(details).toHaveAttribute("aria-expanded","false");
 fireEvent.click(details);fireEvent.change(v.getByLabelText("Center X (pt)"),{target:{value:""}});fireEvent.click(details);
 fireEvent.change(v.getByLabelText("Image action"),{target:{value:"crop"}});
 expect(v.getByRole("button",{name:"Preview changes"})).toBeDisabled();expect(v.getByRole("alert")).toHaveTextContent("valid number");
 fireEvent.change(v.getByLabelText("Image action"),{target:{value:"transform"}});fireEvent.click(v.getByRole("button",{name:"Reset position"}));
 fireEvent.click(v.getByRole("button",{name:"Preview changes"}));
 fireEvent.change(v.getByLabelText("Image action"),{target:{value:"crop"}});act(()=>s.canvas.onCropChange!({...fullCrop,left:.25}));
 await act(async()=>done(result("stale")));await waitFor(()=>expect(s.props.adapter.closeDocument).toHaveBeenCalledWith("stale"));
 await waitFor(()=>expect(s.props.adapter.previewImageEditOccurrence).toHaveBeenCalledTimes(2));
 expect(s.props.adapter.previewImageEditOccurrence).toHaveBeenLastCalledWith("source",[0],0,[2],image.selectionId,expect.objectContaining({kind:"adjust",crop:{...fullCrop,left:.25}}));
});
it("uses a nonmodal full-page raster and debounces a gesture into native preview",async()=>{
 const s=setup(),v=render(<ImageReplacementDialog {...s.props}/>);expect(v.queryByRole("dialog")).toBeNull();expect(v.getByRole("region",{name:"Image editing"})).toBeInTheDocument();expect(v.getByLabelText("Image action")).toHaveValue("transform");
 await waitFor(()=>expect(s.canvas.raster.url).toBeTruthy());expect(s.props.adapter.renderPagePlan).toHaveBeenCalledWith(expect.objectContaining({crop:undefined,rotation:90}),600);
 const old=frameMeasurements(s.canvas.frame,page);act(()=>s.canvas.onGestureActive(true));expect(v.getByRole("button",{name:"Apply changes"})).toBeDisabled();expect(s.props.adapter.previewImageEditOccurrence).not.toHaveBeenCalled();
 act(()=>{s.canvas.onGestureActive(false);s.canvas.onChange({...old,centerX:old.centerX+10});});await waitFor(()=>expect(s.props.adapter.previewImageEditOccurrence).toHaveBeenCalledOnce());
 expect(s.props.adapter.previewImageEditOccurrence).toHaveBeenCalledWith("source",[0],0,[2],image.selectionId,{kind:"adjust",crop:{left:0,top:0,right:1,bottom:1},transform:{deltaX:0,deltaY:-10,scaleX:1,scaleY:1,rotationDegrees:0}});
 await waitFor(()=>expect(v.getByRole("button",{name:"Apply changes"})).toBeEnabled());expect(s.imported).not.toHaveBeenCalled();fireEvent.click(v.getByRole("button",{name:"Locate image"}));expect(s.props.onLocate).toHaveBeenCalledOnce();
 fireEvent.click(v.getByRole("button",{name:"Apply changes"}));expect(s.props.onApply).toHaveBeenCalledOnce();v.unmount();expect(s.props.adapter.closeDocument).not.toHaveBeenCalledWith("candidate");
});
it("prepares only the latest queued draft after a native request settles",async()=>{
 const s=setup();let done!:(v:PageContentReplacement)=>void;s.props.adapter.previewImageEditOccurrence.mockImplementationOnce(()=>new Promise(resolve=>{done=resolve;})).mockResolvedValue(result("latest"));
 const v=render(<ImageReplacementDialog {...s.props}/>),original=frameMeasurements(image.corners,page);
 act(()=>s.canvas.onChange({...original,centerX:original.centerX+1}));await waitFor(()=>expect(s.props.adapter.previewImageEditOccurrence).toHaveBeenCalledTimes(1));
 act(()=>s.canvas.onChange({...original,centerX:original.centerX+3}));act(()=>s.canvas.onChange({...original,centerX:original.centerX+9}));expect(s.props.adapter.previewImageEditOccurrence).toHaveBeenCalledTimes(1);
 await act(async()=>done(result("old")));await waitFor(()=>expect(s.props.adapter.previewImageEditOccurrence).toHaveBeenCalledTimes(2));expect(s.props.adapter.closeDocument).toHaveBeenCalledWith("old");
 expect(s.props.adapter.previewImageEditOccurrence).toHaveBeenLastCalledWith("source",[0],0,[2],image.selectionId,expect.objectContaining({transform:expect.objectContaining({deltaY:-9})}));await waitFor(()=>expect(v.getByRole("button",{name:"Apply changes"})).toBeEnabled());
 v.unmount();await waitFor(()=>expect(s.props.adapter.closeDocument).toHaveBeenCalledWith("latest"));
});
it("keeps gesture measurements unrounded and Reset exactly returns to the source",async()=>{
 const s=setup(),v=render(<ImageReplacementDialog {...s.props}/>),original=frameMeasurements(image.corners,page);const wanted={...original,width:original.width*.7123456789,height:original.height*.7123456789};
 act(()=>s.canvas.onChange(wanted));await waitFor(()=>expect(v.getByRole("button",{name:"Apply changes"})).toBeEnabled());expect(frameMeasurements(s.canvas.frame,page).width).toBeCloseTo(wanted.width,8);
 fireEvent.click(v.getByRole("button",{name:"Preview changes"}));await waitFor(()=>expect(s.props.adapter.previewImageEditOccurrence).toHaveBeenCalledTimes(2));expect(s.props.adapter.previewImageEditOccurrence).toHaveBeenLastCalledWith("source",[0],0,[2],image.selectionId,{kind:"adjust",crop:{left:0,top:0,right:1,bottom:1},transform:expect.objectContaining({scaleX:expect.closeTo(wanted.width/original.width,14)})});
 await waitFor(()=>expect(v.getByRole("button",{name:"Apply changes"})).toBeEnabled());fireEvent.click(v.getByRole("button",{name:"Reset position"}));await waitFor(()=>expect(s.props.adapter.previewImageEditOccurrence).toHaveBeenCalledTimes(3));expect(s.props.adapter.previewImageEditOccurrence).toHaveBeenLastCalledWith("source",[0],0,[2],image.selectionId,{kind:"adjust",crop:{left:0,top:0,right:1,bottom:1},transform:{deltaX:0,deltaY:0,scaleX:1,scaleY:1,rotationDegrees:0}});
});
it("rejects late borrowed candidates and closes the session with Escape",async()=>{
 const s=setup();let done!:(v:PageContentReplacement)=>void;s.props.adapter.previewImageEditOccurrence.mockImplementation(()=>new Promise(resolve=>{done=resolve;}));const v=render(<ImageReplacementDialog {...s.props}/>);
 fireEvent.click(v.getByRole("button",{name:"Preview changes"}));fireEvent.keyDown(v.getByRole("region",{name:"Image editing"}),{key:"Escape"});expect(s.props.onCancel).toHaveBeenCalledOnce();v.unmount();await act(async()=>done(result("other-tab")));expect(s.props.adapter.closeDocument).not.toHaveBeenCalledWith("other-tab");
});

it("keeps a failed raster unappliable and rejects unexpected document mutation",async()=>{
 const s=setup();s.props.adapter.renderPagePlan.mockImplementation(async plan=>{if(plan.sourceId==="candidate")throw new Error("Raster failed");return "data:original";});
 const v=render(<ImageReplacementDialog {...s.props}/>);fireEvent.click(v.getByRole("button",{name:"Preview changes"}));await v.findByText("Raster failed");expect(v.getByRole("button",{name:"Apply changes"})).toBeDisabled();
 v.rerender(<ImageReplacementDialog {...s.props} document={{...document,pages:[{...page}]}}/>);expect(s.canvas.enabled).toBe(false);expect(v.getByRole("button",{name:"Preview changes"})).toBeDisabled();v.unmount();await waitFor(()=>expect(s.props.adapter.closeDocument).toHaveBeenCalledWith("candidate"));
});
it("disables original comparison handles and invalidates a pending candidate on Cancel",async()=>{
 const s=setup();let done!:(v:PageContentReplacement)=>void;s.props.adapter.previewImageEditOccurrence.mockImplementation(()=>new Promise(resolve=>{done=resolve;}));
 const v=render(<ImageReplacementDialog {...s.props}/>);await waitFor(()=>expect(v.getByRole("button",{name:"Original"})).toBeEnabled());fireEvent.click(v.getByRole("button",{name:"Original"}));expect(s.canvas.enabled).toBe(false);expect(s.canvas.frame).toEqual(image.corners);
 fireEvent.click(v.getByRole("button",{name:"Edited"}));expect(s.canvas.enabled).toBe(true);fireEvent.click(v.getByRole("button",{name:"Preview changes"}));fireEvent.click(v.getByRole("button",{name:"Cancel"}));v.unmount();await act(async()=>done(result("late")));expect(s.props.adapter.closeDocument).toHaveBeenCalledWith("late");expect(s.props.onApply).not.toHaveBeenCalled();
});
