import {expect,it} from "vitest";
import {prepareParagraphPaste} from "../src/editor/paragraph-paste";
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

it("transfers hard and blank lines while preserving siblings and one local Undo",()=>{
 const selection=selected(),replacement="one\n\nsecond",editor=transferStyledDraft(selection,mapping(),draft(replacement));
 const request=styledEditorRequest(editor);expect(request.expectedText).toBe("one one");expect(request.runs.map(r=>r.text).join("")).toBe("one one\n\nsecond");
 expect(request.runs[0]).toEqual(selection.runs[0]);expect(editor.present.selection).toEqual({start:replacement.length+4,end:replacement.length+4,direction:"forward"});
 expect(editor.past).toHaveLength(1);expect(styledEditorRequest(undoStyledEditor(editor)).runs).toEqual(selection.runs);
 expect(styledEditorRequest(redoStyledEditor(undoStyledEditor(editor)))).toEqual(request);
});
it("admits paragraph handoff above the ordinary single-line input limit",()=>{
 const replacement="a".repeat(1100)+"\nz",editor=transferStyledDraft(selected(),mapping(),draft(replacement));
 expect(styledEditorRequest(editor).runs.map(r=>r.text).join("")).toBe("one "+replacement);
});
it.each(["one\r\nsecond","one\tsecond","one\0second","\ud800\nsecond"])("refuses unnormalized or unsupported paragraph draft controls",replacement=>{
 expect(()=>transferStyledDraft(selected(),mapping(),draft(replacement))).toThrow();
});

it("does not select a joined Unicode grapheme after clipboard handoff",()=>{
 const selection={...selected(),text:"one 💻tail",runs:[{text:"one ",styleId:0},{text:"💻tail",styleId:1}]};
 const paste=prepareParagraphPaste("💻tail",{start:0,end:0},"x\n👩\u200d");
 const editor=transferStyledDraft(selection,mapping({sourceText:"💻tail",sourceEnd:6,paragraphText:selection.text,paragraphEnd:10}),{originalText:"💻tail",...paste,font:null});
 expect(editor.present.selection).toEqual({start:11,end:11,direction:"forward"});
});

function onePiece(source:string){const base=styledEditorFixture();return {...base,objectIndices:[0],text:source,runs:[{text:source,styleId:0}],styles:[{...base.styles[0],objectIndices:[0]}]};}
function fullMap(source:string):ParagraphDraftMapping{return {sourceId:"source",pageIndex:0,objectIndices:[0],anchorIndex:0,paragraphText:source,sourceText:source,sourceStart:0,sourceEnd:source.length,paragraphStart:0,paragraphEnd:source.length};}
it.each([["AB","B"+"X".repeat(4094)+"\n"],["Aé","é"+"X".repeat(4093)+"\n"]])("accepts an exactly4096byte final replacement without intermediate overflow",(source,replacement)=>{
 expect(new TextEncoder().encode(replacement)).toHaveLength(4096);const selected=onePiece(source),editor=transferStyledDraft(selected,fullMap(source),draft(replacement,source));
 expect(styledEditorRequest(editor).runs.map(r=>r.text).join("")).toBe(replacement);expect(editor.past).toHaveLength(1);expect(styledEditorRequest(undoStyledEditor(editor)).runs).toEqual(selected.runs);expect(styledEditorRequest(redoStyledEditor(undoStyledEditor(editor))).runs).toEqual(styledEditorRequest(editor).runs);
});
it("keeps styled siblings at the exact final capacity while applying one atomic transfer",()=>{
 const chosen={...selected(),text:"LABR",runs:[{text:"L",styleId:0},{text:"AB",styleId:1},{text:"R",styleId:0}]},replacement="B"+"X".repeat(4092)+"\n";
 const editor=transferStyledDraft(chosen,mapping({paragraphText:chosen.text,sourceText:"AB",sourceEnd:2,paragraphStart:1,paragraphEnd:3}),draft(replacement,"AB"));
 expect(styledEditorRequest(editor).runs).toEqual([{text:"L",styleId:0},{text:replacement,styleId:1},{text:"R",styleId:0}]);expect(editor.past).toHaveLength(1);
});
it("still refuses a final paragraph exceeding capacity after unchanged siblings",()=>{
 const chosen={...selected(),text:"LABR",runs:[{text:"L",styleId:0},{text:"AB",styleId:1},{text:"R",styleId:0}]};
 expect(()=>transferStyledDraft(chosen,mapping({paragraphText:chosen.text,sourceText:"AB",sourceEnd:2,paragraphStart:1,paragraphEnd:3}),draft("B"+"X".repeat(4094)+"\n","AB"))).toThrow();expect(chosen.text).toBe("LABR");
});
it("keeps a collapsed caret when the final paragraph joins a grapheme outside the source piece",()=>{
 const chosen={...selected(),text:"A💻",runs:[{text:"A💻",styleId:0}]},map=mapping({anchorIndex:0,sourceText:"A",sourceEnd:1,paragraphText:chosen.text,paragraphStart:0,paragraphEnd:1});
 const editor=transferStyledDraft(chosen,map,draft("x\n👩\u200d","A"));expect(editor.present.selection).toEqual({start:7,end:7,direction:"forward"});
});

it("drops inactive source styles after atomic handoff with a caret at zero",()=>{
 const base=styledEditorFixture(),chosen={...base,objectIndices:[0],text:"AB",runs:[{text:"A",styleId:0},{text:"B",styleId:1}],styles:base.styles.map(style=>({...style,objectIndices:[0]}))};
 const editor=transferStyledDraft(chosen,fullMap("AB"),{...draft("B","AB"),selection:{start:0,end:0,direction:"forward"}});
 expect(editor.present.styles.map(style=>style.id)).toEqual([1]);expect(editor.present.draft.styleIds).toEqual([1]);expect(styledEditorRequest(undoStyledEditor(editor)).runs).toEqual(chosen.runs);
});
