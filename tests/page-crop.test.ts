import {describe,it,expect} from "vitest";
import {clientPointToPage,pageVisibleRect} from "../src/editor/geometry";
import {cropFromMargins,cropMargins,cropPage} from "../src/editor/page-crop";
import {commit,createHistory,duplicatePage,planDigest,redo,undo} from "../src/editor/model";
import type {EditorDocument,PagePlan,Rotation} from "../src/editor/types";

const page:PagePlan={id:"a",sourceId:"source",pageIndex:0,width:300,height:400,rotation:0,overlays:[]};
const crop={x:20,y:30,width:100,height:150};
describe("page cropping",()=>{
  it.each([0,90,180,270] as Rotation[])("roundtrips displayed margins and pointer locations at rotation %s",rotation=>{
    const p={...page,rotation,crop};const visible=pageVisibleRect(p);
    expect(visible).toEqual(({0:{x:20,y:30,width:100,height:150},90:{x:220,y:20,width:150,height:100},180:{x:180,y:220,width:100,height:150},270:{x:30,y:180,width:150,height:100}})[rotation]);
    expect(cropFromMargins(p,cropMargins(p))).toEqual(crop);
    const center=clientPointToPage(60,95,{left:10,top:20,width:100,height:150},p.width,p.height,rotation,crop);
    expect(center).toEqual({x:70,y:105});
  });
  it("rejects inverted, nonfinite and sub-point margin results",()=>{
    for(const right of [300,301,299.5,Infinity,NaN,-1])expect(cropFromMargins(page,{left:0,top:0,right,bottom:0})).toBeNull();
    expect(cropFromMargins(page,{left:0,top:0,right:299,bottom:399})).toEqual({x:0,y:0,width:1,height:1});
  });
  it("commits once, preserves full source geometry and duplicates independently",()=>{
    const doc:EditorDocument={name:"Test",pages:[page],selectedPageId:"a",selectedOverlayId:"hidden"};
    const initial=createHistory(doc),changed=commit(initial,d=>cropPage(d,"a",crop));
    expect(changed.past).toHaveLength(1);expect(changed.present.pages[0]).toMatchObject({width:300,height:400,crop,sourceId:"source"});
    expect(changed.present.selectedOverlayId).toBeNull();expect(planDigest(changed.present)).not.toBe(planDigest(doc));
    expect(commit(changed,d=>cropPage(d,"a",{...crop}))).toBe(changed);
    expect(undo(changed).present).toBe(doc);expect(redo(undo(changed)).present).toBe(changed.present);
    const duplicated=duplicatePage(changed.present,"a","b"),reset=cropPage(duplicated,"b");
    expect(reset.pages[0].crop).toEqual(crop);expect(reset.pages[1].crop).toBeUndefined();
    expect(cropPage(doc,"a",{...crop,x:-1})).toBe(doc);
  });
  it("preserves crop through an immutable text-source replacement",async()=>{
    const {commitTextReplacement}=await import("../src/editor/text-edit");
    const {createSession}=await import("../src/editor/workspace");
    const {createDemoAdapter}=await import("../src/editor/adapter");
    const session=createSession({id:"source",name:"Test",pages:[{width:300,height:400}]},createDemoAdapter(),100);
    session.history.present.pages[0].crop=crop;const original=session.history.present.pages[0];
    const result=commitTextReplacement(session,original,{id:"edited",name:"Test",pages:[{width:300,height:400}]});
    expect(result.history.present.pages[0]).toMatchObject({sourceId:"edited",crop,width:300,height:400});
    expect(undo(result.history).present.pages[0]).toBe(original);
  });
});
