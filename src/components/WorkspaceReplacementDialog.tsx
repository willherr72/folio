import {useEffect,useMemo,useRef,useState} from 'react';
import {Modal} from './Modal';
import type {WorkspaceReplacementRequest,WorkspaceReplacementResult} from '../editor/workspace-replacement';
import {WorkspaceReplacementWorkflow,type CheckedWorkspaceReplacementPreview} from '../editor/workspace-replacement-workflow';
import type {WorkspaceReplacementIntent,WorkspaceReplacementPlan} from '../editor/workspace-replacement-transport';
import {normalizeSearchQuery} from '../editor/search-matching';
import './workspace-replacement.css';

interface Props {request:WorkspaceReplacementRequest;query:string;options:WorkspaceReplacementIntent['options'];isCurrent():boolean;borrowedIds():Iterable<string>;onApply(result:WorkspaceReplacementResult):boolean;onCancel():void}
const message=(error:unknown)=>error instanceof Error?error.message:String(error);
const revoke=(url:string)=>{if(url.startsWith('blob:'))URL.revokeObjectURL(url);};
export function WorkspaceReplacementDialog(props:Props){
 const latest=useRef(props);latest.current=props;const input=useRef<HTMLInputElement>(null);
 const flow=useRef<WorkspaceReplacementWorkflow|null>(null),version=useRef(0);
 const [intent,setIntent]=useState<WorkspaceReplacementIntent>({query:props.query,replacement:'',options:{...props.options},selected:null});
 const [ready,setReady]=useState(false),[busy,setBusy]=useState<string|null>('Preparing document…'),[error,setError]=useState<string|null>(null);
 const [plan,setPlan]=useState<WorkspaceReplacementPlan|null>(null),[preview,setPreview]=useState<CheckedWorkspaceReplacementPreview|null>(null);
 const [pageId,setPageId]=useState('');
 const [expired,setExpired]=useState(false);
 const [matchPage,setMatchPage]=useState(0),[selectionChanged,setSelectionChanged]=useState(false);
 const pageNumbers=useMemo(()=>new Map(props.request.document.pages.map((page,index)=>[page.id,index+1])),[props.request]);
 const [raster,setRaster]=useState<{key:string;before:string;after:string;beforeLoaded:boolean;afterLoaded:boolean;error:string|null}>({key:'',before:'',after:'',beforeLoaded:false,afterLoaded:false,error:null});
 useEffect(()=>{
  let value:WorkspaceReplacementWorkflow;
  try{value=new WorkspaceReplacementWorkflow(props.request,{isCurrent:()=>latest.current.isCurrent(),borrowedIds:()=>latest.current.borrowedIds()});}
  catch(cause){setError(message(cause));setBusy(null);return;}
  flow.current=value;
  void value.begin().then(()=>{if(flow.current===value){setReady(true);setBusy(null);}}).catch(cause=>{if(flow.current===value){setError(message(cause));setBusy(null);}});
  return()=>{flow.current=null;version.current++;value.dispose();};
 },[props.request]);
 useEffect(()=>{if(!props.isCurrent()){flow.current?.dispose();props.onCancel();}},[props.isCurrent,props.onCancel]);
 const cancel=()=>{flow.current?.dispose();props.onCancel();};
 const edit=(change:Partial<WorkspaceReplacementIntent>,resetSelection=true)=>{
  version.current++;flow.current?.invalidate();setPlan(null);setPreview(null);setError(null);
  setMatchPage(0);setSelectionChanged(false);
  setIntent(value=>({...value,...change,...(resetSelection?{selected:null}:{})}));
 };
 async function check(){
  const value=flow.current;if(!value)return;const current=++version.current;setBusy('Checking matches…');setError(null);setPlan(null);setPreview(null);setSelectionChanged(false);
  try{const result=await value.check(intent);if(flow.current===value&&version.current===current)setPlan(result);}
  catch(cause){if(flow.current===value&&version.current===current){setError(message(cause));if(value.disposed)setReady(false);}}
  finally{if(flow.current===value)setBusy(null);}
 }
 async function showPreview(){
  const value=flow.current;if(!value||!plan)return;const current=++version.current;setBusy('Creating preview…');setError(null);setPreview(null);
  try{const result=await value.preview(plan);if(flow.current===value&&version.current===current){setPreview(result);setPageId(result.affectedPageIds.includes(props.request.document.selectedPageId??'')?props.request.document.selectedPageId!:result.affectedPageIds[0]);}}
  catch(cause){if(flow.current===value&&version.current===current){setError(message(cause));if(value.disposed)setReady(false);}}
  finally{if(flow.current===value)setBusy(null);}
 }
 const rasterKey=preview?JSON.stringify([preview.previewId,pageId]):'';
 useEffect(()=>{
  setExpired(false);if(!preview)return;
  const remaining=preview.expiresAt-Date.now();if(remaining<=0){setExpired(true);return;}
  const timer=window.setTimeout(()=>setExpired(true),remaining);return()=>window.clearTimeout(timer);
 },[preview]);
 useEffect(()=>{
  if(!preview||!pageId)return;let active=true;const urls:string[]=[];const key=rasterKey;
  const before=preview.before.find(p=>p.id===pageId),after=preview.after.find(p=>p.id===pageId);
  setRaster({key,before:'',after:'',beforeLoaded:false,afterLoaded:false,error:null});
  if(!before||!after)return;
  const render=(page:typeof before)=>Promise.resolve().then(()=>props.request.adapter.renderPagePlan!(page,900)).then(url=>{if(typeof url!=='string')throw new Error('Invalid page preview.');if(active)urls.push(url);else revoke(url);return url;});
  void Promise.allSettled([render(before),render(after)]).then(results=>{
   if(!active)return;
   if(results[0].status==='fulfilled'&&results[1].status==='fulfilled')setRaster({key,before:results[0].value,after:results[1].value,beforeLoaded:false,afterLoaded:false,error:null});
   else{urls.splice(0).forEach(revoke);setRaster({key,before:'',after:'',beforeLoaded:false,afterLoaded:false,error:'The page preview could not be rendered. Try previewing again.'});}
  });
  return()=>{active=false;urls.splice(0).forEach(revoke);};
 },[preview,pageId,rasterKey,props.request]);
 const imagesReady=!!preview&&!expired&&raster.key===rasterKey&&raster.beforeLoaded&&raster.afterLoaded&&!raster.error;
 async function apply(){
  const value=flow.current;if(!value||!preview||!imagesReady||busy)return;const current=++version.current;setBusy('Applying replacements…');setError(null);
  try{await value.apply(preview,result=>latest.current.onApply(result));}
  catch(cause){if(flow.current===value&&version.current===current){setError(message(cause));setReady(!value.disposed);setPreview(null);}}
  finally{if(flow.current===value)setBusy(null);}
 }
 let validQuery=false,queryError:string|null=null;try{validQuery=!!normalizeSearchQuery(intent.query,intent.options);}catch(cause){queryError=message(cause);}
 const total=plan?.pages.reduce((n,p)=>n+p.matches.length,0)??0;
 const listed=plan?.pages.flatMap(page=>page.matches.map(match=>({pageId:page.pageId,match})))??[];
 function chooseMatches(selected:WorkspaceReplacementIntent['selected']){
  version.current++;flow.current?.invalidate();setPreview(null);setError(null);setIntent(value=>({...value,selected}));setSelectionChanged(true);
  setPlan(value=>{
   if(!value)return value;const chosen=selected==null?null:new Set(selected.map(s=>JSON.stringify([s.pageId,s.matchIndex])));
   const pages=value.pages.map(page=>({...page,matches:page.matches.map(match=>({...match,selected:chosen===null||chosen.has(JSON.stringify([page.pageId,match.matchIndex]))}))}));
   const matches=pages.flatMap(page=>page.matches);return {...value,pages,selectedCount:matches.filter(m=>m.selected).length,blockedCount:matches.filter(m=>m.selected&&m.blocker!==null).length,canApply:false,affectedPageIds:[],batchBlocker:null};
  });
 }
 function selectMatch(pageId:string,index:number,checked:boolean){
  if(!plan)return;const chosen=plan.pages.flatMap(p=>p.matches.filter(m=>m.selected).map(m=>({pageId:p.pageId,matchIndex:m.matchIndex}))).filter(m=>m.pageId!==pageId||m.matchIndex!==index);
  if(checked)chosen.push({pageId,matchIndex:index});chooseMatches(chosen);
 }
 return <Modal title="Replace text" onClose={cancel} initialFocusRef={input} className="workspace-replacement-dialog" description="Check matches, preview changes, then apply them as one Undo action.">
  <div className="replacement-fields">
   <label>Find text<input ref={input} value={intent.query} onChange={event=>edit({query:event.target.value})}/></label>
   <label>Replace with<textarea rows={2} value={intent.replacement} onChange={event=>edit({replacement:event.target.value})}/></label>
  </div>
  <div className="replacement-options">
   <label><input type="checkbox" checked={intent.options.matchCase} onChange={event=>edit({options:{...intent.options,matchCase:event.target.checked}})}/>Match case</label>
   <label><input type="checkbox" checked={intent.options.wholeWords} onChange={event=>edit({options:{...intent.options,wholeWords:event.target.checked}})}/>Whole words</label>
   <button onClick={()=>void check()} disabled={!ready||!!busy||!validQuery}>Check matches</button>
  </div>
  {busy&&<p role="status">{busy}</p>}
  {error&&<p role="alert" className="replacement-error">{error}</p>}
  {queryError&&<p role="alert" className="replacement-error">{queryError}</p>}
  {plan&&<section aria-label="Replacement matches">
   <p role="status">{total} match{total===1?'':'es'} · {plan.selectedCount} selected · {plan.blockedCount} blocked</p>
   <div className="replacement-options"><button disabled={!!busy} onClick={()=>chooseMatches(null)}>Select all matches</button><button disabled={!!busy} onClick={()=>chooseMatches([])}>Clear selection</button></div>
   {selectionChanged&&<p>Selection changed. Check matches again before previewing.</p>}
   <div className="replacement-match-list">{listed.slice(matchPage*100,(matchPage+1)*100).map(({pageId,match})=><label key={JSON.stringify([pageId,match.matchIndex])}>
    <input type="checkbox" checked={match.selected} disabled={!!busy} onChange={event=>selectMatch(pageId,match.matchIndex,event.target.checked)}/>
    <span>Page {pageNumbers.get(pageId)}, match {match.matchIndex+1} · {match.overlayId?'added text':'PDF text'}{match.blocker&&<span className="replacement-error">{match.blocker}</span>}</span>
   </label>)}</div>
   {listed.length>100&&<div className="replacement-options"><button disabled={matchPage===0} onClick={()=>setMatchPage(value=>value-1)}>Previous matches</button><span>Matches {matchPage*100+1}–{Math.min((matchPage+1)*100,listed.length)} of {listed.length}</span><button disabled={(matchPage+1)*100>=listed.length} onClick={()=>setMatchPage(value=>value+1)}>Next matches</button></div>}
   {plan.batchBlocker&&<p className="replacement-error">{plan.batchBlocker}</p>}
  </section>}
  {preview&&<section aria-label="Replacement preview">
   {expired&&<p role="status">This preview expired. Preview replacements again before applying.</p>}
   <label>Preview page<select value={pageId} onChange={event=>setPageId(event.target.value)}>{preview.affectedPageIds.map(id=><option key={id} value={id}>Page {props.request.document.pages.findIndex(p=>p.id===id)+1}</option>)}</select></label>
   <p>{preview.affectedPageIds.length} page{preview.affectedPageIds.length===1?'':'s'} will change.</p>
   {raster.error&&<p role="alert" className="replacement-error">{raster.error}</p>}
   <div className="replacement-preview-pair">{(['before','after'] as const).map(side=><figure key={side}><figcaption>{side==='before'?'Before':'After'}</figcaption>{raster.key===rasterKey&&raster[side]&&<img key={rasterKey+side} src={raster[side]} alt={side==='before'?'Page before replacement':'Page after replacement'} onLoad={()=>setRaster(value=>value.key===rasterKey?{...value,[side==='before'?'beforeLoaded':'afterLoaded']:true}:value)} onError={()=>setRaster(value=>value.key===rasterKey?{...value,error:'The page preview could not be displayed. Try previewing again.'}:value)}/>}</figure>)}</div>
  </section>}
  <div className="dialog-actions"><button onClick={cancel}>Cancel</button><button onClick={()=>void showPreview()} disabled={!ready||!!busy||!plan?.canApply}>Preview replacements</button><button className="primary" onClick={()=>void apply()} disabled={!ready||!!busy||!imagesReady}>Apply replacements</button></div>
 </Modal>;
}
