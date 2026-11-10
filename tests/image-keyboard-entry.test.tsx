import {cleanup,render} from "@testing-library/react";
import {afterEach,expect,it,vi} from "vitest";
import {PageView,clearRenderCache} from "../src/components/PageView";
import {createDemoAdapter} from "../src/editor/adapter";
import type {CanvasImageView} from "../src/editor/existing-image-canvas";
import type {ExistingImage,PagePlan} from "../src/editor/types";

const page:PagePlan={id:"p",sourceId:"image-focus-source",pageIndex:0,width:400,height:300,rotation:0,overlays:[]};
const image:ExistingImage={objectPath:[2],selectionId:"a".repeat(64),corners:[{x:50,y:150},{x:150,y:150},{x:150,y:50},{x:50,y:50}],bounds:{x:50,y:50,width:100,height:100},pixelWidth:20,pixelHeight:10};
const props={adapter:createDemoAdapter(),page,pageNumber:1,zoom:100,tool:"select" as const,selectedOverlayId:null,pendingSignature:null,onSelectOverlay:vi.fn(),onAddText:vi.fn(),onPlaceSignature:vi.fn(),onMoveOverlay:vi.fn()};
function value(mode:CanvasImageView["mode"]="position"):CanvasImageView{return {pageId:page.id,sessionKey:"source:a",image,frame:image.corners,locked:true,enabled:true,mode,raster:{loading:false},onGestureActive:vi.fn(),onChange:vi.fn(),onCropChange:vi.fn()};}
function workspace(view:CanvasImageView){return <><select aria-label="Image action" defaultValue="position"><option value="position">Position</option><option value="crop">Crop</option></select><input aria-label="Precision"/><PageView {...props} canvasImage={view}/></>;}
afterEach(()=>{cleanup();clearRenderCache([page.sourceId]);});

it.each(["position","crop"] as const)("focuses the image on initial %s entry and a new session",mode=>{
 const original=value(mode),v=render(workspace(original));
 const name=mode==="crop"?"Move crop window":"Move selected image";
 expect(v.getByRole("button",{name})).toHaveFocus();
 v.getByRole("combobox",{name:"Image action"}).focus();
 v.rerender(workspace({...original,sessionKey:"source:b"}));
 expect(v.getByRole("button",{name})).toHaveFocus();
});

it.each(["position","crop"] as const)("keeps action-menu focus when switching away from %s",mode=>{
 const original=value(mode),v=render(workspace(original)),menu=v.getByRole("combobox",{name:"Image action"});
 menu.focus();v.rerender(workspace({...original,mode:mode==="crop"?"position":"crop"}));
 expect(menu).toHaveFocus();
});

it("preserves precision-field focus across preview and geometry updates",()=>{
 const original=value(),v=render(workspace(original)),field=v.getByRole("textbox",{name:"Precision"});
 field.focus();v.rerender(workspace({...original,enabled:false,raster:{loading:true}}));expect(field).toHaveFocus();
 v.rerender(workspace({...original,frame:original.frame.map(p=>({...p,x:p.x+5})) as typeof original.frame}));expect(field).toHaveFocus();
});
