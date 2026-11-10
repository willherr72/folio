import {useCallback,useEffect,useRef,useState,type PointerEvent} from 'react';
import {TextRemovalWorkflow,type RemovalRequest} from '../editor/text-removal-workflow';
import type {AnnotationRect,DocumentInfo} from '../editor/types';
import {isContentRemoval,type TextRemovalMark,type RemovalPreview,type TextRemovalSource} from '../editor/text-removal';
import {useSourceRaster} from '../editor/use-source-raster';
import {useContentRemovalRaster} from '../editor/use-content-removal-raster';
import {Modal} from './Modal';
import {TextRemovalGlyphMask} from './TextRemovalGlyphMask';
import './text-removal.css';
export interface TextRemovalDialogProps {request:RemovalRequest;isCurrent():boolean;isBorrowed(id:string):boolean;onApply(info:DocumentInfo,preview:RemovalPreview):boolean;onCancel():void}
const message=(error:unknown)=>{const value=error instanceof Error?error.message:String(error);return value.includes('This PDF is outside the private structural redaction subset.')?'These selections cannot yet be removed safely from this PDF. No changes were applied.':value;};
const labels={x:'Left (pt)',y:'Top (pt)',width:'Width (pt)',height:'Height (pt)'};
type Fields=Record<keyof typeof labels,string>;
export function TextRemovalDialog(props:TextRemovalDialogProps){
 const combined=props.request.mode==='content';
 const latest=useRef(props);latest.current=props;
 const flow=useRef<TextRemovalWorkflow|null>(null);
 const [workflow,setWorkflow]=useState<TextRemovalWorkflow|null>(null),[refresh,setRefresh]=useState(0);
 const [source,setSource]=useState<TextRemovalSource|null>(null),[preview,setPreview]=useState<RemovalPreview|null>(null);
 const [expansionAck,setExpansionAck]=useState(false),[disclosureLoaded,setDisclosureLoaded]=useState('');
 const [reviewed,setReviewed]=useState<Set<string>>(()=>new Set());
 const [marks,setMarks]=useState<TextRemovalMark[]>([]),[fields,setFields]=useState<Fields>({x:'20',y:'40',width:'60',height:'20'});
 const [pageIndex,setPageIndex]=useState(()=>Math.max(0,props.request.document.pages.findIndex(p=>p.id===props.request.document.selectedPageId)));
 const [pageInput,setPageInput]=useState(String(pageIndex+1));
 const [busy,setBusy]=useState<'prepare'|'preview'|'apply'|null>('prepare'),[error,setError]=useState<string|null>(null);
 const [closed,setClosed]=useState(false),[applied,setApplied]=useState(false),[ack,setAck]=useState(false),[expired,setExpired]=useState(false);
 const [sourceLoaded,setSourceLoaded]=useState(''),[candidateLoaded,setCandidateLoaded]=useState(''),[imageError,setImageError]=useState('');
 const [mask,setMask]=useState<{key:string;ready:boolean;error?:string}>({key:'',ready:false});
 const gesture=useRef<{id:number;x:number;y:number}|null>(null);
 const [drag,setDrag]=useState<AnnotationRect|null>(null);
 useEffect(()=>{
  let next:TextRemovalWorkflow;
  try{next=new TextRemovalWorkflow(props.request,{isCurrent:()=>latest.current.isCurrent(),isBorrowed:id=>latest.current.isBorrowed(id),onPreviewFailure:cause=>{if(flow.current===next){setPreview(null);setAck(false);setError(cause);}}});}catch(cause){setError(message(cause));setBusy(null);return;}
  flow.current=next;setWorkflow(next);setSource(null);setPreview(null);setAck(false);setError(null);setBusy('prepare');setImageError('');
  void next.prepare().then(value=>{if(flow.current===next){setSource(value);setBusy(null);}}).catch(cause=>{if(flow.current===next){setError(message(cause));setBusy(null);}});
  return()=>{if(flow.current===next)flow.current=null;next.dispose();};
 },[props.request,refresh]);
 useEffect(()=>{setExpired(false);if(!preview)return;const timer=setTimeout(()=>setExpired(true),preview.expiresInSeconds*1000);return()=>clearTimeout(timer);},[preview]);
 const dimensions=source?.document.pages[pageIndex]??{width:500,height:300};
 const legacyOriginal=useSourceRaster(props.request.adapter,combined?null:source?.document??null,pageIndex);
 const legacyResult=useSourceRaster(props.request.adapter,combined?null:preview?.candidate??null,pageIndex);
 const paired=useContentRemovalRaster(combined?workflow:null,combined?source:null,preview,pageIndex);
 const viewStyle={aspectRatio:`${dimensions.width}/${dimensions.height}`,...(combined?{maxWidth:`${720*dimensions.width/dimensions.height}px`}:{})};
 const original=combined?{key:paired.source??`${paired.key}:source`,url:paired.source,error:paired.error,loading:paired.loading}:legacyOriginal;
 const result=combined?{key:paired.candidate??`${paired.key}:candidate`,url:paired.candidate,error:paired.error,loading:paired.loading}:legacyResult;
 const maskKey=preview?`${preview.previewId}:${pageIndex}`:'';
 const maskReady=useCallback((ready:boolean,cause?:string)=>{setMask({key:maskKey,ready,error:cause});if(cause){flow.current?.invalidate();setPreview(null);setAck(false);setError(cause);}},[maskKey]);
 const sourceReady=!!original.url&&!original.loading&&!original.error&&sourceLoaded===original.key&&imageError!==original.key;
 const candidateReady=!!result.url&&!result.loading&&!result.error&&candidateLoaded===result.key&&imageError!==result.key;
 const disclosureReady=!!paired.disclosure&&disclosureLoaded===paired.disclosure&&imageError!==paired.disclosure;
 useEffect(()=>{
  if(!combined||!workflow||!preview||!paired.raster||!sourceReady||!candidateReady||!disclosureReady)return;
  try{workflow.reviewRaster(paired.raster);const key=`${preview.previewId}:${pageIndex}`;setReviewed(previous=>previous.has(key)?previous:new Set([...previous,key]));}
  catch(cause){setError(message(cause));workflow.invalidate();setPreview(null);}
 },[combined,workflow,preview,paired.raster,sourceReady,candidateReady,disclosureReady,pageIndex]);
 useEffect(()=>{setExpansionAck(false);setReviewed(new Set());},[preview]);
 const locked=!!busy||closed||applied;
 const draft={x:Number(fields.x),y:Number(fields.y),width:Number(fields.width),height:Number(fields.height)};
 const validDraft=Object.values(fields).every(v=>v.trim()!=='')&&Object.values(draft).every(Number.isFinite)&&draft.x>=0&&draft.y>=0&&draft.width>0&&draft.height>0&&draft.x+draft.width<=dimensions.width&&draft.y+draft.height<=dimensions.height;
 const invalidate=()=>{flow.current?.invalidate();setPreview(null);setAck(false);setError(null);setImageError('');};
 const changeMarks=(next:TextRemovalMark[])=>{invalidate();setMarks(next);};
 const add=()=>{if(!locked&&sourceReady&&validDraft&&marks.length<64)changeMarks([...marks,{pageIndex,rect:draft}]);};
 const navigate=(index:number)=>{if(!locked&&source&&Number.isSafeInteger(index)&&index>=0&&index<source.document.pages.length){gesture.current=null;setDrag(null);setPageIndex(index);setPageInput(String(index+1));setImageError('');}};
 const cancel=()=>{setClosed(true);const current=flow.current;flow.current=null;current?.dispose();latest.current.onCancel();};
 const position=(event:PointerEvent<SVGSVGElement>)=>{const bounds=event.currentTarget.getBoundingClientRect();return {x:Math.max(0,Math.min(dimensions.width,(event.clientX-bounds.left)*dimensions.width/bounds.width)),y:Math.max(0,Math.min(dimensions.height,(event.clientY-bounds.top)*dimensions.height/bounds.height))};};
 const finish=(event:PointerEvent<SVGSVGElement>,cancelled=false)=>{
  const start=gesture.current;if(!start||start.id!==event.pointerId)return;gesture.current=null;setDrag(null);
  try{event.currentTarget.releasePointerCapture(event.pointerId);}catch{/* Synthetic pointers may not own capture. */}
  if(cancelled||locked)return;const end=position(event),rect={x:Math.min(start.x,end.x),y:Math.min(start.y,end.y),width:Math.abs(start.x-end.x),height:Math.abs(start.y-end.y)};
  if(rect.width>0&&rect.height>0&&Object.values(rect).every(Number.isFinite)&&marks.length<64){setFields({x:String(rect.x),y:String(rect.y),width:String(rect.width),height:String(rect.height)});changeMarks([...marks,{pageIndex,rect}]);}
 };
 const preparePreview=async()=>{
  const current=flow.current;if(!current||locked||!sourceReady||!marks.length)return;
  setBusy('preview');setError(null);setPreview(null);setAck(false);setImageError('');
  try{const value=await current.preview(marks);if(flow.current===current)setPreview(value);}catch(cause){if(flow.current===current)setError(message(cause));}finally{if(flow.current===current)setBusy(null);}
 };
 const expanded=preview&&isContentRemoval(preview)&&preview.pages.some(p=>p.extendsBeyondMarks);
 const reviewReady=preview&&isContentRemoval(preview)?disclosureReady&&preview.pages.every(p=>reviewed.has(`${preview.previewId}:${p.pageIndex}`)):mask.key===maskKey&&mask.ready&&!mask.error;
 const canApply=!!preview&&!locked&&!drag&&!expired&&sourceReady&&candidateReady&&reviewReady&&(!expanded||expansionAck)&&(!preview.omittedMetadata.length||ack);
 const failedImage=(key:string)=>{setImageError(key);if(combined){setError('The page preview could not be displayed. Preview the selections again.');flow.current?.invalidate();setPreview(null);}};
 const apply=async()=>{
  const current=flow.current;if(!current||!preview||!canApply)return;setBusy('apply');setError(null);
  try{await current.apply(preview,(info,p)=>latest.current.onApply(info,p));if(flow.current===current)setApplied(true);}catch(cause){if(flow.current===current){setError(message(cause));setPreview(null);setAck(false);}}finally{if(flow.current===current)setBusy(null);}
 };
 return <Modal title={combined?'Remove content':'Remove text'} className="text-removal-dialog" description={combined?'Remove text, graphics and image content in selected areas. Whole graphics and JPEG blocks may extend beyond your marks. Review each affected page before applying. Unselected copies elsewhere remain. You can Undo before saving a copy.':'Removes selected text from the PDF. Other content is not redacted. Unsupported PDFs are left unchanged. You can Undo before saving a copy.'} onClose={cancel}>
  <div className="text-removal-navigation">
   <button className="button" disabled={locked||!source||pageIndex===0} onClick={()=>navigate(pageIndex-1)}>Previous page</button>
   <label>Page<input aria-label="Removal page" type="number" min="1" max={source?.document.pages.length??1} value={pageInput} disabled={locked||!source} onChange={event=>{const value=event.target.value;setPageInput(value);if(value.trim()!=='')navigate(Number(value)-1);}} onBlur={()=>setPageInput(String(pageIndex+1))}/></label>
   <span>of {source?.document.pages.length??props.request.document.pages.length}</span>
   <button className="button" disabled={locked||!source||pageIndex+1>=source.document.pages.length} onClick={()=>navigate(pageIndex+1)}>Next page</button>
   <button className="button" disabled={locked} onClick={()=>{flow.current?.dispose();setRefresh(value=>value+1);}}>Refresh working copy</button>
  </div>
  {busy&&<p role="status">{busy==='prepare'?'Preparing your current edits…':busy==='preview'?'Preparing removal preview…':'Applying removal…'}</p>}
  {error&&<p role="alert" className="error-note">{error} Refresh the working copy if it has expired.</p>}
  <div className="text-removal-views">
   <figure><figcaption>Current page · drag to select {combined?'content':'text'}</figcaption><div className="text-removal-page" style={viewStyle}>
    {original.url&&<img key={original.key} src={original.url} alt="Current edited page" onLoad={()=>setSourceLoaded(original.key)} onError={()=>failedImage(original.key)}/>}
    {combined?paired.disclosure&&<img key={paired.disclosure} className="text-removal-disclosure" src={paired.disclosure} alt="Removal bounds and rendering influence" onLoad={()=>setDisclosureLoaded(paired.disclosure!)} onError={()=>failedImage(paired.disclosure!)}/>:<TextRemovalGlyphMask workflow={workflow} preview={preview&&!isContentRemoval(preview)?preview:null} pageIndex={pageIndex} width={dimensions.width} height={dimensions.height} onReady={maskReady}/>}
    <svg aria-label={combined?'Select content to remove':'Select text to remove'} viewBox={`0 0 ${dimensions.width} ${dimensions.height}`} onPointerDown={event=>{if(event.button!==0||gesture.current||locked||!sourceReady||marks.length>=64)return;const point=position(event);if(!Object.values(point).every(Number.isFinite))return;event.preventDefault();gesture.current={id:event.pointerId,...point};setDrag({...point,width:0,height:0});try{event.currentTarget.setPointerCapture(event.pointerId);}catch{/* See finish. */}}} onPointerMove={event=>{const start=gesture.current;if(!start||start.id!==event.pointerId)return;const end=position(event);setDrag({x:Math.min(start.x,end.x),y:Math.min(start.y,end.y),width:Math.abs(start.x-end.x),height:Math.abs(start.y-end.y)});}} onPointerUp={event=>finish(event)} onPointerCancel={event=>finish(event,true)} onLostPointerCapture={event=>finish(event,true)}>
     {marks.map((mark,index)=>mark.pageIndex===pageIndex?<rect key={index} {...mark.rect} className="text-removal-mark"/>:null)}
     {drag&&<rect {...drag} className="text-removal-mark" strokeDasharray="4 3" data-testid="removal-drag"/>}
    </svg>
   </div>{original.loading&&<p role="status">Rendering current page…</p>}{(original.error||imageError===original.key)&&<p role="alert">{original.error||'The current page image could not be displayed.'}</p>}</figure>
   <figure><figcaption>After removal</figcaption>{preview?<div className="text-removal-page" style={viewStyle}>{result.url&&<img key={result.key} src={result.url} alt={combined?'Page after content removal':'Page after text removal'} onLoad={()=>setCandidateLoaded(result.key)} onError={()=>failedImage(result.key)}/>}</div>:<p>Add selections and preview the result.</p>}{result.loading&&<p role="status">Rendering removal preview…</p>}{(result.error||imageError===result.key&&!!result.key)&&<p role="alert">{result.error||'The removal preview image could not be displayed.'}</p>}</figure>
  </div>
  <fieldset disabled={locked||!sourceReady}><legend>Add a selection in page points</legend><div className="text-removal-fields">{(Object.keys(labels) as (keyof Fields)[]).map(key=><label key={key}>{labels[key]}<input type="number" min="0" step="any" value={fields[key]} onChange={event=>setFields({...fields,[key]:event.target.value})}/></label>)}<button className="button" disabled={!validDraft||marks.length>=64} onClick={add}>Add selection</button></div></fieldset>
  <div className="text-removal-selections"><p>{marks.length} of 64 selections</p><ol>{marks.map((mark,index)=><li key={index}>Page {mark.pageIndex+1} · {mark.rect.width.toFixed(1)} × {mark.rect.height.toFixed(1)} pt <button className="button" aria-label={`Remove selection ${index+1}`} disabled={locked} onClick={()=>changeMarks(marks.filter((_,i)=>i!==index))}>Remove</button></li>)}</ol><button className="button" disabled={locked||!marks.length} onClick={()=>changeMarks([])}>Clear selections</button></div>
  {preview&&<section aria-label="Removal summary">{isContentRemoval(preview)?<>
   <p>{preview.removedCharacters} characters, {preview.removedPaths} graphic{preview.removedPaths===1?'':'s'} and {preview.affectedImages} image occurrence{preview.affectedImages===1?'':'s'} affected on {preview.pages.length} page{preview.pages.length===1?'':'s'}.</p>
   <p>Red shading shows conservative removal bounds. Amber shading shows additional rendering influence, including image resampling; it does not select more source content.</p>
   <div className="text-removal-review-pages">{preview.pages.map(p=><button key={p.pageIndex} className="button" disabled={locked} onClick={()=>navigate(p.pageIndex)}>Page {p.pageIndex+1} · {reviewed.has(`${preview.previewId}:${p.pageIndex}`)?'Reviewed':'Review required'}</button>)}</div>
   {expanded&&<label className="text-removal-ack"><input type="checkbox" checked={expansionAck} disabled={locked} onChange={event=>setExpansionAck(event.target.checked)}/>I understand that removal extends beyond the drawn marks.</label>}
  </>:<p>{preview.removedCharacters} characters removed on {preview.pages.length} page{preview.pages.length===1?'':'s'}: {preview.pages.map(p=>p.pageIndex+1).join(', ')}. Red shading shows the selected glyphs.</p>}{mask.key===maskKey&&mask.error&&<p role="alert" className="error-note">{mask.error}</p>}{expired&&<p role="alert">This preview expired. Preview the selections again, or refresh the working copy.</p>}{preview.omittedMetadata.length>0&&<><p>The resulting copy also omits:</p><ul>{preview.omittedMetadata.map(item=><li key={item}>{item}</li>)}</ul><label className="text-removal-ack"><input type="checkbox" checked={ack} disabled={locked} onChange={event=>setAck(event.target.checked)}/>I understand that the listed information will be omitted.</label></>}</section>}
  <div className="dialog-actions"><button className="button" onClick={cancel}>Cancel</button><button className="button" disabled={locked||!!drag||!sourceReady||!marks.length} onClick={()=>void preparePreview()}>Preview removal</button><button className="button primary" disabled={!canApply} onClick={()=>void apply()}>Apply removal</button></div>
 </Modal>;
}
