import {expect,it} from "vitest";
import {commitNativeCommentReplacement,displayNativeCommentBounds,nativeCommentPosition} from "../src/editor/native-comments";
import {createSession} from "../src/editor/workspace";
import {nativeAdapter} from "../src/editor/adapter";
import {undo,redo} from "../src/editor/model";
const info={id:"source",name:"Comments.pdf",pages:[{width:300,height:400},{width:300,height:400}]};
it("replaces all shared-source pages while preserving IDs, duplicates, overlays and bookmarks through undo",()=>{
 let session=createSession(info,nativeAdapter,1);const original=session.history.present;
 const first={...original.pages[0],rotation:90 as const,overlays:[{id:"note",type:"comment" as const,x:20,y:20,text:"Keep",color:"#123456"}]};
 const document={...original,pages:[original.pages[1],first,{...first,id:"duplicate"},{...first,id:"other",sourceId:"other"}],bookmarks:[{id:"b",title:"Target",level:0,destination:{kind:"array" as const,value:[{kind:"page" as const,value:first.id}]}}]};
 session={...session,sourceIds:["source","other"],history:{...session.history,present:document}};
 const next=commitNativeCommentReplacement(session,document,"source",{...info,id:"filled"});
 expect(next.history.present.pages.map(p=>p.sourceId)).toEqual(["filled","filled","filled","other"]);
 expect(next.history.present.pages.map(p=>p.pageIndex)).toEqual([1,0,0,0]);
 expect(next.history.present.pages[1]).toEqual({...first,sourceId:"filled"});
 expect(next.history.present.bookmarks).toBe(document.bookmarks);
 expect(next.sourceIds).toEqual(["source","other","filled"]);
 expect(undo(next.history).present).toBe(document);expect(redo(undo(next.history)).present).toBe(next.history.present);
});
it("rejects stale versions, reused source IDs, missing pages and changed geometry",()=>{
 const session=createSession(info,nativeAdapter,1),doc=session.history.present;
 expect(()=>commitNativeCommentReplacement(session,{...doc,pages:[...doc.pages]},"source",{...info,id:"new"})).toThrow(/changed/);
 for(const replacement of [info,{...info,id:"new",pages:[info.pages[0]]},{...info,id:"new",pages:[{width:301,height:400},info.pages[1]]}]){
  expect(()=>commitNativeCommentReplacement(session,doc,"source",replacement)).toThrow();
 }
});

it("round trips comment positions for every workspace rotation",()=>{
 const session=createSession(info,nativeAdapter,1),bounds={x:30,y:40,width:18,height:26};
 for(const rotation of [0,90,180,270] as const){const page={...session.history.present.pages[0],rotation};expect(nativeCommentPosition(displayNativeCommentBounds(bounds,page),page)).toEqual({x:30,y:40});}
});
