import {afterEach,expect,it,vi} from "vitest";
import {discardOpenedDocument} from "../src/editor/discard-opened-document";
import {fontApi,flushFontReleases,holdCustomFont,retainCustomFonts} from "../src/editor/custom-fonts";
import type {DocumentInfo} from "../src/editor/types";
import {imageApi,retainImages,flushImageReleases} from "../src/editor/image-resources";
function opened(ids:string[]):DocumentInfo {return {id:"incoming",name:"Incoming.pdf",pages:[{width:300,height:400,overlays:ids.map((fontId,i)=>({type:"text",id:`t${i}`,fontId,text:"note",x:10,y:20,fontSize:12,color:"#000000"}))}]};}
afterEach(async()=>{retainCustomFonts(new Set());await flushFontReleases();vi.restoreAllMocks();});
it("reclaims never-adopted fonts once while preserving workspace and dialog owners",async()=>{
 const release=vi.spyOn(fontApi,"release").mockResolvedValue();const close=vi.fn().mockResolvedValue(undefined);
 const shared="a".repeat(64),preview="b".repeat(64),orphan="c".repeat(64);
 retainCustomFonts(new Set([shared]));const endPreview=holdCustomFont(preview);
 try {
  await discardOpenedDocument(opened([shared,preview,orphan,orphan]),close);
  expect(close).toHaveBeenCalledWith("incoming");expect(release.mock.calls).toEqual([[orphan]]);
  expect(close.mock.invocationCallOrder[0]).toBeLessThan(release.mock.invocationCallOrder[0]);
 } finally {await endPreview();}
});
it("releases unowned fonts even when closing an already-removed source fails",async()=>{
 const release=vi.spyOn(fontApi,"release").mockResolvedValue();const fontId="d".repeat(64);
 await expect(discardOpenedDocument(opened([fontId]),async()=>{throw new Error("Source already closed");})).rejects.toThrow("Source already closed");
 expect(release).toHaveBeenCalledWith(fontId);
});
it("waits for a pending workspace image owner before releasing the source owner",async()=>{
 const id="e".repeat(64);let resolve!:(value:any)=>void;
 vi.spyOn(imageApi,"acquire").mockReturnValue(new Promise(done=>resolve=done));
 const release=vi.spyOn(imageApi,"release").mockResolvedValue();const close=vi.fn().mockResolvedValue(undefined);
 const pending=retainImages(new Set([id]));const discarded=discardOpenedDocument(opened([]),close);
 await Promise.resolve();expect(close).not.toHaveBeenCalled();
 resolve({image:{id,width:4,height:2,mimeType:"image/png",hasAlpha:true,bitsPerChannel:8,byteLength:90},token:"workspace-token"});
 await pending;await discarded;expect(close).toHaveBeenCalledWith("incoming");expect(release).not.toHaveBeenCalled();
 await retainImages(new Set());await flushImageReleases();expect(release).toHaveBeenCalledWith("workspace-token");
});
