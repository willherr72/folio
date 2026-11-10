import {useCallback,useEffect,useMemo,useRef,useState} from "react";
import type {FolioAdapter} from "./adapter";
import type {PagePlan} from "./types";
interface Request {adapter:FolioAdapter;plan:PagePlan;width:number;source:string}
interface State {request?:Request;url?:string;error?:string}
const dispose=(url?:string)=>{if(url?.startsWith("blob:"))URL.revokeObjectURL(url);};

/** At most one render per mounted preview, with only the latest pending plan.
 * Native composition keeps color, transparency and annotation layering identical
 * to save/print; SVG above it supplies interaction without painting edits twice.
 */
export function useComposedPage(adapter:FolioAdapter,page:PagePlan,width:number,enabled:boolean,renderRotation=false) {
  const [revision,setRevision]=useState(0);
  const retry=useCallback(()=>setRevision(value=>value+1),[]);
  const rotation=renderRotation?page.rotation:0;
  const request=useMemo<Request|null>(()=>enabled?{adapter,plan:{...page,rotation},width,
    source:`${page.sourceId}:${page.pageIndex}:${page.width}:${page.height}:${rotation}:${JSON.stringify(page.crop)}`} : null,
    [adapter,page.id,page.sourceId,page.pageIndex,page.width,page.height,page.crop,page.overlays,rotation,width,enabled,revision]);
  const latest=useRef<Request|null>(null),alive=useRef(false),running=useRef(false),url=useRef<string>();
  const [state,setState]=useState<State>({});
  const pump=useCallback(function run() {
    const job=latest.current;if(!alive.current||running.current||!job)return;
    running.current=true;
    void Promise.resolve().then(()=>{
      if(!job.adapter.renderPagePlan)throw new Error("Image previews require the desktop PDF service.");
      return job.adapter.renderPagePlan(job.plan,job.width);
    }).then(result=>{
      if(alive.current&&latest.current===job){dispose(url.current);url.current=result;setState({request:job,url:result});}
      else dispose(result);
    }).catch(error=>{
      if(alive.current&&latest.current===job){dispose(url.current);url.current=undefined;setState({request:job,error:error instanceof Error?error.message:String(error)});}
    }).finally(()=>{running.current=false;if(alive.current&&latest.current&&latest.current!==job)run();});
  },[]);
  useEffect(()=>{alive.current=true;return()=>{alive.current=false;latest.current=null;dispose(url.current);url.current=undefined;};},[]);
  useEffect(()=>{
    latest.current=request;
    if(request)pump();else {dispose(url.current);url.current=undefined;setState({});}
  },[request,pump]);
  return {url:request&&state.request?.source===request.source?state.url:undefined,
    error:request===state.request?state.error:undefined,loading:!!request&&request!==state.request,retry};
}
