import {describe,expect,it} from "vitest";
import unicode from "../src/shared/search-unicode.json";
import {normalizeSearchQuery,searchCharacterRanges} from "../src/editor/search-matching";

describe("pinned document-search case folding",()=>{
 it("finds every complete retained mapping and rejects partial generated expansions",()=>{
  for(const [cp,fold]of unicode.folds as [number,string][]){
   const source=String.fromCodePoint(cp);
   expect([...searchCharacterRanges([source],fold)].map(({start,end})=>[start,end]),`U+${cp.toString(16)}`).toEqual([[0,1]]);
   if(Array.from(fold).length>1)expect([...searchCharacterRanges([source],Array.from(fold)[0])],`Partial U+${cp.toString(16)}`).toEqual([]);
  }
 });
 it("validates the raw query budget before full folding expands it",()=>{
  expect(normalizeSearchQuery("İ".repeat(512))).toHaveLength(1024);
  expect(()=>normalizeSearchQuery("İ".repeat(513))).toThrow(/too long/i);
  expect(()=>normalizeSearchQuery("\ud800")).toThrow(/Unicode/i);
  expect([...searchCharacterRanges(["A"]," \ufeff\t\n")]).toEqual([]);
 });
});
