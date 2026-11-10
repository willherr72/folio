import {useEffect,useState} from 'react';
import type {TextRemovalWorkflow} from './text-removal-workflow';
import type {RemovalPreview,TextRemovalSource} from './text-removal';
import type {ValidatedContentRaster} from './content-removal-raster';
interface State {owner:TextRemovalWorkflow|null;key:string;source?:string;candidate?:string;disclosure?:string;raster?:ValidatedContentRaster;error?:string}
/** URLs belong to exactly one request effect, including late or partially created results. */
export function useContentRemovalRaster(workflow:TextRemovalWorkflow|null,source:TextRemovalSource|null,preview:RemovalPreview|null,page:number){
 const key=workflow&&source?`${source.document.id}:${preview?.previewId??'source'}:${page}:900`:'';
 const [state,setState]=useState<State>({owner:null,key:''});
 useEffect(()=>{
  let alive=true;const urls:string[]=[];
  const release=()=>{for(const url of urls.splice(0))URL.revokeObjectURL(url);};
  if(workflow&&source)void workflow.raster(preview,page).then(raster=>{
   if(!alive)return;
   const make=(bytes:Uint8Array<ArrayBuffer>|null)=>{if(!bytes)return undefined;const url=URL.createObjectURL(new Blob([bytes],{type:'image/png'}));urls.push(url);return url;};
   const sourceUrl=make(raster.sourcePng),candidate=make(raster.candidatePng),disclosure=make(raster.disclosurePng);
   setState({owner:workflow,key,source:sourceUrl,candidate,disclosure,raster});
  }).catch(error=>{release();if(alive)setState({owner:workflow,key,error:error instanceof Error?error.message:String(error)});});
  return()=>{alive=false;release();};
 },[workflow,source,preview,page,key]);
 const current=key&&state.key===key&&state.owner===workflow?state:undefined;
 return {key,source:current?.source,candidate:current?.candidate,disclosure:current?.disclosure,raster:current?.raster,error:current?.error,loading:!!key&&!current?.source&&!current?.error};
}
