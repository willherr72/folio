import {expect,it} from "vitest";
import {prepareParagraphDraft,transferStyledDraft,validateParagraphMapping,type ExistingTextDraft,type ParagraphDraftMapping} from "../src/editor/paragraph-draft";
import {createDemoAdapter} from "../src/editor/adapter";
import {redoStyledEditor,styledEditorRequest,undoStyledEditor} from "../src/editor/styled-paragraph-editor";
import {styledEditorFixture} from "./fixtures/styled-editor-data";
const selected=()=>({...styledEditorFixture(),text:"one one",runs:[{text:"one ",styleId:0},{text:"one",styleId:1}]});
const mapping=(change:Partial<ParagraphDraftMapping>={}):ParagraphDraftMapping=>({sourceId:"source",pageIndex:0,objectIndices:[0,1],anchorIndex:1,paragraphText:"one one",sourceText:"one",sourceStart:0,sourceEnd:3,paragraphStart:4,paragraphEnd:7,...change});
const draft=(replacement:string,originalText="one"):ExistingTextDraft=>({originalText,replacement,selection:{start:replacement.length,end:replacement.length,direction:"forward"},font:null});
it("changes only the selected repeated piece, preserving its neighbor and one Undo item",()=>{
 const selection=selected(),editor=transferStyledDraft(selection,mapping(),draft("one this"));
 expect(styledEditorRequest(editor)).toMatchObject({expectedText:"one one",runs:[{text:"one ",styleId:0},{text:"one this",styleId:1}]});
 expect(editor.past).toHaveLength(1);expect(styledEditorRequest(undoStyledEditor(editor)).runs).toEqual(selection.runs);
 expect(styledEditorRequest(redoStyledEditor(undoStyledEditor(editor)))).toEqual(styledEditorRequest(editor));
});
it("does not duplicate unchanged trimmed padding, but retains newly inserted edge spaces",()=>{
 const selection=selected(),map=mapping({sourceText:" one ",sourceStart:1,sourceEnd:4});
 const request=styledEditorRequest(transferStyledDraft(selection,map,draft(" one this  "," one ")));
 expect(request.runs.map(r=>r.text).join("")).toBe("one one this ");
});
it("preserves unchanged highlight/style spans inside the selected source piece",()=>{
 const selection={...selected(),text:"one one",runs:[{text:"one o",styleId:0},{text:"ne",styleId:1}]};
 const request=styledEditorRequest(transferStyledDraft(selection,mapping(),draft("XoneY")));
 expect(request.runs).toEqual([{text:"one Xo",styleId:0},{text:"neY",styleId:1}]);
});
it("uses the selected piece's style at its leading insertion boundary",()=>{
 expect(styledEditorRequest(transferStyledDraft(selected(),mapping(),draft("Xone"))).runs).toEqual([{text:"one ",styleId:0},{text:"Xone",styleId:1}]);
});
it("transfers explicit font only to its selected piece and retains it through Undo/Redo",()=>{
 const font={id:"a".repeat(64),name:"Chosen",weight:400,italic:false,coverage:[],shapedCoverage:[]};
 const editor=transferStyledDraft(selected(),mapping(),{...draft("one"),font} as ExistingTextDraft);
 const request=styledEditorRequest(editor);
 expect(request.styleChanges).toEqual(expect.arrayContaining([expect.objectContaining({sourceStyleId:1,fontId:font.id})]));
 expect(request.runs[0]).toEqual({text:"one ",styleId:0});expect(editor.past).toHaveLength(1);
 expect(styledEditorRequest(undoStyledEditor(editor)).styleChanges).toBeUndefined();
});
it.each([{sourceId:"other"},{pageIndex:1},{anchorIndex:0},{objectIndices:[1,0]},{paragraphText:"wrong"},{sourceStart:-1},{sourceEnd:4},{paragraphEnd:6}])("rejects malformed or stale mapping %j",change=>{
 expect(()=>validateParagraphMapping(mapping(change),{sourceId:"source",pageIndex:0,objectIndices:[0,1],anchorIndex:1,paragraphText:"one one",sourceText:"one"})).toThrow();
});
it("keeps Unicode graphemes together and refuses a range that splits them",()=>{
 const selection={...selected(),text:"one e\u0301😀",runs:[{text:"one ",styleId:0},{text:"e\u0301😀",styleId:1}]};
 const map=mapping({paragraphText:selection.text,sourceText:"e\u0301😀",sourceEnd:4,paragraphEnd:8});
 expect(styledEditorRequest(transferStyledDraft(selection,map,draft("e\u0301😀 this","e\u0301😀"))).runs.at(-1)?.text).toBe("e\u0301😀 this");
 expect(()=>transferStyledDraft(selection,{...map,sourceStart:1,paragraphStart:5},draft("e\u0301😀 this","e\u0301😀"))).toThrow();
});
it("copies the first provider's selection before awaiting native mapping",async()=>{
 const selection=selected();let resolve!:(value:ParagraphDraftMapping)=>void;
 const adapter={...createDemoAdapter(),listTextRuns:async()=>({runs:[]}),inspectStyledParagraph:async()=>selection,suggestStyledParagraph:async()=>selection,previewStyledParagraph:async()=>({id:"new",name:"new",pages:[]}),mapParagraphDraft:()=>new Promise<ParagraphDraftMapping>(done=>{resolve=done;})};
 const anchor={objectIndex:1,text:"one",fontName:"Helvetica",fontSize:20,bounds:selection.bounds,supported:true};
 const pending=prepareParagraphDraft(adapter,"source",0,anchor,draft("one this"));
 await Promise.resolve();selection.runs[0].text="MUTATED ";selection.styles[0].color="#ffffff";
 resolve(mapping());const prepared=await pending;expect(prepared.kind).toBe("styled");
 if(prepared.kind!=="styled")throw new Error("Wrong editor");expect(styledEditorRequest(prepared.editor).runs[0]).toEqual({text:"one ",styleId:0});expect(prepared.editor.original.styles[0].color).toBe("#000000");
});
it("keeps changed leading and trailing whitespace without claiming generated separators",()=>{
 const map=mapping({sourceText:" one ",sourceStart:1,sourceEnd:4});
 expect(styledEditorRequest(transferStyledDraft(selected(),map,draft("  one  "," one "))).runs.map(r=>r.text).join("")).toBe("one  one ");
 expect(styledEditorRequest(transferStyledDraft(selected(),map,draft("one"," one "))).runs.map(r=>r.text).join("")).toBe("one one");
});
it("rejects draft growth beyond 4096 bytes before installing it",()=>{
 const selection={...selected(),text:"x".repeat(4092)+" one",runs:[{text:"x".repeat(4092)+" ",styleId:0},{text:"one",styleId:1}]};
 expect(()=>transferStyledDraft(selection,mapping({paragraphText:selection.text,paragraphStart:4093,paragraphEnd:4096}),draft("one this"))).toThrow("4096");
});
