import {expect,it} from "vitest";
import {workspaceImageIds} from "../src/editor/use-image-resources";
import {createSession} from "../src/editor/workspace";
import {createDemoAdapter} from "../src/editor/adapter";
import type {EditorDocument} from "../src/editor/types";
it("retains images from every native tab's undo, present and redo states",()=>{
 const session=createSession({id:"source",name:"Images",pages:[{width:200,height:300}]},{...createDemoAdapter(),kind:"native"},100);
 const state=(id:string):EditorDocument=>({...session.history.present,pages:session.history.present.pages.map(page=>({...page,overlays:[{type:"image",id,assetId:id.repeat(64),x:10,y:20,width:30,height:20,rotation:0}]}))});
 session.history={past:[state("a")],present:state("b"),future:[state("c")]};
 const other={...session,id:"other",history:{past:[],present:state("d"),future:[]}};
 const demo={...session,id:"demo",adapter:createDemoAdapter(),history:{past:[],present:state("e"),future:[]}};
 expect(workspaceImageIds({activeId:session.id,tabs:[session,other,demo]})).toEqual(new Set(["a","b","c","d"].map(value=>value.repeat(64))));
});
