import {afterEach,describe,expect,it,vi} from 'vitest';
import {PageImageCache} from '../src/editor/page-image-cache';

const page=(pageIndex:number,sourceId='source')=>({sourceId,pageIndex,width:612,height:792});
const tick=async()=>{for(let i=0;i<12;i++)await Promise.resolve();};
const revoke=vi.fn();
afterEach(()=>{vi.restoreAllMocks();revoke.mockReset();});
function backend(){
 Object.defineProperty(URL,'revokeObjectURL',{configurable:true,value:revoke});
 const pending:Array<{index:number;resolve:(url:string)=>void;reject:(error:Error)=>void}>=[];
 const renderPage=vi.fn((_source:string,index:number)=>new Promise<string>((resolve,reject)=>pending.push({index,resolve,reject})));
 return {adapter:{renderPage},pending};
}

describe('page image cache scheduling and ownership',()=>{
 it('prunes decoded idle images within32MiB and still reuses the recent page',async()=>{
  const cache=new PageImageCache();backend();let generation=0;
  const adapter={renderPage:vi.fn((_source:string,index:number)=>Promise.resolve(`blob:${generation++}:${index}`))};
  const portrait=(index:number)=>({...page(index),width:800,height:1600});
  for(let index=0;index<7;index++){
   const lease=cache.acquire(adapter,portrait(index),800);await lease.request;lease.release();
  }
  // Seven decoded5.12MB portraits exceed32MiB despite any small PNG encoding.
  expect(revoke).toHaveBeenCalledWith('blob:0:0');
  const recent=cache.acquire(adapter,portrait(6),800);expect(await recent.request).toBe('blob:6:6');
  expect(adapter.renderPage).toHaveBeenCalledTimes(7);recent.release();
  const cold=cache.acquire(adapter,portrait(0),800);expect(await cold.request).toBe('blob:7:0');
  expect(adapter.renderPage).toHaveBeenCalledTimes(8);cold.release();cache.clear(['source']);
 });

 it('keeps shared mounted large rasters while pruning additional idle decoded images',async()=>{
  const cache=new PageImageCache();backend();
  const adapter={renderPage:vi.fn((_source:string,index:number)=>Promise.resolve(`blob:large-${index}`))};
  const a=cache.acquire(adapter,page(0),2400),shared=cache.acquire(adapter,page(0),2400),b=cache.acquire(adapter,page(1),2400);
  await Promise.all([a.request,shared.request,b.request]);expect(revoke).not.toHaveBeenCalled();
  const idle=cache.acquire(adapter,page(2),2400);await idle.request;idle.release();
  expect(revoke.mock.calls.map(x=>x[0])).toEqual(['blob:large-2']);
  a.release();expect(revoke).not.toHaveBeenCalledWith('blob:large-0');
  shared.release();expect(revoke).toHaveBeenCalledWith('blob:large-0');
  expect(revoke).not.toHaveBeenCalledWith('blob:large-1');b.release();cache.clear(['source']);
 });

 it('starts at most two renders and skips abandoned queued pages',async()=>{
  const cache=new PageImageCache(),{adapter,pending}=backend();
  const leases=Array.from({length:30},(_,i)=>cache.acquire(adapter,page(i),600));
  const settled=Promise.allSettled(leases.map(x=>x.request));
  await tick();expect(adapter.renderPage).toHaveBeenCalledTimes(2);
  for(let i=2;i<29;i++)leases[i].release();
  pending[0].resolve('blob:first');await tick();
  expect(adapter.renderPage.mock.calls.map(x=>x[1])).toEqual([0,1,29]);
  pending[1].resolve('blob:second');pending[2].resolve('blob:last');await settled;
  for(const lease of leases)lease.release();cache.clear(['source']);
  expect(revoke.mock.calls.map(x=>x[0]).sort()).toEqual(['blob:first','blob:last','blob:second']);
 });

 it('keeps a queued render when another consumer still owns it',async()=>{
  const cache=new PageImageCache(),{adapter,pending}=backend();
  const first=cache.acquire(adapter,page(0),600),second=cache.acquire(adapter,page(1),600);
  const left=cache.acquire(adapter,page(2),600),right=cache.acquire(adapter,page(2),600);
  expect(left.request).toBe(right.request);left.release();left.release();
  await tick();pending[0].resolve('blob:zero');await tick();
  expect(adapter.renderPage.mock.calls.map(x=>x[1])).toEqual([0,1,2]);
  pending[1].resolve('blob:one');pending[2].resolve('blob:two');await right.request;
  cache.clear(['source']);expect(revoke).not.toHaveBeenCalledWith('blob:two');
  right.release();expect(revoke.mock.calls.filter(x=>x[0]==='blob:two')).toHaveLength(1);
  first.release();second.release();
 });

 it('does not share same-id documents from different adapters',async()=>{
  const cache=new PageImageCache();backend();
  const a={renderPage:vi.fn().mockResolvedValue('blob:a')},b={renderPage:vi.fn().mockResolvedValue('blob:b')};
  const left=cache.acquire(a,page(0),600),right=cache.acquire(b,page(0),600);
  expect(left.key).not.toBe(right.key);expect(await left.request).toBe('blob:a');expect(await right.request).toBe('blob:b');
  left.release();right.release();cache.clear(['source']);
  expect(revoke.mock.calls.map(x=>x[0]).sort()).toEqual(['blob:a','blob:b']);
 });

 it('releases a disposed running result exactly once without cancelling another live page',async()=>{
  const cache=new PageImageCache(),{adapter,pending}=backend();
  const discarded=cache.acquire(adapter,page(0,'old'),600),visible=cache.acquire(adapter,page(1,'current'),600);
  await tick();cache.clear(['old']);discarded.release();discarded.release();
  pending[0].resolve('blob:old');pending[1].resolve('blob:current');await tick();
  expect(revoke.mock.calls.filter(x=>x[0]==='blob:old')).toHaveLength(1);
  expect(revoke).not.toHaveBeenCalledWith('blob:current');
  visible.release();cache.clear(['current']);
 });

 it('frees slots after rejected renders and permits a cold retry',async()=>{
  const cache=new PageImageCache(),{adapter,pending}=backend();
  const failed=cache.acquire(adapter,page(0),600),other=cache.acquire(adapter,page(1),600),queued=cache.acquire(adapter,page(2),600);
  const settled=Promise.allSettled([failed.request,other.request,queued.request]);
  await tick();pending[0].reject(new Error('backend failed'));await tick();
  expect(adapter.renderPage.mock.calls.map(x=>x[1])).toEqual([0,1,2]);
  const retry=cache.acquire(adapter,page(0),600);failed.release();
  pending[1].resolve('blob:one');await tick();expect(adapter.renderPage.mock.calls.map(x=>x[1])).toEqual([0,1,2,0]);
  pending[2].resolve('blob:two');pending[3].resolve('blob:retry');await settled;await retry.request;
  other.release();queued.release();retry.release();cache.clear(['source']);
 });

 it('handles a synchronous adapter error without leaving the queue blocked',async()=>{
  const cache=new PageImageCache();backend();
  const adapter={renderPage:vi.fn((_source:string,index:number):Promise<string>=>{
   if(index===0)throw new Error('sync failure');return Promise.resolve('blob:'+index);
  })};
  const leases=[0,1,2].map(i=>cache.acquire(adapter,page(i),600));
  const result=await Promise.allSettled(leases.map(x=>x.request));
  expect(result.map(x=>x.status)).toEqual(['rejected','fulfilled','fulfilled']);
  expect(adapter.renderPage).toHaveBeenCalledTimes(3);for(const lease of leases)lease.release();cache.clear(['source']);
 });

 it('retains mounted previews above sixty entries and evicts the oldest idle entries',async()=>{
  const cache=new PageImageCache();backend();
  const adapter={renderPage:vi.fn((_source:string,index:number)=>Promise.resolve(`blob:${index}`))};
  const leases=Array.from({length:65},(_,i)=>cache.acquire(adapter,page(i),240));
  await Promise.all(leases.map(x=>x.request));expect(revoke).not.toHaveBeenCalled();
  for(const lease of leases)lease.release();
  expect(revoke.mock.calls.map(x=>x[0])).toEqual(['blob:0','blob:1','blob:2','blob:3','blob:4']);
  const recent=cache.acquire(adapter,page(64),240);await recent.request;
  expect(adapter.renderPage).toHaveBeenCalledTimes(65);recent.release();cache.clear(['source']);
 });

 it('drops an oversize decoded-image estimate as soon as its last viewer leaves',async()=>{
  const cache=new PageImageCache();backend();
  const adapter={renderPage:vi.fn().mockResolvedValue('blob:tall')};
  const lease=cache.acquire(adapter,{...page(0),width:600,height:3000},2400);await lease.request;
  expect(revoke).not.toHaveBeenCalled();lease.release();expect(revoke).toHaveBeenCalledTimes(1);expect(revoke).toHaveBeenCalledWith('blob:tall');
  cache.clear(['source']);expect(revoke).toHaveBeenCalledTimes(1);
 });
});
