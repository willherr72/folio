import {Profiler} from 'react';
import {act,cleanup,fireEvent,render,waitFor} from '@testing-library/react';
import {afterEach,expect,it,vi} from 'vitest';
import {PdfTextLayer,clearPageTextCache} from '../src/components/PdfTextLayer';
import {createDemoAdapter} from '../src/editor/adapter';
import type {PagePlan,PageText} from '../src/editor/types';
const page:PagePlan={id:'compact-page',sourceId:'compact-layer',pageIndex:0,width:200,height:300,rotation:0,overlays:[]};
const logical='ff𝑴q\u0307\u0323 \r\n';
const rich:PageText={characters:[{text:logical,x:20,y:30,width:40,height:10},{text:'hidden',x:180,y:30,width:10,height:10}]};
const engine=()=>({...createDemoAdapter(),getPageReadingText:vi.fn(async()=>logical+'hidden'),getPageText:vi.fn(async()=>structuredClone(rich))});
afterEach(()=>{cleanup();clearPageTextCache(['compact-layer','compact-next']);window.getSelection()?.removeAllRanges();vi.restoreAllMocks();});
it('uses only exact compact strings until selection needs geometry, retaining reading text while pending',async()=>{
 const a=engine();let done!:(text:PageText)=>void;a.getPageText.mockImplementationOnce(()=>new Promise(resolve=>done=resolve));
 const surface=(selectable:boolean)=><svg><PdfTextLayer adapter={a} page={page} pageNumber={1} selectable={selectable}/></svg>;
 const view=render(surface(false));await waitFor(()=>expect(view.getByLabelText('Page 1 text').textContent).toBe(logical+'hidden'));
 expect(a.getPageText).not.toHaveBeenCalled();expect(a.getPageReadingText).toHaveBeenCalledTimes(1);
 view.rerender(surface(true));expect(view.getByLabelText('Page 1 text').textContent).toBe(logical+'hidden');
 expect(view.getByLabelText('Page 1 text')).toHaveAttribute('data-selectable','false');
 const range=document.createRange();range.selectNodeContents(view.getByLabelText('Page 1 text'));window.getSelection()!.addRange(range);
 const setData=vi.fn();fireEvent.copy(document,{clipboardData:{setData}});expect(setData).not.toHaveBeenCalled();
 await waitFor(()=>expect(done).toBeTypeOf('function'));await act(async()=>done(rich));
 expect(view.getByLabelText('Page 1 text')).toHaveAttribute('data-selectable','true');
 expect(view.container.querySelectorAll('[data-pdf-character]')).toHaveLength(2);
 view.rerender(surface(false));expect(view.container.querySelectorAll('[data-pdf-character]')).toHaveLength(0);
 await waitFor(()=>expect(view.getByLabelText('Page 1 text').textContent).toBe(logical+'hidden'));
 expect(a.getPageReadingText).toHaveBeenCalledTimes(1);
});
it('hides full-source fallback immediately when a crop starts and uses rich crop visibility',async()=>{
 const a=engine();let done!:(text:PageText)=>void;a.getPageText.mockImplementationOnce(()=>new Promise(resolve=>done=resolve));
 const surface=(p:PagePlan)=><svg><PdfTextLayer adapter={a} page={p} pageNumber={1} selectable={false}/></svg>;
 const view=render(surface(page));await waitFor(()=>expect(view.getByLabelText('Page 1 text').textContent).toContain('hidden'));
 view.rerender(surface({...page,crop:{x:0,y:0,width:100,height:100}}));
 expect(view.getByLabelText('Page 1 text').textContent).toBe('');
 await waitFor(()=>expect(done).toBeTypeOf('function'));await act(async()=>done(rich));
 expect(Array.from(view.container.querySelectorAll<HTMLElement>('.pdf-logical-text'),s=>[s.textContent,s.hidden])).toEqual([[logical,false],['hidden',true]]);
 expect(a.getPageReadingText).toHaveBeenCalledTimes(1);
});
it('clears stale adapter/source data, ignores late responses and gates extraction on copy permission',async()=>{
 const a=engine();let done!:(text:string)=>void;a.getPageReadingText.mockImplementationOnce(()=>new Promise(resolve=>done=resolve));
 const surface=(p:PagePlan,adapter=a)=><svg><PdfTextLayer adapter={adapter} page={p} pageNumber={1} selectable={false}/></svg>;
 const view=render(surface(page));await waitFor(()=>expect(done).toBeTypeOf('function'));
 const b=engine();b.getPageReadingText.mockResolvedValue('new');view.rerender(surface({...page,sourceId:'compact-next'},b));
 expect(view.getByLabelText('Page 1 text').textContent).toBe('');await waitFor(()=>expect(view.getByLabelText('Page 1 text').textContent).toBe('new'));
 await act(async()=>done('old'));expect(view.getByLabelText('Page 1 text').textContent).toBe('new');
 const denied={...page,protection:{permissions:{copyContent:false}}} as PagePlan;
 view.rerender(surface(denied));expect(view.getByLabelText('Page 1 text').textContent).toBe('');
 expect(a.getPageReadingText).toHaveBeenCalledTimes(1);expect(a.getPageText).not.toHaveBeenCalled();
});

it('installs exact Copy handling in the same commit that exposes selection geometry',async()=>{
 const a=engine();const attempts:Array<{copied:string;prevented:boolean}>=[];
 const observe=()=>{
  const span=document.querySelector('[aria-label="Page 1 text"] [data-pdf-character]');
  if(!span?.firstChild||attempts.length)return;
  const range=document.createRange();range.selectNodeContents(span);
  const selection=window.getSelection()!;selection.removeAllRanges();selection.addRange(range);
  let copied='';const event=new Event('copy',{bubbles:true,cancelable:true});
  Object.defineProperty(event,'clipboardData',{value:{setData:(_type:string,value:string)=>{copied=value;}}});
  document.dispatchEvent(event);attempts.push({copied,prevented:event.defaultPrevented});
 };
 render(<Profiler id="selection-readiness" onRender={observe}><svg><PdfTextLayer adapter={a} page={page} pageNumber={1} selectable={true}/></svg></Profiler>);
 await waitFor(()=>expect(attempts).toEqual([{copied:logical,prevented:true}]));
});
