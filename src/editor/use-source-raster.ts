import {useEffect,useState} from 'react';
import type {FolioAdapter} from './adapter';
import type {DocumentInfo} from './types';
const release=(url?:string)=>{if(url?.startsWith('blob:'))URL.revokeObjectURL(url);};
/** Render immutable source bytes, with no second composition of baked edits. */
export function useSourceRaster(adapter:FolioAdapter,info:DocumentInfo|null,pageIndex:number,width=900){
 const key=info?`${info.id}:${pageIndex}:${width}`:'';
 const [state,setState]=useState<{key:string;url?:string;error?:string}>({key:''});
 useEffect(()=>{
  let alive=true,url:string|undefined;
  if(info)void Promise.resolve().then(()=>adapter.renderPage(info.id,pageIndex,width)).then(value=>{
   if(!alive){release(value);return;}
   if(typeof value!=='string'||!value)throw new Error('The PDF page could not be rendered.');
   url=value;setState({key,url});
  }).catch(error=>{if(alive)setState({key,error:error instanceof Error?error.message:String(error)});});
  return()=>{alive=false;release(url);};
 },[adapter,key]);
 return {key,url:key&&state.key===key?state.url:undefined,error:key&&state.key===key?state.error:undefined,loading:!!key&&(state.key!==key||(!state.url&&!state.error))};
}
