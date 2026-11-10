import {createContext,useCallback,useContext,useLayoutEffect,useMemo,useRef,useState,type ReactNode} from "react";
import type {CanvasParagraphView} from "../editor/paragraph-canvas";

type Session=Pick<CanvasParagraphView,"pageId"|"sessionKey">;
interface Publication {owner:symbol;view:CanvasParagraphView}
interface Host extends Partial<Session> {
 publish(owner:symbol,view:CanvasParagraphView):void;
 release(owner:symbol):void;
}
const Context=createContext<Host|null>(null);
const ignore=()=>{};

/** Keep the reading viewport mounted while a paragraph panel publishes its view. */
export function ParagraphViewportHost({session,renderViewport,children}:{session?:Session;renderViewport(view?:CanvasParagraphView):ReactNode;children?:ReactNode}){
 const [publication,setPublication]=useState<Publication>();
 const current=useRef(session);current.current=session;
 const publish=useCallback((owner:symbol,view:CanvasParagraphView)=>{
  if(current.current?.sessionKey!==view.sessionKey||current.current.pageId!==view.pageId)return;
  setPublication(value=>value?.owner===owner&&value.view===view?value:{owner,view});
 },[]);
 const release=useCallback((owner:symbol)=>setPublication(value=>value?.owner===owner?undefined:value),[]);
 const host=useMemo(()=>({sessionKey:session?.sessionKey,pageId:session?.pageId,publish,release}),[session?.sessionKey,session?.pageId,publish,release]);
 const pending=useMemo<CanvasParagraphView|undefined>(()=>session?{
  pageId:session.pageId,sessionKey:session.sessionKey,runs:[],selected:[],enabled:false,choosing:false,
  raster:{loading:true},onChange:ignore,onToggle:ignore,onGestureActive:ignore,
 }:undefined,[session?.pageId,session?.sessionKey]);
 const view=session&&publication?.view.sessionKey===session.sessionKey&&publication.view.pageId===session.pageId?publication.view:pending;
 return <Context.Provider value={host}><div className={session?"paragraph-workspace":"paragraph-viewport-host"}>
  {children}
  <div key="viewport" className="paragraph-workspace-page">{renderViewport(view)}</div>
 </div></Context.Provider>;
}

/** Publish after commit, with cleanup restricted to this mounted workspace. */
export function useParagraphViewport(view:CanvasParagraphView):boolean{
 const host=useContext(Context),owner=useRef(Symbol("paragraph viewport"));
 useLayoutEffect(()=>{host?.publish(owner.current,view);},[host,view]);
 useLayoutEffect(()=>{const id=owner.current;return()=>host?.release(id);},[host]);
 return host!==null;
}
