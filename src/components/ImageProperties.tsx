import {useEffect,useId,useState} from "react";
import type {ImageOverlay} from "../editor/types";
import {imageBounds,moveImage,resizeImageTo,rotateImage} from "../editor/image-geometry";
import "./image-controls.css";
function ImageNumber({label,value,onCommit}:{label:string;value:number;onCommit(value:number|null):string|null}){
 const [draft,setDraft]=useState<string|null>(null),[error,setError]=useState<string|null>(null),id=useId();
 useEffect(()=>{setDraft(null);setError(null);},[value]);
 function save(raw:string|null){if(raw===null)return;const number=raw.trim()===""?null:Number(raw);setDraft(null);setError(onCommit(number));}
 return <label className="field-label">{label}<input type="number" step="any" value={draft??Number(value.toFixed(4))} aria-invalid={!!error} aria-describedby={error?id:undefined}
  onChange={event=>{setDraft(event.target.value);setError(null);}} onBlur={()=>save(draft)} onKeyDown={event=>{
   if(event.key==="Enter"){event.preventDefault();event.stopPropagation();save(draft);}
   if(event.key==="Escape"){event.preventDefault();event.stopPropagation();setDraft(null);setError(null);}
  }}/>{error&&<span id={id} className="image-properties-error" role="alert">{error}</span>}</label>;
}
export function ImageProperties({image,pageWidth,pageHeight,locked,onLockChange,onChange}:{image:ImageOverlay;pageWidth:number;pageHeight:number;locked:boolean;onLockChange(value:boolean):void;onChange(image:ImageOverlay):void}){
 const box=imageBounds(image);
 function update(axis:"x"|"y"|"width"|"height",value:number|null):string|null {
  if(value===null||!Number.isFinite(value)||(axis==="width"||axis==="height"?value<=0:value<0))return "Enter a valid number; size must be greater than zero.";
  let next:ImageOverlay;
  if(axis==="x"||axis==="y"){
   if(value+(axis==="x"?box.width:box.height)>(axis==="x"?pageWidth:pageHeight)+.00001)return "The image must stay inside the page.";
   next=moveImage(image,image.x+(axis==="x"?value-box.x:0),image.y+(axis==="y"?value-box.y:0),pageWidth,pageHeight);
  }else{
   const width=axis==="width"?value:locked?value*box.width/box.height:box.width;
   const height=axis==="height"?value:locked?value*box.height/box.width:box.height;
   if(box.x+width>pageWidth+.00001||box.y+height>pageHeight+.00001)return "That size does not fit on the page. Move the image or enter a smaller size.";
   next=resizeImageTo(image,width,height,pageWidth,pageHeight,locked,axis);
   const actual=imageBounds(next);
   if(Math.abs(actual.width-width)>Math.max(Number.EPSILON*32,width*1e-9)||Math.abs(actual.height-height)>Math.max(Number.EPSILON*32,height*1e-9))return "That size is too small to edit reliably. Enter a larger size.";
  }
  if(next!==image)onChange(next);return null;
 }
 return <div className="image-properties">
  <div className="image-dimensions">
   <ImageNumber label="Image left (pt)" value={box.x} onCommit={value=>update("x",value)}/><ImageNumber label="Image top (pt)" value={box.y} onCommit={value=>update("y",value)}/>
   <ImageNumber label="Image width (pt)" value={box.width} onCommit={value=>update("width",value)}/><ImageNumber label="Image height (pt)" value={box.height} onCommit={value=>update("height",value)}/>
  </div>
  <label className="field-label"><span><input type="checkbox" checked={locked} onChange={event=>onLockChange(event.target.checked)}/> Lock image proportions</span></label>
  <div className="property-grid"><button aria-label="Rotate image counterclockwise" onClick={()=>onChange(rotateImage(image,-1,pageWidth,pageHeight))}>Rotate left</button><button aria-label="Rotate image clockwise" onClick={()=>onChange(rotateImage(image,1,pageWidth,pageHeight))}>Rotate right</button></div>
  <p>{image.rotation}° clockwise · Dimensions follow the image frame on the original page. 72 pt = 1 inch.</p>
  <p>Drag a corner to resize. Arrow keys move the selected image or resize a focused corner; Shift makes larger steps. Esc cancels a drag.</p>
 </div>;
}
