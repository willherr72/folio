import {useEffect,useLayoutEffect,useMemo,useRef,useState,type ReactNode} from "react";
import type {FolioAdapter} from "../editor/adapter";
import type {AnnotationRect,EditableTextRun,Overlay} from "../editor/types";
import type {ParagraphPreview} from "../editor/paragraph-transaction";
import {paragraphRasterWidth,type ParagraphCanvasPresentation} from "../editor/paragraph-canvas";
import {renderParagraphPage} from "../editor/paragraph-preview";
import {usePageImage} from "./PageView";
import {useParagraphViewport} from "./ParagraphViewportHost";
import "./paragraph-workspace.css";

const revoke=(url?:string)=>{if(url?.startsWith("blob:"))URL.revokeObjectURL(url);};
function useRaster(adapter:FolioAdapter,sourceId:string|undefined,pageIndex:number,width:number,height:number,overlays:Overlay[],pixels:number,candidate?:ParagraphPreview|null){
 const [revision,setRevision]=useState(0),[state,setState]=useState<{request:object;url?:string;error?:string}>();
 const tail=useRef(Promise.resolve());
 const request=useMemo(()=>({sourceId,pageIndex,width,height,overlays,pixels,candidate,revision}),[sourceId,pageIndex,width,height,overlays,pixels,candidate,revision]);
 useEffect(()=>{
  if(!request.sourceId)return;
  let active=true,url:string|undefined;
  const done=request.candidate?.retain();
  // Serialize renders. Superseded queued requests never reach the native service.
  tail.current=tail.current.then(async()=>{
   if(!active)return;
   const result=await renderParagraphPage(adapter,request.sourceId!,request.pageIndex,request.width,request.height,request.overlays,request.pixels);
   if(!active){revoke(result);return;}url=result;setState({request,url});
  }).catch(error=>{if(active)setState({request,error:error instanceof Error?error.message:String(error)});}).finally(()=>{void done?.();});
  return()=>{active=false;revoke(url);};
 },[adapter,request]);
 return {url:state?.request===request?state.url:undefined,error:state?.request===request?state.error:undefined,loading:!!sourceId&&state?.request!==request,retry:()=>setRevision(v=>v+1)};
}

interface Props {
 canvas:ParagraphCanvasPresentation;adapter:FolioAdapter;sourceId:string;pageIndex:number;pageWidth:number;pageHeight:number;overlays:Overlay[];
 candidate:ParagraphPreview|null;onReady(candidate:ParagraphPreview|null,pixels:number):void;
 runs:EditableTextRun[];selected:number[];bounds?:AnnotationRect;choosing:boolean;enabled:boolean;loading?:boolean;dragging?:boolean;picker?:ReactNode;
 onToggle(index:number):void;onChange(bounds:AnnotationRect):void;onGestureActive(active:boolean):void;children:ReactNode;
}
export function ParagraphWorkspace({canvas,adapter,sourceId,pageIndex,pageWidth,pageHeight,overlays,candidate,onReady,runs,selected,bounds,choosing,enabled,loading=false,dragging,picker,onToggle,onChange,onGestureActive,children}:Props){
 const panelRef=useRef<HTMLElement>(null),entryFocusPending=useRef(true);
 useLayoutEffect(()=>{entryFocusPending.current=true;panelRef.current?.focus({preventScroll:true});},[]);
 useLayoutEffect(()=>{
  if(loading||!entryFocusPending.current)return;
  entryFocusPending.current=false;
  const panel=panelRef.current;
  // Inspection must not steal focus if the user moved elsewhere while waiting.
  if(!panel||panel.ownerDocument.activeElement!==panel)return;
  panel.querySelector<HTMLElement>(choosing?"fieldset.text-group-pieces":'textarea, [role="textbox"]')?.focus({preventScroll:true});
 },[loading,choosing]);
 const pixels=paragraphRasterWidth(pageWidth,canvas.zoom);
 const [originalRevision,setOriginalRevision]=useState(0);
 const cachedOriginal=usePageImage(adapter,{sourceId,pageIndex,width:pageWidth,height:pageHeight},pixels,!overlays.length,originalRevision);
 const composedOriginal=useRaster(adapter,overlays.length?sourceId:undefined,pageIndex,pageWidth,pageHeight,overlays,pixels);
 const original=overlays.length?composedOriginal:{...cachedOriginal,loading:!cachedOriginal.url&&!cachedOriginal.error,retry:()=>setOriginalRevision(value=>value+1)};
 const edited=useRaster(adapter,candidate?.document.id,candidate?.pageIndex??0,pageWidth,pageHeight,overlays,pixels,candidate);
 const [comparison,setComparison]=useState<"original"|"edited">("edited");
 const report=useRef(onReady);report.current=onReady;
 useLayoutEffect(()=>{report.current(candidate&&edited.url&&!edited.loading?candidate:null,pixels);},[candidate,edited.url,edited.loading,pixels]);
 useEffect(()=>setComparison("edited"),[candidate]);
 const modalChange=useRef(canvas.onModalChange);modalChange.current=canvas.onModalChange;
 useEffect(()=>{modalChange.current(!!picker);return()=>modalChange.current(false);},[!!picker]);
 const showEdited=!!candidate&&!choosing&&comparison==="edited",raster=showEdited?edited:original;
 const view={pageId:canvas.pageId,sessionKey:canvas.sessionKey,bounds,runs,selected,choosing,enabled:enabled&&!picker&&!raster.loading&&!!raster.url,raster,onChange,onToggle,onGestureActive};
 const hosted=useParagraphViewport(view);
 const panel=<>
  <section ref={panelRef} tabIndex={-1} className="paragraph-workspace-panel existing-text-dialog" aria-label="Edit paragraph">
   <header><h2>Edit paragraph</h2><p>Drag the area or its handles on the page. Preview checks the PDF before applying.</p>
    <div className="paragraph-workspace-toolbar"><button className="button" disabled={dragging||!!picker} onClick={canvas.onLocate}>Locate paragraph</button><button className="button" aria-pressed={!showEdited} disabled={dragging||choosing||!original.url} onClick={()=>setComparison("original")}>Original</button><button className="button" aria-pressed={showEdited} disabled={dragging||choosing||!candidate} onClick={()=>setComparison("edited")}>Edited</button></div>
    {raster.loading&&<p role="status">Rendering paragraph page…</p>}{raster.error&&<p role="alert">{raster.error} <button className="button" onClick={raster.retry}>Retry preview</button></p>}
   </header>
   {children}
  </section>
  {!hosted&&<div className="paragraph-workspace-page">{canvas.renderCanvas(view)}</div>}
  {picker}
 </>;
 return hosted?panel:<div className="paragraph-workspace">{panel}</div>;
}

/** Area changes queue one preview of the latest valid draft after the gesture. */
export function useParagraphCanvasPreview(canvas:ParagraphCanvasPresentation|undefined,candidate:ParagraphPreview|null,valid:boolean,busy:boolean,makePreview:()=>Promise<void>){
 const [ready,setReady]=useState<{candidate:ParagraphPreview|null;pixels:number}>(),[revision,setRevision]=useState(0);
 const consumed=useRef(0),preview=useRef(makePreview);preview.current=makePreview;
 useEffect(()=>{if(!canvas||!valid||busy||revision===consumed.current)return;const timer=setTimeout(()=>{consumed.current=revision;void preview.current();},350);return()=>clearTimeout(timer);},[!!canvas,valid,busy,revision]);
 return {ready:(pixels:number)=>!canvas||!!candidate&&ready?.candidate===candidate&&ready.pixels===pixels,onReady:(candidate:ParagraphPreview|null,pixels:number)=>setReady({candidate,pixels}),consume:()=>{consumed.current=revision;},queue:()=>setRevision(v=>v+1)};
}
