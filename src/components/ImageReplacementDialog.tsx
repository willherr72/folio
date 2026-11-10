import type {FolioAdapter} from "../editor/adapter";
import {useEffect,useRef,useState,type ReactNode} from "react";
import type {DocumentInfo,EditorDocument,ExistingImage,ExistingImageEdit,ImageCrop,ImageFitMode,PageContentReplacement,PagePlan} from "../editor/types";
import {imageMeasurements,imagePositionDraft,readImageMeasurements,imageTransform,resizeImageDraft,type ImageMeasurements} from "../editor/existing-image-transform";
import {reconstructImageFrame,type CanvasImageView,type ImageQuad} from "../editor/existing-image-canvas";
import {cropFrame,fullCrop,fullFrameFromVisible,validateCrop} from "../editor/existing-image-crop";
import {displayDimensions} from "../editor/geometry";
import {Modal} from "./Modal";
import {importImageLease,type ImageLease} from "../editor/image-resources";
import {pageReplacementRequest,replacementPages} from "../editor/page-replacement";
import {discardOpenedDocument} from "../editor/discard-opened-document";
import {useComposedPage} from "../editor/use-composed-page";
import "./existing-images.css";
export interface ImageReplacementProps {renderCanvas?(view:CanvasImageView):ReactNode;canvasZoom?:number;onLocate?():void;adapter:FolioAdapter;document:EditorDocument;page:PagePlan;image:ExistingImage;sourceIds:string[];isSourceRetained?(id:string):boolean;onApply(result:PageContentReplacement):boolean;onCancel():void}
const cropText=(crop:ImageCrop)=>Object.fromEntries(Object.entries(crop).map(([key,value])=>[key,String(Number((value*100).toFixed(6)))])) as Record<keyof ImageCrop,string>;
export function ImageReplacementDialog({adapter,document,page,image,sourceIds,isSourceRetained,onApply,onCancel,renderCanvas,canvasZoom=100,onLocate}:ImageReplacementProps){
 const [asset,setAsset]=useState<ImageLease|null>(null),assetOwner=useRef<ImageLease|null>(null),pendingAsset=useRef<ImageLease|null>(null);
 const [mode,setMode]=useState<ImageFitMode>("fit"),[comparison,setComparison]=useState<"original"|"edited">("edited"),[zoom,setZoom]=useState(100);
 const [action,setAction]=useState<"replace"|"transform"|"crop"|"remove">(renderCanvas?"transform":"replace");
 const [precision,setPrecision]=useState(!renderCanvas);
 const [crop,setCrop]=useState<ImageCrop>(()=>({...image.crop??fullCrop}));
 const [cropFields,setCropFields]=useState(()=>cropText(image.crop??fullCrop)),[cropInputError,setCropInputError]=useState<string|null>(null);
 const [fullBasis,setFullBasis]=useState<ImageQuad>(image.corners);
 const measurements=useRef(imageMeasurements(image,page)).current;
 const visibleMeasurements=useRef(imageMeasurements({corners:cropFrame(image.corners,image.crop)},page)).current;
 const [positionBasis,setPositionBasis]=useState(visibleMeasurements);
 const [gestureActive,setGestureActive]=useState(false),[autoRequest,setAutoRequest]=useState(0),autoIssued=useRef(0);
 const queuePreview=()=>{if(renderCanvas)setAutoRequest(value=>value+1);};
 const [position,setPosition]=useState(()=>imagePositionDraft(visibleMeasurements)),[lockedRatio,setLockedRatio]=useState<number|null>(visibleMeasurements.width/visibleMeasurements.height);
 const [candidate,setCandidate]=useState<{result:PageContentReplacement;page:PagePlan}|null>(null),candidateOwner=useRef<DocumentInfo|null>(null);
 const [busy,setBusy]=useState<"choosing"|"preparing"|null>(null),[error,setError]=useState<string|null>(null),[applied,setApplied]=useState(false);
 const alive=useRef(false),closed=useRef(false),running=useRef(false),generation=useRef(0),submitted=useRef(false);
 const initial=useRef({adapter,pages:document.pages,page,selection:image.selectionId});
 const sessionValid=initial.current.adapter===adapter&&initial.current.pages===document.pages&&initial.current.page===page&&initial.current.selection===image.selectionId;
 const currentSession=useRef(sessionValid);currentSession.current=sessionValid;
 const sources=useRef({sourceIds,isSourceRetained});sources.current={sourceIds,isSourceRetained};
 const borrowed=(id:string)=>sources.current.sourceIds.includes(id)||!!sources.current.isSourceRetained?.(id);
 const release=(info:DocumentInfo|null)=>{if(info?.id&&!borrowed(info.id))void discardOpenedDocument(info,id=>adapter.closeDocument(id)).catch(()=>{});};
 const disposeCandidate=()=>{const info=candidateOwner.current;candidateOwner.current=null;release(info);};
 const disposeAsset=()=>{const owned=assetOwner.current;assetOwner.current=null;if(owned&&owned!==pendingAsset.current)void owned.release().catch(()=>{});};
 useEffect(()=>{alive.current=true;return()=>{alive.current=false;generation.current++;disposeCandidate();disposeAsset();};},[]);
 const dimensions=displayDimensions(page.width,page.height,page.rotation);
 const rasterWidth=renderCanvas?Math.max(600,Math.min(2400,Math.round(dimensions.width*canvasZoom/100*(window.devicePixelRatio||1)))):1000;
 const original=useComposedPage(adapter,renderCanvas?{...page,crop:undefined}:page,rasterWidth,true,true);
 const previewPage=candidate?.page??page;
 const preview=useComposedPage(adapter,renderCanvas?{...previewPage,crop:undefined}:previewPage,rasterWidth,!!candidate,true);
 const adjusting=action==="transform"||action==="crop";
 let edit:ExistingImageEdit|null=null,draftError:string|null=null,fullFrame=fullBasis;
 const positioned=(value:ImageMeasurements):ImageQuad=>{
  if((Object.keys(value) as (keyof ImageMeasurements)[]).every(key=>value[key]===positionBasis[key]))return fullBasis;
  return fullFrameFromVisible(reconstructImageFrame({...image,corners:cropFrame(fullBasis,crop)},page,value),crop);
 };
 try{if(adjusting){fullFrame=positioned(readImageMeasurements(position,positionBasis));edit={kind:"adjust",transform:imageTransform(measurements,imageMeasurements({corners:fullFrame},page),page),crop};}else if(action==="remove")edit={kind:"remove"};}catch(cause){draftError=cause instanceof Error?cause.message:String(cause);}
 const geometryError=draftError;if(adjusting&&!draftError)draftError=cropInputError;
 let admission:string|null=null;try{pageReplacementRequest(document,page);if(!(action==="replace"?adapter.previewImageReplacementOccurrence:adapter.previewImageEditOccurrence)||!adapter.renderPagePlan)admission="Image editing requires the desktop PDF service.";if(!sessionValid)admission="The document changed. Close this dialog and select the image again.";}catch(cause){admission=cause instanceof Error?cause.message:String(cause);}
 const ready=!!candidate&&!!preview.url&&!preview.loading&&!preview.error&&!busy&&!applied&&!admission&&!gestureActive;
 const invalidate=()=>{generation.current++;disposeCandidate();setCandidate(null);setError(null);};
 const rebase=(full:ImageQuad,nextCrop:ImageCrop,refreshCropFields=true)=>{
  validateCrop(nextCrop);invalidate();queuePreview();setFullBasis(full);setCrop(nextCrop);
  if(refreshCropFields){setCropFields(cropText(nextCrop));setCropInputError(null);}
  const basis=imageMeasurements({corners:cropFrame(full,nextCrop)},page);
  setPositionBasis(basis);setPosition(imagePositionDraft(basis));if(lockedRatio!==null)setLockedRatio(basis.width/basis.height);
 };
 const close=()=>{closed.current=true;invalidate();disposeAsset();onCancel();};
 const choose=async()=>{
  if(running.current||submitted.current||closed.current||admission)return;invalidate();const request=generation.current;running.current=true;setBusy("choosing");let orphan:ImageLease|null=null;
  try{orphan=await importImageLease();if(!orphan||!alive.current||closed.current||!currentSession.current||request!==generation.current)return;disposeAsset();assetOwner.current=orphan;setAsset(orphan);orphan=null;}
  catch(cause){if(alive.current&&!closed.current&&request===generation.current)setError(cause instanceof Error?cause.message:String(cause));}
  finally{if(orphan)void orphan.release().catch(()=>{});running.current=false;if(alive.current)setBusy(null);}
 };
 const prepare=async()=>{
  if(running.current||submitted.current||closed.current||admission||draftError||(action==="replace"&&!asset))return;
  autoIssued.current=autoRequest;invalidate();const request=generation.current;running.current=true;setBusy("preparing");let orphan:DocumentInfo|null=null;pendingAsset.current=action==="replace"?asset:null;
  try{const selection=pageReplacementRequest(document,page);const result=action==="replace"
   ?await adapter.previewImageReplacementOccurrence!(selection.sourceId,selection.occurrences,selection.selectedOccurrence,[...image.objectPath],image.selectionId,asset!.registration.image.id,mode)
   :await adapter.previewImageEditOccurrence!(selection.sourceId,selection.occurrences,selection.selectedOccurrence,[...image.objectPath],image.selectionId,edit!);
   if(result?.document?.id&&!borrowed(result.document.id))orphan=result.document;
   if(!alive.current||closed.current||!currentSession.current||request!==generation.current)return;
   if(!result?.document?.id||borrowed(result.document.id))throw new Error("Image editing must return a new source version.");
   const pages=replacementPages(document,page,result),replaced=pages.find(v=>v.id===page.id)!;candidateOwner.current=result.document;orphan=null;setCandidate({result,page:replaced});setComparison("edited");
  }catch(cause){if(alive.current&&!closed.current&&request===generation.current)setError(cause instanceof Error?cause.message:String(cause));}
  finally{release(orphan);const pending=pendingAsset.current;pendingAsset.current=null;if(pending&&pending!==assetOwner.current)void pending.release().catch(()=>{});running.current=false;if(alive.current)setBusy(null);}
 };
 useEffect(()=>{
  if(!renderCanvas||autoRequest===autoIssued.current||!adjusting||gestureActive||busy||applied||admission||draftError)return;
  const timer=window.setTimeout(()=>void prepare(),300);return()=>window.clearTimeout(timer);
 },[autoRequest,action,gestureActive,busy,applied,admission,draftError,renderCanvas]);
 const apply=()=>{
  if(!ready||!candidate||submitted.current)return;submitted.current=true;const owned=candidateOwner.current;candidateOwner.current=null;
  try{if(!onApply(candidate.result))throw new Error("The document changed. Close this dialog and try again.");setApplied(true);}
  catch(cause){submitted.current=false;if(alive.current&&!closed.current){candidateOwner.current=owned;setError(cause instanceof Error?cause.message:String(cause));}else release(owned);}
 };
 const edited=comparison==="edited"&&!!candidate,shown=edited?preview:original;
 const controls=<>
  <div className="image-replacement-controls"><label>Image action<select aria-label="Image action" value={action} disabled={applied||!sessionValid||gestureActive} onChange={event=>{const next=event.target.value as typeof action;if(!(adjusting&&(next==="transform"||next==="crop"))){invalidate();autoIssued.current=autoRequest;}setAction(next);setComparison("edited");}}><option value="replace">Replace</option><option value="transform">Position and size</option><option value="crop">Crop</option><option value="remove">Remove</option></select></label>
   {renderCanvas&&<button className="button" aria-expanded={precision} disabled={gestureActive} onClick={()=>setPrecision(value=>!value)}>Precision and help</button>}
   {adjusting&&<button className="button" disabled={applied||!!admission||gestureActive} onClick={()=>rebase(image.corners,crop,false)}>Reset position</button>}
   {action==="crop"&&<button className="button" disabled={applied||!!admission||!!geometryError||gestureActive} onClick={()=>rebase(fullFrame,{...fullCrop})}>Restore full image</button>}
  </div>
  <div className="image-editing-settings">
  {action==="replace"&&<><div className="image-replacement-controls"><button className="button" disabled={!!busy||applied||!!admission} onClick={()=>void choose()}>Choose image…</button>
   {asset&&<span>{asset.registration.image.width} × {asset.registration.image.height} pixels</span>}
   <label>Image fitting<select aria-label="Image fitting" value={mode} disabled={applied||!!admission||gestureActive} onChange={event=>{invalidate();setMode(event.target.value as ImageFitMode);}}><option value="fit">Fit — show whole image</option><option value="fill">Fill — crop to frame</option><option value="stretch">Stretch — fill without cropping</option></select></label>
  </div>
  <p className="image-replacement-help">Replacement keeps the source position and crop. Pending position or crop adjustments are not included.</p></>}
  {adjusting&&<div hidden={!precision}>
   {action==="crop"&&<div className="image-position-fields">{([['left','Keep from left (%)'],['top','Keep from top (%)'],['right','Keep to right (%)'],['bottom','Keep to bottom (%)']] as [keyof ImageCrop,string][]).map(([key,label])=><label key={key}>{label}<input aria-label={label} type="number" step="any" min={0} max={100} value={cropFields[key]} disabled={applied||!!admission||!!geometryError||gestureActive} onChange={event=>{
    const fields={...cropFields,[key]:event.target.value};invalidate();setCropFields(fields);
    try{const next={...crop},prior=cropText(crop);for(const name of Object.keys(next) as (keyof ImageCrop)[]){if(!fields[name].trim()||!Number.isFinite(Number(fields[name])))throw new Error("Enter a valid percentage in every crop field.");next[name]=fields[name]===prior[name]?crop[name]:Number(fields[name])/100;}validateCrop(next);setCropInputError(null);rebase(fullFrame,next,false);}catch(cause){setCropInputError(cause instanceof Error?cause.message:String(cause));}
   }}/></label>)}</div>}
   <div className="image-position-fields">
   {([['centerX','Center X (pt)'],['centerY','Center Y (pt)'],['width','Width (pt)'],['height','Height (pt)'],['rotation','Clockwise angle (°)']] as [keyof ImageMeasurements,string][]).map(([key,label])=><label key={key}>{label}<input aria-label={label} type="number" step="any" value={position[key]} disabled={applied||!!admission||gestureActive} onChange={event=>{invalidate();queuePreview();const text=event.target.value;setPosition(previous=>key==="width"||key==="height"?resizeImageDraft(previous,key,text,lockedRatio):{...previous,[key]:text});}}/></label>)}
  </div>
   <p className="image-replacement-help">Position and size describe the visible crop. Center is measured from the full displayed page’s top-left corner. Width and height follow the image’s axes. Other PDF clipping stays in place.</p>
   {action==="crop"&&<p className="image-replacement-help">Drag crop corners to trim, or drag inside to slide the crop window. Arrows adjust; Shift takes larger steps. Cropping hides pixels and can be restored after saving. It does not redact them.</p>}
  </div>}
  {action==="remove"&&<p className="image-replacement-help">Remove this source image occurrence. Pending adjustments are not included. Other copies stay in place. You can Undo after applying. This is not secure redaction.</p>}
  </div>
  <div className="image-editing-status">{admission&&<p role="alert" className="error-note">{admission}</p>}{draftError&&<p role="alert" className="error-note">{draftError}</p>}{error&&<p role="alert" className="error-note">{error}</p>}{busy&&<p role="status">{busy==="choosing"?"Choosing image…":"Preparing image preview…"}</p>}
   {shown.error&&<p role="alert" className="error-note">{shown.error}</p>}{shown.loading&&<p role="status">Rendering page…</p>}
  </div>
  <div className="image-replacement-controls"><button className="button" aria-pressed={renderCanvas?comparison==="original":!edited} disabled={!original.url||gestureActive} onClick={()=>setComparison("original")}>Original</button><button className="button" aria-pressed={renderCanvas?comparison==="edited":edited} disabled={gestureActive||(!renderCanvas&&(!candidate||!preview.url))} onClick={()=>setComparison("edited")}>Edited</button>
   {adjusting&&<label><input type="checkbox" checked={lockedRatio!==null} disabled={applied||!!admission||gestureActive} onChange={event=>{if(!event.target.checked)setLockedRatio(null);else{try{const v=readImageMeasurements(position,positionBasis);setLockedRatio(v.width>0&&v.height>0?v.width/v.height:positionBasis.width/positionBasis.height);}catch{setLockedRatio(positionBasis.width/positionBasis.height);}}}}/>Lock aspect ratio</label>}
   {!renderCanvas&&<label>Zoom<select aria-label="Preview zoom" value={zoom} onChange={event=>setZoom(Number(event.target.value))}><option value={100}>Fit width</option><option value={150}>150%</option><option value={200}>200%</option><option value={300}>300%</option></select></label>}
  </div>
  {!renderCanvas&&<div className="image-replacement-scroll" tabIndex={0} aria-label="Scrollable image replacement preview">{shown.url&&<img src={shown.url} alt={edited?"Edited image preview":"Original page preview"} style={{width:`${zoom}%`}} draggable={false}/>}</div>}
  <div className="dialog-actions"><button className="button" onClick={close}>Cancel</button><button className="button" disabled={(action==="replace"&&!asset)||!!busy||applied||!!admission||!!draftError||gestureActive} onClick={()=>void prepare()}>{action==="replace"?"Preview replacement":"Preview changes"}</button><button className="button primary" disabled={!ready||!!draftError} onClick={apply}>{action==="replace"?"Apply replacement":"Apply changes"}</button></div>
 </>;
 if(!renderCanvas)return <Modal title="Edit image" className="image-replacement-dialog" description="Preview changes on the page before applying. You can undo after applying." onClose={close}>{controls}</Modal>;
 const shownFull=adjusting&&comparison==="edited"?fullFrame:image.corners,shownCrop=adjusting&&comparison==="edited"?crop:image.crop??fullCrop;
 const frame=cropFrame(shownFull,shownCrop);
 const view:CanvasImageView={pageId:page.id,sessionKey:`${page.sourceId}:${page.id}:${image.selectionId}`,image:{...image,corners:cropFrame(image.corners,shownCrop),crop:undefined},frame,fullFrame:shownFull,crop:shownCrop,mode:action==="crop"?"crop":"position",locked:lockedRatio!==null,
  enabled:adjusting&&comparison==="edited"&&!admission&&!draftError&&!applied,raster:shown,
  onGestureActive:setGestureActive,onChange:value=>{rebase(positioned(value),crop);setComparison("edited");},onCropChange:value=>{rebase(fullFrame,value);setComparison("edited");}};
 return <section role="region" aria-label="Image editing" className="image-editing-workspace" onKeyDown={event=>{if(event.key==="Escape"&&!event.defaultPrevented){event.preventDefault();event.stopPropagation();close();}}}>
  <div className="image-editing-bar"><div className="image-replacement-controls"><strong>Edit image · Page {document.pages.findIndex(v=>v.id===page.id)+1}</strong><button className="button" onClick={onLocate}>Locate image</button><span>{action==="crop"?"Drag corners to crop or inside to slide the crop window.":"Drag the outline or handles."} Release to preview; Apply saves this edit.</span></div>{controls}</div>
  {renderCanvas(view)}
 </section>;
}
