import { afterEach, expect, it, vi } from "vitest";
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { acquireShapedText, clearShapedTextCache, shapedTextApi, shapedTextState, useShapedText, type PreparedTextOverlay } from "../src/editor/shaped-text";
import { fontApi, retainCustomFonts, flushFontReleases } from "../src/editor/custom-fonts";
import { textOverlayCharacters, textOverlayBounds } from "../src/editor/text-overlay-geometry";
import { useDocumentSearch } from "../src/editor/search";
import { createDemoAdapter } from "../src/editor/adapter";
import type { TextOverlay } from "../src/editor/types";
const id="a".repeat(64);
const box:TextOverlay={type:"text",id:"box",x:10,y:20,text:"fi",fontSize:24,fontId:id,color:"#112233",shaping:{version:1,direction:"auto",ligatures:true}};
function result(text="fi"):PreparedTextOverlay {return {preview:{version:1,fontId:id,text,width:300,height:160,rotation:0,color:[0.1,0.2,0.3],outlines:[],glyphs:[]},origin:[48,48],bounds:{x:0,y:3,width:16,height:21},characters:Array.from(text,text=>({text,x:0,y:3,width:16,height:21}))};}
afterEach(async()=>{cleanup();clearShapedTextCache();retainCustomFonts(new Set());await flushFontReleases();vi.restoreAllMocks();});
it("shares exact native work across duplicate boxes and ignores placement in its key",async()=>{
 vi.spyOn(fontApi,"release").mockResolvedValue();retainCustomFonts(new Set([id]));
 const prepare=vi.spyOn(shapedTextApi,"prepare").mockResolvedValue(result());
 const a=acquireShapedText(box), b=acquireShapedText({...box,id:"duplicate",x:100,rotation:90});
 expect(await a.promise).toBe(await b.promise);expect(prepare).toHaveBeenCalledTimes(1);a.release();b.release();
 const text=textOverlayCharacters({...box,rotation:90});expect(text.characters.map(c=>c.text).join("")).toBe("fi");
 expect(text.characters[0]).toEqual({text:"f",x:-14,y:20,width:21,height:16});
 expect({...text.characters[0],text:"i"}).toEqual(text.characters[1]);
 expect(textOverlayBounds(box)).toEqual({x:6,y:19,width:24,height:32});
});
it("holds the exact font through an abandoned native response then releases it",async()=>{
 const released=vi.spyOn(fontApi,"release").mockResolvedValue();
 let done!:(value:PreparedTextOverlay)=>void;vi.spyOn(shapedTextApi,"prepare").mockImplementation(()=>new Promise(resolve=>done=resolve));
 const lease=acquireShapedText(box);await waitFor(()=>expect(done).toBeTypeOf("function"));lease.release();
 expect(released).not.toHaveBeenCalled();done(result());await lease.promise;await flushFontReleases();
 expect(released).toHaveBeenCalledWith(id);expect(shapedTextState(box)).toBeUndefined();
});
it("an old response cannot replace the current typing and canceled queued work never starts",async()=>{
 vi.spyOn(fontApi,"release").mockResolvedValue();retainCustomFonts(new Set([id]));
 const pending:Array<(value:PreparedTextOverlay)=>void>=[];
 const prepare=vi.spyOn(shapedTextApi,"prepare").mockImplementation(()=>new Promise(resolve=>pending.push(resolve)));
 const hook=renderHook(({overlay})=>useShapedText(overlay),{initialProps:{overlay:box}});
 await waitFor(()=>expect(pending).toHaveLength(1));
 hook.rerender({overlay:{...box,text:"old queued"}});hook.rerender({overlay:{...box,text:"new"}});
 await act(async()=>pending[0](result("fi")));
 await waitFor(()=>expect(pending).toHaveLength(2));expect(hook.result.current?.status).toBe("loading");
 expect(prepare.mock.calls[1][0].text).toBe("new");
 await act(async()=>pending[1](result("new")));await waitFor(()=>expect(hook.result.current?.result?.preview.text).toBe("new"));
});
it("keeps failures scoped to the exact text and never substitutes approximate geometry",async()=>{
 vi.spyOn(fontApi,"release").mockResolvedValue();retainCustomFonts(new Set([id]));
 vi.spyOn(shapedTextApi,"prepare").mockRejectedValue(new Error("Mixed directional runs unsupported"));
 const lease=acquireShapedText(box);await expect(lease.promise).rejects.toThrow("Mixed directional");
 expect(shapedTextState(box)?.status).toBe("error");expect(textOverlayCharacters(box).characters).toEqual([]);lease.release();
});

it("evicts unused results before rejecting a new preview on its memory budget",async()=>{
 vi.spyOn(fontApi,"release").mockResolvedValue();retainCustomFonts(new Set([id]));
 vi.spyOn(shapedTextApi,"prepare").mockImplementation(async overlay=>{const value=result(overlay.text);value.preview={...value.preview,outlines:[{glyphId:1,path:"M".repeat(7*1024*1024)}]};return value;});
 for (const text of ["first","second","third"]) {
  const lease=acquireShapedText({...box,text});await expect(lease.promise).resolves.toMatchObject({preview:{text}});lease.release();
 }
});

it("search retains every match when a page has more shaped boxes than the cache",async()=>{
 vi.spyOn(fontApi,"release").mockResolvedValue();retainCustomFonts(new Set([id]));
 vi.spyOn(shapedTextApi,"prepare").mockImplementation(async overlay=>result(overlay.text));
 const adapter={...createDemoAdapter(),getPageText:async()=>({characters:[]})};
 const pages=[{id:"many",sourceId:"many",pageIndex:0,width:600,height:800,rotation:0 as const,overlays:Array.from({length:70},(_,i)=>({...box,id:`box-${i}`,text:`match ${i}`}))}];
 const hook=renderHook(()=>useDocumentSearch(adapter,pages,"match",true));
 await waitFor(()=>expect(hook.result.current.searching).toBe(false));
 expect(hook.result.current.matches).toHaveLength(70);
});

it("can retry an admission error after unused document previews are released",async()=>{
 vi.spyOn(fontApi,"release").mockResolvedValue();retainCustomFonts(new Set([id]));
 vi.spyOn(shapedTextApi,"prepare").mockImplementation(async overlay=>result(overlay.text));
 const leases=Array.from({length:64},(_,i)=>acquireShapedText({...box,text:`held${i}`}));await Promise.all(leases.map(lease=>lease.promise));
 const hook=renderHook(()=>useShapedText({...box,text:"waiting"}));
 await waitFor(()=>expect(hook.result.current?.status).toBe("error"));
 leases[0].release();await act(async()=>hook.result.current?.retry?.());
 await waitFor(()=>expect(hook.result.current?.status).toBe("ready"));leases.forEach(lease=>lease.release());
});
