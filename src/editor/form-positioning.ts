import {pageVisibleRect,toDisplayPoint} from './geometry';
import {fieldSourceBounds,formAuthorSelection} from './form-authoring';
import type {AnnotationRect,EditorDocument,FormField,FormFieldValue,PagePlan} from './types';
export interface PositionFormFieldRequest {annotationIndex:number;expectedBounds:AnnotationRect;expectedValue:FormFieldValue;bounds:AnnotationRect;reflowText?:boolean}
export function positionedFieldBounds(page:PagePlan,field:FormField):AnnotationRect {
 if(!field.bounds||field.noRotate||field.visible===false||!['text','checkbox','radio','combo','list'].includes(field.kind))throw new Error('This field does not support positioning.');
 const r=field.bounds,a=toDisplayPoint(r,page.width,page.height,page.rotation),b=toDisplayPoint({x:r.x+r.width,y:r.y+r.height},page.width,page.height,page.rotation);
 return {x:Math.min(a.x,b.x),y:Math.min(a.y,b.y),width:Math.abs(a.x-b.x),height:Math.abs(a.y-b.y)};
}
export function fieldPositionRequest(document:EditorDocument,page:PagePlan,field:FormField,rect:AnnotationRect,reflowText=false):PositionFormFieldRequest {
 formAuthorSelection(document,page);positionedFieldBounds(page,field);
 const v=pageVisibleRect(page),r=rect;
 if(![r.x,r.y,r.width,r.height].every(Number.isFinite)||r.width<4||r.height<4||r.x<v.x||r.y<v.y||r.x+r.width>v.x+v.width+0.001||r.y+r.height>v.y+v.height+0.001)throw new Error('Place the entire field inside the visible page, at least 4 pt wide and tall.');
 return {annotationIndex:field.annotationIndex,expectedBounds:field.bounds!,expectedValue:field.value,bounds:fieldSourceBounds(page,r),...(reflowText?{reflowText:true}:{})};
}
