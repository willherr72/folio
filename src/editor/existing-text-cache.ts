import type {FolioAdapter} from "./adapter";
import type {TextRuns} from "./types";
import {supportsOccurrenceForm} from "./text-edit";

type TopReader=FolioAdapter["listTextRuns"];
type FormReader=FolioAdapter["listFormTextRuns"];
interface Entry {
 sourceId:string;
 top:TopReader;
 forms:FormReader;
 request?:Promise<TextRuns>;
 topValue?:TextRuns;
 formsValue?:TextRuns;
 bytes:number;
}
const entries=new Map<string,Entry>();
const adapterIds=new WeakMap<FolioAdapter,number>();
let nextAdapterId=0;
const MAX_ENTRIES=32;
const MAX_BYTES=2*1024*1024;

function copy(result:TextRuns):TextRuns {
 return {...result,runs:result.runs.map(run=>({...run,bounds:{...run.bounds},...(run.objectPath?{objectPath:[...run.objectPath]}:{})}))};
}
function estimatedBytes(result:TextRuns):number {
 let bytes=256+2*(result.reason?.length??0);
 for(const run of result.runs){
  bytes+=512+8*(run.objectPath?.length??0);
  for(const value of Object.values(run))if(typeof value==="string")bytes+=2*value.length;
  if(bytes>MAX_BYTES)return bytes;
 }
 return bytes;
}
function prune(){
 let bytes=Array.from(entries.values()).reduce((sum,entry)=>sum+entry.bytes,0);
 while(entries.size>MAX_ENTRIES||bytes>MAX_BYTES){
  const key=entries.keys().next().value;if(key===undefined)return;
  bytes-=entries.get(key)!.bytes;entries.delete(key);
 }
}

/** Release completed and pending discovery; late responses cannot repopulate it. */
export function clearExistingTextCache(sourceIds?:readonly string[]):void {
 if(!sourceIds){entries.clear();return;}
 const sources=new Set(sourceIds);
 for(const [key,entry] of entries)if(sources.has(entry.sourceId))entries.delete(key);
}

/** Discovery only. Every edit still passes the native source and output proofs. */
export function readExistingTextRuns(adapter:FolioAdapter,sourceId:string,pageIndex:number):Promise<TextRuns> {
 let adapterId=adapterIds.get(adapter);
 if(adapterId===undefined){adapterId=++nextAdapterId;adapterIds.set(adapter,adapterId);}
 const key=JSON.stringify([adapterId,sourceId,pageIndex]);
 const top=adapter.listOccurrenceTextRuns&&adapter.replaceTextOccurrence?adapter.listOccurrenceTextRuns:adapter.listTextRuns;
 const forms=supportsOccurrenceForm(adapter)?adapter.listOccurrenceFormTextRuns:adapter.listFormTextRuns;
 let entry=entries.get(key);
 if(entry&&(entry.top!==top||entry.forms!==forms)){entries.delete(key);entry=undefined;}
 if(!entry)entry={sourceId,top,forms,bytes:256+key.length*2};
 if(!entry.request){
  const next=entry;
  next.request=Promise.allSettled([
   // Snapshot each provider before a slow sibling can expose adapter-owned mutation.
   next.topValue?Promise.resolve(next.topValue):Promise.resolve().then(()=>top?.call(adapter,sourceId,pageIndex)??{runs:[]}).then(copy),
   next.formsValue?Promise.resolve(next.formsValue):Promise.resolve().then(()=>forms?.call(adapter,sourceId,pageIndex)??{runs:[]}).then(result=>copy({...result,runs:result.runs.map(run=>({...run,objectIndex:run.objectPath[0],canSubstitute:run.canSubstitute??false}))})),
  ]).then(([topResult,formResult])=>{
   const reason=(error:unknown)=>error instanceof Error?error.message:String(error);
   const a=topResult.status==="fulfilled"?topResult.value:{runs:[],reason:reason(topResult.reason)};
   const b=formResult.status==="fulfilled"?formResult.value:{runs:[],reason:reason(formResult.reason)};
   const result=copy({runs:[...a.runs,...b.runs],reason:b.reason??a.reason});
   if(entries.get(key)===next){
    next.request=undefined;
    // A form refusal must not discard successful top-level discovery (or vice versa).
    next.topValue=topResult.status==="fulfilled"&&!a.reason?copy(a):undefined;
    next.formsValue=formResult.status==="fulfilled"&&!b.reason?copy(b):undefined;
    if(!next.topValue&&!next.formsValue)entries.delete(key);
    else {
     next.bytes=estimatedBytes({runs:[...(next.topValue?.runs??[]),...(next.formsValue?.runs??[])]})+key.length*2;
     // One oversized page must not flush every useful smaller inspection.
     if(next.bytes>MAX_BYTES)entries.delete(key);else prune();
    }
   }
   return result;
  }).catch((error:unknown)=>{if(entries.get(key)===next)entries.delete(key);throw error;});
 }
 // Retain access order for both pending and completed requests.
 entries.delete(key);entries.set(key,entry);prune();
 return entry.request!.then(copy);
}
