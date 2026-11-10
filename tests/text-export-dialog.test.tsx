import {cleanup,fireEvent,render,screen,waitFor} from '@testing-library/react';
import {afterEach,expect,it,vi} from 'vitest';
import {ExportTextDialog} from '../src/components/ExportTextDialog';
import {textExportApi} from '../src/editor/text-export';
import type {PagePlan} from '../src/editor/types';
const pages:PagePlan[]=[0,1].map(i=>({id:`p${i}`,sourceId:'s',pageIndex:i,width:300,height:400,rotation:0,overlays:[]}));
afterEach(()=>{cleanup();vi.restoreAllMocks();});
it('exports the selected page and displays empty-page and omitted-field results',async()=>{
 vi.spyOn(textExportApi,'register').mockResolvedValue('job');vi.spyOn(textExportApi,'cancel').mockResolvedValue(false);
 const start=vi.spyOn(textExportApi,'start').mockResolvedValue({path:'output.txt',pageCount:1,emptyPages:1,skippedFields:2,bytes:3});const complete=vi.fn();
 render(<ExportTextDialog pages={pages} currentPageId="p1" onClose={vi.fn()} onComplete={complete} onBusyChange={vi.fn()}/>);
 fireEvent.click(screen.getByRole('checkbox',{name:'Include form values'}));fireEvent.click(screen.getByRole('button',{name:'Choose file and export…'}));fireEvent.click(screen.getByRole('button',{name:'Choose file and export…'}));
 await waitFor(()=>expect(complete).toHaveBeenCalledOnce());expect(start).toHaveBeenCalledOnce();expect(start.mock.calls[0][1]).toEqual({pages:[pages[1]],includeFormValues:false});
 expect(screen.getByText(/1 page had no extractable text/)).toBeInTheDocument();expect(screen.getByText(/2 password or unsupported fields/)).toBeInTheDocument();
});
it('keeps the dialog and draft until cancellation settles, then permits retry',async()=>{
 vi.spyOn(textExportApi,'register').mockResolvedValue('job');const cancel=vi.spyOn(textExportApi,'cancel').mockResolvedValue(true);let finish!:(value:null)=>void;
 vi.spyOn(textExportApi,'start').mockImplementation(()=>new Promise(r=>{finish=r;}));const close=vi.fn(),busy=vi.fn();
 render(<ExportTextDialog pages={pages} currentPageId="p0" onClose={close} onComplete={vi.fn()} onBusyChange={busy}/>);
 fireEvent.change(screen.getByLabelText('Pages'),{target:{value:'range'}});fireEvent.change(screen.getByLabelText('Page range'),{target:{value:'2'}});fireEvent.click(screen.getByRole('button',{name:'Choose file and export…'}));
 await waitFor(()=>expect(finish).toBeTypeOf('function'));fireEvent.click(screen.getByRole('button',{name:'Cancel export'}));expect(close).not.toHaveBeenCalled();expect(cancel).toHaveBeenCalledWith('job');finish(null);
 await waitFor(()=>expect(screen.getByRole('button',{name:'Choose file and export…'})).toBeEnabled());expect(screen.getByLabelText('Page range')).toHaveValue('2');expect(busy).toHaveBeenLastCalledWith(false);
});
