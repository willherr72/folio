import {expect,it} from "vitest";
import {commitPageReplacement,pageReplacementRequest,replacementPages} from "../src/editor/page-replacement";
import {createSession} from "../src/editor/workspace";
import {nativeAdapter} from "../src/editor/adapter";
import {redo,undo} from "../src/editor/model";

const info={id:"source",name:"Source.pdf",pages:[{width:300,height:400},{width:300,height:400}]};
function setup(){
 const session=createSession(info,nativeAdapter,1),base=session.history.present;
 const first={...base.pages[0],rotation:90 as const,crop:{x:10,y:20,width:200,height:300},overlays:[{type:"comment" as const,id:"note",x:30,y:40,text:"Retain",color:"#123456"}]};
 const document={...base,pages:[first,{...first,id:"other",sourceId:"other"},base.pages[1],{...first,id:"duplicate"}],bookmarks:[{id:"b",title:"Retained",level:0,destination:{kind:"page" as const,value:first.id}}]};
 return {session:{...session,sourceIds:["source","other"],history:{...session.history,present:document}},document,first};
}
it("maps only source occurrences and commits a selected duplicate in one reversible step",()=>{
 const {session,document,first}=setup();
 expect(pageReplacementRequest(document,first)).toEqual({sourceId:"source",occurrences:[0,1,0],selectedOccurrence:0});
 const result={document:{...info,id:"candidate",pages:[...info.pages,info.pages[0]]},pageIndices:[0,1,2]};
 const next=commitPageReplacement(session,document,first,result);
 expect(next.history.present.pages.map(p=>[p.sourceId,p.pageIndex])).toEqual([["candidate",0],["other",0],["candidate",1],["candidate",2]]);
 expect(next.history.present.pages[1]).toBe(document.pages[1]);
 expect(next.history.present.pages[0]).toEqual({...first,sourceId:"candidate"});
 expect(next.history.present.bookmarks).toBe(document.bookmarks);
 expect(next.sourceIds).toEqual(["source","other","candidate"]);
 expect(undo(next.history).present).toBe(document);
 expect(redo(undo(next.history)).present).toBe(next.history.present);
});
it("uses authoritative workspace overlays and preserves current bookmark edits",()=>{
 const {document,first}=setup();const foreign={type:"comment" as const,id:"imported",x:0,y:0,text:"Old overlay",color:"#000000"};
 const result={document:{...info,id:"candidate",pages:[...info.pages,{...info.pages[0],overlays:[foreign]}]},pageIndices:[0,1,2]};
 const pages=replacementPages(document,document.pages[3],{...result,pageIndices:[0,1,2]});
 expect(pages[3].overlays).toBe(first.overlays);expect(pages[3].crop).toBe(first.crop);
});
it("rejects stale state, reused candidates and malformed or conflated mappings",()=>{
 const {session,document,first}=setup();const result={document:{...info,id:"candidate",pages:[...info.pages,info.pages[0]]},pageIndices:[0,1,2]};
 expect(()=>commitPageReplacement(session,{...document,pages:[...document.pages]},first,result)).toThrow(/changed/);
 expect(()=>commitPageReplacement(session,document,first,{...result,document:{...result.document,id:"other"}})).toThrow(/new source/);
 for(const pageIndices of [[0,1],[0,1,0],[0,0,2],[0,1,2.5],[0,1,-1],[0,1,99]])expect(()=>replacementPages(document,first,{...result,pageIndices})).toThrow();
 expect(()=>replacementPages(document,first,{...result,document:{...result.document,pages:[...info.pages,{width:301,height:400}]}})).toThrow(/dimensions/);
 expect(()=>pageReplacementRequest(document,{...first})).toThrow(/changed/);
});
