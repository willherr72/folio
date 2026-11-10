import {act,cleanup,render,waitFor} from "@testing-library/react";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {PageView,Thumbnail,clearRenderCache} from "../src/components/PageView";
import {createDemoAdapter,type FolioAdapter} from "../src/editor/adapter";
import {displayNativeCommentBounds,nativeCommentPageHitBounds} from "../src/editor/native-comments";
import {displayDimensions,pageTransform} from "../src/editor/geometry";
import type {NativeComment,PagePlan,Rotation} from "../src/editor/types";
const page:PagePlan={id:"rotation",sourceId:"rotation-source",pageIndex:0,width:300,height:400,rotation:0,overlays:[]};
const note:NativeComment={pageIndex:0,annotationIndex:0,text:"Upright note",bounds:{x:20,y:30,width:18,height:26},hitBounds:{x:20,y:30,width:18,height:26},noRotate:true};
const service=()=>({...createDemoAdapter(),kind:"native" as const,renderPage:vi.fn().mockResolvedValue("blob:original"),renderPagePlan:vi.fn().mockResolvedValue("blob:rotated"),getPageText:vi.fn().mockResolvedValue({characters:[]})});
const props=(adapter:FolioAdapter=service(),value=page)=>({adapter,page:value,pageNumber:1,zoom:100,tool:"select" as const,selectedOverlayId:null,pendingSignature:null,onSelectOverlay:vi.fn(),onAddText:vi.fn(),onPlaceSignature:vi.fn(),onMoveOverlay:vi.fn()});
beforeEach(()=>{Object.defineProperty(URL,"revokeObjectURL",{configurable:true,value:vi.fn()});});
afterEach(()=>{cleanup();clearRenderCache([page.sourceId]);vi.restoreAllMocks();});
it.each([0,90,180,270] as Rotation[])("keeps NoRotate targets upright about the transformed anchor at rotation %s",rotation=>{
 const value={...page,rotation};const target=nativeCommentPageHitBounds(note,value);const expected={0:{x:20,y:30,width:18,height:26},90:{x:370,y:20,width:18,height:26},180:{x:280,y:370,width:18,height:26},270:{x:30,y:280,width:18,height:26}};
 expect(displayNativeCommentBounds(target,value)).toEqual(expected[rotation]);expect(nativeCommentPageHitBounds({...note,noRotate:false},value)).toEqual(note.hitBounds);
});
it.each([90,180,270] as Rotation[])("renders rotated plain pages natively and cancels raster rotation exactly once at %s",async rotation=>{
 const adapter=service(),value={...page,rotation},view=render(<PageView {...props(adapter,value)}/>);await waitFor(()=>expect(adapter.renderPagePlan).toHaveBeenCalledWith(expect.objectContaining({rotation}),expect.any(Number)));
 const image=view.container.querySelector('image[href="blob:rotated"]')!;const dimensions=displayDimensions(page.width,page.height,rotation);expect(image).toHaveAttribute("width",String(dimensions.width));expect(image).toHaveAttribute("height",String(dimensions.height));expect(image).toHaveAttribute("transform",pageTransform(dimensions.width,dimensions.height,(360-rotation) as Rotation));expect(adapter.renderPage).not.toHaveBeenCalled();
});
it("uses the same native rotation for thumbnails",async()=>{
 const adapter=service(),view=render(<Thumbnail adapter={adapter} page={{...page,rotation:90}}/>);await waitFor(()=>expect(adapter.renderPagePlan).toHaveBeenCalledWith(expect.objectContaining({rotation:90}),240));expect(view.container.querySelector('image[href="blob:rotated"]')).toHaveAttribute("width","400");
});
it("hides the old-angle raster immediately and disposes an obsolete in-flight angle",async()=>{
 let finish!:(url:string)=>void;const adapter=service();adapter.renderPagePlan.mockResolvedValueOnce("blob:first").mockImplementationOnce(()=>new Promise(resolve=>{finish=resolve;}));const view=render(<PageView {...props(adapter,{...page,rotation:90})}/>);await waitFor(()=>expect(view.container.querySelector('image[href="blob:first"]')).toBeInTheDocument());
 view.rerender(<PageView {...props(adapter,{...page,rotation:180})}/>);expect(view.container.querySelector('image[href="blob:first"]')).not.toBeInTheDocument();await waitFor(()=>expect(adapter.renderPagePlan).toHaveBeenCalledTimes(2));
 view.rerender(<PageView {...props(adapter,{...page,rotation:270})}/>);await act(async()=>finish("blob:obsolete"));await waitFor(()=>expect(adapter.renderPagePlan).toHaveBeenCalledTimes(3));expect(adapter.renderPagePlan.mock.calls[2][0].rotation).toBe(270);expect(URL.revokeObjectURL).toHaveBeenCalledWith("blob:obsolete");
});
it("keeps ordinary unrotated pages on the direct render cache",async()=>{
 const adapter=service(),view=render(<PageView {...props(adapter)}/>);await waitFor(()=>expect(view.container.querySelector('image[href="blob:original"]')).toBeInTheDocument());expect(adapter.renderPagePlan).not.toHaveBeenCalled();
});

it.each(["native","demo"] as const)("keeps NoRotate targets on the rotated bitmap when %s has no native composition",async kind=>{
 const adapter={...service(),kind,renderPagePlan:undefined};const view=render(<PageView {...props(adapter,{...page,rotation:90})} nativeComments={[note]}/>);await waitFor(()=>expect(view.container.querySelector('image[href="blob:original"]')).toBeInTheDocument());const rect=view.getByRole("button",{name:"PDF comment: Upright note"}).querySelector("rect")!;expect(rect).toHaveAttribute("x","20");expect(rect).toHaveAttribute("y","30");expect(rect).toHaveAttribute("width","18");expect(rect).toHaveAttribute("height","26");
});
