import {useEffect,useMemo,useState} from "react";
import {formAnchorIdentity,type FormAnchorCallback} from '../editor/form-canvas-entry';
import type {FolioAdapter} from "../editor/adapter";
import type {FormField,PagePlan} from "../editor/types";
import {pageVisibleRect} from "../editor/geometry";
import {pageAllows} from "../editor/protection";
import {FormPageTargets} from "./FormPageTargets";
import "./form-fill.css";

export function CanvasFormLayer({adapter,page,selectedAnnotationIndex,disabled,dirty,onSelect,onAnchor}:{adapter:FolioAdapter;page:PagePlan;selectedAnnotationIndex?:number;disabled:boolean;dirty:boolean;onSelect(field:FormField):void;onAnchor?:FormAnchorCallback}){
 const [loaded,setLoaded]=useState<{adapter:FolioAdapter;sourceId:string;pageIndex:number;fields:FormField[];error?:string}|null>(null);
 const allowed=pageAllows(page,"fillForms");
 useEffect(()=>{
  if(!allowed||!adapter.listFormFields)return;
  let active=true;
  void adapter.listFormFields(page.sourceId,page.pageIndex).then(fields=>{if(active)setLoaded({adapter,sourceId:page.sourceId,pageIndex:page.pageIndex,fields});})
   .catch(error=>{if(active)setLoaded({adapter,sourceId:page.sourceId,pageIndex:page.pageIndex,fields:[],error:error instanceof Error?error.message:String(error)});});
  return()=>{active=false;};
 },[adapter,page.sourceId,page.pageIndex,allowed]);
 const current=loaded?.adapter===adapter&&loaded.sourceId===page.sourceId&&loaded.pageIndex===page.pageIndex?loaded:null;
 const fields=allowed?(current?.fields??[]):[],selected=fields.findIndex(field=>field.annotationIndex===selectedAnnotationIndex);
 const field=fields[selected];
 const identity=useMemo(()=>field?formAnchorIdentity(adapter,page,field):null,[adapter,page.id,page.sourceId,page.pageIndex,page.width,page.height,page.rotation,page.crop,field]);
 const targetRef=useMemo(()=>{
  let owned:SVGRectElement|null=null;
  return (node:SVGRectElement|null)=>{const previous=owned;owned=node;if(identity)onAnchor?.(identity,node,previous);};
 },[identity,onAnchor]);
 const message=!allowed?"This PDF restricts filling forms.":current?.error??(!current?"Finding form fields…":!fields.length?"No interactive form fields on this page.":null);
 const visible=pageVisibleRect(page);
 return <>
  <FormPageTargets embedded clipToCrop page={page} fields={fields} selected={selected} selectedTargetRef={targetRef} disabled={disabled} dirty={dirty} onSelect={index=>{if(!disabled&&(!dirty||index===selected)&&fields[index])onSelect(fields[index]);}}/>
  {message&&<foreignObject x={visible.x+8} y={visible.y+8} width={Math.max(1,visible.width-16)} height={Math.max(1,Math.min(70,visible.height-16))} pointerEvents="none"><div className="canvas-form-cue" role={current?.error?"alert":"status"}>{message}</div></foreignObject>}
 </>;
}
