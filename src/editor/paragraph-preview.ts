import type {FolioAdapter} from "./adapter";
import type {AnnotationRect,Overlay} from "./types";

export const NO_PARAGRAPH_OVERLAYS:Overlay[]=[];
export const MAX_PARAGRAPH_PIECES=64;
export function validParagraphBounds(bounds:AnnotationRect,pageWidth:number,pageHeight:number):boolean {
 return [bounds.x,bounds.y,bounds.width,bounds.height,pageWidth,pageHeight].every(Number.isFinite)
  && pageWidth>0&&pageHeight>0&&bounds.x>=0&&bounds.y>=0&&bounds.width>0&&bounds.height>0
  && bounds.x+bounds.width<=pageWidth&&bounds.y+bounds.height<=pageHeight;
}
export function moveParagraphBounds(bounds:AnnotationRect,x:number,y:number,pageWidth:number,pageHeight:number):AnnotationRect|null {
 if(!validParagraphBounds(bounds,pageWidth,pageHeight)||![x,y].every(Number.isFinite))return null;
 return {...bounds,x:Math.max(0,Math.min(x,pageWidth-bounds.width)),y:Math.max(0,Math.min(y,pageHeight-bounds.height))};
}
/** Preview in source axes so text bounds and selection targets remain aligned. */
export function renderParagraphPage(adapter:FolioAdapter,sourceId:string,pageIndex:number,width:number,height:number,overlays:Overlay[],pixelWidth=1600):Promise<string> {
 if(!overlays.length)return adapter.renderPage(sourceId,pageIndex,pixelWidth);
 if(!adapter.renderPagePlan)return Promise.reject(new Error("Annotated paragraph previews are unavailable in this adapter."));
 return adapter.renderPagePlan({id:"paragraph-preview",sourceId,pageIndex,width,height,rotation:0,overlays},pixelWidth);
}
