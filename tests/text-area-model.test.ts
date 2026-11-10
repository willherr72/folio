import { expect, it } from "vitest";
import { cloneOverlay } from "../src/editor/model";
import type { TextOverlay } from "../src/editor/types";

it("clones saved area settings independently for duplicated pages", () => {
 const original: TextOverlay={type:"text",id:"area",x:0,y:0,text:"A\r\n\r\nB  ",fontSize:18,color:"#123456",fontId:"font",shaping:{version:1,direction:"ltr",ligatures:true},area:{version:1,width:180,height:240,inset:5,lineSpacing:1.2,alignment:"center"}};
 const copy=cloneOverlay(original) as TextOverlay;
 copy.area!.width=220;
 expect(original.area!.width).toBe(180);
 expect(copy.text).toBe(original.text);
});
