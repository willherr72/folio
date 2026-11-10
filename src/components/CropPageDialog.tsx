import {useMemo,useRef,useState,type PointerEvent} from "react";
import {Modal} from "./Modal";
import type {FolioAdapter} from "../editor/adapter";
import type {AnnotationRect,PagePlan} from "../editor/types";
import {cropFromMargins,cropMargins,type CropMargins} from "../editor/page-crop";
import {displayDimensions,pageVisibleRect} from "../editor/geometry";
import {pageAllows} from "../editor/protection";
import {useComposedPage} from "../editor/use-composed-page";
import "./page-crop.css";

const sides=["left","top","right","bottom"] as const;
type Fields=Record<keyof CropMargins,string>;
const fields=(m:CropMargins):Fields=>({left:String(m.left),top:String(m.top),right:String(m.right),bottom:String(m.bottom)});
const labels={left:"Left",top:"Top",right:"Right",bottom:"Bottom"};
const handles=["nw","n","ne","e","se","s","sw","w"] as const;
type Handle=typeof handles[number];
interface Props {adapter:FolioAdapter;page:PagePlan;onApply(crop?:AnnotationRect):void;onCancel():void}

export function CropPageDialog({adapter,page,onApply,onCancel}:Props) {
  const [values,setValues]=useState(()=>fields(cropMargins(page)));
  const [previewPlan,setPreviewPlan]=useState<PagePlan|null>(null);
  const [applied,setApplied]=useState(false),submitted=useRef(false);
  const gesture=useRef<{pointer:number;handle:Handle;box:AnnotationRect;start:{x:number;y:number}}|null>(null);
  const full=displayDimensions(page.width,page.height,page.rotation);
  const base=useMemo(()=>({...page,crop:undefined}),[page]);
  const original=useComposedPage(adapter,base,900,true,true);
  const draft=useMemo(()=>sides.some(side=>values[side].trim()==="")?null:cropFromMargins(page,{left:Number(values.left),top:Number(values.top),right:Number(values.right),bottom:Number(values.bottom)}),[page,values]);
  const box=draft?pageVisibleRect({...page,crop:draft}):null;
  const preview=useComposedPage(adapter,previewPlan??base,900,!!previewPlan,true);
  const allowed=pageAllows(page,"editContent")&&!!adapter.renderPagePlan;
  const error=!allowed?"This PDF does not permit cropping pages.":!draft?"Use nonnegative margins that leave at least 1 pt of width and height.":null;
  const ready=!!previewPlan&&!!preview.url&&!preview.loading&&!preview.error&&!error&&!applied;
  const change=(next:Fields)=>{setValues(next);setPreviewPlan(null);};
  const position=(event:PointerEvent<SVGSVGElement>)=>{const r=event.currentTarget.getBoundingClientRect();return {x:(event.clientX-r.left)*full.width/r.width,y:(event.clientY-r.top)*full.height/r.height};};
  const move=(event:PointerEvent<SVGSVGElement>)=>{
    const active=gesture.current;if(!active||active.pointer!==event.pointerId)return;
    const p=position(event),dx=p.x-active.start.x,dy=p.y-active.start.y,b=active.box;
    let left=b.x,top=b.y,right=b.x+b.width,bottom=b.y+b.height;
    if(active.handle.includes("w"))left=Math.max(0,Math.min(right-1,b.x+dx));
    if(active.handle.includes("e"))right=Math.min(full.width,Math.max(left+1,b.x+b.width+dx));
    if(active.handle.includes("n"))top=Math.max(0,Math.min(bottom-1,b.y+dy));
    if(active.handle.includes("s"))bottom=Math.min(full.height,Math.max(top+1,b.y+b.height+dy));
    change(fields({left,top,right:full.width-right,bottom:full.height-bottom}));
  };
  const finish=(event:PointerEvent<SVGSVGElement>,cancel=false)=>{
    const active=gesture.current;if(!active||active.pointer!==event.pointerId)return;
    if(cancel)change(fields({left:active.box.x,top:active.box.y,right:full.width-active.box.x-active.box.width,bottom:full.height-active.box.y-active.box.height}));else move(event);
    gesture.current=null;try{event.currentTarget.releasePointerCapture(event.pointerId);}catch{/* Capture can already be released. */}
  };
  const apply=()=>{if(!ready||!draft||submitted.current)return;submitted.current=true;setApplied(true);onApply(draft.x===0&&draft.y===0&&draft.width===page.width&&draft.height===page.height?undefined:draft);};
  return <Modal className="crop-page-dialog" title="Crop page" description="Cropping hides content outside the selected area; it does not remove it." onClose={onCancel}>
    <div className="crop-page-layout">
      <div className="crop-source-panel">
        <p>Original visible area · drag an edge or corner</p>
        <div className="crop-source-frame" style={{aspectRatio:`${full.width}/${full.height}`,maxWidth:Math.min(600,420*full.width/full.height)}}>
          {original.url&&<img src={original.url} alt="Original page with current edits"/>}
          {!original.url&&<span role="status">Rendering original page…</span>}
          <svg aria-label="Crop area" viewBox={`0 0 ${full.width} ${full.height}`} onPointerMove={move} onPointerUp={event=>finish(event)} onPointerCancel={event=>finish(event,true)} onLostPointerCapture={event=>finish(event,true)}>
            {box&&<>
              <path className="crop-shade" fillRule="evenodd" d={`M0 0H${full.width}V${full.height}H0Z M${box.x} ${box.y}h${box.width}v${box.height}h${-box.width}Z`}/>
              <rect className="crop-outline" {...box}/>
              {handles.map(handle=>{const x=handle.includes("w")?box.x:handle.includes("e")?box.x+box.width:box.x+box.width/2,y=handle.includes("n")?box.y:handle.includes("s")?box.y+box.height:box.y+box.height/2;
                const radius=Math.max(full.width,full.height)/65;
                return <rect key={handle} data-crop-handle={handle} className="crop-handle" x={x-radius} y={y-radius} width={radius*2} height={radius*2} style={{cursor:`${handle}-resize`}} onPointerDown={event=>{
                  if(event.button!==0||!allowed||applied)return;event.preventDefault();const svg=event.currentTarget.ownerSVGElement!;const r=svg.getBoundingClientRect();
                  gesture.current={pointer:event.pointerId,handle,box,start:{x:(event.clientX-r.left)*full.width/r.width,y:(event.clientY-r.top)*full.height/r.height}};
                  try{svg.setPointerCapture(event.pointerId);}catch{/* Synthetic events may not own capture. */}
                }}><title>{handle.toUpperCase()} crop handle; numeric margins provide keyboard control</title></rect>;
              })}
            </>}
          </svg>
        </div>
        {original.error&&<p role="alert" className="error-note">{original.error}</p>}
      </div>
      <div className="crop-settings">
        <fieldset disabled={!allowed||applied}><legend>Margins in displayed orientation</legend><div className="crop-margin-fields">
          {sides.map(side=><label key={side}>{labels[side]} margin (pt)<input type="number" min="0" step="any" value={values[side]} aria-invalid={!!error} onChange={event=>change({...values,[side]:event.target.value})}/></label>)}
        </div></fieldset>
        <p aria-live="polite">{box?`Retained size: ${box.width.toFixed(2)} × ${box.height.toFixed(2)} pt`:"Enter valid margins to set the retained size."}</p>
        <button className="button" disabled={!allowed||applied} onClick={()=>change(fields({left:0,top:0,right:0,bottom:0}))}>Reset crop</button>
        <p>Reset returns to this source’s original visible area. After saving and reopening, the saved crop becomes the new visible area.</p>
        {error&&<p role="alert" className="error-note">{error}</p>}
        <button className="button" disabled={!!error||applied||preview.loading} onClick={()=>{if(draft)setPreviewPlan({...page,crop:{...draft}});}}>Preview crop</button>
        {preview.loading&&<p role="status">Rendering cropped page…</p>}
        {preview.error&&<p role="alert" className="error-note">{preview.error}</p>}
        {ready&&<figure className="crop-result"><img src={preview.url} alt="Cropped page preview"/><figcaption>Saved and printed visible area</figcaption></figure>}
      </div>
    </div>
    <div className="dialog-actions"><button className="button" onClick={onCancel}>Cancel</button><button className="button primary" disabled={!ready} onClick={apply}>Apply crop</button></div>
  </Modal>;
}
