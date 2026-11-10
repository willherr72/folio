import {useEffect,useMemo,useRef,useState} from 'react';
import type {FolioAdapter} from '../adapter';
import type {SourceReplacement} from '../types';
import type {OcrLanguage,OcrPreparedPage,OcrSession} from './types';
import {startOcrSession} from './session';
import {validateOcrRaster} from './result';
import {applyOcrDrafts,createOcrDraft,ocrDraftChanged,ocrWords,type OcrDraft,type OcrTarget} from './workflow';

interface Options {adapter:FolioAdapter;knownSourceIds():ReadonlySet<string>;onAdopt(replacements:SourceReplacement[]):boolean}
type Stage='setup'|'recognizing'|'review'|'applying';
const message=(error:unknown)=>error instanceof Error?error.message:String(error);
const abortError=()=>new DOMException('Recognition canceled.','AbortError');
function raster(prepared:OcrPreparedPage){
  if(!Number.isFinite(prepared.effectiveDpi)||prepared.effectiveDpi<=0)throw new Error('Invalid recognition resolution.');
  const value={png:prepared.png,width:prepared.width,height:prepared.height,dpi:Math.max(72,Math.min(600,prepared.effectiveDpi))};
  validateOcrRaster(value);return value;
}
function validateJob(drafts:OcrDraft[]):void {
  let words=0,scalars=0;
  for(const draft of drafts)for(const word of ocrWords(draft.page)){
    words++;scalars+=[...word.text].length;
    if(words>500_000||scalars>2_000_000)throw new Error('This recognition job exceeds 500,000 words or 2,000,000 characters. Choose fewer pages.');
  }
}

/** Own one local worker, one preview URL and every unadopted native candidate. */
export function useOcrWorkflow(options:Options){
  const latest=useRef(options);latest.current=options;
  const [stage,setStage]=useState<Stage>('setup'),[drafts,setDrafts]=useState<OcrDraft[]>([]),[active,setActive]=useState(0);
  const [preview,setPreview]=useState<string|null>(null),[previewLoading,setPreviewLoading]=useState(false);
  const [error,setError]=useState<string|null>(null),[notice,setNotice]=useState<string|null>(null);
  const [progress,setProgress]=useState({page:0,total:0,fraction:0,message:''});
  const alive=useRef(true),pending=useRef(false),abort=useRef<AbortController|null>(null),previewGeneration=useRef(0),url=useRef<string|null>(null);
  const currentDrafts=useRef(drafts);currentDrafts.current=drafts;
  const activeRef=useRef(active);activeRef.current=active;
  function releasePreview(){if(url.current){URL.revokeObjectURL(url.current);url.current=null;}}
  useEffect(()=>{alive.current=true;return()=>{alive.current=false;abort.current?.abort();previewGeneration.current++;releasePreview();};},[]);
  function display(prepared:OcrPreparedPage){
    raster(prepared);const copy=new Uint8Array(prepared.png);
    const next=URL.createObjectURL(new Blob([copy],{type:'image/png'}));releasePreview();url.current=next;setPreview(next);
  }
  function publish(drafts:OcrDraft[]){currentDrafts.current=drafts;setDrafts(drafts);}
  function assertActive(controller:AbortController){if(!alive.current||controller.signal.aborted)throw abortError();}
  async function loadPreview(target:OcrTarget,index:number){
    const generation=++previewGeneration.current,adapter=latest.current.adapter;
    setActive(index);activeRef.current=index;setPreviewLoading(true);releasePreview();setPreview(null);
    try {
      if(!adapter.prepareOcrPage)throw new Error('PDF OCR is unavailable in this adapter.');
      const prepared=await adapter.prepareOcrPage(target.page.sourceId,target.page.pageIndex,target.page.rotation);
      if(!alive.current||generation!==previewGeneration.current)return;
      display(prepared);
    }catch(error){if(alive.current&&generation===previewGeneration.current)setError(message(error));}
    finally{if(alive.current&&generation===previewGeneration.current)setPreviewLoading(false);}
  }
  async function execute(targets:OcrTarget[],languages:OcrLanguage[],force:boolean,replaceIndex?:number){
    if(pending.current)return;
    if(!targets.length||targets.length>100){setError('Choose between 1 and 100 source pages.');return;}
    const adapter=latest.current.adapter;if(!adapter.prepareOcrPage){setError('PDF OCR is unavailable in this adapter.');return;}
    // Own the target list before awaiting native preparation.
    const selected=targets.map(t=>({page:{...t.page},pageNumbers:[...t.pageNumbers]})),prior=currentDrafts.current;
    const controller=new AbortController();abort.current=controller;previewGeneration.current++;pending.current=true;
    setStage('recognizing');setError(null);setNotice(null);setPreviewLoading(false);
    let worker:OcrSession|undefined,index=0;
    try {
      const recognized:OcrDraft[]=[];
      for(const target of selected){
        assertActive(controller);setProgress({page:index+1,total:selected.length,fraction:0,message:'Preparing page…'});
        const prepared=await adapter.prepareOcrPage(target.page.sourceId,target.page.pageIndex,target.page.rotation);
        assertActive(controller);const input=raster(prepared);display(prepared);
        let result=prepared.owned;
        if(!result||force){
          if(!worker){
            worker=await startOcrSession([...languages],{signal:controller.signal,onProgress:progress=>{
              if(alive.current&&!controller.signal.aborted)setProgress({page:index+1,total:selected.length,fraction:progress.fraction,
                message:progress.stage==='initializing'?'Loading local recognition…':'Recognizing text…'});
            }});
          }
          assertActive(controller);result=await worker.recognize(input,controller.signal);assertActive(controller);
        }
        if(result.width!==prepared.width||result.height!==prepared.height)throw new Error('Recognized text dimensions differ from the page.');
        recognized.push(createOcrDraft(target,result,prepared.owned,prepared.exclusions,!!prepared.owned&&!force));validateJob(recognized);index++;
      }
      assertActive(controller);
      worker?.dispose();worker=undefined;
      const next=replaceIndex===undefined?recognized:prior.map((draft,i)=>i===replaceIndex?recognized[0]:draft);validateJob(next);
      publish(next);const selectedIndex=replaceIndex??0;activeRef.current=selectedIndex;setActive(selectedIndex);setStage('review');
      setNotice(next.every(draft=>!ocrWords(draft.page).length)?'No text was recognized.':null);
      if(selected.length>1)await loadPreview(next[selectedIndex].target,selectedIndex);
    }catch(error){
      if(alive.current){setStage(prior.length?'review':'setup');
        if(controller.signal.aborted)setNotice('Recognition canceled. Your PDF is unchanged.');else setError(message(error));
      }
    }finally{
      worker?.dispose();if(abort.current===controller)abort.current=null;pending.current=false;
    }
  }
  const start=(targets:OcrTarget[],languages:OcrLanguage[])=>execute(targets,languages,false);
  const recognizeAgain=(languages:OcrLanguage[])=>{
    const index=activeRef.current,draft=currentDrafts.current[index];return draft?execute([draft.target],languages,true,index):Promise.resolve();
  };
  async function view(index:number){
    if(pending.current||!currentDrafts.current[index])return;setError(null);await loadPreview(currentDrafts.current[index].target,index);
  }
  function update(change:(draft:OcrDraft)=>OcrDraft){
    if(pending.current)return;
    try{const next=currentDrafts.current.map((draft,i)=>i===activeRef.current?change(draft):draft);validateJob(next);publish(next);setError(null);setNotice(null);}
    catch(error){setError(message(error));}
  }
  async function apply():Promise<boolean>{
    if(pending.current)return false;
    const controller=new AbortController();abort.current=controller;pending.current=true;previewGeneration.current++;
    setStage('applying');setError(null);setNotice(null);
    const {adapter,knownSourceIds,onAdopt}=latest.current;
    try{
      const adopted=await applyOcrDrafts(adapter,currentDrafts.current,knownSourceIds(),controller.signal,
        replacements=>alive.current&&!controller.signal.aborted&&onAdopt(replacements));
      if(alive.current&&!adopted)setNotice('No new searchable text to apply.');return adopted;
    }catch(error){if(alive.current){if(controller.signal.aborted)setNotice('Apply canceled. Your PDF is unchanged.');else setError(message(error));}return false;}
    finally{pending.current=false;if(abort.current===controller)abort.current=null;if(alive.current)setStage('review');}
  }
  function cancel(){abort.current?.abort();previewGeneration.current++;}
  function reset(){if(pending.current)return;previewGeneration.current++;publish([]);releasePreview();setPreview(null);setStage('setup');setError(null);setNotice(null);}
  const changed=useMemo(()=>drafts.filter(ocrDraftChanged).length,[drafts]);
  return {stage,drafts,active,preview,previewLoading,error,notice,progress,changed,start,recognizeAgain,view,update,apply,cancel,reset};
}
