import {expect,it} from "vitest";
import {createStyledParagraph,styledText,replaceStyledRange,formatStyledRange,styledRangeStyle,snapStyledCaret} from "../src/editor/styled-paragraph";
const run=(text:string,styleId:number)=>({text,styleId});
const source=()=>createStyledParagraph([run("plain ",0),run("bold",1),run(" end",0)],[0,1,2]);
it("coalesces adjacent styles and owns immutable snapshots without changing input",()=>{
 const input=[run("a",0),run("b",0),run("",1),run("c",1)];const doc=createStyledParagraph(input,[0,1]);expect(doc.runs).toEqual([run("ab",0),run("c",1)]);expect(input).toHaveLength(4);expect(Object.isFrozen(doc)).toBe(true);expect(Object.isFrozen(doc.runs)).toBe(true);expect(Object.isFrozen(doc.runs[0])).toBe(true);input[0].text="changed";expect(styledText(doc)).toBe("abc");
});
it("uses preceding style for boundary insertion and the first selected style for replacement",()=>{
 expect(replaceStyledRange(source(),6,6,"new ").runs).toEqual([run("plain new ",0),run("bold",1),run(" end",0)]);
 expect(replaceStyledRange(source(),6,10,"strong").runs).toEqual([run("plain ",0),run("strong",1),run(" end",0)]);
 expect(replaceStyledRange(source(),0,0,"start ").runs[0]).toEqual(run("start plain ",0));
});
it("keeps untouched ranges styled when replacing across boundaries or repeated words",()=>{
 expect(replaceStyledRange(source(),4,8,"X").runs).toEqual([run("plaiX",0),run("ld",1),run(" end",0)]);
 const doc=createStyledParagraph([run("word ",0),run("word",1),run(" word",2)],[0,1,2]);expect(replaceStyledRange(doc,5,9,"word word").runs).toEqual([run("word ",0),run("word word",1),run(" word",2)]);
});
it("deletes and rejoins matching neighbors without altering other formatting",()=>{
 expect(replaceStyledRange(source(),6,10,"").runs).toEqual([run("plain  end",0)]);expect(styledText(source())).toBe("plain bold end");
});
it("accepts explicit admitted insertion style and normalizes pasted line endings only",()=>{
 const doc=replaceStyledRange(source(),6,6,"café\r\nnew\r",2);expect(doc.runs).toEqual([run("plain ",0),run("café\nnew\n",2),run("bold",1),run(" end",0)]);
});
it("applies the size bound to normalized paste while still bounding raw input",()=>{
 const doc=createStyledParagraph([],[0]);expect(styledText(replaceStyledRange(doc,0,0,"\r\n".repeat(2049)))).toBe("\n".repeat(2049));
 expect(styledText(replaceStyledRange(doc,0,0,"\r\n".repeat(4096)))).toHaveLength(4096);
 expect(()=>replaceStyledRange(doc,0,0,"\r\n".repeat(4097))).toThrow(/4096/);
 expect(()=>replaceStyledRange(doc,0,0,"é".repeat(2049))).toThrow(/4096/);
});
it("formats only the selected range and reports mixed selection accurately",()=>{
 const doc=formatStyledRange(source(),3,8,2);expect(doc.runs).toEqual([run("pla",0),run("in bo",2),run("ld",1),run(" end",0)]);
 expect(styledRangeStyle(source(),0,14)).toBeNull();expect(styledRangeStyle(source(),6,10)).toBe(1);expect(styledRangeStyle(source(),6,6)).toBe(0);expect(formatStyledRange(source(),3,3,2).runs).toEqual(source().runs);
});
it("retains typing style after deleting everything and supports empty drafts",()=>{
 const doc=replaceStyledRange(createStyledParagraph([run("bold",1)],[0,1]),0,4,"");expect(doc.runs).toEqual([run("",1)]);expect(replaceStyledRange(doc,0,0,"again").runs).toEqual([run("again",1)]);expect(formatStyledRange(doc,0,0,0).runs).toEqual([run("",0)]);
});
it("retains the empty typing style when copying or restoring a serialized draft",()=>{
 const doc=replaceStyledRange(createStyledParagraph([run("bold",1)],[0,1]),0,4,"");const restored=JSON.parse(JSON.stringify(doc));
 expect(createStyledParagraph(restored.runs,restored.styleIds).runs).toEqual([run("",1)]);
});
it("keeps Unicode unchanged and rejects partial grapheme or surrogate edits",()=>{
 const doc=createStyledParagraph([run("x😀",0),run("a\u0301y",1)],[0,1]);expect(replaceStyledRange(doc,1,3,"é").runs).toEqual([run("xé",0),run("a\u0301y",1)]);
 for(const [start,end] of [[2,2],[3,4],[4,5]])expect(()=>replaceStyledRange(doc,start,end,"z")).toThrow(/grapheme/);
 expect(()=>createStyledParagraph([run("a",0),run("\u0301",1)],[0,1])).toThrow(/grapheme/);expect(()=>replaceStyledRange(doc,0,0,"\uD800")).toThrow(/Unicode/);
});
it("rejects newly joined graphemes with incompatible styles atomically",()=>{
 const doc=createStyledParagraph([run("a\n",0),run("\u0301b",1)],[0,1]);expect(()=>replaceStyledRange(doc,1,2,"")).toThrow(/grapheme/);expect(doc.runs).toEqual([run("a\n",0),run("\u0301b",1)]);
 const plain=createStyledParagraph([run("a",0)],[0,1]);expect(()=>replaceStyledRange(plain,1,1,"\u0301",1)).toThrow(/grapheme/);expect(styledText(plain)).toBe("a");
});
it.each([["\u0301b","a",1,2],["\u200d💻!","👩",2,5],["🇸!","🇺",2,4]] as const)("snaps caret after Unicode joins in %j",(tail,inserted,raw,after)=>{
 const doc=replaceStyledRange(createStyledParagraph([run(tail,0)],[0]),0,0,inserted);expect(snapStyledCaret(doc,raw)).toBe(after);expect(snapStyledCaret(doc,raw,"backward")).toBe(0);expect(styledRangeStyle(doc,after,after)).toBe(0);expect(snapStyledCaret(doc,after)).toBe(after);expect(()=>snapStyledCaret(doc,-1)).toThrow(/range/);
});
it("copies and freezes the palette independently of caller mutations",()=>{
 const palette=[0,1];const doc=createStyledParagraph([run("a",1)],palette);palette[1]=99;expect(doc.styleIds).toEqual([0,1]);expect(Object.isFrozen(doc.styleIds)).toBe(true);
});
it("bounds palettes, ranges and UTF-8 bytes before admitting a draft",()=>{
 expect(()=>createStyledParagraph([run("x",2)],[0,1])).toThrow(/style/);expect(()=>createStyledParagraph([],[0,0])).toThrow(/palette/);expect(()=>createStyledParagraph([],Array.from({length:65},(_,i)=>i))).toThrow(/palette/);
 const doc=createStyledParagraph([run("é".repeat(2048),0)],[0]);expect(()=>replaceStyledRange(doc,2048,2048,"a")).toThrow(/4096/);
 for(const [a,b] of [[-1,0],[0,100],[2,1],[0.5,1]])expect(()=>replaceStyledRange(source(),a,b,"x")).toThrow(/range/);
 expect(()=>replaceStyledRange(source(),0,0,"x",99)).toThrow(/style/);expect(()=>createStyledParagraph([run("bad\rbreak",0)],[0])).toThrow(/line/);
});
it("matches a per-character style oracle for every ASCII replacement range",()=>{
 const doc=createStyledParagraph([run("ab",0),run("cd",1),run("ef",2)],[0,1,2]),text=styledText(doc),styles=[0,0,1,1,2,2];
 for(let start=0;start<=6;start++)for(let end=start;end<=6;end++)for(const inserted of ["","X Y","\n"]){
  const next=replaceStyledRange(doc,start,end,inserted),style=styles[start===end?Math.max(0,start-1):start]??2;
  expect(styledText(next)).toBe(text.slice(0,start)+inserted+text.slice(end));
  expect(next.runs.flatMap(r=>Array.from(r.text,()=>r.styleId))).toEqual([...styles.slice(0,start),...Array(inserted.length).fill(style),...styles.slice(end)]);
 }
 expect(doc.runs).toEqual([run("ab",0),run("cd",1),run("ef",2)]);
});
