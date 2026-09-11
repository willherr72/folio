import {cleanup,render,waitFor} from '@testing-library/react';
import {afterEach,expect,it,vi} from 'vitest';
import {PdfTextLayer,clearPageTextCache} from '../src/components/PdfTextLayer';
import type {FolioAdapter} from '../src/editor/adapter';
import type {PagePlan,PageText} from '../src/editor/types';
const source='measured-glyphs';
const plan:PagePlan={id:'page',sourceId:source,pageIndex:0,width:612,height:792,rotation:0,overlays:[]};
const text:PageText={intrinsicRotation:90,characters:[{text:'A',x:20,y:30,width:12,height:8},{text:' ',x:20,y:38,width:0,height:0},{text:'B',x:20,y:40,width:12,height:14}]};
const engine=()=>({kind:'native',openPdf:async()=>null,renderPage:async()=>'',exportPdf:async()=>null,closeDocument:async()=>{},engineStatus:async()=>'',getPageText:vi.fn(async()=>structuredClone(text))} satisfies FolioAdapter);
function surface(adapter:FolioAdapter){return <svg><PdfTextLayer adapter={adapter} page={plan} pageNumber={1} selectable/></svg>;}
afterEach(()=>{cleanup();clearPageTextCache([source]);vi.restoreAllMocks();});
it('reuses measured glyph transforms when a cached page remounts without reading layout again',async()=>{
 const reads=vi.spyOn(HTMLElement.prototype,'offsetWidth','get').mockReturnValue(10);
 const adapter=engine();const first=render(surface(adapter));
 await waitFor(()=>expect(first.container.querySelectorAll('[data-pdf-character]')).toHaveLength(3));
 const transforms=Array.from(first.container.querySelectorAll<HTMLElement>('[data-pdf-character]'),span=>span.style.transform);
 expect(transforms).toEqual(['rotate(90deg) scaleX(0.8)','rotate(90deg) scaleX(1)','rotate(90deg) scaleX(1.4)']);
 expect(reads).toHaveBeenCalledTimes(3);first.unmount();
 const second=render(surface(adapter));await waitFor(()=>expect(second.container.querySelectorAll('[data-pdf-character]')).toHaveLength(3));
 expect(Array.from(second.container.querySelectorAll<HTMLElement>('[data-pdf-character]'),span=>span.style.transform)).toEqual(transforms);
 expect(reads).toHaveBeenCalledTimes(3);expect(adapter.getPageText).toHaveBeenCalledTimes(1);
});
it('remeasures new extraction data after source eviction instead of retaining stale geometry',async()=>{
 const reads=vi.spyOn(HTMLElement.prototype,'offsetWidth','get').mockReturnValue(10);
 const adapter=engine();const first=render(surface(adapter));await waitFor(()=>expect(first.container.querySelectorAll('[data-pdf-character]')).toHaveLength(3));first.unmount();clearPageTextCache([source]);
 adapter.getPageText.mockResolvedValueOnce({...text,intrinsicRotation:0,characters:[{text:'C',x:70,y:80,width:20,height:12}]});
 const second=render(surface(adapter));await waitFor(()=>expect(second.container.querySelectorAll('[data-pdf-character]')).toHaveLength(1));
 expect(second.container.querySelector<HTMLElement>('[data-pdf-character]')!.style.transform).toBe('rotate(0deg) scaleX(2)');expect(reads).toHaveBeenCalledTimes(4);expect(adapter.getPageText).toHaveBeenCalledTimes(2);
});

it('applies cached geometry to simultaneous copies of the same source page',async()=>{
 const reads=vi.spyOn(HTMLElement.prototype,'offsetWidth','get').mockReturnValue(10);const adapter=engine();
 const view=render(<svg><PdfTextLayer adapter={adapter} page={plan} pageNumber={1} selectable/><PdfTextLayer adapter={adapter} page={{...plan,id:'duplicate',rotation:180}} pageNumber={2} selectable/></svg>);
 await waitFor(()=>expect(view.container.querySelectorAll('[data-pdf-character]')).toHaveLength(6));
 expect(Array.from(view.container.querySelectorAll<HTMLElement>('[data-pdf-character]'),span=>span.style.transform)).toEqual(Array(2).fill(['rotate(90deg) scaleX(0.8)','rotate(90deg) scaleX(1)','rotate(90deg) scaleX(1.4)']).flat());
 expect(reads).toHaveBeenCalledTimes(3);expect(adapter.getPageText).toHaveBeenCalledTimes(1);
});
