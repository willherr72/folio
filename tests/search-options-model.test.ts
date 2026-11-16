import {describe,expect,it} from "vitest";
import {findPageMatches} from "../src/editor/search";
import cases from "./fixtures/search/matching.json";
import type {PagePlan,PageText} from "../src/editor/types";

const page:PagePlan={id:"find-options",sourceId:"find-options-source",pageIndex:0,width:600,height:800,rotation:0,overlays:[]};

describe("document Find Unicode and word options",()=>{
 for(const fixture of cases)it(fixture.name,()=>{
  const parts=fixture.parts??Array.from(fixture.text!);
  const text:PageText={characters:parts.map((text,index)=>({text,x:index*6,y:20,width:6,height:6}))};
  const matches=findPageMatches(page,text,fixture.query,fixture.options??{matchCase:false,wholeWords:false});
  // Fixed one-box-per-part geometry makes the original glyph range observable.
  const actual=matches.map(match=>[Math.min(...match.rects.map(r=>r.x))/6,Math.max(...match.rects.map(r=>r.x+r.width))/6]);
  expect(actual).toEqual(fixture.expected);
 });
});
