import {act,cleanup,render,waitFor} from '@testing-library/react';
import {StrictMode} from 'react';
import {afterEach,expect,it,vi} from 'vitest';
import {clearRenderCache,usePageImage} from '../src/components/PageView';
import {createDemoAdapter,type FolioAdapter} from '../src/editor/adapter';

const sourceId='navigation-render-queue';
afterEach(()=>{cleanup();clearRenderCache([sourceId]);vi.restoreAllMocks();});

function Page({adapter,index,revision=0}:{adapter:FolioAdapter;index:number;revision?:number}){
 const result=usePageImage(adapter,{sourceId,pageIndex:index,width:612,height:792},600,true,revision);
 return <output data-page={index}>{result.url??result.error??'Loading'}</output>;
}

it('bounds native work and skips pages abandoned during rapid navigation',async()=>{
 const finishes:Array<()=>void>=[];
 const renderPage=vi.fn((_source:string,index:number)=>new Promise<string>(resolve=>{
  finishes.push(()=>resolve(`blob:render-${index}`));
 }));
 const revoke=vi.fn();Object.defineProperty(URL,'revokeObjectURL',{configurable:true,value:revoke});
 const adapter={...createDemoAdapter(),kind:'native' as const,renderPage};
 const view=render(<>{Array.from({length:80},(_,index)=><Page key={index} adapter={adapter} index={index}/>)}</>);
 try{
  await waitFor(()=>expect(renderPage).toHaveBeenCalled());
  expect(renderPage).toHaveBeenCalledTimes(2);
  view.rerender(<Page key={79} adapter={adapter} index={79}/>);
  await act(async()=>{for(const finish of finishes.splice(0))finish();});
  await waitFor(()=>expect(renderPage).toHaveBeenCalledTimes(3));
  expect(renderPage.mock.calls.map(call=>call[1])).toEqual([0,1,79]);
  await act(async()=>{for(const finish of finishes.splice(0))finish();});
  await waitFor(()=>expect(view.getByText('blob:render-79')).toBeInTheDocument());
  expect(revoke).not.toHaveBeenCalledWith('blob:render-79');
 }finally{
  view.unmount();clearRenderCache([sourceId]);
  await act(async()=>{for(const finish of finishes.splice(0))finish();});
 }
});

it('keeps the shared render alive through StrictMode effect replay',async()=>{
 Object.defineProperty(URL,'revokeObjectURL',{configurable:true,value:vi.fn()});
 let finish!:(url:string)=>void;
 const adapter={...createDemoAdapter(),renderPage:vi.fn(()=>new Promise<string>(resolve=>{finish=resolve;}))};
 const view=render(<StrictMode><Page adapter={adapter} index={0}/></StrictMode>);
 await waitFor(()=>expect(adapter.renderPage).toHaveBeenCalledTimes(1));
 await act(async()=>finish('blob:strict'));
 expect(view.getByText('blob:strict')).toBeInTheDocument();
 expect(URL.revokeObjectURL).not.toHaveBeenCalledWith('blob:strict');
});

it('shows backend errors and retries without borrowing the failed request',async()=>{
 Object.defineProperty(URL,'revokeObjectURL',{configurable:true,value:vi.fn()});
 const adapter={...createDemoAdapter(),renderPage:vi.fn().mockRejectedValueOnce(new Error('Unavailable')).mockResolvedValue('blob:retry')};
 const view=render(<Page adapter={adapter} index={0}/>);
 await waitFor(()=>expect(view.getByText('Unavailable')).toBeInTheDocument());
 view.rerender(<Page adapter={adapter} index={0} revision={1}/>);
 await waitFor(()=>expect(view.getByText('blob:retry')).toBeInTheDocument());
 expect(adapter.renderPage).toHaveBeenCalledTimes(2);
});
