import {expect,it} from "vitest";
import {pageWindow,revealPage,anchorPageWindow,type PageWindowMetrics} from "../src/editor/page-window";
const metrics:PageWindowMetrics={columns:4,itemWidth:135,itemHeight:180,gap:12,paddingTop:8,height:400};
it("bounds mounted indexes independently of document length and includes partial final rows",()=>{
 const first=pageWindow(10000,metrics,0,[]);expect(first.indices).toEqual(Array.from({length:20},(_,i)=>i));expect(first.totalHeight).toBe(2500*192-12);
 const last=pageWindow(9999,metrics,9999999,[]);expect(last.indices.at(-1)).toBe(9998);expect(last.indices.length).toBeLessThanOrEqual(28);
 expect(pageWindow(0,metrics,0,[])).toEqual({indices:[],totalHeight:0});
});
it("pins only valid focus and drag rows, deduplicated and ordered",()=>{
 const result=pageWindow(10000,metrics,0,[9999,9999,5000,-1,10000,NaN]);expect(result.indices).toContain(9999);expect(result.indices).toContain(5000);expect(result.indices.length).toBe(28);expect(result.indices).toEqual([...new Set(result.indices)].sort((a,b)=>a-b));
});
it("reveals outside targets without moving an already visible tile",()=>{
 expect(revealPage(0,metrics,0)).toBe(0);expect(revealPage(4,metrics,0)).toBe(0);
 const end=revealPage(9999,metrics,0);expect(end).toBe(8+2499*192+180-400);expect(revealPage(0,metrics,end)).toBe(8);
});
it("preserves the first visible page on responsive column and row-height changes",()=>{
 const next={...metrics,columns:2,itemHeight:150,gap:6};const at=8+20*192+40;
 expect(anchorPageWindow(at,metrics,next)).toBe(8+40*156+40);
 expect(anchorPageWindow(0,metrics,next)).toBe(0);
});
it("keeps a bounded window for zero-height containers and clamps negative scrolling",()=>{
 const zero=pageWindow(10000,{...metrics,height:0},0,[]);expect(zero.indices.length).toBeLessThanOrEqual(16);expect(zero.indices[0]).toBe(0);
 expect(pageWindow(10000,metrics,-100,[])).toEqual(pageWindow(10000,metrics,0,[]));
});
