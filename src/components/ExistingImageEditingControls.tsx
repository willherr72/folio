import {useLayoutEffect,useRef} from "react";
import type {CanvasImageView} from "../editor/existing-image-canvas";
import type {PagePlan} from "../editor/types";
import {ExistingImageControls} from "./ExistingImageControls";
import {ExistingImageCropControls} from "./ExistingImageCropControls";

export function ExistingImageEditingControls(props:{view:CanvasImageView;page:PagePlan;zoom:number}){
 const root=useRef<SVGGElement>(null);
 // Focus belongs to the editing session, not to a newly mounted mode. Keep the
 // action menu's focus when switching between position and crop controls.
 useLayoutEffect(()=>{
  root.current?.querySelector<SVGElement>(".existing-image-frame[role=button]")?.focus({preventScroll:true});
 },[props.view.sessionKey]);
 return <g ref={root}>{props.view.mode==="crop"?<ExistingImageCropControls {...props}/>:<ExistingImageControls {...props}/>}</g>;
}
