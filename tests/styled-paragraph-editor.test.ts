import {expect,it} from "vitest";
import {createStyledEditor,editStyledEditor,undoStyledEditor,redoStyledEditor,styledEditorRequest,styledEditorFontIds} from "../src/editor/styled-paragraph-editor";
import {styledText} from "../src/editor/styled-paragraph";
import type {StyledParagraphSelection,StyledParagraphLayoutRequest} from "../src/editor/types";
const fixture=():StyledParagraphSelection=>({objectIndices:[0,1],text:"one two",runs:[{text:"one ",styleId:0},{text:"two",styleId:1}],styles:[
 {styleId:0,objectIndices:[0],fontName:"Courier",fontDisplayName:null,fontSize:12,color:"#000000",isEmbedded:false},
 {styleId:1,objectIndices:[1],fontName:"Helvetica-Bold",fontDisplayName:null,fontSize:20,color:"#882244",isEmbedded:false}],bounds:{x:20,y:30,width:200,height:100},alignment:"right",lineSpacing:1.6});
const text=(state:ReturnType<typeof createStyledEditor>)=>styledText(state.present.draft);
function resolved(request:StyledParagraphLayoutRequest,source=fixture()){
 return request.runs.map(run=>{const change=request.styleChanges?.find(s=>s.styleId===run.styleId);const original=source.styles[change?.sourceStyleId??run.styleId];return {text:run.text,font:change?.fontId??original.fontName,size:change?.fontSize??original.fontSize,color:change?.color??original.color};});
}
it("copies native identity and immutable snapshots without losing exact paragraph defaults",()=>{
 const input=fixture(),state=createStyledEditor(input);input.runs[0].text="mutated";input.styles[0].fontSize=50;input.bounds.x=99;
 const request=styledEditorRequest(state);expect(request.expectedText).toBe("one two");expect(request.runs).toEqual(fixture().runs);expect(request.bounds).toEqual(fixture().bounds);expect(request.alignment).toBe("right");expect(request.lineSpacing).toBe(1.6);
 expect(Object.isFrozen(state.present.draft.runs)).toBe(true);expect(Object.isFrozen(state.original.styles[0])).toBe(true);
});
it("edits explicit ranges in repeated text and normalizes paste while retaining untouched styles",()=>{
 const input=fixture();input.text="one one";input.runs[1].text="one";let state=createStyledEditor(input);
 state=editStyledEditor(state,{type:"replace",start:4,end:7,text:"this\r\nline"});
 expect(text(state)).toBe("one this\nline");expect(resolved(styledEditorRequest(state),input)).toEqual([{text:"one ",font:"Courier",size:12,color:"#000000"},{text:"this\nline",font:"Helvetica-Bold",size:20,color:"#882244"}]);
 expect(state.present.selection).toEqual({start:13,end:13,direction:"forward"});expect(text(undoStyledEditor(state))).toBe("one one");
});
it("formats across source fonts without flattening either source and resets one property only",()=>{
 let state=editStyledEditor(createStyledEditor(fixture()),{type:"select",start:1,end:6});
 state=editStyledEditor(state,{type:"format",patch:{fontSize:17,color:"#abcdef"}});
 expect(resolved(styledEditorRequest(state))).toEqual([{text:"o",font:"Courier",size:12,color:"#000000"},{text:"ne ",font:"Courier",size:17,color:"#ABCDEF"},{text:"tw",font:"Helvetica-Bold",size:17,color:"#ABCDEF"},{text:"o",font:"Helvetica-Bold",size:20,color:"#882244"}]);
 state=editStyledEditor(state,{type:"format",patch:{color:null}});const request=styledEditorRequest(state);
 expect(resolved(request).map(r=>[r.size,r.color])).toEqual([[12,"#000000"],[17,"#000000"],[17,"#882244"],[20,"#882244"]]);expect(request.styleChanges?.every(s=>s.sourceStyleId<2)).toBe(true);
});
it("caret formatting affects new text and survives undo and redo including the typing override",()=>{
 let state=editStyledEditor(createStyledEditor(fixture()),{type:"select",start:4,end:4});state=editStyledEditor(state,{type:"format",patch:{fontSize:15}});
 expect(text(state)).toBe("one two");expect(state.present.typingStyleId).not.toBeNull();expect(styledEditorRequest(state)).toEqual(styledEditorRequest(createStyledEditor(fixture())));state=editStyledEditor(state,{type:"replace",text:"this "});
 expect(resolved(styledEditorRequest(state))).toEqual([{text:"one ",font:"Courier",size:12,color:"#000000"},{text:"this ",font:"Courier",size:15,color:"#000000"},{text:"two",font:"Helvetica-Bold",size:20,color:"#882244"}]);
 const undo=undoStyledEditor(state);expect(text(undo)).toBe("one two");expect(undo.present.selection.start).toBe(4);expect(undo.present.typingStyleId).not.toBeNull();expect(text(redoStyledEditor(undo))).toBe("one this two");
 const selected=editStyledEditor(undo,{type:"select",start:0,end:0});expect(selected.future.length).toBe(1);expect(selected.present.typingStyleId).toBeNull();expect(editStyledEditor(selected,{type:"replace",text:"x"}).future.length).toBe(0);
});
it("retains empty-draft typing style and blank-line formatting",()=>{
 let state=editStyledEditor(createStyledEditor(fixture()),{type:"replace",start:0,end:7,text:""});state=editStyledEditor(state,{type:"format",patch:{fontSize:30}});state=editStyledEditor(state,{type:"replace",text:"\n\n"});
 expect(text(state)).toBe("\n\n");expect(resolved(styledEditorRequest(state))[0].size).toBe(30);expect(text(undoStyledEditor(state))).toBe("");
});
it("rejects grapheme splits and atomically preserves a draft that cannot accept the replacement",()=>{
 const input=fixture();input.text="a\u0301 two";input.runs[0].text="a\u0301 ";let state=createStyledEditor(input);const before=JSON.stringify(state);
 expect(()=>editStyledEditor(state,{type:"select",start:1,end:1})).toThrow(/grapheme/);expect(()=>editStyledEditor(state,{type:"replace",start:0,end:0,text:"\u00e9".repeat(2049)})).toThrow(/4096/);expect(JSON.stringify(state)).toBe(before);
 state=editStyledEditor(state,{type:"replace",start:2,end:2,text:"\u0300"});expect(state.present.selection.start).toBe(3);expect(text(state)).toBe("a\u0301\u0300 two");
});
it("compacts request targets to the native 64-style bound without chaining derived styles",()=>{
 let state=createStyledEditor(fixture());state=editStyledEditor(state,{type:"select",start:0,end:7});
 for(let i=0;i<120;i++)state=editStyledEditor(state,{type:"format",patch:{fontSize:12+i}});
 const request=styledEditorRequest(state);expect(state.past.length).toBe(100);expect(state.present.styles.length).toBe(2);expect(new Set(request.runs.map(r=>r.styleId))).toEqual(new Set([0,1]));expect(request.styleChanges?.map(s=>s.sourceStyleId)).toEqual([0,1]);expect(request.styleChanges?.every(s=>s.styleId<64)).toBe(true);
 let undone=state;for(let i=0;i<100;i++)undone=undoStyledEditor(undone);expect(undone.past).toHaveLength(0);expect(undone.future).toHaveLength(100);expect(undoStyledEditor(undone)).toBe(undone);
});
it("rejects 65 simultaneous styles but permits later formatting after unused styles leave the current draft",()=>{
 const input=fixture();input.text="x".repeat(64);input.runs=Array.from({length:64},(_,styleId)=>({text:"x",styleId}));input.styles=Array.from({length:64},(_,styleId)=>({...fixture().styles[0],styleId,objectIndices:[styleId],fontSize:styleId+1}));input.objectIndices=Array.from({length:64},(_,i)=>i);
 let state=createStyledEditor(input);state=editStyledEditor(state,{type:"select",start:0,end:0});expect(()=>editStyledEditor(state,{type:"format",patch:{fontSize:200}})).toThrow(/64/);
 state=editStyledEditor(state,{type:"select",start:0,end:1});state=editStyledEditor(state,{type:"format",patch:{fontSize:200}});expect(state.present.styles.length).toBe(64);expect(styledEditorRequest(state).styleChanges?.every(s=>s.styleId<64)).toBe(true);
});
it("reports fonts referenced by history and redo so dialog leases outlive formatting changes",()=>{
 const fontId="a".repeat(64);let state=editStyledEditor(createStyledEditor(fixture()),{type:"select",start:0,end:7});state=editStyledEditor(state,{type:"format",patch:{fontId}});expect([...styledEditorFontIds(state)]).toEqual([fontId]);
 state=undoStyledEditor(state);expect([...styledEditorFontIds(state)]).toEqual([fontId]);state=editStyledEditor(state,{type:"replace",text:"new"});expect([...styledEditorFontIds(state)]).toEqual([]);
});
it("changes area and paragraph formatting atomically and restores them with undo",()=>{
 let state=createStyledEditor(fixture());state=editStyledEditor(state,{type:"paragraph",alignment:"justify",lineSpacing:2});state=editStyledEditor(state,{type:"area",bounds:{x:30,y:40,width:150,height:80}});
 expect(styledEditorRequest(state).bounds.x).toBe(30);expect(styledEditorRequest(undoStyledEditor(state)).bounds.x).toBe(20);expect(styledEditorRequest(undoStyledEditor(undoStyledEditor(state))).alignment).toBe("right");
 for(const patch of [{fontSize:NaN},{color:"red"},{fontId:"unknown"}])expect(()=>editStyledEditor(state,{type:"format",patch})).toThrow();
 expect(()=>editStyledEditor(state,{type:"area",bounds:{x:-1,y:0,width:20,height:20}})).toThrow();expect(()=>editStyledEditor(state,{type:"paragraph",lineSpacing:4})).toThrow();
});
it("groups one typing burst into an undo step and breaks the group at selection, formatting, or undo",()=>{
 let state=editStyledEditor(createStyledEditor(fixture()),{type:"select",start:4,end:7});
 for(const text of ["n","e","w"])state=editStyledEditor(state,{type:"replace",text,historyGroup:"typing-1"});
 expect(state.past).toHaveLength(1);expect(text(state)).toBe("one new");expect(text(undoStyledEditor(state))).toBe("one two");
 state=editStyledEditor(state,{type:"select",start:0,end:0});state=editStyledEditor(state,{type:"replace",text:"x",historyGroup:"typing-1"});expect(state.past).toHaveLength(2);
 state=editStyledEditor(state,{type:"format",patch:{fontSize:13}});state=editStyledEditor(state,{type:"replace",text:"y",historyGroup:"typing-1"});expect(state.past).toHaveLength(4);
 const undo=undoStyledEditor(state);expect(editStyledEditor(undo,{type:"replace",text:"z",historyGroup:"typing-1"}).past).toHaveLength(4);
});
it("preserves distinct source identities with identical visible formatting through range format and reset",()=>{
 const source=fixture();source.styles[1]={...source.styles[0],styleId:1,objectIndices:[1]};
 let state=editStyledEditor(createStyledEditor(source),{type:"select",start:0,end:7});
 const identities=(request:StyledParagraphLayoutRequest)=>request.runs.map(run=>({text:run.text,source:request.styleChanges?.find(c=>c.styleId===run.styleId)?.sourceStyleId??run.styleId}));
 state=editStyledEditor(state,{type:"format",patch:{fontSize:18,color:"#123456",fontId:"a".repeat(64)}});
 expect(identities(styledEditorRequest(state))).toEqual([{text:"one ",source:0},{text:"two",source:1}]);
 state=editStyledEditor(state,{type:"format",patch:{fontSize:null,color:null,fontId:null}});
 expect(identities(styledEditorRequest(state))).toEqual([{text:"one ",source:0},{text:"two",source:1}]);
 expect(resolved(styledEditorRequest(state),source).map(r=>[r.font,r.size,r.color])).toEqual([["Courier",12,"#000000"],["Courier",12,"#000000"]]);
 expect(identities(styledEditorRequest(redoStyledEditor(undoStyledEditor(state))))).toEqual(identities(styledEditorRequest(state)));
});
it("retains original style ownership when UTF-16 replacement and deletion densely remap the active palette",()=>{
 const source=fixture();source.styles[1]={...source.styles[0],styleId:1,objectIndices:[1]};source.text="a éé";source.runs=[{text:"a ",styleId:0},{text:"éé",styleId:1}];
 let state=editStyledEditor(createStyledEditor(source),{type:"replace",start:2,end:4,text:"é\r\né"});
 state=editStyledEditor(state,{type:"replace",start:0,end:2,text:""});
 const request=styledEditorRequest(state);expect(request.runs).toEqual([{text:"é\né",styleId:0}]);expect(request.styleChanges).toEqual([{styleId:0,sourceStyleId:1}]);
 state=editStyledEditor(state,{type:"select",start:1,end:1});state=editStyledEditor(state,{type:"replace",text:" plus"});
 expect(styledEditorRequest(state).styleChanges).toEqual([{styleId:0,sourceStyleId:1}]);
 const undo=undoStyledEditor(state);expect(styledEditorRequest(undo).runs).toEqual(request.runs);expect(styledEditorRequest(redoStyledEditor(undo)).runs[0].text).toBe("é plus\né");
});

it("validates optional native background colors without accepting background formatting requests",()=>{
 const source=fixture();source.styles[1]={...source.styles[1],backgroundColor:"#ffff00"};const state=createStyledEditor(source);source.styles[1].backgroundColor="#000000";
 expect(state.original.styles[1].backgroundColor).toBe("#ffff00");expect(Object.isFrozen(state.original.styles[1])).toBe(true);
 for(const color of ["red","#ffff0080","url(example)","",12]){const bad=fixture();Object.assign(bad.styles[1],{backgroundColor:color});expect(()=>createStyledEditor(bad)).toThrow(/palette/);}
 expect(()=>editStyledEditor(state,{type:"format",patch:{backgroundColor:"#000000"} as never})).toThrow(/Unknown/);
});
