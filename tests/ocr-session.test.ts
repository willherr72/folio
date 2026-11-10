import {afterEach,beforeEach,describe,expect,it,vi} from 'vitest';
import {startOcrSession} from '../src/editor/ocr/session';
import type {OcrLanguage} from '../src/editor/ocr/types';

type Packet={jobId:string;workerId:string;action:string;payload:Record<string,unknown>};
class FakeWorker {
  static instances:FakeWorker[]=[];
  onmessage:((event:MessageEvent)=>void)|null=null;
  onerror:((event:ErrorEvent)=>void)|null=null;
  onmessageerror:((event:MessageEvent)=>void)|null=null;
  packets:Packet[]=[];terminated=0;
  constructor(readonly url:URL){FakeWorker.instances.push(this);}
  postMessage(packet:Packet){this.packets.push(packet);}
  terminate(){this.terminated++;}
  reply(status='resolve',data:unknown={},packet=this.packets.at(-1)!){
    this.onmessage?.({data:{...packet,status,data}} as MessageEvent);
  }
}
const tick=async()=>{await Promise.resolve();await Promise.resolve();};
const image=()=>{
  const bytes=new Uint8Array(33);bytes.set([137,80,78,71,13,10,26,10]);
  const v=new DataView(bytes.buffer);v.setUint32(8,13);bytes.set([73,72,68,82],12);v.setUint32(16,10);v.setUint32(20,10);
  return {png:bytes,width:10,height:10,dpi:300};
};
async function ready(options:Parameters<typeof startOcrSession>[1]={}) {
  const pending=startOcrSession(['eng'],options),worker=FakeWorker.instances.at(-1)!;
  for(const action of ['load','loadLanguage','initialize']){
    expect(worker.packets.at(-1)?.action).toBe(action);worker.reply();await tick();
  }
  return {session:await pending,worker};
}
beforeEach(()=>{vi.useFakeTimers();FakeWorker.instances=[];vi.stubGlobal('Worker',FakeWorker);});
afterEach(()=>{vi.useRealTimers();vi.unstubAllGlobals();});
describe('owned OCR worker lifecycle',()=>{
  it('uses only pinned local paths and disables persistent model cache',async()=>{
    const {session,worker}=await ready();
    expect(worker.url.pathname).toBe('/ocr/worker.js');
    expect(worker.packets[0].payload).toMatchObject({options:{lstmOnly:true,corePath:expect.stringContaining('/ocr/core/')}});
    expect(worker.packets[1].payload).toMatchObject({langs:['eng'],options:{gzip:false,cacheMethod:'none',langPath:expect.stringContaining('/ocr/models/')}});
    session.dispose();session.dispose();expect(worker.terminated).toBe(1);expect(vi.getTimerCount()).toBe(0);
  });
  it.each([[],['eng','eng'],['deu'],['../../secret']].map(languages=>({languages})))('rejects invalid language choices before allocating: $languages',async ({languages})=>{
    await expect(startOcrSession(languages as OcrLanguage[])).rejects.toThrow(/language/);
    expect(FakeWorker.instances).toHaveLength(0);
  });
  it('rejects already aborted start without allocating',async()=>{
    const abort=new AbortController();abort.abort();
    await expect(startOcrSession(['eng'],{signal:abort.signal})).rejects.toMatchObject({name:'AbortError'});
    expect(FakeWorker.instances).toHaveLength(0);
  });
  it.each([0,1,2])('fails promptly during initialization stage %i and releases worker',async stage=>{
    const pending=startOcrSession(['eng']);const assertion=expect(pending).rejects.toThrow(/missing model/);
    const worker=FakeWorker.instances[0];
    for(let i=0;i<stage;i++){worker.reply();await tick();}
    worker.reply('reject','missing model');await assertion;
    expect(worker.terminated).toBe(1);expect(vi.getTimerCount()).toBe(0);
  });
  it('cancels startup and ignores a captured late reply',async()=>{
    const abort=new AbortController(),pending=startOcrSession(['eng'],{signal:abort.signal});
    const assertion=expect(pending).rejects.toMatchObject({name:'AbortError'});
    const worker=FakeWorker.instances[0],handler=worker.onmessage!,packet=worker.packets[0];
    abort.abort();await assertion;handler({data:{...packet,status:'resolve',data:{}}} as MessageEvent);
    await tick();expect(worker.terminated).toBe(1);expect(worker.packets).toHaveLength(1);
  });
  it('enforces the initialization deadline',async()=>{
    const pending=startOcrSession(['eng']),assertion=expect(pending).rejects.toThrow(/timed out/);
    await vi.advanceTimersByTimeAsync(120000);await assertion;
    expect(FakeWorker.instances[0].terminated).toBe(1);expect(vi.getTimerCount()).toBe(0);
  });
  it('reuses the worker across pages and preserves caller PNG ownership',async()=>{
    const {session,worker}=await ready();
    for(let i=0;i<2;i++){
      const raster=image(),pending=session.recognize(raster);
      expect(worker.packets.at(-1)?.action).toBe('setParameters');worker.reply();await tick();
      expect(worker.packets.at(-1)?.action).toBe('recognize');
      expect(worker.packets.at(-1)?.payload).toMatchObject({options:{rotateAuto:false},output:{text:true,blocks:true}});
      worker.reply('resolve',{blocks:null});expect(await pending).toEqual({width:10,height:10,blocks:[]});
      expect(raster.png.length).toBe(33);
    }
    expect(FakeWorker.instances).toHaveLength(1);session.dispose();
  });
  it('rejects concurrent page requests without disrupting the first',async()=>{
    const {session,worker}=await ready(),pending=session.recognize(image());
    await expect(session.recognize(image())).rejects.toThrow(/already/);
    worker.reply();await tick();worker.reply('resolve',{blocks:null});await pending;
    expect(worker.terminated).toBe(0);session.dispose();
  });
  it('snapshots raster properties before the caller can replace them',async()=>{
    const {session,worker}=await ready(),raster=image(),pending=session.recognize(raster);
    raster.png=new Uint8Array(32*1024*1024+1);raster.width=6000;raster.height=6000;raster.dpi=900;
    worker.reply();await tick();
    const submitted=worker.packets.at(-1)!.payload.image as Uint8Array;
    expect(submitted.length).toBe(33);expect(submitted[0]).toBe(137);
    worker.reply('resolve',{blocks:null});expect(await pending).toEqual({width:10,height:10,blocks:[]});
    session.dispose();
  });
  it('snapshots the original byte buffer without detaching or mutating caller ownership',async()=>{
    const {session,worker}=await ready(),raster=image(),original=raster.png,pending=session.recognize(raster);
    original.fill(0);worker.reply();await tick();
    const submitted=worker.packets.at(-1)!.payload.image as Uint8Array;
    expect(submitted).not.toBe(original);expect(submitted[0]).toBe(137);expect(original[0]).toBe(0);expect(original.length).toBe(33);
    worker.reply('resolve',{blocks:null});await pending;session.dispose();
  });
  it('cancels a page and rejects further use of that session',async()=>{
    const {session,worker}=await ready(),abort=new AbortController();
    const pending=session.recognize(image(),abort.signal),assertion=expect(pending).rejects.toMatchObject({name:'AbortError'});
    worker.reply();await tick();abort.abort();await assertion;
    await expect(session.recognize(image())).rejects.toMatchObject({name:'AbortError'});
    expect(worker.terminated).toBe(1);expect(vi.getTimerCount()).toBe(0);
  });
  it('session abort remains active after initialization',async()=>{
    const abort=new AbortController(),{session,worker}=await ready({signal:abort.signal});
    abort.abort();await expect(session.recognize(image())).rejects.toMatchObject({name:'AbortError'});
    expect(worker.terminated).toBe(1);
  });
  it('does not publish a ready session when completion progress cancels startup',async()=>{
    const abort=new AbortController();
    const pending=startOcrSession(['eng'],{signal:abort.signal,onProgress:p=>{if(p.fraction===1)abort.abort();}});
    const assertion=expect(pending).rejects.toMatchObject({name:'AbortError'}),worker=FakeWorker.instances[0];
    for(let i=0;i<3;i++){worker.reply();await tick();}
    await assertion;expect(worker.terminated).toBe(1);
  });
  it('does not publish a page when completion progress cancels recognition',async()=>{
    const abort=new AbortController();
    const {session,worker}=await ready({onProgress:p=>{if(p.stage==='recognizing'&&p.fraction===1)abort.abort();}});
    const pending=session.recognize(image(),abort.signal),assertion=expect(pending).rejects.toMatchObject({name:'AbortError'});
    worker.reply();await tick();worker.reply('resolve',{blocks:null});await assertion;
    expect(worker.terminated).toBe(1);
  });
  it('invalid input is rejected before posting and malformed output ends the session',async()=>{
    const {session,worker}=await ready(),count=worker.packets.length;
    await expect(session.recognize({...image(),width:11})).rejects.toThrow(/OCR/);
    expect(worker.packets).toHaveLength(count);
    const pending=session.recognize(image()),assertion=expect(pending).rejects.toThrow(/OCR/);
    worker.reply();await tick();worker.reply('resolve',{blocks:'invalid'});await assertion;
    expect(worker.terminated).toBe(1);
  });
  it.each(['error','messageerror','postMessage','protocol','timeout'])('settles and cleans up after %s',async failure=>{
    const {session,worker}=await ready();
    if(failure==='postMessage')worker.postMessage=()=>{throw new Error('clone failed');};
    const pending=session.recognize(image()),assertion=expect(pending).rejects.toThrow();
    if(failure==='error')worker.onerror?.({message:'engine crashed',preventDefault(){}} as ErrorEvent);
    if(failure==='messageerror')worker.onmessageerror?.({} as MessageEvent);
    if(failure==='protocol')worker.reply('unexpected',{});
    if(failure==='timeout')await vi.advanceTimersByTimeAsync(120000);
    await assertion;expect(worker.terminated).toBe(1);expect(vi.getTimerCount()).toBe(0);
  });
  it('reports bounded progress and tolerates a throwing progress observer',async()=>{
    const values:unknown[]=[];
    const {session,worker}=await ready({onProgress:p=>{values.push(p);throw new Error('observer');}});
    const pending=session.recognize(image());worker.reply();await tick();
    worker.reply('progress',{progress:0.5});worker.reply('progress',{progress:Infinity});
    worker.reply('resolve',{blocks:null});await pending;
    expect(values).toContainEqual({stage:'recognizing',fraction:0.5});
    expect(values).not.toContainEqual({stage:'recognizing',fraction:Infinity});session.dispose();
  });
});
