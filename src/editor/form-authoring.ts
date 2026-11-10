import {fromDisplayPoint,pageVisibleRect} from "./geometry";
import {pageAllows} from "./protection";
import {pageReplacementRequest} from "./page-replacement";
import type {AnnotationRect,EditorDocument,PagePlan,Rotation} from "./types";

export type NewFormField={kind:"text";multiline:boolean}|{kind:"checkbox"}|{kind:"dropdown";options:string[]}|{kind:"list";options:string[];multiSelect:boolean}|{kind:"radio";options:string[]};
export interface CreateFormFieldRequest {bounds:AnnotationRect;rotation:Rotation;name:string;label:string;required:boolean;fontSize:number;field:NewFormField}
export function formAuthorSelection(document:EditorDocument,page:PagePlan){
 if(!pageAllows(page,"annotate"))throw new Error("This PDF does not permit creating form fields.");
 return pageReplacementRequest(document,page);
}
export function initialFieldBounds(page:PagePlan):AnnotationRect {
 const r=pageVisibleRect(page);return {x:r.x+Math.min(24,r.width/10),y:r.y+Math.min(24,r.height/10),width:Math.min(180,r.width*0.8),height:Math.min(32,r.height*0.8)};
}
export function fieldSourceBounds(page:PagePlan,r:AnnotationRect):AnnotationRect {
 const a=fromDisplayPoint(r,page.width,page.height,page.rotation),b=fromDisplayPoint({x:r.x+r.width,y:r.y+r.height},page.width,page.height,page.rotation);
 return {x:Math.min(a.x,b.x),y:Math.min(a.y,b.y),width:Math.abs(b.x-a.x),height:Math.abs(b.y-a.y)};
}
const validLabel=(s:string)=>[...s].length<=256&&new TextEncoder().encode(s).length<=1024&&!/[\u0000-\u001f\u007f-\u009f]/u.test(s);
export function validateFieldDraft(page:PagePlan,r:CreateFormFieldRequest):string|null {
 if(!r.name.trim()||r.name.includes(".")||!validLabel(r.name)||!validLabel(r.label))return "Use a field name without dots or control characters. Names and labels may contain up to 256 characters.";
 const b=r.bounds,c=page.crop??{x:0,y:0,width:page.width,height:page.height};
 if(![b.x,b.y,b.width,b.height].every(Number.isFinite)||b.width<4||b.height<4||b.x<c.x||b.y<c.y||b.x+b.width>c.x+c.width+0.001||b.y+b.height>c.y+c.height+0.001)return "Place the entire field inside the visible page, at least 4 pt wide and tall.";
 if(!Number.isFinite(r.fontSize)||r.fontSize<1||r.fontSize>300)return "Font size must be between 1 and 300 pt.";
 if(r.field.kind==="dropdown"||r.field.kind==="list"||r.field.kind==="radio") {
  const options=r.field.options;
  if(r.field.kind==="radio"&&(options.length<2||options.length>32))return "Enter 2–32 radio choices, one per line.";
  if(!options.length||options.length>256||options.some(s=>!s.trim()||!validLabel(s))||new Set(options).size!==options.length||options.reduce((n,s)=>n+new TextEncoder().encode(s).length,0)>32768)return "Enter 1–256 unique nonempty choices, one per line (up to 256 characters each).";
  if(options.some(s=>/[^\u0020-\u007e\u00a0-\u00ff€‚ƒ„…†‡ˆ‰Š‹ŒŽ‘’“”•–—˜™š›œžŸ]/u.test(s)))return "Choice labels must use characters supported by Helvetica (Western European text).";
  if(r.field.kind==="radio") {
   const turned=r.rotation===90||r.rotation===270,height=turned?b.width:b.height,width=turned?b.height:b.width;
   if(height/options.length+0.001<Math.max(16,r.fontSize*1.2+4))return "Make the radio group taller or reduce the font size so each choice fits.";
   if(width<20)return "Make the radio group wider to fit its buttons and labels.";
  }
 }
 return null;
}
