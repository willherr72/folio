import {useEffect,useRef,useState} from "react";
import type {FolioAdapter} from "../editor/adapter";
import {displayDimensions,pageVisibleRect} from "../editor/geometry";
import type {FormField,PagePlan} from "../editor/types";
import {FormPageTargets,formTargetBounds} from "./FormPageTargets";

const revoke=(url:string|null)=>{if(url?.startsWith("blob:"))URL.revokeObjectURL(url);};
export function FormPagePreview({adapter,page,fields,selected,waiting,busy,dirty,onSelect}:{adapter:FolioAdapter;page:PagePlan;fields:FormField[];selected:number;waiting:boolean;busy:boolean;dirty:boolean;onSelect(index:number):void}){
 const [zoom,setZoom]=useState(1),[failed,setFailed]=useState(false);
 const [raster,setRaster]=useState<{adapter:FolioAdapter;page:PagePlan;url:string}|null>(null);
 const owned=useRef<string|null>(null),scroll=useRef<HTMLDivElement>(null);
 const preview=raster?.adapter===adapter&&raster.page===page?raster.url:null;
 const targetPage=adapter.renderPagePlan?page:{...page,rotation:0 as const};
 const size=displayDimensions(targetPage.width,targetPage.height,targetPage.rotation);
 const renderWidth=Math.min(2400,Math.round(900*zoom));
 useEffect(()=>{
  setRaster(null);setFailed(false);
  return()=>{revoke(owned.current);owned.current=null;};
 },[adapter,page]);
 useEffect(()=>{
  let cancelled=false;setFailed(false);
  void Promise.resolve().then(()=>adapter.renderPagePlan?adapter.renderPagePlan({...page,crop:undefined},renderWidth):adapter.renderPage(page.sourceId,page.pageIndex,renderWidth)).then(url=>{
   if(cancelled){revoke(url);return;}
   const previous=owned.current;owned.current=url;setRaster({adapter,page,url});if(previous!==url)revoke(previous);
  }).catch(()=>{if(!cancelled)setFailed(true);});
  return()=>{cancelled=true;};
 },[adapter,page,renderWidth]);
 const canLocate=!!preview&&!waiting&&!!fields[selected]&&!!formTargetBounds(fields[selected],targetPage);
 const locate=()=>{
  const viewport=scroll.current,target=viewport?.querySelector<SVGRectElement>('.form-page-target[aria-pressed="true"]');
  if(!canLocate||!viewport||!target)return;
  const outer=viewport.getBoundingClientRect(),box=target.getBoundingClientRect();
  viewport.scrollLeft+=box.left+box.width/2-outer.left-viewport.clientLeft-viewport.clientWidth/2;
  viewport.scrollTop+=box.top+box.height/2-outer.top-viewport.clientTop-viewport.clientHeight/2;
 };
 return <section className="form-preview-panel" aria-label="Form page">
  <div className="form-preview-toolbar">
   <label>Zoom<select aria-label="Form preview zoom" value={zoom} onChange={event=>setZoom(Number(event.target.value))}>
    <option value={1}>Fit width</option><option value={1.5}>1.5× fit width</option><option value={2}>2× fit width</option><option value={3}>3× fit width</option>
   </select></label>
   <button type="button" className="button" disabled={!canLocate} onClick={locate}>Show selected field</button>
  </div>
  {page.crop&&<p>Full source shown for form editing. The dashed outline marks the saved and printed crop.</p>}
  {failed&&preview&&<p role="status">Higher-resolution preview unavailable.</p>}
  <div className="form-fill-preview" ref={scroll} role="region" aria-label="Scrollable form preview" tabIndex={0}>
   {preview?<div className="form-page-surface" style={{width:`${zoom*100}%`,aspectRatio:`${size.width} / ${size.height}`}}>
    <img src={preview} alt="Form page preview" draggable={false}/>
    {page.crop&&<svg className="form-page-targets" viewBox={`0 0 ${size.width} ${size.height}`} style={{pointerEvents:"none"}} aria-label="Retained crop area"><rect {...pageVisibleRect(targetPage)} fill="none" stroke="#2874b8" strokeWidth={2} strokeDasharray="6 4" vectorEffect="non-scaling-stroke"/></svg>}
    {!waiting&&<FormPageTargets page={targetPage} fields={fields} selected={selected} disabled={busy} dirty={dirty} onSelect={onSelect}/>}
   </div>:<p role="status">{failed?"Page preview unavailable.":"Rendering page…"}</p>}
  </div>
 </section>;
}
