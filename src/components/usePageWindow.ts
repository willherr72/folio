import {useCallback,useLayoutEffect,useRef,useState,type RefObject,type CSSProperties} from "react";
import {anchorPageWindow,pageWindow,revealPage,type PageWindowMetrics} from "../editor/page-window";
interface Options{count:number;initialIndex:number;retained:number[];singleColumn?:boolean}
const number=(value:string,fallback:number,min=0)=>{const n=parseFloat(value);return Number.isFinite(n)&&n>=min?n:fallback;};
export function usePageWindow(hostRef:RefObject<HTMLElement>,options:Options){
 const {singleColumn=false}=options,initial=useRef(options.initialIndex),latest=useRef(options);latest.current=options;
 const defaults:PageWindowMetrics={columns:1,itemWidth:135,itemHeight:singleColumn?140:186,gap:singleColumn?4:12,paddingTop:0,height:600};
 const [snapshot,setSnapshot]=useState(()=>({metrics:defaults,scrollTop:0}));
 const metrics=useRef(snapshot.metrics),measured=useRef(false),pendingAnchor=useRef<{metrics:PageWindowMetrics;top:number}|null>(null);
 const publish=useCallback((next:PageWindowMetrics,scrollTop:number,force=false)=>{
  metrics.current=next;setSnapshot(previous=>!force&&Object.keys(next).every(key=>next[key as keyof PageWindowMetrics]===previous.metrics[key as keyof PageWindowMetrics])&&previous.scrollTop===scrollTop?previous:{metrics:next,scrollTop});
 },[]);
 const clamp=useCallback((top:number,m:PageWindowMetrics)=>Math.max(0,Math.min(top,Math.ceil(latest.current.count/m.columns)*(m.itemHeight+m.gap)-m.gap+2*m.paddingTop-m.height)),[]);
 const reveal=useCallback((index:number)=>{
  const host=hostRef.current,m=metrics.current;if(!host||index<0||index>=latest.current.count)return;
  const top=clamp(revealPage(index,m,pendingAnchor.current?.top??host.scrollTop),m);
  if(pendingAnchor.current){pendingAnchor.current={metrics:m,top};publish(m,top,true);}
  else{host.scrollTop=top;publish(m,host.scrollTop);}
 },[hostRef,clamp,publish]);
 useLayoutEffect(()=>{
  const host=hostRef.current;if(!host)return;let frame=0;
  const measure=()=>{
   const css=getComputedStyle(host),gap=number(css.getPropertyValue("--page-window-gap"),singleColumn?4:12),itemHeight=number(css.getPropertyValue("--page-window-height"),singleColumn?140:186,1),minimum=number(css.getPropertyValue("--page-window-min-width"),135,1);
   const width=Math.max(1,(host.clientWidth||minimum)-number(css.paddingLeft,0)-number(css.paddingRight,0)),columns=singleColumn?1:Math.max(1,Math.floor((width+gap)/(minimum+gap)));
   const next:PageWindowMetrics={columns,itemWidth:(width-(columns-1)*gap)/columns,itemHeight,gap,paddingTop:number(css.paddingTop,0),height:host.clientHeight||600};
   let top=pendingAnchor.current?.top??host.scrollTop;
   if(!measured.current){top=revealPage(Math.max(0,initial.current),next,top);measured.current=true;}
   else if(next.columns!==metrics.current.columns||next.itemHeight!==metrics.current.itemHeight||next.gap!==metrics.current.gap)top=anchorPageWindow(top,metrics.current,next);
   // The new spacer must exist before a larger offset can be assigned: browsers
   // clamp scrollTop against the old content extent until React commits it.
   top=clamp(top,next);pendingAnchor.current={metrics:next,top};publish(next,top,true);
  };
  const scroll=()=>{if(!frame)frame=requestAnimationFrame(()=>{frame=0;if(!pendingAnchor.current)publish(metrics.current,host.scrollTop);});};
  measure();const observer=typeof ResizeObserver!=="undefined"?new ResizeObserver(measure):null;observer?.observe(host);host.addEventListener("scroll",scroll);window.addEventListener("resize",measure);
  return()=>{observer?.disconnect();host.removeEventListener("scroll",scroll);window.removeEventListener("resize",measure);if(frame)cancelAnimationFrame(frame);};
 },[hostRef,singleColumn,clamp,publish]);
 useLayoutEffect(()=>{
  const host=hostRef.current;if(!host)return;const pending=pendingAnchor.current;
  if(pending&&pending.metrics!==snapshot.metrics)return;
  const top=clamp(pending?.top??host.scrollTop,snapshot.metrics);pendingAnchor.current=null;host.scrollTop=top;publish(snapshot.metrics,host.scrollTop);
 },[options.count,snapshot.metrics,hostRef,clamp,publish]);
 const result=pageWindow(options.count,snapshot.metrics,snapshot.scrollTop,options.retained);
 const style=(index:number):CSSProperties=>({position:"absolute",top:Math.floor(index/snapshot.metrics.columns)*(snapshot.metrics.itemHeight+snapshot.metrics.gap),left:(index%snapshot.metrics.columns)*(snapshot.metrics.itemWidth+snapshot.metrics.gap),width:snapshot.metrics.itemWidth,height:snapshot.metrics.itemHeight,margin:0});
 return {...result,metrics:snapshot.metrics,style,reveal};
}
