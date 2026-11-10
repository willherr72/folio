import {act,cleanup,fireEvent,render,waitFor} from '@testing-library/react';
import {afterEach,expect,it,vi} from 'vitest';
import {PdfTextLayer,clearPageTextCache} from '../src/components/PdfTextLayer';
import {createDemoAdapter} from '../src/editor/adapter';
import type {PagePlan,PageText} from '../src/editor/types';

const plan:PagePlan={id:'lazy-page',sourceId:'lazy-old',pageIndex:0,width:200,height:300,rotation:0,overlays:[]};
const text:PageText={intrinsicRotation:90,characters:[
 {text:'f',x:20,y:30,width:12,height:8},
 {text:'f',x:20,y:30,width:12,height:8},
 {text:'i',x:20,y:38,width:12,height:5},
 {text:' \r\n𝑴q\u0307\u0323',x:20,y:50,width:12,height:25},
]};
const logical=text.characters.map(c=>c.text).join('');
const engine=()=>({...createDemoAdapter(),getPageText:vi.fn(async()=>structuredClone(text))});
afterEach(()=>{cleanup();clearPageTextCache(['lazy-old','lazy-new']);window.getSelection()?.removeAllRanges();vi.restoreAllMocks();});

it('retains exact accessible reading text without glyph nodes or layout reads while nonselectable',async()=>{
 const reads=vi.spyOn(HTMLElement.prototype,'offsetWidth','get').mockReturnValue(10);
 const adapter=engine();const view=render(<svg><PdfTextLayer adapter={adapter} page={plan} pageNumber={1} selectable={false}/></svg>);
 await waitFor(()=>expect(view.getByRole('document',{name:'Page 1 text'}).textContent).toBe(logical));
 expect(view.container.querySelectorAll('[data-pdf-character]')).toHaveLength(0);
 expect(view.container.querySelectorAll('.pdf-logical-text')).toHaveLength(1);
 expect(view.container.querySelector('.pdf-logical-text')).not.toHaveAttribute('hidden');
 expect(reads).not.toHaveBeenCalled();expect(adapter.getPageText).toHaveBeenCalledTimes(1);
});

it('activates exact rotated selection and copy, then drops glyphs and reuses geometry on reactivation',async()=>{
 const reads=vi.spyOn(HTMLElement.prototype,'offsetWidth','get').mockReturnValue(10);
 const adapter=engine();const surface=(selectable:boolean)=><svg><PdfTextLayer adapter={adapter} page={plan} pageNumber={1} selectable={selectable}/></svg>;
 const view=render(surface(false));await waitFor(()=>expect(view.getByLabelText('Page 1 text').textContent).toBe(logical));
 view.rerender(surface(true));
 const glyphs=Array.from(view.container.querySelectorAll<HTMLElement>('[data-pdf-character]'));
 expect(glyphs.map(s=>s.textContent)).toEqual(['ff','i',' \r\n𝑴q\u0307\u0323']);
 expect(glyphs.map(s=>s.style.transform)).toEqual(['rotate(90deg) scaleX(0.8)','rotate(90deg) scaleX(0.5)','rotate(90deg) scaleX(2.5)']);
 expect(reads).toHaveBeenCalledTimes(3);expect(view.container.querySelector('.pdf-logical-text')).toBeNull();
 const range=document.createRange();range.selectNodeContents(view.getByLabelText('Page 1 text'));window.getSelection()!.addRange(range);
 const setData=vi.fn();fireEvent.copy(document,{clipboardData:{setData}});expect(setData).toHaveBeenCalledWith('text/plain',logical);
 view.rerender(surface(false));expect(view.container.querySelectorAll('[data-pdf-character]')).toHaveLength(0);expect(view.getByLabelText('Page 1 text').textContent).toBe(logical);
 view.rerender(surface(true));expect(Array.from(view.container.querySelectorAll<HTMLElement>('[data-pdf-character]'),s=>s.style.transform)).toEqual(glyphs.map(s=>s.style.transform));
 expect(reads).toHaveBeenCalledTimes(3);expect(adapter.getPageText).toHaveBeenCalledTimes(1);
});

it('compacts crop visibility runs without changing source text order or exposing crop-hidden text',async()=>{
 const cropped={...plan,crop:{x:10,y:10,width:50,height:50}};
 const adapter=engine();adapter.getPageText.mockResolvedValue({characters:[
  {text:'visible',x:20,y:20,width:20,height:10},
  {text:' hidden',x:150,y:20,width:20,height:10},
  {text:' still hidden',x:150,y:40,width:20,height:10},
  {text:' visible again',x:20,y:40,width:20,height:10},
 ]});
 const view=render(<svg><PdfTextLayer adapter={adapter} page={cropped} pageNumber={1} selectable={false}/></svg>);
 await waitFor(()=>expect(view.getByLabelText('Page 1 text').textContent).toBe('visible hidden still hidden visible again'));
 const runs=Array.from(view.container.querySelectorAll<HTMLElement>('.pdf-logical-text'));
 expect(runs.map(r=>[r.textContent,r.hidden])).toEqual([['visible',false],[' hidden still hidden',true],[' visible again',false]]);
 expect(view.container.querySelectorAll('[data-pdf-character]')).toHaveLength(0);
});

it('never leaves old-source semantic text or geometry visible while a new extraction is pending',async()=>{
 const adapter=engine();let resolve!:(value:PageText)=>void;
 adapter.getPageText.mockImplementation(async(_source?:string)=>structuredClone(text));
 const surface=(page:PagePlan)=><svg><PdfTextLayer adapter={adapter} page={page} pageNumber={1} selectable={false}/></svg>;
 const view=render(surface(plan));await waitFor(()=>expect(view.getByLabelText('Page 1 text').textContent).toBe(logical));
 adapter.getPageText.mockImplementationOnce(()=>new Promise(done=>{resolve=done;}));
 view.rerender(surface({...plan,sourceId:'lazy-new'}));expect(view.getByLabelText('Page 1 text').textContent).toBe('');
 await waitFor(()=>expect(resolve).toBeTypeOf('function'));
 await act(async()=>resolve({characters:[{text:'New',x:10,y:10,width:30,height:10}]}));
 expect(view.getByLabelText('Page 1 text').textContent).toBe('New');expect(view.container.querySelectorAll('[data-pdf-character]')).toHaveLength(0);
});

it('keeps no-copy extraction gated in inactive and active tools',async()=>{
 const adapter=engine();const locked={...plan,protection:{permissions:{copyContent:false}}} as PagePlan;
 const view=render(<svg><PdfTextLayer adapter={adapter} page={locked} pageNumber={1} selectable={false}/></svg>);
 await act(async()=>{});expect(adapter.getPageText).not.toHaveBeenCalled();expect(view.getByLabelText('Page 1 text').textContent).toBe('');
 view.rerender(<svg><PdfTextLayer adapter={adapter} page={locked} pageNumber={1} selectable/></svg>);
 expect(adapter.getPageText).not.toHaveBeenCalled();expect(view.container.querySelectorAll('[data-pdf-character]')).toHaveLength(0);
});
