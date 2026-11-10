import {act,cleanup,fireEvent,render} from "@testing-library/react";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {NativeCommentLayer} from "../src/components/NativeCommentLayer";
import {displayDimensions,toDisplayPoint} from "../src/editor/geometry";
import type {NativeComment,PagePlan,Rotation} from "../src/editor/types";
const page:PagePlan={id:"page",sourceId:"source",pageIndex:0,width:300,height:400,rotation:0,overlays:[]};
const note:NativeComment={pageIndex:0,annotationIndex:2,text:"Saved note",bounds:{x:40,y:50,width:20,height:18},onPage:true};
function options(value=page){return {page:value,notes:[note],zoom:200,enabled:true,disabled:false,onOpen:vi.fn(),onSelect:vi.fn(),onMove:vi.fn().mockResolvedValue("updated"),onStatus:vi.fn()};}
function setup(value=page){const props=options(value);const view=render(<svg><NativeCommentLayer {...props}/></svg>);const svg=view.container.querySelector("svg")!;const size=displayDimensions(value.width,value.height,value.rotation);vi.spyOn(svg,"getBoundingClientRect").mockReturnValue({left:10,top:20,width:size.width*2,height:size.height*2} as DOMRect);const at=(x:number,y:number,pointerId=1)=>{const p=toDisplayPoint({x,y},value.width,value.height,value.rotation);return {clientX:10+p.x*2,clientY:20+p.y*2,pointerId,button:0};};return {props,view,at,target:()=>view.getByRole("button",{name:"PDF comment: Saved note"})};}
beforeEach(()=>{vi.stubGlobal("PointerEvent",class extends MouseEvent{pointerId:number;constructor(type:string,init:PointerEventInit={}){super(type,init);this.pointerId=init.pointerId??1;}});Object.defineProperty(SVGElement.prototype,"setPointerCapture",{configurable:true,value:vi.fn()});Object.defineProperty(SVGElement.prototype,"releasePointerCapture",{configurable:true,value:vi.fn()});});
afterEach(()=>{cleanup();vi.restoreAllMocks();vi.unstubAllGlobals();});
it.each([0,90,180,270] as Rotation[])("moves the outline in native page coordinates at workspace rotation %s",async rotation=>{
 const {props,view,at,target}=setup({...page,rotation});fireEvent.pointerDown(target(),at(45,55));fireEvent.pointerMove(target(),at(75,95));expect(props.onMove).not.toHaveBeenCalled();expect(view.container.querySelector(".native-comment-draft")).toHaveAttribute("x","70");
 await act(async()=>fireEvent.pointerUp(target(),at(75,95)));expect(props.onMove).toHaveBeenCalledOnce();expect(props.onMove).toHaveBeenCalledWith(note,{x:70,y:90});
});
it.each(["escape","cancel","capture","source","zoom","rotation","disabled","tool","blur","unmount"])("cancels the draft on %s",reason=>{
 const {props,view,at,target}=setup();fireEvent.pointerDown(target(),at(45,55));fireEvent.pointerMove(target(),at(75,95));
 if(reason==="escape")fireEvent.keyDown(window,{key:"Escape"});else if(reason==="cancel")fireEvent.pointerCancel(target(),at(75,95));else if(reason==="capture")fireEvent.lostPointerCapture(target(),at(75,95));else if(reason==="blur")fireEvent.blur(window);else if(reason==="unmount")view.unmount();
 else view.rerender(<svg><NativeCommentLayer {...props} {...(reason==="source"?{page:{...page,sourceId:"other"}}:reason==="rotation"?{page:{...page,rotation:90}}:reason==="zoom"?{zoom:100}:reason==="tool"?{enabled:false}:{disabled:true})}/></svg>);
 if(reason!=="unmount"&&reason!=="tool")fireEvent.pointerUp(target(),at(75,95));expect(props.onMove).not.toHaveBeenCalled();
});
it("ignores clicks and competing pointers and clamps the complete note to the page",async()=>{
 const {props,at,target}=setup();fireEvent.pointerDown(target(),at(45,55));fireEvent.pointerUp(target(),at(45,55));expect(props.onMove).not.toHaveBeenCalled();fireEvent.doubleClick(target());expect(props.onOpen).toHaveBeenCalledWith(note);
 fireEvent.pointerDown(target(),at(45,55));fireEvent.pointerUp(target(),at(200,200,2));expect(props.onMove).not.toHaveBeenCalled();await act(async()=>fireEvent.pointerUp(target(),at(900,-300)));expect(props.onMove).toHaveBeenCalledOnce();expect(props.onMove).toHaveBeenCalledWith(note,{x:280,y:0});
});
it.each([0,90,180,270] as Rotation[])("keeps keyboard nudges local until Enter at rotation %s",async rotation=>{
 const {props,target}=setup({...page,rotation});fireEvent.focus(target());fireEvent.keyDown(target(),{key:"ArrowRight"});fireEvent.keyDown(target(),{key:"ArrowRight",shiftKey:true});expect(props.onMove).not.toHaveBeenCalled();await act(async()=>fireEvent.keyDown(target(),{key:"Enter"}));
 const points={0:{x:51,y:50},90:{x:40,y:39},180:{x:29,y:50},270:{x:40,y:61}};expect(props.onMove).toHaveBeenCalledOnce();expect(props.onMove).toHaveBeenCalledWith(note,points[rotation]);expect(props.onOpen).not.toHaveBeenCalled();
});
it("does not move read-only notes or expose hidden/page-mismatched notes",()=>{
 const props=options();props.notes=[{...note,reason:"Locked"},{...note,annotationIndex:3,onPage:false},{...note,annotationIndex:4,pageIndex:1}];const view=render(<svg><NativeCommentLayer {...props}/></svg>);const target=view.getByRole("button");expect(view.getAllByRole("button")).toHaveLength(1);fireEvent.keyDown(target,{key:"ArrowRight"});fireEvent.keyDown(target,{key:"Enter"});expect(props.onMove).not.toHaveBeenCalled();expect(props.onOpen).toHaveBeenCalledWith(props.notes[0]);
});
it("keeps the original note and reports a failed native movement",async()=>{
 const {props,target}=setup();props.onMove.mockRejectedValue(new Error("Cannot preserve popup"));fireEvent.keyDown(target(),{key:"ArrowRight"});await act(async()=>fireEvent.keyDown(target(),{key:"Enter"}));expect(props.onStatus).toHaveBeenLastCalledWith({text:"Cannot preserve popup",error:true});
});

it("retains a native error after the app busy state clears",async()=>{
 const {props,view,target}=setup();let reject!:(error:Error)=>void;props.onMove.mockReturnValue(new Promise((_,fail)=>{reject=fail;}));fireEvent.keyDown(target(),{key:"ArrowRight"});fireEvent.keyDown(target(),{key:"Enter"});
 view.rerender(<svg><NativeCommentLayer {...props} disabled/></svg>);await act(async()=>reject(new Error("Movement refused")));view.rerender(<svg><NativeCommentLayer {...props}/></svg>);expect(props.onStatus).toHaveBeenLastCalledWith({text:"Movement refused",error:true});
});
it("refuses movement when page annotation permissions deny it",()=>{
 const {props,view,target}=setup();view.rerender(<svg><NativeCommentLayer {...props} page={{...page,protection:{documentId:"protected",owner:false,revision:6,permissions:{annotate:false,print:true,printHighQuality:true,copyContent:true,editContent:true,fillForms:true,assemble:true}}}}/></svg>);fireEvent.keyDown(target(),{key:"ArrowRight"});fireEvent.keyDown(target(),{key:"Enter"});expect(props.onMove).not.toHaveBeenCalled();expect(props.onOpen).toHaveBeenCalledWith(note);
});
it("restores keyboard focus after the replacement note list arrives",async()=>{
 const {props,view,target}=setup();fireEvent.keyDown(target(),{key:"ArrowRight"});await act(async()=>fireEvent.keyDown(target(),{key:"Enter"}));view.rerender(<svg><NativeCommentLayer {...props} page={{...page,sourceId:"updated"}} notes={[]}/></svg>);view.rerender(<svg><NativeCommentLayer {...props} page={{...page,sourceId:"updated"}} notes={[{...note,bounds:{...note.bounds,x:41}}]}/></svg>);expect(target()).toHaveFocus();
});

it("uses rendered NoRotate bounds for the target and draft but commits annotation coordinates",async()=>{
 const {props,view,at,target}=setup();const rendered={...note,hitBounds:{x:70,y:50,width:18,height:20}};view.rerender(<svg><NativeCommentLayer {...props} notes={[rendered]}/></svg>);expect(target().querySelector("rect")).toHaveAttribute("x","70");fireEvent.pointerDown(target(),at(75,55));fireEvent.pointerMove(target(),at(85,65));expect(view.container.querySelector(".native-comment-draft")).toHaveAttribute("x","80");await act(async()=>fireEvent.pointerUp(target(),at(85,65)));expect(props.onMove).toHaveBeenCalledWith(rendered,{x:50,y:60});
});
it("bounds both the native rectangle and the displaced visible NoRotate icon",async()=>{
 const {props,view,at,target}=setup();const rendered={...note,hitBounds:{x:70,y:50,width:18,height:20}};view.rerender(<svg><NativeCommentLayer {...props} notes={[rendered]}/></svg>);fireEvent.pointerDown(target(),at(75,55));await act(async()=>fireEvent.pointerUp(target(),at(900,55)));expect(props.onMove).toHaveBeenCalledWith(rendered,{x:252,y:50});
});
it("cancels a draft immediately if annotation permission is revoked",()=>{
 const {props,view,target}=setup();fireEvent.keyDown(target(),{key:"ArrowRight"});view.rerender(<svg><NativeCommentLayer {...props} onMove={undefined}/></svg>);expect(view.container.querySelector(".native-comment-draft")).not.toBeInTheDocument();
});
it("does not steal focus if another control is chosen while movement finishes",async()=>{
 const {props,view,target}=setup();let finish!:(value:string)=>void;props.onMove.mockReturnValue(new Promise(resolve=>{finish=resolve;}));fireEvent.keyDown(target(),{key:"ArrowRight"});fireEvent.keyDown(target(),{key:"Enter"});const outside=document.createElement("button");document.body.append(outside);outside.focus();await act(async()=>finish("updated"));view.rerender(<svg><NativeCommentLayer {...props} page={{...page,sourceId:"updated"}} notes={[{...note}]}/></svg>);expect(outside).toHaveFocus();outside.remove();
});
it("does not restore movement focus after switching tools during replacement loading",async()=>{
 const {props,view,target}=setup();fireEvent.keyDown(target(),{key:"ArrowRight"});await act(async()=>fireEvent.keyDown(target(),{key:"Enter"}));view.rerender(<svg><NativeCommentLayer {...props} enabled={false}/></svg>);view.rerender(<svg><NativeCommentLayer {...props} page={{...page,sourceId:"updated"}} notes={[{...note}]}/></svg>);expect(target()).not.toHaveFocus();
});

it.each([90,180,270] as Rotation[])("moves a final-rotation NoRotate target without changing native coordinate semantics at %s",async rotation=>{
 const {props,view,at,target}=setup({...page,rotation});const rendered={...note,noRotate:true,hitBounds:{x:70,y:50,width:18,height:20}};view.rerender(<svg><NativeCommentLayer nativeRotation {...props} notes={[rendered]}/></svg>);const hit=target().querySelector("rect")!,x=Number(hit.getAttribute("x")),y=Number(hit.getAttribute("y"));fireEvent.pointerDown(target(),at(x+5,y+5));fireEvent.pointerMove(target(),at(x+15,y+15));await act(async()=>fireEvent.pointerUp(target(),at(x+15,y+15)));expect(props.onMove).toHaveBeenCalledWith(rendered,{x:50,y:60});
});
it.each([
 [90, -1000, {x:0,y:18}], [90, 1000, {x:250,y:382}],
 [180, -1000, {x:0,y:20}], [180, 1000, {x:270,y:382}],
 [270, -1000, {x:0,y:0}], [270, 1000, {x:270,y:382}],
] as [Rotation,number,{x:number;y:number}][])("keeps both the native rectangle and upright icon inside page edges at %s toward %s",async(rotation,destination,expected)=>{
 const {props,view,at,target}=setup({...page,rotation});const rendered={...note,noRotate:true,hitBounds:{x:70,y:50,width:18,height:20}};
 view.rerender(<svg><NativeCommentLayer nativeRotation {...props} notes={[rendered]}/></svg>);
 const hit=target().querySelector("rect")!,x=Number(hit.getAttribute("x")),y=Number(hit.getAttribute("y"));
 fireEvent.pointerDown(target(),at(x+5,y+5));await act(async()=>fireEvent.pointerUp(target(),at(destination,destination)));
 expect(props.onMove).toHaveBeenCalledWith(rendered,expected);
});
it("cancels a move draft when the renderer rotation mode changes",()=>{
 const {props,view,target}=setup({...page,rotation:90});const rendered={...note,noRotate:true};
 view.rerender(<svg><NativeCommentLayer nativeRotation {...props} notes={[rendered]}/></svg>);
 fireEvent.keyDown(target(),{key:"ArrowRight"});expect(view.container.querySelector(".native-comment-draft")).toBeInTheDocument();
 view.rerender(<svg><NativeCommentLayer {...props} notes={[rendered]}/></svg>);
 expect(view.container.querySelector(".native-comment-draft")).not.toBeInTheDocument();expect(props.onMove).not.toHaveBeenCalled();
});
