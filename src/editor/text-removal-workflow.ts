import type {AnnotationRect,DocumentInfo,EditorDocument} from './types';
import type {DocumentSession} from './workspace';
import type {FolioAdapter} from './adapter';
import {pageVisibleRect} from './geometry';
import {commit} from './model';
import {importBookmarks} from './bookmarks';
import {isContentRemoval,type ContentRemovalAdapter,type RemovalPreview,type TextRemovalAdapter,type TextRemovalMark,type TextRemovalSource,type TextRemovalBounds} from './text-removal';
import {validateContentRaster,type ValidatedContentRaster} from './content-removal-raster';

export interface RemovalRequest {
 readonly mode:'text'|'content';
 readonly tabId:string;
 readonly adapter:FolioAdapter;
 readonly history:DocumentSession['history'];
 readonly document:EditorDocument;
}
const clone=<T>(value:T):T=>structuredClone(value);
function frozen<T>(value:T):T {
 if(value&&typeof value==='object'){Object.freeze(value);for(const child of Object.values(value))frozen(child);}
 return value;
}
function requireValue(valid:unknown,message='The text-removal result is invalid.'):asserts valid {if(!valid)throw new Error(message);}
const validId=(id:unknown):id is string=>typeof id==='string'&&id.length>0&&id.length<=256;
/** Match the native outline shape before handing values to the sidebar/remapper. */
function validateBookmarks(items:unknown,pageCount:number):void {
 requireValue(Array.isArray(items)&&items.length<=4000);
 let nodes=0,bytes=0;
 const take=(size=0)=>{nodes++;bytes+=size;requireValue(nodes<=100_000&&bytes<=4*1024*1024);};
 const byteArray=(value:unknown):value is number[]=>{
  requireValue(Array.isArray(value));take(value.length);requireValue(value.every(n=>Number.isInteger(n)&&n>=0&&n<=255));return true;
 };
 const destination=(value:any)=>{
  requireValue(value?.kind==='array'&&Array.isArray(value.value));const a=value.value;
  requireValue(a.length>=2&&a[0]?.kind==='page'&&a[1]?.kind==='name'&&Array.isArray(a[1].value)&&a[1].value.length<=5);
  const mode=String.fromCharCode(...a[1].value),count:Record<string,number>={Fit:2,FitB:2,FitH:3,FitV:3,FitBH:3,FitBV:3,XYZ:5,FitR:6};
  requireValue(a.length===count[mode]&&a.slice(2).every((p:any)=>p?.kind==='null'||p?.kind==='integer'||p?.kind==='real'));
 };
 const value=(v:any,depth:number):void=>{
  take();requireValue(depth<=32&&v&&typeof v==='object'&&!Array.isArray(v));
  switch(v.kind){
   case 'null':return;
   case 'boolean':requireValue(typeof v.value==='boolean');return;
   case 'integer':requireValue(Number.isSafeInteger(v.value));return;
   case 'real':requireValue(typeof v.value==='number'&&Number.isFinite(v.value));return;
   case 'name':case 'string':byteArray(v.value);return;
   case 'page':requireValue(typeof v.value==='string'&&/^(0|[1-9]\d*)$/.test(v.value)&&Number(v.value)<pageCount);return;
   case 'array':requireValue(Array.isArray(v.value)&&v.value.length<=100_000-nodes);for(const item of v.value)value(item,depth+1);return;
   case 'dictionary':case 'stream':{
    const entries=v.kind==='dictionary'?v.value:v.value?.dictionary;
    requireValue(Array.isArray(entries)&&entries.length<=100_000-nodes);
    const keys=new Set<string>();let action:any,target:any;
    for(const entry of entries){requireValue(Array.isArray(entry)&&entry.length===2);byteArray(entry[0]);const key=entry[0].join(',');requireValue(!keys.has(key));keys.add(key);value(entry[1],depth+1);if(key==='83')action=entry[1];if(key==='68')target=entry[1];}
    if(action?.kind==='name'&&action.value.join(',')==='71,111,84,111')destination(target);
    if(v.kind==='stream')byteArray(v.value.content);return;
   }
   default:throw new Error('The text-removal bookmarks are invalid.');
  }
 };
 const ids=new Set<string>(),allowed=new Set(['id','title','level','destination','action','bold','italic','closed','color']);
 for(let i=0;i<items.length;i++){
  const b=items[i];requireValue(b&&typeof b==='object'&&!Array.isArray(b)&&Object.keys(b).every(k=>allowed.has(k)));
  requireValue(typeof b.id==='string'&&b.id.length>0&&b.id.length<=1024&&!ids.has(b.id)&&typeof b.title==='string'&&b.title.length<=4096);ids.add(b.id);
  const idBytes=new TextEncoder().encode(b.id).length,titleBytes=new TextEncoder().encode(b.title).length;requireValue(idBytes<=1024&&titleBytes<=4096);take(idBytes+titleBytes);
  requireValue(Number.isInteger(b.level)&&b.level>=0&&b.level<32&&(i===0?b.level===0:b.level<=items[i-1].level+1));
  for(const flag of ['bold','italic','closed'])requireValue(b[flag]===undefined||typeof b[flag]==='boolean');
  requireValue(b.color===undefined||(Array.isArray(b.color)&&b.color.length===3&&b.color.every((n:unknown)=>typeof n==='number'&&Number.isFinite(n)&&n>=0&&n<=1)));
  if(b.destination!==undefined){value(b.destination,0);destination(b.destination);}
  if(b.action!==undefined){requireValue(b.action?.kind==='dictionary');value(b.action,0);}
 }
}
export function captureTextRemoval(session:DocumentSession,mode:'text'|'content'='text'):RemovalRequest {
 requireValue(mode==='text'||mode==='content');
 const document=session.history.present;
 requireValue(!document.bookmarkError,'Unreadable bookmarks must be discarded explicitly in the Bookmarks panel first.');
 requireValue(document.pages.length>0&&document.pages.length<=1000,'Text removal supports documents with 1 to 1000 pages.');
 requireValue(document.pages.every(page=>!page.protection),'Text removal is not available for protected PDFs.');
 return Object.freeze({mode,tabId:session.id,adapter:session.adapter,history:session.history,document:frozen(clone(document))});
}
function validateDocument(info:DocumentInfo,request:RemovalRequest):void {
 requireValue(info&&validId(info.id)&&typeof info.name==='string'&&!info.protection&&!info.bookmarkError);
 requireValue(Array.isArray(info.pages)&&info.pages.length===request.document.pages.length);
 info.pages.forEach((page,index)=>{
  const expected=pageVisibleRect(request.document.pages[index]);
  requireValue(page&&Number.isFinite(page.width)&&Number.isFinite(page.height)&&page.width>0&&page.height>0);
  requireValue(Math.abs(page.width-expected.width)<=.01&&Math.abs(page.height-expected.height)<=.01,'Text removal changed the page dimensions.');
  requireValue(page.overlays===undefined||(Array.isArray(page.overlays)&&page.overlays.length===0));
 });
 if(info.bookmarks!==undefined)validateBookmarks(info.bookmarks,info.pages.length);
}
export function commitTextRemoval(session:DocumentSession,request:RemovalRequest,info:DocumentInfo):DocumentSession {
 requireValue(session.id===request.tabId&&session.adapter===request.adapter&&session.history===request.history,'The document changed. Reopen text removal and try again.');
 validateDocument(info,request);
 requireValue(!session.sourceIds.includes(info.id),'Text removal must return a new source.');
 const pages=request.document.pages.map((page,index)=>({id:page.id,sourceId:info.id,pageIndex:index,width:info.pages[index].width,height:info.pages[index].height,rotation:0 as const,overlays:[]}));
 const document:EditorDocument={name:session.history.present.name,pages,selectedPageId:request.document.selectedPageId,selectedOverlayId:null,bookmarks:importBookmarks(clone(info.bookmarks??[]),pages)};
 return {...session,sourceIds:[...session.sourceIds,info.id],history:commit(session.history,()=>document)};
}
const capabilities=(['beginTextRemovalSession','prepareTextRemovalSource','previewSourceTextRemoval','textRemovalPreviewBounds','applyTextRemovalPreview','discardTextRemovalPreview','releaseTextRemovalSession'] as const);
export function supportsTextRemoval(adapter:FolioAdapter):adapter is FolioAdapter&TextRemovalAdapter {
 return capabilities.every(key=>typeof adapter[key]==='function');
}
export function supportsContentRemoval(adapter:FolioAdapter):adapter is FolioAdapter&TextRemovalAdapter&ContentRemovalAdapter {
 return supportsTextRemoval(adapter)&&typeof adapter.previewSourceContentRemoval==='function'&&typeof adapter.renderContentRemoval==='function';
}
function rectValid(rect:AnnotationRect,width:number,height:number):boolean {
 return !!rect&&[rect.x,rect.y,rect.width,rect.height].every(Number.isFinite)&&rect.x>=0&&rect.y>=0&&rect.width>0&&rect.height>0&&rect.x+rect.width<=width+.01&&rect.y+rect.height<=height+.01;
}
function omissions(value:string[]):void {requireValue(Array.isArray(value)&&value.length<=32&&value.every(v=>typeof v==='string'&&v.length>0&&v.length<=2000));}
const integer=(value:number,max:number)=>Number.isSafeInteger(value)&&value>0&&value<=max;
/** The native session owns pending PDFs; only a successful Apply can transfer one. */
export class TextRemovalWorkflow {
 private readonly adapter:FolioAdapter&TextRemovalAdapter;
 private session:string|null=null;
 private released=false;
 private closed=false;
 private busy=false;
 private prepared:TextRemovalSource|null=null;
 private currentPreview:RemovalPreview|null=null;
 private readonly reviewed=new Set<number>();
 private readonly rasters=new WeakMap<ValidatedContentRaster,{preview:RemovalPreview|null;page:number}>();
 private deadline=0;
 private discarded:Promise<void>=Promise.resolve();
 private readonly seen=new Set<string>();
 constructor(private readonly request:RemovalRequest,private readonly guards:{isCurrent():boolean;isBorrowed(id:string):boolean;onPreviewFailure?(message:string):void}){
  requireValue(supportsTextRemoval(request.adapter),'Text removal requires the desktop PDF service.');this.adapter=request.adapter;
  requireValue(request.mode!=='content'||supportsContentRemoval(request.adapter),'Combined content removal requires the desktop PDF service.');
 }
 private current():void {
  if(!this.guards.isCurrent()){this.dispose();throw new Error('The document changed. Reopen text removal and try again.');}
  requireValue(!this.closed,'Text removal was cancelled.');
 }
 private fresh(id:string):void {requireValue(validId(id)&&!this.guards.isBorrowed(id)&&!this.seen.has(id),'Text removal must return a new source.');}
 private async run<T>(work:()=>Promise<T>):Promise<T>{
  this.current();requireValue(!this.busy,'Text removal is already working.');this.busy=true;
  try{return await work();}finally{this.busy=false;}
 }
 private release():void {
  if(!this.session||this.released)return;this.released=true;
  void Promise.resolve().then(()=>this.adapter.releaseTextRemovalSession(this.session!)).catch(()=>{});
 }
 dispose():void {this.closed=true;this.currentPreview=null;this.release();}
 async prepare():Promise<TextRemovalSource>{
  return this.run(async()=>{
   requireValue(!this.session&&!this.prepared);
   try{
    const id=await this.adapter.beginTextRemovalSession();requireValue(validId(id));this.session=id;
    if(this.closed)this.release();this.current();
    const source=clone(await this.adapter.prepareTextRemovalSource(id,clone(this.request.document.pages),clone(this.request.document.bookmarks)));
    this.current();validateDocument(source.document,this.request);this.fresh(source.document.id);omissions(source.omittedMetadata);
    this.seen.add(source.document.id);this.prepared=frozen(source);return this.prepared;
   }catch(error){this.dispose();throw error;}
  });
 }
 invalidate():void {
  this.reviewed.clear();
  const previous=this.currentPreview;this.currentPreview=null;this.deadline=0;
  if(previous&&this.session&&!this.closed){const id=this.session;this.discarded=this.discarded.then(()=>this.adapter.discardTextRemovalPreview(id,previous.previewId)).catch(()=>{});}
 }
 async preview(marks:TextRemovalMark[]):Promise<RemovalPreview>{
  return this.run(async()=>{
   requireValue(this.prepared&&this.session,'Refresh the working copy before previewing.');
   requireValue(Array.isArray(marks)&&marks.length>0&&marks.length<=64,'Add between 1 and 64 selections.');
   const selected=new Set<number>();
   for(const mark of marks){const p=this.prepared.document.pages[mark.pageIndex];requireValue(Number.isSafeInteger(mark.pageIndex)&&mark.pageIndex>=0&&p&&rectValid(mark.rect,p.width,p.height),'Selections must be inside the page.');selected.add(mark.pageIndex);}
   const copied=clone(marks);this.invalidate();await this.discarded;this.current();
   const value=clone(await (this.request.mode==='content'
    ?this.adapter.previewSourceContentRemoval!(this.session,this.prepared.document.id,copied)
    :this.adapter.previewSourceTextRemoval(this.session,this.prepared.document.id,copied)));
   this.current();
   try{
    requireValue(validId(value.previewId)&&value.sourceId===this.prepared.document.id);validateDocument(value.candidate,this.request);this.fresh(value.candidate.id);
    requireValue(isContentRemoval(value)===(this.request.mode==='content')&&integer(value.expiresInSeconds,900));
    omissions(value.omittedMetadata);requireValue(this.prepared.omittedMetadata.every(item=>value.omittedMetadata.includes(item)));
    requireValue(Array.isArray(value.pages)&&value.pages.length>0&&value.pages.length<=selected.size);
    const unique=new Set<number>();
    for(const p of value.pages){const dimensions=value.candidate.pages[p.pageIndex];requireValue(Number.isSafeInteger(p.pageIndex)&&selected.has(p.pageIndex)&&!unique.has(p.pageIndex)&&dimensions);unique.add(p.pageIndex);requireValue(Math.abs(p.width-dimensions.width)<=.01&&Math.abs(p.height-dimensions.height)<=.01);}
    if(isContentRemoval(value)){
     const nonnegative=(n:number,max:number)=>Number.isSafeInteger(n)&&n>=0&&n<=max;
     requireValue(nonnegative(value.removedCharacters,1_000_000)&&nonnegative(value.removedPaths,100_000)&&nonnegative(value.affectedImages,100_000));
     let characters=0,paths=0,images=0;
     for(const p of value.pages){
      requireValue(nonnegative(p.removedCharacters,1_000_000)&&nonnegative(p.removedPaths,100_000)&&nonnegative(p.affectedImages,100_000)&&nonnegative(p.removedImagePixels,Number.MAX_SAFE_INTEGER)&&typeof p.extendsBeyondMarks==='boolean');
      requireValue(p.removedCharacters+p.removedPaths+p.affectedImages>0&&(p.extent===null||rectValid(p.extent,p.width,p.height)));
      characters+=p.removedCharacters;paths+=p.removedPaths;images+=p.affectedImages;
     }
     requireValue(characters===value.removedCharacters&&paths===value.removedPaths&&images===value.affectedImages);
    }else{
     requireValue(integer(value.removedCharacters,1_000_000)&&integer(value.affectedBlocks,100_000)&&value.pages.length===selected.size);
     let glyphs=0;
     for(const p of value.pages){requireValue(integer(p.selectedGlyphCount,1_000_000)&&rectValid(p.extent,p.width,p.height));glyphs+=p.selectedGlyphCount;}
     requireValue(glyphs<=1_000_000);
    }
    this.seen.add(value.candidate.id);this.deadline=Date.now()+value.expiresInSeconds*1000;this.currentPreview=frozen(value);return this.currentPreview;
   }catch(error){this.dispose();throw error;}
  });
 }
 private matching(preview:RemovalPreview):void {
  this.current();requireValue(preview===this.currentPreview&&Date.now()<this.deadline,'Preview these selections again before applying.');
  requireValue(!this.guards.isBorrowed(preview.candidate.id),'The preview source is already owned by a document.');
 }
 async bounds(preview:RemovalPreview,page:number,offset:number):Promise<TextRemovalBounds>{
  try{
   this.matching(preview);requireValue(!isContentRemoval(preview));const summary=preview.pages.find(p=>p.pageIndex===page);
   requireValue(summary&&Number.isSafeInteger(offset)&&offset>=0&&offset<summary.selectedGlyphCount);
   const result=clone(await this.adapter.textRemovalPreviewBounds(this.session!,preview.previewId,page,offset,1000));this.matching(preview);
   requireValue(result.pageIndex===page&&result.offset===offset&&result.total===summary.selectedGlyphCount&&Array.isArray(result.boxes)&&result.boxes.length===Math.min(1000,result.total-offset));
   requireValue(result.boxes.every(box=>rectValid(box,summary.width,summary.height)));
   return frozen(result);
  }catch(error){
   if(this.currentPreview===preview){this.invalidate();this.guards.onPreviewFailure?.(error instanceof Error?error.message:String(error));}
   throw error;
  }
 }
 async raster(preview:RemovalPreview|null,page:number,width=900):Promise<ValidatedContentRaster>{
  const matches=()=>{this.current();requireValue(this.request.mode==='content'&&this.prepared&&this.session);if(preview)this.matching(preview);else requireValue(!this.currentPreview);};
  matches();requireValue(Number.isSafeInteger(page)&&page>=0&&page<this.prepared!.document.pages.length&&Number.isInteger(width)&&width>=64&&width<=1200);
  const dimensions=this.prepared!.document.pages[page],sourceId=this.prepared!.document.id;
  try{
   const value=await this.adapter.renderContentRemoval!(this.session!,sourceId,preview?.previewId??null,page,width);matches();
   const result=validateContentRaster(value,{sourceId,previewId:preview?.previewId??null,candidateId:preview?.candidate.id??null,pageIndex:page,pageWidth:dimensions.width,pageHeight:dimensions.height,requestedWidth:width});
   this.rasters.set(result,{preview,page});return result;
  }catch(error){if(preview&&this.currentPreview===preview){this.invalidate();this.guards.onPreviewFailure?.(error instanceof Error?error.message:String(error));}throw error;}
 }
 reviewRaster(raster:ValidatedContentRaster):void {
  const bound=this.rasters.get(raster);requireValue(bound?.preview,'Render and review this page before applying.');this.matching(bound.preview);this.reviewed.add(bound.page);
 }
 async apply(preview:RemovalPreview,adopt:(info:DocumentInfo,preview:RemovalPreview)=>boolean):Promise<void>{
  return this.run(async()=>{
   this.matching(preview);requireValue(!isContentRemoval(preview)||preview.pages.every(p=>this.reviewed.has(p.pageIndex)),'Review every affected page before applying.');const candidate=preview.candidate;this.currentPreview=null;
   try{
    const result=clone(await this.adapter.applyTextRemovalPreview(this.session!,preview.previewId));this.current();
    requireValue(!this.guards.isBorrowed(candidate.id));validateDocument(result,this.request);
    requireValue(JSON.stringify(result)===JSON.stringify(candidate),'Apply returned a different preview.');
    requireValue(adopt(frozen(result),preview),'The document changed. Reopen text removal and try again.');
    this.dispose();
   }catch(error){
    // Success may have reached native code even when the IPC reply was lost.
    // This ID was validated before Apply. Unknown response IDs are never closed.
    if(!this.guards.isBorrowed(candidate.id))await Promise.resolve().then(()=>this.adapter.closeDocument(candidate.id)).catch(()=>{});
    throw error;
   }
  });
 }
}
