import {expect,it} from 'vitest';
import {nativeAdapter} from '../src/editor/adapter';
import {formAnchorIdentity,matchesFormAnchor,placeFormEntry,updateFormAnchor} from '../src/editor/form-canvas-entry';
import type {FormField,PagePlan} from '../src/editor/types';
const page:PagePlan={id:'p',sourceId:'s',pageIndex:0,width:300,height:400,rotation:0,overlays:[]};
const field={annotationIndex:2,bounds:{x:20,y:30,width:150,height:20}} as FormField;
const clip={left:100,top:100,right:700,bottom:600},size={width:800,height:700};
it('places below a visible field and clamps at right and lower window edges',()=>{
 expect(placeFormEntry({left:200,top:200,right:400,bottom:230},clip,size,180)).toEqual({left:200,top:238,width:320,maxHeight:676});
 expect(placeFormEntry({left:650,top:550,right:750,bottom:620},clip,size,180)).toEqual({left:468,top:362,width:320,maxHeight:676});
});
it('clips partially visible targets and refuses fully hidden or malformed geometry',()=>{
 expect(placeFormEntry({left:60,top:70,right:150,bottom:130},clip,size,180)).toMatchObject({left:100,top:138});
 for(const box of [{left:0,top:0,right:90,bottom:90},{left:200,top:800,right:400,bottom:900},{left:NaN,top:100,right:200,bottom:150}])expect(placeFormEntry(box,clip,size,180)).toBeNull();
});
it('keeps tall controls within a narrow window with internal scrolling',()=>{
 expect(placeFormEntry({left:20,top:50,right:60,bottom:70},{left:0,top:0,right:220,bottom:180},{width:220,height:180},600)).toEqual({left:12,top:12,width:196,maxHeight:156});
});
it('binds an anchor to exact source, occurrence, page, adapter and field identity',()=>{
 const element=document.createElementNS('http://www.w3.org/2000/svg','rect');const id=formAnchorIdentity(nativeAdapter,page,field);
 const anchor=updateFormAnchor(null,id,element);expect(anchor?.element).toBe(element);expect(matchesFormAnchor(anchor,nativeAdapter,page,field)).toBe(true);
 for(const other of [{...page,id:'duplicate'},{...page,sourceId:'new'},{...page,pageIndex:1}])expect(matchesFormAnchor(anchor,nativeAdapter,other,field)).toBe(false);
 expect(matchesFormAnchor(anchor,{...nativeAdapter},page,field)).toBe(false);expect(matchesFormAnchor(anchor,nativeAdapter,page,{...field,annotationIndex:9})).toBe(false);
});
it('ignores old ref cleanup and remeasures geometry changes without changing ownership',()=>{
 const element=document.createElementNS('http://www.w3.org/2000/svg','rect');const old=formAnchorIdentity(nativeAdapter,page,field),next=formAnchorIdentity(nativeAdapter,{...page,rotation:90},field);
 const anchor=updateFormAnchor(null,next,element);expect(updateFormAnchor(anchor,old,null)).toBe(anchor);expect(updateFormAnchor(anchor,next,element)).toBe(anchor);expect(updateFormAnchor(anchor,next,null,element)).toBeNull();
 expect(matchesFormAnchor(anchor,nativeAdapter,{...page,rotation:90},field)).toBe(true);expect(matchesFormAnchor(anchor,nativeAdapter,page,field)).toBe(false);
});
it('does not clear a newer DOM target when an old mount has the same field identity',()=>{
 const old=document.createElementNS('http://www.w3.org/2000/svg','rect'),next=document.createElementNS('http://www.w3.org/2000/svg','rect');const identity=formAnchorIdentity(nativeAdapter,page,field);
 const anchor=updateFormAnchor(null,identity,next);expect(updateFormAnchor(anchor,identity,null,old)).toBe(anchor);
});
