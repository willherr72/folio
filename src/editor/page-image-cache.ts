import type {FolioAdapter} from './adapter';

type Adapter=Pick<FolioAdapter,'renderPage'>;
type Page={sourceId:string;pageIndex:number;width:number;height:number};
type Entry={
 key:string;sourceId:string;references:number;estimatedBytes:number;
 phase:'queued'|'running'|'done';disposalRequested:boolean;evicted:boolean;revoked:boolean;
 request:Promise<string>;render:()=>Promise<string>;resolve:(url:string)=>void;reject:(error:unknown)=>void;url?:string;
};
export interface PageImageLease {key:string;request:Promise<string>;release():void}

/** Owns rendered URLs and queued work. A lease borrows its URL until release. */
export class PageImageCache {
 private readonly entries=new Map<string,Entry>();
 private readonly queue=new Set<Entry>();
 private readonly adapters=new WeakMap<Adapter,number>();
 private nextAdapter=0;
 private active=0;

 key(adapter:Adapter,page:Page,pixelWidth:number):string {
  let id=this.adapters.get(adapter);
  if(id===undefined){id=++this.nextAdapter;this.adapters.set(adapter,id);}
  return JSON.stringify([id,page.sourceId,page.pageIndex,Math.round(pixelWidth)]);
 }

 acquire(adapter:Adapter,page:Page,pixelWidth:number):PageImageLease {
  const key=this.key(adapter,page,pixelWidth);
  let entry=this.entries.get(key);
  if(!entry){
   let resolve!:(url:string)=>void,reject!:(error:unknown)=>void;
   const request=new Promise<string>((yes,no)=>{resolve=yes;reject=no;});
   // Releasing a queued lease is normal, including callers that never await it.
   void request.catch(()=>{});
   entry={key,sourceId:page.sourceId,references:0,estimatedBytes:pixelWidth*Math.ceil(pixelWidth*page.height/page.width)*4,
    phase:'queued',disposalRequested:false,evicted:false,revoked:false,request,resolve,reject,
    render:()=>adapter.renderPage(page.sourceId,page.pageIndex,pixelWidth)};
   this.entries.set(key,entry);this.queue.add(entry);
  }
  this.entries.delete(key);this.entries.set(key,entry);entry.references++;
  this.prune();this.pump();
  let released=false;
  return {key,request:entry.request,release:()=>{
   if(released)return;released=true;entry.references--;
   if(entry.references===0&&(entry.phase==='queued'||entry.disposalRequested))this.evict(entry);
   else this.prune();
   this.pump();
  }};
 }

 clear(sourceIds:readonly string[]):void {
  const targets=new Set(sourceIds);
  for(const entry of this.entries.values())if(targets.has(entry.sourceId)){
   entry.disposalRequested=true;
   if(entry.references===0)this.evict(entry);
  }
  this.pump();
 }

 private revoke(entry:Entry):void {
  if(entry.revoked||entry.url===undefined)return;
  entry.revoked=true;
  if(entry.url.startsWith('blob:'))URL.revokeObjectURL(entry.url);
 }

 private evict(entry:Entry):void {
  if(this.entries.get(entry.key)!==entry)return;
  this.entries.delete(entry.key);this.queue.delete(entry);entry.evicted=true;
  if(entry.phase==='queued'){
   entry.phase='done';entry.reject(new Error('Page render no longer needed'));
  }
  this.revoke(entry);
 }

 private prune():void {
  let bytes=0;for(const entry of this.entries.values())bytes+=entry.estimatedBytes;
  while(this.entries.size>60||bytes>32*1024*1024){
   const entry=[...this.entries.values()].find(value=>value.references===0);
   if(!entry)return; // A mounted preview keeps its image even above budget.
   bytes-=entry.estimatedBytes;this.evict(entry);
  }
 }

 private pump():void {
  while(this.active<2){
   const entry=this.queue.values().next().value as Entry|undefined;
   if(!entry)return;
   this.queue.delete(entry);entry.phase='running';this.active++;
   // Register started work before the caller can close its source. Only work
   // still in queue may wait until after the current effect/ownership turn.
   let rendered:Promise<string>;
   try{rendered=entry.render();}catch(error){rendered=Promise.reject(error);}
   void Promise.resolve(rendered).then(url=>{
    entry.phase='done';entry.url=url;
    if(entry.evicted)this.revoke(entry);
    entry.resolve(url);
   },error=>{
    entry.phase='done';
    if(this.entries.get(entry.key)===entry)this.entries.delete(entry.key);
    entry.reject(error);
   }).finally(()=>{this.active--;this.prune();this.pump();});
  }
 }
}

export const pageImageCache=new PageImageCache();
