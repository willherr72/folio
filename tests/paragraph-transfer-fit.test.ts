import {expect,it,vi} from "vitest";
import {createDemoAdapter} from "../src/editor/adapter";
import {prepareParagraphDraft,type ExistingTextDraft} from "../src/editor/paragraph-draft";
import {redoStyledEditor,styledEditorRequest,undoStyledEditor} from "../src/editor/styled-paragraph-editor";
import {styledEditorFixture} from "./fixtures/styled-editor-data";
const selection=()=>{const old=styledEditorFixture();return {...old,objectIndices:[0],text:"Short",runs:[{text:"Short",styleId:0}],styles:[{...old.styles[0],objectIndices:[0]}],bounds:{x:20,y:30,width:40,height:20},baselineAnchor:{x:20,y:42}};};
const changed=():ExistingTextDraft=>({originalText:"Short",replacement:"Short plus more",selection:{start:6,end:10,direction:"backward"},font:null});
const viewport={width:300,height:400};
function setup(fit:any=vi.fn(async()=>({x:20,y:28,width:120,height:24}))){
 const selected=selection();const adapter={...createDemoAdapter(),listTextRuns:vi.fn(async()=>({runs:[]})),inspectStyledParagraph:vi.fn(async()=>selected),suggestStyledParagraph:vi.fn(async()=>selected),previewStyledParagraph:vi.fn(async()=>({id:"new",name:"new",pages:[]})),fitStyledParagraph:fit};
 const anchor={objectIndex:0,text:"Short",fontName:"Courier",fontSize:12,bounds:selected.bounds,supported:true};return {adapter,anchor,selected,fit};
}
it("fits the transferred request, preserves baseline/styles/caret and retains one Undo/Redo step",async()=>{
 const {adapter,anchor,selected,fit}=setup();const prepared=await prepareParagraphDraft(adapter,"source",0,anchor,changed(),viewport);
 expect(fit).toHaveBeenCalledOnce();expect(fit).toHaveBeenCalledWith("source",0,expect.objectContaining({expectedText:"Short",runs:[{text:"Short plus more",styleId:0}],baselineAnchor:{x:20,y:42},bounds:selected.bounds,alignment:"right",lineSpacing:1.6}));
 expect(prepared.editor.present.bounds).toEqual({x:20,y:28,width:120,height:24});expect(prepared.editor.present.baselineAnchor).toEqual({x:20,y:42});expect(prepared.editor.present.selection).toEqual(changed().selection);expect(prepared.editor.present.styles).toEqual([{id:0,sourceStyleId:0}]);expect(prepared.fitFeedback?.kind).toBe("fitted");
 expect(prepared.editor.past).toHaveLength(1);const undone=undoStyledEditor(prepared.editor);expect(styledEditorRequest(undone)).toMatchObject({runs:selected.runs,bounds:selected.bounds});expect(styledEditorRequest(redoStyledEditor(undone))).toEqual(styledEditorRequest(prepared.editor));
});
it("keeps an already-fitting area unchanged without adding history",async()=>{
 const {adapter,anchor,fit}=setup(vi.fn(async(_s,_p,request)=>({...request.bounds})));const prepared=await prepareParagraphDraft(adapter,"source",0,anchor,changed(),viewport);
 expect(fit).toHaveBeenCalledOnce();expect(prepared.editor.present.bounds).toEqual(selection().bounds);expect(prepared.editor.past).toHaveLength(1);expect(prepared.fitFeedback?.kind).toBe("unchanged");
});
it("keeps the transferred draft/font/selection and manual recovery when native fitting refuses",async()=>{
 const {adapter,anchor}=setup(vi.fn(async()=>{throw new Error("No clear area fits this paragraph.");}));const input={...changed(),font:{id:"a".repeat(64),name:"Chosen",weight:400,italic:false,coverage:[[32,126]]as[number,number][]}};
 const prepared=await prepareParagraphDraft(adapter,"source",0,anchor,input,viewport);expect(styledEditorRequest(prepared.editor).runs.map(r=>r.text).join("")).toBe(input.replacement);expect(prepared.editor.present.bounds).toEqual(selection().bounds);expect(prepared.editor.present.selection).toEqual(input.selection);expect(prepared.font?.id).toBe(input.font.id);expect(prepared.fitFeedback?.kind).toBe("unavailable");expect(prepared.fitFeedback?.message).toContain("No clear area");expect(prepared.editor.past).toHaveLength(1);
});
it.each([null,{x:20,y:30,width:NaN,height:20},{x:-1,y:30,width:50,height:20},{x:20,y:30,width:400,height:20},{x:20,y:30,width:60,height:20}])("refuses malformed or cropped-away proposals while keeping the draft: %j",async bounds=>{
 const {adapter,anchor}=setup(vi.fn(async()=>bounds));const prepared=await prepareParagraphDraft(adapter,"source",0,anchor,changed(),{...viewport,crop:{x:10,y:20,width:50,height:100}});
 expect(prepared.editor.present.bounds).toEqual(selection().bounds);expect(prepared.fitFeedback?.kind).toBe("unavailable");expect(styledEditorRequest(prepared.editor).runs[0].text).toBe(changed().replacement);
});
it("owns the returned bounds and does not let an adapter mutate the original draft request",async()=>{
 const bounds={x:20,y:28,width:120,height:24};const {adapter,anchor}=setup(vi.fn(async(_s,_p,request)=>{request.runs[0].text="MUTATED";request.baselineAnchor.x=99;return bounds;}));
 const prepared=await prepareParagraphDraft(adapter,"source",0,anchor,changed(),viewport);bounds.width=999;
 expect(prepared.editor.present.bounds.width).toBe(120);expect(styledEditorRequest(prepared.editor).runs[0].text).toBe(changed().replacement);expect(prepared.editor.present.baselineAnchor).toEqual({x:20,y:42});
});
it("retains the ordinary transferred draft when the adapter has no fitter",async()=>{
 const {adapter,anchor}=setup(undefined);adapter.fitStyledParagraph=undefined;const prepared=await prepareParagraphDraft(adapter,"source",0,anchor,changed(),viewport);expect(prepared.editor.present.bounds).toEqual(selection().bounds);expect(prepared.fitFeedback).toBeUndefined();expect(prepared.editor.past).toHaveLength(1);
});

it("does not request fitting for an unchanged transfer",async()=>{
 const {adapter,anchor,fit}=setup();const input={...changed(),replacement:"Short",selection:{start:5,end:5,direction:"forward"as const}};const prepared=await prepareParagraphDraft(adapter,"source",0,anchor,input,viewport);expect(fit).not.toHaveBeenCalled();expect(prepared.editor.past).toHaveLength(0);expect(prepared.fitFeedback).toBeUndefined();
});
it("owns the visible frame before awaiting the first provider",async()=>{
 const {adapter,anchor}=setup();const context={...viewport,crop:{x:10,y:20,width:200,height:200}};const pending=prepareParagraphDraft(adapter,"source",0,anchor,changed(),context);context.width=30;context.crop.width=20;
 const prepared=await pending;expect(prepared.editor.present.bounds.width).toBe(120);expect(prepared.fitFeedback?.kind).toBe("fitted");
});
it("does not dispatch a new fit after the owner has gone away",async()=>{
 const {adapter,anchor,fit}=setup();let current=true;const pending=prepareParagraphDraft(adapter,"source",0,anchor,changed(),{...viewport,isCurrent:()=>current});current=false;
 const prepared=await pending;expect(fit).not.toHaveBeenCalled();expect(prepared.fitFeedback).toBeUndefined();expect(prepared.editor.present.bounds).toEqual(selection().bounds);
});
it("ignores a late fitted proposal after ownership changes",async()=>{
 let resolve!:(v:any)=>void;const {adapter,anchor,fit}=setup(vi.fn(()=>new Promise(done=>{resolve=done;})));let current=true;
 const pending=prepareParagraphDraft(adapter,"source",0,anchor,changed(),{...viewport,isCurrent:()=>current});await vi.waitFor(()=>expect(fit).toHaveBeenCalledOnce());current=false;resolve({x:20,y:28,width:120,height:24});const prepared=await pending;expect(prepared.editor.present.bounds).toEqual(selection().bounds);expect(prepared.fitFeedback).toBeUndefined();
});

it("uses complete-source fitting when the prepared adapter supplies both routes",async()=>{
 const {adapter,anchor,fit}=setup();const complete=vi.fn(async()=>({x:20,y:28,width:140,height:24}));
 const prepared=await prepareParagraphDraft({...adapter,fitOccurrenceStyledParagraph:complete},"source",0,anchor,changed(),viewport);
 expect(complete).toHaveBeenCalledOnce();expect(fit).not.toHaveBeenCalled();expect(prepared.editor.present.bounds.width).toBe(140);
});
