import {useLayoutEffect,useRef,useState,type PointerEvent} from "react";
import {screenArrowDelta} from "../editor/geometry";
import {visibleImagePolygon} from "../editor/existing-images";
import {frameMeasurements,imagePointerPoint,moveImageFrame,resizeImageFrame,rotateImageFrame,type CanvasImageView,type ImageQuad} from "../editor/existing-image-canvas";
import type {ImagePoint,PagePlan} from "../editor/types";
import "./existing-images.css";
type Kind="move"|"rotate"|0|1|2|3;
type Gesture={pointer:number;kind:Kind;start:ImagePoint;frame:ImageQuad;moved:boolean};
export function ExistingImageControls({view,page,zoom}:{view:CanvasImageView;page:PagePlan;zoom:number}){
 const root=useRef<SVGGElement>(null),gesture=useRef<Gesture|null>(null),latest=useRef({view,page,zoom});latest.current={view,page,zoom};
 const [draft,setDraft]=useState<ImageQuad|null>(null);
 const release=(pointer:number)=>{try{root.current?.releasePointerCapture(pointer);}catch{/* The pointer can already have been released by the host. */}};
 const cancel=()=>{const active=gesture.current;gesture.current=null;setDraft(null);if(active){release(active.pointer);latest.current.view.onGestureActive(false);}};
 const signature=JSON.stringify(view.frame);
 useLayoutEffect(cancel,[view.sessionKey,view.enabled,signature,page.sourceId,page.pageIndex,page.width,page.height,page.rotation,page.crop?.x,page.crop?.y,page.crop?.width,page.crop?.height,zoom]);
 useLayoutEffect(()=>{
  const escape=(event:KeyboardEvent)=>{if(event.key==="Escape"&&gesture.current){event.preventDefault();event.stopImmediatePropagation();cancel();}};
  window.addEventListener("keydown",escape,true);
  return()=>{window.removeEventListener("keydown",escape,true);const active=gesture.current;gesture.current=null;if(active){release(active.pointer);latest.current.view.onGestureActive(false);}};
 },[]);
 const point=(event:PointerEvent)=>{const rect=root.current?.ownerSVGElement?.getBoundingClientRect();return rect?imagePointerPoint(event.clientX,event.clientY,rect,latest.current.page):null;};
 const change=(active:Gesture,p:ImagePoint,snap:boolean):ImageQuad=>{
  const {view,page,zoom}=latest.current,delta={x:p.x-active.start.x,y:p.y-active.start.y};
  active.moved ||= Math.hypot(delta.x,delta.y)*zoom/100>=3;
  if(!active.moved)return active.frame;
  return active.kind==="move"?moveImageFrame(view.image.corners,active.frame,delta):active.kind==="rotate"?rotateImageFrame(active.frame,active.start,p,page,snap):resizeImageFrame(view.image.corners,active.frame,active.kind,{x:active.frame[active.kind].x+delta.x,y:active.frame[active.kind].y+delta.y},view.locked);
 };
 const begin=(event:PointerEvent,kind:Kind)=>{
  if(!latest.current.view.enabled||gesture.current||event.button!==0||event.isPrimary===false)return;
  const p=point(event);if(!p)return;event.preventDefault();event.stopPropagation();event.currentTarget instanceof SVGElement&&event.currentTarget.focus({preventScroll:true});
  gesture.current={pointer:event.pointerId,kind,start:p,frame:latest.current.view.frame,moved:false};setDraft(latest.current.view.frame);latest.current.view.onGestureActive(true);
  try{root.current?.setPointerCapture(event.pointerId);}catch{/* Synthetic tests may not own a pointer. */}
 };
 const finish=(event:PointerEvent,cancelled=false)=>{
  const active=gesture.current;if(!active||event.pointerId!==active.pointer)return;
  event.preventDefault();event.stopPropagation();const p=cancelled?null:point(event),next=p?change(active,p,event.shiftKey):active.frame;
  cancel();if(!cancelled&&p&&latest.current.view.enabled&&next!==active.frame)latest.current.view.onChange(frameMeasurements(next,latest.current.page));
 };
 const keyboard=(event:React.KeyboardEvent,kind:Kind)=>{
  if(!view.enabled||gesture.current||event.ctrlKey||event.metaKey||event.altKey)return;
  let next:ImageQuad|undefined;
  if(kind==="rotate"){
   if(event.key!=="ArrowLeft"&&event.key!=="ArrowRight")return;
   const c={x:(view.frame[0].x+view.frame[2].x)/2,y:(view.frame[0].y+view.frame[2].y)/2},angle=(event.key==="ArrowRight"?1:-1)*(event.shiftKey?15:1)*Math.PI/180;
   next=rotateImageFrame(view.frame,{x:c.x+100,y:c.y},{x:c.x+100*Math.cos(angle),y:c.y+100*Math.sin(angle)},page,false);
  }else{
   const delta=screenArrowDelta(event.key,event.shiftKey,page.rotation);if(!delta)return;
   next=kind==="move"?moveImageFrame(view.image.corners,view.frame,delta):resizeImageFrame(view.image.corners,view.frame,kind,{x:view.frame[kind].x+delta.x,y:view.frame[kind].y+delta.y},view.locked);
  }
  event.preventDefault();event.stopPropagation();if(next!==view.frame)view.onChange(frameMeasurements(next,page));
 };
 const q=draft??view.frame,unit=100/zoom,visible=visibleImagePolygon({...view.image,corners:q},page),crop=page.crop??{x:0,y:0,width:page.width,height:page.height};
 const handleVisible=(p:ImagePoint)=>p.x>=crop.x&&p.x<=crop.x+crop.width&&p.y>=crop.y&&p.y<=crop.y+crop.height&&p.x>=0&&p.y>=0&&p.x<=page.width&&p.y<=page.height;
 const c={x:(q[0].x+q[2].x)/2,y:(q[0].y+q[2].y)/2},top={x:(q[2].x+q[3].x)/2,y:(q[2].y+q[3].y)/2},axis={x:q[1].x-q[0].x,y:q[1].y-q[0].y},length=Math.hypot(axis.x,axis.y);
 const normal={x:-axis.y/length,y:axis.x/length},sign=normal.x*(top.x-c.x)+normal.y*(top.y-c.y)>=0?1:-1;
 const rotation={x:top.x+normal.x*sign*28*unit,y:top.y+normal.y*sign*28*unit};
 return <g ref={root} data-existing-image-controls="true" className="existing-image-controls"
  onPointerMove={event=>{const active=gesture.current;if(!active||event.pointerId!==active.pointer)return;event.preventDefault();event.stopPropagation();const p=point(event);if(p)setDraft(change(active,p,event.shiftKey));}}
  onPointerUp={event=>finish(event)} onPointerCancel={event=>finish(event,true)} onLostPointerCapture={event=>finish(event,true)}>
  {visible.length>=3&&<polygon data-image-draft={draft?"true":undefined} className={`existing-image-frame${draft?" dragging":""}`} points={visible.map(p=>`${p.x},${p.y}`).join(" ")} role="button" tabIndex={view.enabled?0:-1} aria-disabled={!view.enabled||undefined} aria-label="Move selected image" onPointerDown={event=>begin(event,"move")} onKeyDown={event=>keyboard(event,"move")}><title>Drag to move · Arrows move · Shift for larger steps · Escape cancels</title></polygon>}
  {view.enabled&&([0,1,2,3] as const).filter(i=>handleVisible(q[i])).map(i=><g key={i} role="button" tabIndex={0} aria-label={`Resize image corner ${i+1}`} className="existing-image-corner" onPointerDown={event=>begin(event,i)} onKeyDown={event=>keyboard(event,i)}>
   <title>Drag or use arrows to resize · Opposite corner stays fixed</title><rect className="existing-image-handle-target" x={q[i].x-12*unit} y={q[i].y-12*unit} width={24*unit} height={24*unit}/><rect className="existing-image-handle-mark" x={q[i].x-4*unit} y={q[i].y-4*unit} width={8*unit} height={8*unit}/>
  </g>)}
  {view.enabled&&handleVisible(rotation)&&<g role="button" tabIndex={0} aria-label="Rotate selected image" className="existing-image-rotation" onPointerDown={event=>begin(event,"rotate")} onKeyDown={event=>keyboard(event,"rotate")}>
   <title>Drag to rotate · Shift snaps to 15° · Left/Right rotates</title><line x1={top.x} y1={top.y} x2={rotation.x} y2={rotation.y}/><circle className="existing-image-handle-target" cx={rotation.x} cy={rotation.y} r={12*unit}/><circle className="existing-image-handle-mark" cx={rotation.x} cy={rotation.y} r={5*unit}/>
  </g>}
 </g>;
}
