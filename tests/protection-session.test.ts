import {expect,it} from "vitest";
import {commitOwnerUnlock,canFlattenPages} from "../src/editor/protection";
import {createSession} from "../src/editor/workspace";
import {nativeAdapter,documentToPages} from "../src/editor/adapter";
import {planDigest,undo,redo} from "../src/editor/model";
import {commitTextReplacement} from "../src/editor/text-edit";
import type {DocumentInfo,ProtectionInfo} from "../src/editor/types";
const protection:ProtectionInfo={documentId:"origin",owner:false,revision:4,permissions:{print:true,printHighQuality:false,copyContent:false,editContent:false,annotate:true,fillForms:true,assemble:false}};
const original:DocumentInfo={id:"reader",name:"Protected.pdf",pages:[{width:300,height:400}],protection};
const owner:DocumentInfo={...original,id:"owner",protection:{...protection,owner:true,permissions:{print:true,printHighQuality:true,copyContent:true,editContent:true,annotate:true,fillForms:true,assemble:true}}};
it("requires content-edit permission to flatten an image-only page",()=>{
 const page=documentToPages(original)[0];page.overlays=[{type:"image",id:"image",assetId:"a".repeat(64),x:20,y:30,width:80,height:40,rotation:0}];
 expect(canFlattenPages([page])).toBe(false);
 expect(canFlattenPages([{...page,protection:owner.protection}])).toBe(true);
 expect(canFlattenPages([{...page,overlays:[]}])).toBe(true);
});
it("owner unlock retains undo restrictions and does not dirty unchanged PDF content",()=>{
 const session=createSession(original,nativeAdapter,100),expected=session.history.present;
 const unlocked=commitOwnerUnlock(session,expected,[{sourceId:"reader",document:owner}]);
 expect(unlocked.history.present.pages[0].protection?.owner).toBe(true);
 expect(unlocked.history.past[0].pages[0].protection?.owner).toBe(false);
 expect(planDigest(unlocked.history.present)).toBe(session.savedDigest);
 expect(planDigest(undo(unlocked.history).present)).toBe(session.savedDigest);
 expect(planDigest(redo(undo(unlocked.history)).present)).toBe(session.savedDigest);
 const edited=commitTextReplacement(unlocked,unlocked.history.present.pages[0],{...owner,id:"edited"});
 expect(edited.history.present.pages[0].protection?.owner).toBe(true);
 expect(planDigest(edited.history.present)).not.toBe(session.savedDigest);
 expect(session.history.present).toBe(expected);
});
it("rejects stale, incomplete, duplicate and mismatched owner results before changing the session",()=>{
 const session=createSession(original,nativeAdapter,100),expected=session.history.present;
 expect(documentToPages(original)[0].protection).toEqual(protection);
 for(const results of [[],[{sourceId:"missing",document:owner}],[{sourceId:"reader",document:owner},{sourceId:"reader",document:{...owner,id:"other"}}],[{sourceId:"reader",document:{...owner,protection}}],[{sourceId:"reader",document:{...owner,protection:{...owner.protection!,documentId:"different"}}}]]) {
  expect(()=>commitOwnerUnlock(session,expected,results)).toThrow();
 }
 expect(()=>commitOwnerUnlock(session,{...expected,pages:[...expected.pages]},[{sourceId:"reader",document:owner}])).toThrow(/changed/);
 expect(session.sourceIds).toEqual(["reader"]);
});
