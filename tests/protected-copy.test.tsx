import {act,cleanup,fireEvent,render,screen,waitFor} from '@testing-library/react';
import {afterEach,expect,it,vi} from 'vitest';
import {ProtectedCopyDialog} from '../src/components/ProtectedCopyDialog';
import {nativeAdapter} from '../src/editor/adapter';
import type {PagePlan} from '../src/editor/types';
import type {PdfSaveProgress} from '../src/editor/pdf-save';
const pages:PagePlan[]=[{id:'p',sourceId:'s',pageIndex:0,width:300,height:400,rotation:0,overlays:[]}];
afterEach(()=>{cleanup();vi.restoreAllMocks();});
function setup(input=pages){let finish!:(path:string|null)=>void,fail!:(error:Error)=>void,report!:(event:PdfSaveProgress)=>void;const cancel=vi.fn().mockResolvedValue(true);const start=vi.fn().mockImplementation((_p,_o,cb)=>{report=cb;return {result:new Promise<string|null>((r,j)=>{finish=r;fail=j;}),cancel};});const props={pages:input,bookmarks:[],adapter:{...nativeAdapter,startPdfSave:start},onClose:vi.fn(),onComplete:vi.fn(),onBusyChange:vi.fn()};const view=render(<ProtectedCopyDialog {...props}/>);return {props,start,cancel,view,finish:(path:string|null)=>finish(path),fail:(error:Error)=>fail(error),report:(event:PdfSaveProgress)=>report(event)};}
function passwords(){for(const label of ['Opening password','Confirm opening password'])fireEvent.change(screen.getByLabelText(label,{exact:true}),{target:{value:'reader-test'}});for(const label of ['Owner password','Confirm owner password'])fireEvent.change(screen.getByLabelText(label,{exact:true}),{target:{value:'owner-test'}});}
const submit=()=>screen.getByRole('button',{name:'Choose file and save…'});
it('requires matching credentials and sends the edited pages with explicit permissions',async()=>{
 const {start,finish,props}=setup();expect(submit()).toBeDisabled();passwords();fireEvent.change(screen.getByLabelText('Confirm owner password',{exact:true}),{target:{value:'wrong'}});expect(submit()).toBeDisabled();fireEvent.change(screen.getByLabelText('Confirm owner password',{exact:true}),{target:{value:'owner-test'}});
 fireEvent.change(screen.getByLabelText('Printing'),{target:{value:'low'}});fireEvent.click(screen.getByLabelText('Copy text and graphics'));fireEvent.click(submit());fireEvent.click(submit());expect(start).toHaveBeenCalledOnce();
 expect(start.mock.calls[0][0]).toEqual(pages);expect(start.mock.calls[0][1]).toMatchObject({bookmarks:[],security:{mode:'passwords',userPassword:'reader-test',ownerPassword:'owner-test',permissions:{print:true,printHighQuality:false,copyContent:false}}});
 for(const input of document.querySelectorAll('input[type=password]'))expect(input).toBeDisabled();await act(async()=>finish('protected.pdf'));expect(props.onComplete).toHaveBeenCalledWith('protected.pdf');expect(screen.getByLabelText('Owner password',{exact:true})).toHaveValue('');
});
it('reflects annotation permission granting form filling and supports removal',async()=>{
 const {start,finish}=setup();expect(screen.getByLabelText('Fill form fields')).toBeChecked();expect(screen.getByLabelText('Fill form fields')).toBeDisabled();fireEvent.click(screen.getByLabelText('Add comments and annotations'));fireEvent.click(screen.getByLabelText('Fill form fields'));expect(screen.getByLabelText('Fill form fields')).not.toBeChecked();
 fireEvent.change(screen.getByLabelText('Security'),{target:{value:'remove'}});expect(screen.queryByLabelText('Owner password',{exact:true})).toBeNull();fireEvent.click(submit());expect(start.mock.calls[0][1]).toEqual({bookmarks:[],security:{mode:'remove'}});await act(async()=>finish(null));
});
it('requires owner access for every protected source',()=>{
 const protection={documentId:'d',owner:false,revision:6,permissions:{print:true,printHighQuality:true,copyContent:true,editContent:true,annotate:true,fillForms:true,assemble:true}};
 const {start}=setup([...pages,{...pages[0],id:'restricted',sourceId:'restricted',protection}]);passwords();expect(submit()).toBeDisabled();expect(screen.getByRole('alert')).toHaveTextContent('Unlock editing');fireEvent.click(submit());expect(start).not.toHaveBeenCalled();
});
it('keeps cancellation pending and clears passwords when it settles',async()=>{
 const {cancel,finish,props}=setup();passwords();fireEvent.click(submit());fireEvent.keyDown(screen.getByRole('dialog'),{key:'Escape'});await waitFor(()=>expect(cancel).toHaveBeenCalledOnce());expect(props.onClose).not.toHaveBeenCalled();expect(screen.getByRole('button',{name:'Cancel save'})).toBeDisabled();await act(async()=>finish(null));expect(screen.getByRole('status')).toHaveTextContent('cancelled');expect(screen.getByLabelText('Opening password',{exact:true})).toHaveValue('');expect(props.onBusyChange).toHaveBeenLastCalledWith(false);
});
it('supports cancellation retry and a late successful publication',async()=>{
 const {cancel,finish,report,props}=setup();cancel.mockRejectedValueOnce(Error('offline')).mockResolvedValue(false);passwords();fireEvent.click(submit());fireEvent.click(screen.getByRole('button',{name:'Cancel save'}));await screen.findByText(/Could not request cancellation/);fireEvent.click(screen.getByRole('button',{name:'Cancel save'}));await screen.findByText('Finishing save…');expect(screen.getByRole('button',{name:'Cancel save'})).toBeDisabled();act(()=>report({jobId:'j',stage:'securing',completed:0,total:1}));await act(async()=>finish('late.pdf'));expect(props.onComplete).toHaveBeenCalledWith('late.pdf');
});
it('clears secrets after failure and permits retry without closing the dialog',async()=>{
 const {fail,finish,start,props}=setup();passwords();fireEvent.click(submit());await act(async()=>fail(Error('Destination is locked')));expect(screen.getByRole('alert')).toHaveTextContent('Destination is locked');expect(screen.getByLabelText('Owner password',{exact:true})).toHaveValue('');passwords();fireEvent.click(submit());expect(start).toHaveBeenCalledTimes(2);await act(async()=>finish('retry.pdf'));expect(props.onComplete).toHaveBeenCalledWith('retry.pdf');
});
it('requests cancellation on unmount and ignores late completion',async()=>{
 const {view,cancel,finish,props}=setup();passwords();fireEvent.click(submit());view.unmount();expect(cancel).toHaveBeenCalledOnce();await act(async()=>finish('late.pdf'));expect(props.onComplete).not.toHaveBeenCalled();
});
it('allows closing or retrying when publication fails after cancellation is too late',async()=>{
 const {cancel,fail,props}=setup();cancel.mockResolvedValue(false);passwords();fireEvent.click(submit());fireEvent.click(screen.getByRole('button',{name:'Cancel save'}));await screen.findByText('Finishing save…');await act(async()=>fail(Error('Publication failed')));expect(screen.getByRole('button',{name:'Cancel'})).toBeEnabled();fireEvent.click(screen.getByRole('button',{name:'Cancel'}));expect(props.onClose).toHaveBeenCalledOnce();
});
it('ignores a late cancellation response belonging to an earlier failed export',async()=>{
 const {cancel,fail,finish}=setup();let reply!:(accepted:boolean)=>void;cancel.mockImplementationOnce(()=>new Promise<boolean>(resolve=>{reply=resolve;}));passwords();fireEvent.click(submit());fireEvent.click(screen.getByRole('button',{name:'Cancel save'}));await act(async()=>fail(Error('Old export failed')));passwords();fireEvent.click(submit());await act(async()=>reply(false));expect(screen.queryByText('Finishing save…')).toBeNull();fireEvent.click(screen.getByRole('button',{name:'Cancel save'}));await waitFor(()=>expect(cancel).toHaveBeenCalledTimes(2));await act(async()=>finish(null));
});
