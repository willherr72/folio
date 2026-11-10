import {act,cleanup,fireEvent,render,screen,waitFor} from '@testing-library/react';
import {afterEach,beforeEach,expect,it,vi} from 'vitest';
import {App} from '../src/App';
import {nativeAdapter} from '../src/editor/adapter';
import {pdfSaveApi,type PdfSaveProgress} from '../src/editor/pdf-save';
import {clearRenderCache} from '../src/components/PageView';
vi.mock('../src/editor/useWorkspaceRecovery',()=>({useWorkspaceRecovery:()=>({ready:true,checkpoint:vi.fn().mockResolvedValue(undefined)})}));
vi.mock('../src/components/DocumentViewport',()=>({DocumentViewport:()=>null}));
let resolve:(path:string|null)=>void,report:(value:PdfSaveProgress)=>void;
beforeEach(()=>{
 localStorage.clear();Object.defineProperty(window,'__TAURI_INTERNALS__',{value:{invoke:vi.fn().mockResolvedValue(0),transformCallback:()=>0},configurable:true});
 vi.spyOn(nativeAdapter,'openPdf').mockResolvedValue({id:'save-source',name:'Save.pdf',pages:[{width:300,height:400}]});
 vi.spyOn(nativeAdapter,'renderPage').mockResolvedValue('data:image/png;base64,AA');vi.spyOn(nativeAdapter,'closeDocument').mockResolvedValue();vi.spyOn(nativeAdapter,'exportPdf').mockResolvedValue('unused.pdf');
 vi.spyOn(pdfSaveApi,'register').mockResolvedValue('job');vi.spyOn(pdfSaveApi,'cancel').mockResolvedValue(true);
 vi.spyOn(pdfSaveApi,'start').mockImplementation((_id,_request,progress)=>{report=progress;return new Promise(done=>{resolve=done;});});
});
afterEach(()=>{cleanup();clearRenderCache(['save-source']);vi.restoreAllMocks();delete(window as any).__TAURI_INTERNALS__;});
async function open(){const view=render(<App initialDemo={false}/>);fireEvent.click(screen.getByRole('button',{name:'Open a PDF'}));await screen.findByRole('tab',{name:'Save.pdf'});fireEvent.click(screen.getByRole('button',{name:'Rotate'}));return view;}
async function save(){fireEvent.keyDown(window,{key:'s',ctrlKey:true});await waitFor(()=>expect(pdfSaveApi.start).toHaveBeenCalledOnce());}
it('reports real page progress and marks editable work saved only after success',async()=>{
 await open();await save();expect(screen.getByRole('dialog',{name:'Saving PDF'})).toBeInTheDocument();expect(screen.getByLabelText('Unsaved changes')).toBeInTheDocument();
 act(()=>report({jobId:'job',stage:'copying',completed:0,total:1}));expect(screen.getByRole('progressbar')).toHaveAttribute('value','0');expect(screen.getByText(/Copying pages: 0 of 1/)).toBeInTheDocument();
 fireEvent.keyDown(window,{key:'s',ctrlKey:true});expect(pdfSaveApi.start).toHaveBeenCalledOnce();
 await act(async()=>resolve('C:/Edited.pdf'));await screen.findByText('Saved a copy to C:/Edited.pdf');expect(screen.queryByRole('dialog')).toBeNull();expect(screen.queryByLabelText('Unsaved changes')).toBeNull();expect(nativeAdapter.exportPdf).not.toHaveBeenCalled();
});
it('keeps the dialog open while cancellation settles and preserves dirty work',async()=>{
 await open();await save();fireEvent.keyDown(screen.getByRole('dialog'),{key:'Escape'});await screen.findByText(/Cancelling…/);expect(screen.getByRole('button',{name:'Cancel save'})).toBeDisabled();
 expect(screen.getByLabelText('Unsaved changes')).toBeInTheDocument();await act(async()=>resolve(null));await screen.findByText('Save cancelled.');expect(screen.queryByRole('dialog')).toBeNull();expect(screen.getByLabelText('Unsaved changes')).toBeInTheDocument();expect(screen.getByRole('button',{name:'Undo'})).toBeEnabled();
});
it('allows cancellation retry after an IPC error and accepts a late successful save',async()=>{
 vi.mocked(pdfSaveApi.cancel).mockRejectedValueOnce(Error('Connection interrupted')).mockResolvedValue(false);
 await open();await save();fireEvent.click(screen.getByRole('button',{name:'Cancel save'}));await screen.findByText(/Could not request cancellation/);expect(screen.getByRole('button',{name:'Cancel save'})).toBeEnabled();
 fireEvent.click(screen.getByRole('button',{name:'Cancel save'}));await screen.findByText(/Finishing save/);expect(screen.getByRole('button',{name:'Cancel save'})).toBeDisabled();
 await act(async()=>resolve('C:/Late.pdf'));await screen.findByText('Saved a copy to C:/Late.pdf');expect(screen.queryByLabelText('Unsaved changes')).toBeNull();
});
it('retains editable dirty state after flattening',async()=>{
 await open();fireEvent.click(screen.getByRole('button',{name:'Save options'}));fireEvent.click(screen.getByRole('menuitem',{name:'Flatten text, images, and ink'}));await waitFor(()=>expect(pdfSaveApi.start).toHaveBeenCalledOnce());
 expect(vi.mocked(pdfSaveApi.start).mock.calls[0][1]).toMatchObject({flatten:true,pages:[{rotation:90}]});await act(async()=>resolve('C:/Flat.pdf'));await screen.findByText(/Exported flattened text, images, and ink/);expect(screen.getByLabelText('Unsaved changes')).toBeInTheDocument();
});
it('requests cancellation on unmount and ignores subsequent progress',async()=>{
 const view=await open();await save();view.unmount();await waitFor(()=>expect(pdfSaveApi.cancel).toHaveBeenCalledWith('job'));act(()=>report({jobId:'job',stage:'writing',completed:0,total:1}));await act(async()=>resolve(null));expect(screen.queryByRole('dialog')).toBeNull();
});
it('reports a failed save without clearing dirty work and allows a retry',async()=>{
 vi.mocked(pdfSaveApi.start).mockRejectedValueOnce(Error('Destination is locked'));
 await open();fireEvent.keyDown(window,{key:'s',ctrlKey:true});await screen.findByText(/Couldn’t export: Destination is locked/);expect(screen.queryByRole('dialog')).toBeNull();expect(screen.getByLabelText('Unsaved changes')).toBeInTheDocument();
 fireEvent.keyDown(window,{key:'s',ctrlKey:true});await waitFor(()=>expect(pdfSaveApi.start).toHaveBeenCalledTimes(2));await act(async()=>resolve('C:/Retry.pdf'));await screen.findByText('Saved a copy to C:/Retry.pdf');expect(screen.queryByLabelText('Unsaved changes')).toBeNull();
});
