import type {FolioAdapter} from './adapter';
import type {FormField,PagePlan} from './types';
export interface FormAnchorIdentity{adapter:FolioAdapter;pageId:string;sourceId:string;pageIndex:number;annotationIndex:number;layoutKey:string}
export interface FormCanvasAnchor extends FormAnchorIdentity{element:SVGRectElement}
export type FormAnchorCallback=(identity:FormAnchorIdentity,element:SVGRectElement|null,previousElement:SVGRectElement|null)=>void;
export interface ScreenBox{left:number;top:number;right:number;bottom:number}
export function formAnchorIdentity(adapter:FolioAdapter,page:PagePlan,field:FormField):FormAnchorIdentity{return {adapter,pageId:page.id,sourceId:page.sourceId,pageIndex:page.pageIndex,annotationIndex:field.annotationIndex,layoutKey:JSON.stringify([page.width,page.height,page.rotation,page.crop,field.bounds,field.noRotate])};}
function same(a:FormAnchorIdentity,b:FormAnchorIdentity){return a.adapter===b.adapter&&a.pageId===b.pageId&&a.sourceId===b.sourceId&&a.pageIndex===b.pageIndex&&a.annotationIndex===b.annotationIndex&&a.layoutKey===b.layoutKey;}
export function updateFormAnchor(current:FormCanvasAnchor|null,identity:FormAnchorIdentity,element:SVGRectElement|null,previousElement:SVGRectElement|null=null):FormCanvasAnchor|null{
 if(!element)return current&&same(current,identity)&&current.element===previousElement?null:current;
 return current&&same(current,identity)&&current.element===element?current:{...identity,element};
}
export function matchesFormAnchor(anchor:FormCanvasAnchor|null,adapter:FolioAdapter,page:PagePlan,field:FormField):boolean{return !!anchor&&same(anchor,formAnchorIdentity(adapter,page,field));}
export function placeFormEntry(anchor:ScreenBox,clip:ScreenBox,window:{width:number;height:number},height:number):{left:number;top:number;width:number;maxHeight:number}|null{
 if(![anchor.left,anchor.top,anchor.right,anchor.bottom,clip.left,clip.top,clip.right,clip.bottom,window.width,window.height,height].every(Number.isFinite)||window.width<=24||window.height<=24||height<=0)return null;
 const left=Math.max(anchor.left,clip.left,0),top=Math.max(anchor.top,clip.top,0),right=Math.min(anchor.right,clip.right,window.width),bottom=Math.min(anchor.bottom,clip.bottom,window.height);
 if(right<=left||bottom<=top)return null;
 const width=Math.min(320,window.width-24),maxHeight=window.height-24,actualHeight=Math.min(height,maxHeight);
 const below=bottom+8,above=top-actualHeight-8;
 return {left:Math.max(12,Math.min(left,window.width-width-12)),top:below+actualHeight<=window.height-12?below:above>=12?above:Math.max(12,Math.min(top,window.height-actualHeight-12)),width,maxHeight};
}
