import {expect,it} from "vitest";
import {validateExistingImages,visibleImagePolygon} from "../src/editor/existing-images";
import type {ExistingImage,PagePlan} from "../src/editor/types";
export const image:ExistingImage={objectPath:[2,0],selectionId:"a".repeat(64),corners:[{x:10,y:20},{x:110,y:20},{x:110,y:70},{x:10,y:70}],bounds:{x:10,y:20,width:100,height:50},pixelWidth:20,pixelHeight:10};
const page:PagePlan={id:"p",sourceId:"s",pageIndex:0,width:400,height:300,rotation:0,overlays:[]};
it("validates and copies image discovery without trusting malformed geometry",()=>{
 const result=validateExistingImages({images:[image]});expect(result).toEqual([image]);expect(result[0]).not.toBe(image);
 expect(()=>validateExistingImages({images:[{...image,objectPath:[]}]})).toThrow();expect(()=>validateExistingImages({images:[image,image]})).toThrow();
 expect(()=>validateExistingImages({images:[{...image,corners:[{x:0,y:0},{x:1,y:1},{x:0,y:1},{x:1,y:0}]}]})).toThrow();
});
it("clips actual polygons to the crop and excludes invisible skewed corners",()=>{
 expect(visibleImagePolygon(image,{...page,crop:{x:50,y:30,width:40,height:30}})).toEqual(expect.arrayContaining([{x:50,y:30},{x:90,y:30},{x:90,y:60},{x:50,y:60}]));
 const diamond={...image,corners:[{x:0,y:50},{x:50,y:0},{x:100,y:50},{x:50,y:100}] as ExistingImage["corners"]};
 expect(visibleImagePolygon(diamond,{...page,crop:{x:0,y:0,width:10,height:10}})).toEqual([]);
});
