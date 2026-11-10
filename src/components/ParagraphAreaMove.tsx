import {useEffect,useLayoutEffect,useRef,useState,type PointerEvent} from 'react';
import type {AnnotationRect,InkPoint} from '../editor/types';
import {moveParagraphBounds,validParagraphBounds} from '../editor/paragraph-preview';

interface Props {
 bounds:AnnotationRect;pageWidth:number;pageHeight:number;
 onChange(bounds:AnnotationRect):void;onDraggingChange(dragging:boolean):void;
}
type Gesture={pointer:number;target:SVGRectElement;start:InkPoint;bounds:AnnotationRect};
export function ParagraphAreaMove(props:Props){
 const latest=useRef(props);latest.current=props;
 const gesture=useRef<Gesture|null>(null),[draft,setDraft]=useState<AnnotationRect|null>(null);
 const end=(render=true)=>{
  const active=gesture.current;if(!active)return;
  gesture.current=null;if(render)setDraft(null);
  try{active.target.releasePointerCapture(active.pointer);}catch{/* Capture may already be lost. */}
  latest.current.onDraggingChange(false);
 };
 useLayoutEffect(()=>{end();},[props.bounds.x,props.bounds.y,props.bounds.width,props.bounds.height,props.pageWidth,props.pageHeight]);
 useEffect(()=>{
  const escape=(event:KeyboardEvent)=>{if(event.key==='Escape'&&gesture.current){event.preventDefault();event.stopPropagation();end();}};
  window.addEventListener('keydown',escape,true);
  return()=>{window.removeEventListener('keydown',escape,true);end(false);};
 },[]);
 const point=(event:PointerEvent<SVGRectElement>):InkPoint|null=>{
  const svg=event.currentTarget.ownerSVGElement;
  try{
   const matrix=svg?.getScreenCTM();if(!svg||!matrix)return null;
   const point=svg.createSVGPoint();point.x=event.clientX;point.y=event.clientY;
   const result=point.matrixTransform(matrix.inverse());return [result.x,result.y].every(Number.isFinite)?{x:result.x,y:result.y}:null;
  }catch{return null;}
 };
 const moved=(event:PointerEvent<SVGRectElement>)=>{
  const active=gesture.current,position=point(event);if(!active||!position)return null;
  // Keep pointer-derived fields readable while retaining exact clamping at page edges.
  const x=active.bounds.x+Math.round((position.x-active.start.x)*100)/100;
  const y=active.bounds.y+Math.round((position.y-active.start.y)*100)/100;
  return moveParagraphBounds(active.bounds,x,y,latest.current.pageWidth,latest.current.pageHeight);
 };
 return <rect {...(draft??props.bounds)} className={`paragraph-area-target paragraph-area-move${draft?' dragging':''}`} aria-hidden="true"
  onPointerDown={event=>{
   if(event.button!==0||gesture.current||!validParagraphBounds(props.bounds,props.pageWidth,props.pageHeight))return;
   const start=point(event);if(!start)return;event.preventDefault();event.stopPropagation();
   try{event.currentTarget.setPointerCapture(event.pointerId);}catch{return;}
   gesture.current={pointer:event.pointerId,target:event.currentTarget,start,bounds:{...props.bounds}};
   setDraft(props.bounds);props.onDraggingChange(true);
  }}
  onPointerMove={event=>{if(gesture.current?.pointer===event.pointerId)setDraft(moved(event));}}
  onPointerUp={event=>{
   const active=gesture.current;if(!active||active.pointer!==event.pointerId)return;
   const result=moved(event);end();
   if(result&&(result.x!==active.bounds.x||result.y!==active.bounds.y))latest.current.onChange(result);
  }}
  onPointerCancel={event=>{if(gesture.current?.pointer===event.pointerId)end();}}
  onLostPointerCapture={event=>{if(gesture.current?.pointer===event.pointerId)end();}}/>
}
