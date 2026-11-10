import {act,cleanup,fireEvent,render,waitFor} from "@testing-library/react";
import {afterEach,expect,it,vi} from "vitest";
import {ImageReplacementDialog} from "../src/components/ImageReplacementDialog";
import {createDemoAdapter} from "../src/editor/adapter";
import {imageApi} from "../src/editor/image-resources";
import type {EditorDocument,ExistingImage,PageContentReplacement,PagePlan} from "../src/editor/types";
const page:PagePlan={id:"p",sourceId:"source",pageIndex:0,width:300,height:400,rotation:90,crop:{x:10,y:20,width:200,height:300},overlays:[]};
const document:EditorDocument={name:"Original.pdf",pages:[page],selectedPageId:"p",selectedOverlayId:null};
const image:ExistingImage={objectPath:[2],selectionId:"a".repeat(64),corners:[{x:20,y:20},{x:100,y:20},{x:100,y:100},{x:20,y:100}],bounds:{x:20,y:20,width:80,height:80},pixelWidth:20,pixelHeight:10};
const result:PageContentReplacement={document:{id:"candidate",name:"Edited.pdf",pages:[{width:300,height:400}]},pageIndices:[0]};
const imported={image:{id:"b".repeat(64),width:80,height:40,mimeType:"image/png",hasAlpha:true,bitsPerChannel:8,byteLength:100},token:"lease"};
afterEach(()=>{cleanup();vi.restoreAllMocks();});
it("previews position and removal without importing an asset, invalidating old action previews",async()=>{
 const base=setup(),p={...base,adapter:{...base.adapter,previewImageEditOccurrence:vi.fn(async()=>result)}};
 const v=render(<ImageReplacementDialog {...p}/>);
 fireEvent.change(v.getByLabelText("Image action"),{target:{value:"transform"}});
 fireEvent.change(v.getByLabelText("Center X (pt)"),{target:{value:"350"}});
 fireEvent.change(v.getByLabelText("Width (pt)"),{target:{value:"160"}});
 fireEvent.click(v.getByRole("button",{name:"Preview changes"}));await waitFor(()=>expect(v.getByRole("button",{name:"Apply changes"})).toBeEnabled());
 expect(p.adapter.previewImageEditOccurrence).toHaveBeenCalledWith("source",[0],0,[2],image.selectionId,{kind:"adjust",crop:{left:0,top:0,right:1,bottom:1},transform:{deltaX:0,deltaY:-10,scaleX:2,scaleY:2,rotationDegrees:0}});
 expect(imageApi.importImage).not.toHaveBeenCalled();
 fireEvent.change(v.getByLabelText("Image action"),{target:{value:"remove"}});expect(v.getByRole("button",{name:"Apply changes"})).toBeDisabled();await waitFor(()=>expect(p.adapter.closeDocument).toHaveBeenCalledWith("candidate"));
 fireEvent.click(v.getByRole("button",{name:"Preview changes"}));await waitFor(()=>expect(v.getByRole("button",{name:"Apply changes"})).toBeEnabled());
 expect(p.adapter.previewImageEditOccurrence).toHaveBeenLastCalledWith("source",[0],0,[2],image.selectionId,{kind:"remove"});
 fireEvent.click(v.getByRole("button",{name:"Apply changes"}));expect(p.onApply).toHaveBeenCalledOnce();
});
it("Reset restores invalid position fields and preserves an exact no-op",async()=>{
 const p=setup(),adapter={...p.adapter,previewImageEditOccurrence:vi.fn(async()=>result)};const v=render(<ImageReplacementDialog {...p} adapter={adapter}/>);
 fireEvent.change(v.getByLabelText("Image action"),{target:{value:"transform"}});fireEvent.change(v.getByLabelText("Center X (pt)"),{target:{value:""}});
 expect(v.getByRole("button",{name:"Preview changes"})).toBeDisabled();fireEvent.click(v.getByRole("button",{name:"Reset position"}));
 fireEvent.click(v.getByRole("button",{name:"Preview changes"}));await waitFor(()=>expect(v.getByRole("button",{name:"Apply changes"})).toBeEnabled());
 expect(adapter.previewImageEditOccurrence).toHaveBeenLastCalledWith("source",[0],0,[2],image.selectionId,{kind:"adjust",crop:{left:0,top:0,right:1,bottom:1},transform:{deltaX:0,deltaY:0,scaleX:1,scaleY:1,rotationDegrees:0}});
});
it("discards a pending transform after a draft or action change",async()=>{
 const p=setup();let done!:(v:PageContentReplacement)=>void;const adapter={...p.adapter,previewImageEditOccurrence:vi.fn(()=>new Promise<PageContentReplacement>(resolve=>{done=resolve;}))};
 const v=render(<ImageReplacementDialog {...p} adapter={adapter}/>);fireEvent.change(v.getByLabelText("Image action"),{target:{value:"transform"}});fireEvent.click(v.getByRole("button",{name:"Preview changes"}));
 fireEvent.change(v.getByLabelText("Clockwise angle (°)"),{target:{value:"45"}});await act(async()=>done(result));await waitFor(()=>expect(adapter.closeDocument).toHaveBeenCalledWith("candidate"));expect(v.getByRole("button",{name:"Apply changes"})).toBeDisabled();
});
it.each(["transform","remove"])("keeps %s ownership safe through action changes, unmount and borrowed late results",async(action)=>{
 const p=setup();let done!:(v:PageContentReplacement)=>void;const adapter={...p.adapter,previewImageEditOccurrence:vi.fn(()=>new Promise<PageContentReplacement>(resolve=>{done=resolve;}))};
 const v=render(<ImageReplacementDialog {...p} adapter={adapter}/>);fireEvent.change(v.getByLabelText("Image action"),{target:{value:action}});fireEvent.click(v.getByRole("button",{name:"Preview changes"}));
 fireEvent.change(v.getByLabelText("Image action"),{target:{value:"replace"}});await act(async()=>done(result));await waitFor(()=>expect(adapter.closeDocument).toHaveBeenCalledWith("candidate"));expect(v.getByRole("button",{name:"Apply replacement"})).toBeDisabled();
 fireEvent.change(v.getByLabelText("Image action"),{target:{value:action}});fireEvent.click(v.getByRole("button",{name:"Preview changes"}));v.unmount();await act(async()=>done({...result,document:{...result.document,id:"other-tab"}}));expect(adapter.closeDocument).not.toHaveBeenCalledWith("other-tab");expect(imageApi.importImage).not.toHaveBeenCalled();
});
function setup(){
 const release=vi.spyOn(imageApi,"release").mockResolvedValue();vi.spyOn(imageApi,"importImage").mockResolvedValue(imported);
 const adapter={...createDemoAdapter(),kind:"native" as const,previewImageReplacementOccurrence:vi.fn(async()=>result),closeDocument:vi.fn(async(_id:string)=>{}),renderPagePlan:vi.fn(async()=>"data:image/png;base64,AA")};
 return {adapter,page,document,image,sourceIds:["source"],isSourceRetained:(id:string)=>id==="source"||id==="other-tab",onApply:vi.fn(()=>true),onCancel:vi.fn(),release};
}
async function choose(v:ReturnType<typeof render>){fireEvent.click(v.getByRole("button",{name:"Choose image…"}));await v.findByText(/80 × 40/);}
async function preview(v:ReturnType<typeof render>){fireEvent.click(v.getByRole("button",{name:"Preview replacement"}));await waitFor(()=>expect(v.getByRole("button",{name:"Apply replacement"})).toBeEnabled());}
it("requires explicit rendered preview and transfers one candidate on Apply",async()=>{
 const p=setup(),v=render(<ImageReplacementDialog {...p}/>);expect(v.getByRole("button",{name:"Apply replacement"})).toBeDisabled();await choose(v);await preview(v);
 expect(p.adapter.previewImageReplacementOccurrence).toHaveBeenCalledWith("source",[0],0,[2],image.selectionId,imported.image.id,"fit");
 fireEvent.click(v.getByRole("button",{name:"Original"}));fireEvent.click(v.getByRole("button",{name:"Edited"}));fireEvent.click(v.getByRole("button",{name:"Apply replacement"}));v.unmount();
 expect(p.onApply).toHaveBeenCalledOnce();expect(p.onApply).toHaveBeenCalledWith(result);expect(p.adapter.closeDocument).not.toHaveBeenCalledWith("candidate");await waitFor(()=>expect(p.release).toHaveBeenCalledOnce());
});
it("invalidates previews when fitting changes and closes the old candidate",async()=>{
 const p=setup(),v=render(<ImageReplacementDialog {...p}/>);await choose(v);await preview(v);fireEvent.change(v.getByLabelText("Image fitting"),{target:{value:"fill"}});expect(v.getByRole("button",{name:"Apply replacement"})).toBeDisabled();await waitFor(()=>expect(p.adapter.closeDocument).toHaveBeenCalledWith("candidate"));
});
it("holds image ownership through a pending preview and releases a late candidate",async()=>{
 const p=setup();let done!:(value:PageContentReplacement)=>void;p.adapter.previewImageReplacementOccurrence.mockImplementation(()=>new Promise(resolve=>{done=resolve;}));const v=render(<ImageReplacementDialog {...p}/>);await choose(v);fireEvent.click(v.getByRole("button",{name:"Preview replacement"}));v.unmount();expect(p.release).not.toHaveBeenCalled();await act(async()=>done(result));await waitFor(()=>expect(p.adapter.closeDocument).toHaveBeenCalledWith("candidate"));expect(p.release).toHaveBeenCalledOnce();
});
it("never closes another tab's borrowed source returned as a malformed candidate",async()=>{
 const p=setup();p.adapter.previewImageReplacementOccurrence.mockResolvedValue({...result,document:{...result.document,id:"other-tab"}});const v=render(<ImageReplacementDialog {...p}/>);await choose(v);fireEvent.click(v.getByRole("button",{name:"Preview replacement"}));await v.findByRole("alert");v.unmount();expect(p.adapter.closeDocument).not.toHaveBeenCalledWith("other-tab");expect(p.onApply).not.toHaveBeenCalled();
});
it("releases an imported image arriving after unmount",async()=>{
 const p=setup();let done!:(value:typeof imported)=>void;vi.mocked(imageApi.importImage).mockImplementation(()=>new Promise(resolve=>{done=resolve;}));const v=render(<ImageReplacementDialog {...p}/>);fireEvent.click(v.getByRole("button",{name:"Choose image…"}));v.unmount();await act(async()=>done(imported));await waitFor(()=>expect(p.release).toHaveBeenCalledOnce());
});
it("keeps Apply disabled when rendering fails and retains the original document",async()=>{
 const p=setup();p.adapter.renderPagePlan.mockImplementation(async(plan?:PagePlan)=>{if(plan?.sourceId==="candidate")throw new Error("Preview rendering failed");return "data:image/png;base64,AA";});
 const v=render(<ImageReplacementDialog {...p}/>);await choose(v);fireEvent.click(v.getByRole("button",{name:"Preview replacement"}));await v.findByText(/Preview rendering failed/);expect(v.getByRole("button",{name:"Apply replacement"})).toBeDisabled();v.unmount();await waitFor(()=>expect(p.adapter.closeDocument).toHaveBeenCalledWith("candidate"));expect(p.onApply).not.toHaveBeenCalled();
});
it("rejects a pending result after the document session changes",async()=>{
 const p=setup();let done!:(value:PageContentReplacement)=>void;p.adapter.previewImageReplacementOccurrence.mockImplementation(()=>new Promise(resolve=>{done=resolve;}));const v=render(<ImageReplacementDialog {...p}/>);await choose(v);fireEvent.click(v.getByRole("button",{name:"Preview replacement"}));v.rerender(<ImageReplacementDialog {...p} document={{...document,pages:[{...page}]}}/>);await act(async()=>done(result));await waitFor(()=>expect(p.adapter.closeDocument).toHaveBeenCalledWith("candidate"));expect(v.getByRole("button",{name:"Apply replacement"})).toBeDisabled();
});
it("a fitting change during preparation discards the obsolete preview",async()=>{
 const p=setup();let done!:(value:PageContentReplacement)=>void;p.adapter.previewImageReplacementOccurrence.mockImplementation(()=>new Promise(resolve=>{done=resolve;}));const v=render(<ImageReplacementDialog {...p}/>);await choose(v);fireEvent.click(v.getByRole("button",{name:"Preview replacement"}));fireEvent.change(v.getByLabelText("Image fitting"),{target:{value:"stretch"}});await act(async()=>done(result));await waitFor(()=>expect(p.adapter.closeDocument).toHaveBeenCalledWith("candidate"));expect(v.getByRole("button",{name:"Apply replacement"})).toBeDisabled();
});
it("does not close a borrowed source delivered after unmount",async()=>{
 const p=setup();let done!:(value:PageContentReplacement)=>void;p.adapter.previewImageReplacementOccurrence.mockImplementation(()=>new Promise(resolve=>{done=resolve;}));const v=render(<ImageReplacementDialog {...p}/>);await choose(v);fireEvent.click(v.getByRole("button",{name:"Preview replacement"}));v.unmount();await act(async()=>done({...result,document:{...result.document,id:"other-tab"}}));expect(p.adapter.closeDocument).not.toHaveBeenCalled();expect(p.release).toHaveBeenCalledOnce();
});
