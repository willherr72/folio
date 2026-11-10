import type {FolioAdapter} from "../editor/adapter";
import {useEffect,useState} from "react";
import {validateExistingImages,visibleImagePolygon} from "../editor/existing-images";
import "./existing-images.css";
import type {ExistingImage,PagePlan} from "../editor/types";
export function ExistingImageLayer({adapter,page,disabled,onEditImage}:{adapter:FolioAdapter;page:PagePlan;disabled?:boolean;onEditImage?(image:ExistingImage):void}){
 const key=`${page.sourceId}:${page.pageIndex}`;
 const [state,setState]=useState<{key:string;adapter:FolioAdapter;images?:ExistingImage[];error?:string}>({key,adapter});
 useEffect(()=>{let active=true;setState({key,adapter});
  void (async()=>{try{if(!adapter.listExistingImages)throw new Error("Image replacement requires the desktop PDF service.");const images=validateExistingImages(await adapter.listExistingImages(page.sourceId,page.pageIndex));if(active)setState({key,adapter,images});}
   catch(error){if(active)setState({key,adapter,images:[],error:error instanceof Error?error.message:String(error)});}})();
  return()=>{active=false;};
 },[adapter,key,page.sourceId,page.pageIndex]);
 const current=state.key===key&&state.adapter===adapter?state:undefined;
 const visible=(current?.images??[]).map(image=>({image,points:visibleImagePolygon(image,page)})).filter(v=>v.points.length>=3);
 const message=!current?.images?"Finding images…":!visible.length?current.error??"No editable images in the visible page area.":null;
 return <g className="existing-image-layer" onPointerDown={event=>event.stopPropagation()}>
  {visible.map(({image,points},i)=><polygon key={image.selectionId} points={points.map(p=>`${p.x},${p.y}`).join(" ")} className="existing-image-target" role="button" tabIndex={disabled?-1:0} aria-disabled={disabled||undefined} aria-label={`Edit image ${i+1}: ${image.pixelWidth} by ${image.pixelHeight} pixels`}
   onClick={event=>{event.stopPropagation();if(!disabled)onEditImage?.(image);}} onKeyDown={event=>{if(event.key==="Enter"||event.key===" "){event.preventDefault();event.stopPropagation();if(!disabled)onEditImage?.(image);}}}><title>Edit this image</title></polygon>)}
  {message&&<foreignObject x={(page.crop?.x??0)+12} y={(page.crop?.y??0)+12} width={Math.max(1,(page.crop?.width??page.width)-24)} height={Math.max(1,Math.min(120,(page.crop?.height??page.height)-24))} pointerEvents="none"><div className="existing-image-cue" role="status">{message}</div></foreignObject>}
 </g>;
}
