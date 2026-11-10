import {expect,it} from "vitest";
import {importBookmarks,bookmarkPage,filterBookmarks,addBookmark,editBookmark,removeBookmark,moveBookmark} from "../src/editor/bookmarks";
import {createHistory,commit,undo,redo,movePage,deletePage,duplicatePage,planDigest} from "../src/editor/model";
import {createSession,emptyWorkspace,addSession} from "../src/editor/workspace";
import {snapshotWorkspace,restoreWorkspace} from "../src/editor/recovery";
import {commitTextReplacement} from "../src/editor/text-edit";
import {createDemoAdapter} from "../src/editor/adapter";
import type {DocumentBookmark,EditorDocument,PdfValue} from "../src/editor/types";
const destination=(page:string):PdfValue=>({kind:"array",value:[{kind:"page",value:page},{kind:"name",value:[70,105,116]}]});
const imported:DocumentBookmark[]=[{id:"a",title:"Chapter",level:0,destination:destination("1"),bold:true},{id:"b",title:"Section",level:1,destination:destination("2")},{id:"c",title:"Other",level:0,destination:destination("0")}];
const adapter=createDemoAdapter();
const info={id:"s",name:"Book.pdf",pages:Array.from({length:3},()=>({width:612,height:792})),bookmarks:imported};
it("maps imported page indices to stable IDs and keeps targets through reordering and source replacement",()=>{
 const session=createSession(info,adapter,100);const doc=session.history.present;
 expect(bookmarkPage(doc.bookmarks![0])).toBe(doc.pages[1].id);
 const moved=movePage(doc,doc.pages[1].id,0);expect(bookmarkPage(moved.bookmarks![0])).toBe(moved.pages[0].id);
 const replaced=commitTextReplacement(session,doc.pages[1],{id:"replacement",name:"Part.pdf",pages:[{width:612,height:792}]});
 expect(replaced.history.present.bookmarks).toEqual(doc.bookmarks);expect(replaced.history.present.pages[1].sourceId).toBe("replacement");
 const duplicate=duplicatePage(doc,doc.pages[1].id,"copy");expect(bookmarkPage(duplicate.bookmarks![0])).toBe(doc.pages[1].id);
});
it("deletion and extraction retain ancestors as headings, remove absent targets, and undo restores them",()=>{
 const doc=createSession(info,adapter,100).history.present;const history=commit(createHistory(doc),d=>deletePage(d,d.pages[1].id));
 const items=history.present.bookmarks!;expect(items).toHaveLength(3);expect(bookmarkPage(items[0])).toBeNull();expect(bookmarkPage(items[1])).toBe(doc.pages[2].id);
 expect(undo(history).present.bookmarks).toEqual(doc.bookmarks);
 const subset=filterBookmarks(doc.bookmarks!,[doc.pages[2]]);expect(subset.map(b=>b.title)).toEqual(["Chapter","Section"]);expect(bookmarkPage(subset[0])).toBeNull();
});
it("adds nested bookmarks, renames and moves whole branches, with one history step and explicit empty digest",()=>{
 let doc=createSession({...info,bookmarks:[]},adapter,100).history.present;
 doc=addBookmark(doc,"Root",doc.pages[0].id);const root=doc.bookmarks![0];doc=addBookmark(doc,"Child",doc.pages[1].id,root.id);doc=addBookmark(doc,"Second",doc.pages[2].id);
 expect(doc.bookmarks!.map(b=>b.level)).toEqual([0,1,0]);
 const history=commit(createHistory(doc),d=>moveBookmark(d,root.id,1));expect(history.present.bookmarks!.map(b=>b.title)).toEqual(["Second","Root","Child"]);
 expect(undo(history).present).toEqual(doc);expect(redo(undo(history))).toEqual(history);
 doc=editBookmark(doc,root.id,{title:"Renamed",pageId:doc.pages[2].id});expect(doc.bookmarks![0].title).toBe("Renamed");expect(bookmarkPage(doc.bookmarks![0])).toBe(doc.pages[2].id);
 doc=removeBookmark(doc,root.id);expect(doc.bookmarks!.map(b=>b.title)).toEqual(["Second"]);
 doc=removeBookmark(doc,doc.bookmarks![0].id);expect(doc.bookmarks).toEqual([]);
 const old:EditorDocument={...doc};delete old.bookmarks;expect(planDigest(doc)).not.toEqual(planDigest(old));
});
it("preserves external actions and nested GoTo references while mapping pages",()=>{
 const action:PdfValue={kind:"dictionary",value:[[[83],{kind:"name",value:[85,82,73]}],[[85,82,73],{kind:"string",value:[104,116,116,112]}],[[78,101,120,116],{kind:"dictionary",value:[[[83],{kind:"name",value:[71,111,84,111]}],[[68],destination("1")]]}]]};
 const pages=createSession(info,adapter,100).history.present.pages;
 const bookmarks=importBookmarks([{id:"link",title:"External",level:0,action}],pages);
 expect(bookmarkPage(bookmarks[0])).toBeNull();const filtered=filterBookmarks(bookmarks,[pages[0]]);expect(filtered).toHaveLength(1);
 expect(JSON.stringify(filtered)).not.toContain('"kind":"page"');expect(JSON.stringify(filtered)).toContain('"kind":"string"');
});
it("recovers bookmark content and dirty status without depending on source IDs",()=>{
 const native={...adapter,kind:"native" as const};let session=createSession(info,native,100);
 session={...session,history:commit(session.history,d=>addBookmark(d,"New",d.pages[0].id))};
 const snapshot=snapshotWorkspace(addSession(emptyWorkspace(),session),new Map());
 snapshot.tabs[0].document={...snapshot.tabs[0].document,pages:snapshot.tabs[0].document.pages.map(p=>({...p,sourceId:"recovered"}))};
 const restored=restoreWorkspace(snapshot,native).workspace.tabs[0];expect(restored.history.present.bookmarks).toEqual(session.history.present.bookmarks);expect(planDigest(restored.history.present)).not.toEqual(restored.savedDigest);
});
it("handles large legal PDF names without spreading them onto the JavaScript call stack",()=>{
 const item:DocumentBookmark={id:"long",title:"External",level:0,action:{kind:"dictionary",value:[[Array(200000).fill(65),{kind:"null"}]]}};
 expect(()=>bookmarkPage(item)).not.toThrow();expect(()=>filterBookmarks([item],[])).not.toThrow();
});
it("removes deleted-page actions from a Next array while preserving the remaining actions",()=>{
 const go:PdfValue={kind:"dictionary",value:[[[83],{kind:"name",value:[71,111,84,111]}],[[68],destination("gone")]]};
 const external:PdfValue={kind:"dictionary",value:[[[83],{kind:"name",value:[85,82,73]}],[[85,82,73],{kind:"string",value:[104,116,116,112]}]]};
 const action:PdfValue={kind:"dictionary",value:[...external.value,[[78,101,120,116],{kind:"array",value:[go,external]}]]};
 const [item]=filterBookmarks([{id:"a",title:"External",level:0,action}],[]);
 const result=item.action as Extract<PdfValue,{kind:"dictionary"}>;
 expect(result.value[2][1]).toEqual({kind:"array",value:[external]});
});
