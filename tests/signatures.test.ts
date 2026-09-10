import { afterEach, describe, expect, it, vi } from "vitest";
import { deleteSignature, loadSignatures, MAX_SIGNATURES, renameSignature, resetSignatureLibrary, saveSignature, SIGNATURES_KEY } from "../src/editor/signatures";
const paths = [[{x:12,y:45},{x:88,y:15},{x:132,y:70}]];
afterEach(() => { vi.restoreAllMocks(); localStorage.clear(); });
describe("local signature library", () => {
 it("persists drawings across fresh reads, renames and deletes only the selected signature", () => {
  expect(loadSignatures()).toEqual([]);
  const first = saveSignature("  Everyday  ", paths)[0];
  const second = saveSignature("Initials", [[{x:2,y:3},{x:10,y:20}]])[1];
  expect(loadSignatures()).toEqual([{id:first.id,name:"Everyday",paths},second]);
  renameSignature(first.id,"Full name");
  expect(loadSignatures().map(item=>item.name)).toEqual(["Full name","Initials"]);
  deleteSignature(first.id);
  expect(loadSignatures()).toEqual([second]);
 });
 it("does not retain mutable drawing references", () => {
  const draft = structuredClone(paths);
  const entry = saveSignature("Original", draft)[0];
  draft[0][0].x = 300;
  entry.paths[0][0].y = 130;
  expect(loadSignatures()[0].paths).toEqual(paths);
 });
 it("rejects empty, oversized and duplicate names while preserving saved drawings", () => {
  const saved = saveSignature("Everyday", paths);
  for(const name of ["  ", "x".repeat(81), "EVERYDAY"]){expect(()=>saveSignature(name,paths)).toThrow();}
  expect(()=>renameSignature("missing","New name")).toThrow();
  expect(()=>deleteSignature("missing")).toThrow();
  expect(loadSignatures()).toEqual(saved);
 });
 it("rejects invalid points and excessive point counts before persisting", () => {
  for(const invalid of [[], [[]], [[{x:1,y:1}]], [[{x:NaN,y:0},{x:10,y:20}]], [[{x:361,y:0},{x:10,y:20}]], [Array.from({length:10_001},()=>({x:20,y:40}))]]){
   expect(()=>saveSignature("Invalid",invalid)).toThrow();
  }
  expect(localStorage.getItem(SIGNATURES_KEY)).toBeNull();
 });
 it("keeps malformed or unsupported storage untouched until explicitly reset", () => {
  for(const text of ["{broken", JSON.stringify({version:2,signatures:[]}),JSON.stringify({version:1,signatures:[{id:"a",name:"Bad",paths:[[{x:1,y:1},{x:2,y:null}]]}]}),"x".repeat(2_000_001)]){
   localStorage.setItem(SIGNATURES_KEY,text);
   expect(()=>loadSignatures()).toThrow();
   expect(()=>saveSignature("Fresh",paths)).toThrow();
   expect(localStorage.getItem(SIGNATURES_KEY)).toBe(text);
  }
  resetSignatureLibrary();
  expect(loadSignatures()).toEqual([]);
 });
 it("bounds library size without dropping existing entries", () => {
  for(let i=0;i<MAX_SIGNATURES;i++)saveSignature(`Signature ${i}`,paths);
  expect(()=>saveSignature("One too many",paths)).toThrow();
  expect(loadSignatures()).toHaveLength(MAX_SIGNATURES);
 });
 it("surfaces storage failures and leaves the last persisted library intact", () => {
  const saved=saveSignature("Kept",paths);
  vi.spyOn(Storage.prototype,"setItem").mockImplementation(()=>{throw new DOMException("Storage full","QuotaExceededError");});
  expect(()=>renameSignature(saved[0].id,"Lost rename")).toThrow(/save|storage/i);
  expect(loadSignatures()).toEqual(saved);
 });
});
