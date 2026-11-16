import {describe,expect,it} from "vitest";
import {prepareParagraphPaste} from "../src/editor/paragraph-paste";

describe("paragraph clipboard preparation",()=>{
 it("splices normalized hard and blank lines into the selected draft range",()=>{
  const range={start:5,end:9};const result=prepareParagraphPaste("keep THIS tail",range,"first\r\n\r\nsecond\rthird");
  expect(result).toEqual({replacement:"keep first\n\nsecond\nthird tail",selection:{start:24,end:24,direction:"forward"}});
  expect(range).toEqual({start:5,end:9});expect(Object.isFrozen(result)).toBe(true);expect(Object.isFrozen(result.selection)).toBe(true);
 });
 it("keeps a Unicode grapheme at either edge of the replacement intact",()=>{
  expect(prepareParagraphPaste("e\u0301😀tail",{start:2,end:4},"A\nB")).toEqual({replacement:"e\u0301A\nBtail",selection:{start:5,end:5,direction:"forward"}});
 });
 it.each([{start:1,end:2},{start:2,end:3}])("rejects a selection splitting an emoji or combining sequence %j",range=>{
  expect(()=>prepareParagraphPaste("😀e\u0301",range,"A\nB")).toThrow("whole characters");
 });
 it.each([{start:-1,end:0},{start:0,end:5},{start:2,end:1},{start:.5,end:1},{start:NaN,end:1}])("rejects invalid input selection %j",range=>{
  expect(()=>prepareParagraphPaste("text",range,"A\nB")).toThrow();
 });
 it("applies the byte limit after Windows newline normalization",()=>{
  const value="a".repeat(4094)+"\r\nz";const result=prepareParagraphPaste("",{start:0,end:0},value);
  expect(new TextEncoder().encode(result.replacement)).toHaveLength(4096);expect(result.selection.end).toBe(4096);
  expect(()=>prepareParagraphPaste("x",{start:1,end:1},value)).toThrow("too long");
 });
 it("uses complete Unicode bytes rather than input code-unit count",()=>{
  expect(()=>prepareParagraphPaste("",{start:0,end:0},"界".repeat(1366)+"\nZ")).toThrow("too long");
 });
 it.each(["A\t\nB","A\0\nB","A\u007f\nB","A\u0085\nB","\ud800\nB","A\n\udfff"])("rejects unsupported clipboard text without echoing it",value=>{
  expect(()=>prepareParagraphPaste("text",{start:0,end:4},value)).toThrow();
  try{prepareParagraphPaste("text",{start:0,end:4},value);}catch(error){expect(String(error)).not.toContain(value);}
 });
 it("rejects a blank replacement without changing the input",()=>{expect(()=>prepareParagraphPaste("text",{start:0,end:4},"\r\n ")).toThrow("Enter");});
});

it.each([["💻tail","x\n👩\u200d",7],["🇸tail","x\n🇺",6]])("keeps the caret collapsed when pasted text joins a suffix grapheme",(source,clipboard,caret)=>{
 expect(prepareParagraphPaste(source as string,{start:0,end:0},clipboard as string).selection).toEqual({start:caret,end:caret,direction:"forward"});
});
