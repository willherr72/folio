import {expect,it} from "vitest";
import {findParagraphMatches,nextParagraphMatch} from "../src/editor/paragraph-find";
import {createStyledEditor,editStyledEditor,redoStyledEditor,styledEditorRequest,undoStyledEditor} from "../src/editor/styled-paragraph-editor";
import {styledText} from "../src/editor/styled-paragraph";
import {styledEditorFixture} from "./fixtures/styled-editor-data";

const options={matchCase:false,wholeWords:false};
const matches=(text:string,query:string,patch={})=>findParagraphMatches(text,query,{...options,...patch});
function editor(text:string){const source=styledEditorFixture();return createStyledEditor({...source,text,runs:[{text,styleId:0}]});}
function replace(state:ReturnType<typeof createStyledEditor>,query:string,text:string){return editStyledEditor(state,{type:"replace-many",expectedText:styledText(state.present.draft),ranges:matches(styledText(state.present.draft),query),text});}
it("finds literal nonoverlapping text with case and hard-line options",()=>{
 expect(matches("A.b a.b A?B", "a.b")).toEqual([{start:0,end:3},{start:4,end:7}]);
 expect(matches("A.b a.b", "a.b",{matchCase:true})).toEqual([{start:4,end:7}]);
 expect(matches("aaaa", "aa")).toEqual([{start:0,end:2},{start:2,end:4}]);
 expect(matches("one\n\ntwo","one\r\n\r\ntwo")).toEqual([{start:0,end:8}]);
 expect(matches("text","")).toEqual([]);
});
it("uses Unicode word and grapheme boundaries without canonical normalization",()=>{
 expect(matches("cat cats cat's cat", "cat",{wholeWords:true})).toEqual([{start:0,end:3},{start:15,end:18}]);
 expect(matches("αβ αβγ αβ", "αβ",{wholeWords:true})).toEqual([{start:0,end:2},{start:7,end:9}]);
 expect(matches("é e\u0301 👩‍💻", "e")).toEqual([]);
 expect(matches("é e\u0301", "é")).toEqual([{start:0,end:1}]);
 expect(matches("👩‍💻", "👩")).toEqual([]);
 expect(matches("𐐀 𐐨", "𐐨")).toEqual([{start:0,end:2},{start:3,end:5}]);
 expect(matches("---", "-",{wholeWords:true})).toEqual([]);
});
it("rejects malformed or excessive query work without returning unsafe ranges",()=>{
 for(const query of ["\ud800","x\t","\u0000","x".repeat(4097)])expect(()=>matches("text",query)).toThrow();
 expect(()=>matches("text","é".repeat(2049))).toThrow();
});
it.each([
 {name:"word overlap",text:"bobcat cat cat",query:"cat cat",wholeWords:true,start:7,end:14},
 {name:"combining overlap",text:"a\u0301a\u0301a",query:"a\u0301a",wholeWords:false,start:2,end:5},
 {name:"flag overlap",text:"🇺🇸🇸🇸",query:"🇸🇸",wholeWords:false,start:4,end:8},
])("keeps a valid $name match after rejecting an overlapping candidate",({text,query,wholeWords,start,end})=>{
 const ranges=matches(text,query,{wholeWords});expect(ranges).toEqual([{start,end}]);
 const next=editStyledEditor(editor(text),{type:"replace-many",expectedText:text,ranges,text:"X"});expect(styledText(next.present.draft)).toBe(text.slice(0,start)+"X"+text.slice(end));expect(next.past).toHaveLength(1);
});
it("navigates exact selected matches or the nearest caret range and wraps",()=>{
 const found=matches("cat cat cat","cat");
 expect(nextParagraphMatch(found,{start:0,end:3},1)).toEqual(found[1]);
 expect(nextParagraphMatch(found,{start:0,end:3},-1)).toEqual(found[2]);
 expect(nextParagraphMatch(found,{start:3,end:3},1)).toEqual(found[1]);
 expect(nextParagraphMatch(found,{start:7,end:7},-1)).toEqual(found[1]);
 expect(nextParagraphMatch(found,{start:11,end:11},1)).toEqual(found[0]);
 expect(nextParagraphMatch([],{start:0,end:0},1)).toBeNull();
});
it("replaces all once while preserving source, surrounding styles and formatting",()=>{
 const source=styledEditorFixture();source.text="one two one";source.runs=[{text:"one ",styleId:0},{text:"two",styleId:1},{text:" one",styleId:0}];source.styles[1].backgroundColor="#FFFF00";source.baselineAnchor={x:20,y:45};
 let state=createStyledEditor(source);state=editStyledEditor(state,{type:"select",start:4,end:7});state=editStyledEditor(state,{type:"format",patch:{color:"#123456",fontId:"a".repeat(64)}});
 const before=state,changed=replace(state,"one","$&");
 expect(styledText(changed.present.draft)).toBe("$& two $&");expect(changed.past).toHaveLength(before.past.length+1);
 expect(changed.original).toBe(before.original);expect(changed.present.baselineAnchor).toEqual(before.present.baselineAnchor);
 const request=styledEditorRequest(changed);expect(request.expectedText).toBe(source.text);expect(request.runs.map(run=>run.text)).toEqual(["$& ","two"," $&"]);
 expect(request.styleChanges).toEqual(styledEditorRequest(before).styleChanges);expect(changed.present.bounds).toEqual(before.present.bounds);
 expect(undoStyledEditor(changed).present).toBe(before.present);expect(redoStyledEditor(undoStyledEditor(changed)).present).toBe(changed.present);
});
it("uses the first matched style for inserted text across a mixed match",()=>{
 const state=createStyledEditor(styledEditorFixture()),next=replace(state,"e tw","NEW");
 expect(styledEditorRequest(next).runs).toEqual([{text:"onNEW",styleId:0},{text:"o",styleId:1}]);
});
it("treats no matches and identical replacements as no-op history, preserving mixed spans",()=>{
 const state=createStyledEditor(styledEditorFixture());expect(replace(state,"missing","x")).toBe(state);expect(replace(state,"one two","one two")).toBe(state);
});
it("normalizes replacement breaks and admits exactly final capacity",()=>{
 const state=editor("A A");const changed=replace(state,"A","X".repeat(2046)+"\r\n");
 expect(new TextEncoder().encode(styledText(changed.present.draft))).toHaveLength(4095);
 expect(()=>replace(state,"A","X".repeat(2047)+"\n")).toThrow(/4096/);
 expect(styledText(state.present.draft)).toBe("A A");expect(state.past).toHaveLength(0);
 expect(new TextEncoder().encode(styledText(replace(editor("A A"),"A","é".repeat(1023)+"\n").present.draft))).toHaveLength(4095);
 expect(new TextEncoder().encode(styledText(replace(editor("AA"),"AA","X".repeat(4096)).present.draft))).toHaveLength(4096);
});
it("validates every batch range and stale text atomically",()=>{
 const state=editor("one two");
 for(const ranges of [[{start:0,end:3},{start:2,end:4}],[{start:4,end:7},{start:0,end:3}],[{start:2,end:2}],[{start:0,end:99}]])expect(()=>editStyledEditor(state,{type:"replace-many",expectedText:"one two",ranges,text:"x"})).toThrow();
 expect(()=>editStyledEditor(state,{type:"replace-many",expectedText:"stale",ranges:[{start:0,end:3}],text:"x"})).toThrow();
 for(const text of ["\ud800","\t","\u0000"])expect(()=>replace(state,"one",text)).toThrow();
 expect(state.past).toHaveLength(0);
});
it("keeps final carets on joined graphemes and refuses cross-style joins",()=>{
 const state=replace(editor("aX"),"X","\u0301");expect(styledText(state.present.draft)).toBe("a\u0301");expect(state.present.selection).toMatchObject({start:2,end:2});
 const source=styledEditorFixture();source.text="aX";source.runs=[{text:"a",styleId:0},{text:"X",styleId:1}];
 expect(()=>replace(createStyledEditor(source),"X","\u0301")).toThrow(/grapheme/);
});
it("supports deletion of every match and keeps unchanged mixed matches intact",()=>{
 const state=editor("AAA"),empty=replace(state,"A","");expect(styledText(empty.present.draft)).toBe("");expect(empty.present.selection).toMatchObject({start:0,end:0});expect(undoStyledEditor(empty).present).toBe(state.present);
 const source=styledEditorFixture();source.text="one ONE";source.runs=[{text:"on",styleId:0},{text:"e ",styleId:1},{text:"ONE",styleId:0}];
 const changed=replace(createStyledEditor(source),"one","one");expect(styledEditorRequest(changed).runs).toEqual([{text:"on",styleId:0},{text:"e ",styleId:1},{text:"one",styleId:0}]);
});
