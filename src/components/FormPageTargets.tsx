import {displayDimensions,toDisplayPoint,pageVisibleRect} from "../editor/geometry";
import type {AnnotationRect,FormField,PagePlan} from "../editor/types";

export function formTargetBounds(field:FormField,page:PagePlan):AnnotationRect|null {
 const box=field.bounds;
 if(field.visible!==true||!box||![box.x,box.y,box.width,box.height].every(Number.isFinite)||box.width<=0||box.height<=0)return null;
 const size=displayDimensions(page.width,page.height,page.rotation);
 const anchor=toDisplayPoint(box,page.width,page.height,page.rotation);
 const opposite=field.noRotate?{x:anchor.x+box.width,y:anchor.y+box.height}:toDisplayPoint({x:box.x+box.width,y:box.y+box.height},page.width,page.height,page.rotation);
 const x=Math.max(0,Math.min(anchor.x,opposite.x)),y=Math.max(0,Math.min(anchor.y,opposite.y));
 const right=Math.min(size.width,Math.max(anchor.x,opposite.x)),bottom=Math.min(size.height,Math.max(anchor.y,opposite.y));
 return right>x&&bottom>y?{x,y,width:right-x,height:bottom-y}:null;
}
export function formCanvasTargetBounds(field:FormField,page:PagePlan):AnnotationRect|null {
 const box=formTargetBounds(field,page);if(!box)return null;
 const visible=pageVisibleRect(page),x=Math.max(box.x,visible.x),y=Math.max(box.y,visible.y);
 const right=Math.min(box.x+box.width,visible.x+visible.width),bottom=Math.min(box.y+box.height,visible.y+visible.height);
 return right>x&&bottom>y?{x,y,width:right-x,height:bottom-y}:null;
}
export function FormPageTargets({page,fields,selected,disabled,dirty,onSelect,embedded=false,clipToCrop=false,selectedTargetRef}:{page:PagePlan;fields:FormField[];selected:number;disabled:boolean;dirty:boolean;onSelect(index:number):void;embedded?:boolean;clipToCrop?:boolean;selectedTargetRef?(node:SVGRectElement|null):void}) {
 const size=displayDimensions(page.width,page.height,page.rotation);
 const targets=<g role={embedded?"group":undefined} aria-label={embedded?"Interactive form fields":undefined} onPointerDown={event=>event.stopPropagation()}>
  {fields.map((field,index)=>{
   const box=clipToCrop?formCanvasTargetBounds(field,page):formTargetBounds(field,page);if(!box)return null;
   const blocked=disabled||(dirty&&index!==selected),label=(field.label||field.name||"Unnamed field")+(field.kind==="radio"?` — ${field.options[0]||`Option ${index+1}`}`:"");
   const choose=()=>{if(!blocked)onSelect(index);};
   return <rect key={field.annotationIndex} ref={index===selected?selectedTargetRef:undefined} {...box} className={`form-page-target${field.reason?" unavailable":""}`} role="button" tabIndex={blocked?-1:0}
    aria-label={`Select form field: ${label}`} aria-pressed={selected===index} aria-disabled={blocked}
    onClick={choose} onKeyDown={event=>{if(event.key==="Enter"||event.key===" "){event.preventDefault();choose();}}}>
    <title>{label}{field.reason?`: ${field.reason}`:""}</title>
   </rect>;
  })}
 </g>;
 return embedded?targets:<svg className="form-page-targets" viewBox={`0 0 ${size.width} ${size.height}`} role="group" aria-label="Interactive form fields">{targets}</svg>;
}
