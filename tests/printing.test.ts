import {it,expect} from "vitest";
import {selectPrintPages} from "../src/editor/printing";
import {createSession} from "../src/editor/workspace";
import {nativeAdapter} from "../src/editor/adapter";
const pages=createSession({id:"s",name:"a.pdf",pages:Array.from({length:5},()=>({width:300,height:400}))},nativeAdapter,100).history.present.pages;
it("prints selected unique pages in document order preserving unsaved plans",()=>{
 pages[2].rotation=90;
 const selected=selectPrintPages(pages,"range",null,"5, 2-3, 2");
 expect(selected).toEqual([pages[1],pages[2],pages[4]]);
 expect(selected[1]).toBe(pages[2]);
 expect(selectPrintPages(pages,"current",pages[3].id,"")).toEqual([pages[3]]);
 expect(selectPrintPages(pages,"all",null,"")).toEqual(pages);
});
it.each(["","0","6","3-1","1-","1,,2","1.5","1-two"])("rejects invalid page ranges %s",range=>{
 expect(()=>selectPrintPages(pages,"range",null,range)).toThrow(/page|range/i);
});
