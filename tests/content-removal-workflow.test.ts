import {expect,it,vi} from 'vitest';
import {captureTextRemoval,TextRemovalWorkflow,supportsContentRemoval} from '../src/editor/text-removal-workflow';
import {createSession} from '../src/editor/workspace';
import {createDemoAdapter} from '../src/editor/adapter';
import {validateContentRaster} from '../src/editor/content-removal-raster';
const png=()=>Array.from(Uint8Array.from(atob('iVBORw0KGgoAAAANSUhEUgAAAEAAAAAmCAYAAAB0xJ2ZAAAAVUlEQVR4nOXOQQEAMAyEMIZ/zzcZfRAFedtGmMRJnMRJnMRJnMRJnMRJnMRJnMRJnMRJnMRJnMRJnMRJnMRJnMRJnMRJnMRJnMRJnMRJnMRJnMRJnNeBax/ObwRIGbWQGQAAAABJRU5ErkJggg=='),c=>c.charCodeAt(0)));
const marks=[0,1].map(pageIndex=>({pageIndex,rect:{x:20,y:40,width:60,height:20}}));
function fixture(){
 const adapter=createDemoAdapter(),pages=[{width:500,height:300},{width:500,height:300}];
 const tab=createSession({id:'original',name:'Source.pdf',pages},adapter,100);
 const source={document:{id:'working',name:'Working.pdf',pages},omittedMetadata:[]};
 const candidate={id:'candidate',name:'Result.pdf',pages};
 const preview={previewId:'preview',sourceId:'working',candidate,removedCharacters:0,removedPaths:0,affectedImages:2,omittedMetadata:[],expiresInSeconds:900,
  pages:pages.map((p,pageIndex)=>({...p,pageIndex,removedCharacters:0,removedPaths:0,affectedImages:1,removedImagePixels:256,extendsBeyondMarks:true,extent:marks[pageIndex].rect}))};
 const raster=(pageIndex:number,previewId:string|null='preview')=>({sourceId:'working',previewId,candidateId:previewId?'candidate':null,pageIndex,pageWidth:500,pageHeight:300,width:64,height:38,sourcePng:png(),candidatePng:previewId?png():null,disclosurePng:previewId?png():null});
 Object.assign(adapter,{beginTextRemovalSession:vi.fn().mockResolvedValue('session'),prepareTextRemovalSource:vi.fn().mockResolvedValue(source),
  previewSourceTextRemoval:vi.fn(),textRemovalPreviewBounds:vi.fn(),previewSourceContentRemoval:vi.fn().mockResolvedValue(preview),
  renderContentRemoval:vi.fn().mockImplementation((_s,_source,previewId,page)=>Promise.resolve(raster(page,previewId))),
  applyTextRemovalPreview:vi.fn().mockResolvedValue(candidate),discardTextRemovalPreview:vi.fn().mockResolvedValue(undefined),releaseTextRemovalSession:vi.fn().mockResolvedValue(undefined),closeDocument:vi.fn()});
 const request=captureTextRemoval(tab,'content');
 const guards={isCurrent:vi.fn(()=>true),isBorrowed:vi.fn((id:string)=>id==='original'||id==='foreign')};
 return {adapter:adapter as any,preview,raster,request,guards,flow:new TextRemovalWorkflow(request,guards)};
}
it('requires explicit combined capability and never silently uses text-only removal',()=>{
 const {request,adapter}=fixture();expect(request.mode).toBe('content');expect(supportsContentRemoval(adapter)).toBe(true);
 delete adapter.previewSourceContentRemoval;expect(supportsContentRemoval(adapter)).toBe(false);
 expect(()=>new TextRemovalWorkflow(request,{isCurrent:()=>true,isBorrowed:()=>false})).toThrow();
});
it('accepts image-only summaries but requires every affected page to be displayed before Apply',async()=>{
 const {flow,adapter}=fixture();await flow.prepare();const p=await flow.preview(marks),adopt=vi.fn(()=>true);
 expect(p.removedCharacters).toBe(0);expect(adapter.previewSourceTextRemoval).not.toHaveBeenCalled();
 await expect(flow.apply(p,adopt)).rejects.toThrow(/review/i);
 const first=await flow.raster(p,0,64);await expect(flow.apply(p,adopt)).rejects.toThrow(/review/i);
 flow.reviewRaster(first);await expect(flow.apply(p,adopt)).rejects.toThrow(/review/i);
 flow.reviewRaster(await flow.raster(p,1,64));await flow.apply(p,adopt);
 expect(adopt).toHaveBeenCalledTimes(1);expect(adapter.applyTextRemovalPreview).toHaveBeenCalledTimes(1);
});
it('accepts a marked blank page being omitted but refuses inconsistent totals and unknown pages',async()=>{
 const good=fixture();good.preview.pages.pop();good.preview.affectedImages=1;
 await good.flow.prepare();expect((await good.flow.preview(marks)).pages).toHaveLength(1);good.flow.dispose();
 for(const mutate of [(p:any)=>p.affectedImages=3,(p:any)=>p.pages[0].pageIndex=9,(p:any)=>p.pages[0].affectedImages=-1,(p:any)=>p.pages[0].extendsBeyondMarks='yes']){
  const f=fixture();mutate(f.preview);await f.flow.prepare();await expect(f.flow.preview(marks)).rejects.toThrow();
 }
});
it('validates PNG dimensions, identities, page and byte values before review',async()=>{
 for(const mutate of [(r:any)=>r.candidateId='foreign',(r:any)=>r.previewId='stale',(r:any)=>r.pageIndex=1,(r:any)=>r.width=65,(r:any)=>r.sourcePng[0]=0,(r:any)=>r.disclosurePng[23]=39,(r:any)=>r.candidatePng[40]=256]){
  const f=fixture();await f.flow.prepare();const p=await f.flow.preview(marks),r=f.raster(0);mutate(r);f.adapter.renderContentRemoval.mockResolvedValue(r);
  await expect(f.flow.raster(p,0,64)).rejects.toThrow();expect(f.adapter.applyTextRemovalPreview).not.toHaveBeenCalled();f.flow.dispose();
 }
});
it('refuses borrowed, fabricated, invalidated and late raster review without native Apply',async()=>{
 const f=fixture();await f.flow.prepare();const p=await f.flow.preview(marks);
 expect(()=>f.flow.reviewRaster(f.raster(0) as any)).toThrow();
 const valid=await f.flow.raster(p,0,64);f.flow.invalidate();expect(()=>f.flow.reviewRaster(valid)).toThrow();
 expect(f.adapter.applyTextRemovalPreview).not.toHaveBeenCalled();f.flow.dispose();
 const late=fixture();await late.flow.prepare();const lp=await late.flow.preview(marks);
 let resolve!:(v:any)=>void;late.adapter.renderContentRemoval.mockReturnValue(new Promise(r=>resolve=r));
 const pending=late.flow.raster(lp,0,64),refused=expect(pending).rejects.toThrow();late.flow.dispose();resolve(late.raster(0));await refused;
});
it('renders the owned source before Preview but cannot mark that image reviewed',async()=>{
 const f=fixture();await f.flow.prepare();const r=await f.flow.raster(null,0,64);
 expect(r.candidateId).toBeNull();expect(()=>f.flow.reviewRaster(r)).toThrow();
 expect(f.adapter.renderContentRemoval).toHaveBeenCalledWith('session','working',null,0,64);f.flow.dispose();
});
it('binds fractional native page dimensions through their original float32 representation',()=>{
 const value={sourceId:'working',previewId:null,candidateId:null,pageIndex:0,pageWidth:Math.fround(500.1),pageHeight:Math.fround(300.1),width:64,height:38,sourcePng:png(),candidatePng:null,disclosurePng:null};
 const expected={sourceId:'working',previewId:null,candidateId:null,pageIndex:0,pageWidth:500.1,pageHeight:300.1,requestedWidth:64};
 expect(validateContentRaster(value,expected).sourcePng).toHaveLength(png().length);
 expect(()=>validateContentRaster({...value,pageWidth:value.pageWidth+.001},expected)).toThrow();
});
