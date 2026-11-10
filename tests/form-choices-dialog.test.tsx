import {act,cleanup,fireEvent,render,waitFor} from '@testing-library/react';
import {afterEach,expect,it,vi} from 'vitest';
import {FormChoicesDialog} from '../src/components/FormChoicesDialog';
import type {FolioAdapter} from '../src/editor/adapter';
import type {FormChoices} from '../src/editor/form-choices';
import type {DocumentInfo,EditorDocument,FormField,PagePlan} from '../src/editor/types';

const page:PagePlan={id:'p',sourceId:'source',pageIndex:0,width:300,height:400,rotation:0,overlays:[]};
const document:EditorDocument={name:'Form.pdf',pages:[page],selectedPageId:'p',selectedOverlayId:null};
const field={annotationIndex:2,name:'Answer',label:'Answer',kind:'combo'} as FormField;
const settings:FormChoices={expectedKey:'key',name:'Answer',widgets:[{pageIndex:0,annotationIndex:2},{pageIndex:1,annotationIndex:0}],model:{options:[{exportValue:'a',display:'Alpha'},{exportValue:'b',display:'Beta'}],selection:{kind:'rows',value:[1]},defaults:['a'],combo:true,editable:false,multiple:false,sorted:false,topIndex:0}};
const result:DocumentInfo={id:'candidate',name:'Updated.pdf',pages:[{width:300,height:400,overlays:[]},{width:500,height:600,overlays:[]}]};
function setup(){return {adapter:{kind:'native',inspectFormChoices:vi.fn().mockResolvedValue(settings),setFormChoices:vi.fn().mockResolvedValue(result),renderPagePlan:vi.fn().mockResolvedValue('data:image/png;base64,AA=='),renderPage:vi.fn().mockResolvedValue('data:image/png;base64,AA=='),closeDocument:vi.fn().mockResolvedValue(undefined)} as unknown as FolioAdapter,document,page,field,isBorrowed:vi.fn((id:string)=>id==='source'),onApply:vi.fn().mockResolvedValue(undefined),onClose:vi.fn()};}
function deferred<T>(){let resolve!:(value:T)=>void;let reject!:(reason:unknown)=>void;const promise=new Promise<T>((a,b)=>{resolve=a;reject=b;});return {promise,resolve,reject};}
async function edit(v:ReturnType<typeof render>){const input=await v.findByLabelText('Display label');fireEvent.change(input,{target:{value:'Changed'}});}
async function preview(v:ReturnType<typeof render>){fireEvent.click(v.getByRole('button',{name:'Preview choices'}));await waitFor(()=>expect(v.getByRole('button',{name:'Apply choices'})).toBeEnabled());}
afterEach(()=>{cleanup();vi.restoreAllMocks();});

it.each(['Current answer','Default answer'])('requires a chosen replacement or explicit Clear after removing the %s row',async title=>{
 const p=setup(),v=render(<FormChoicesDialog {...p}/>);await v.findByLabelText('Display label');
 fireEvent.change(v.getByLabelText('Option to edit'),{target:{value:title==='Current answer'?'original-1':'original-0'}});
 fireEvent.click(v.getByRole('button',{name:'Remove option'}));
 fireEvent.change(v.getByLabelText(`${title} action`),{target:{value:'rows'}});
 expect(v.getByRole('button',{name:'Preview choices'})).toBeDisabled();expect(v.getByRole('alert')).toHaveTextContent(/choose.*row.*Clear/i);
 fireEvent.change(v.getByLabelText(`${title} rows`),{target:{value:title==='Current answer'?'original-0':'original-1'}});
 expect(v.getByRole('button',{name:'Preview choices'})).toBeEnabled();
 fireEvent.change(v.getByLabelText(`${title} action`),{target:{value:'clear'}});await preview(v);
 expect(vi.mocked(p.adapter.setFormChoices!).mock.calls[0][1].edit[title==='Current answer'?'selection':'defaults']).toEqual({kind:'clear'});
});
it.each(['keep','rows'])('explains duplicate default values for %s and updates matching labels after reorder/edit',async mode=>{
 const p=setup();vi.mocked(p.adapter.inspectFormChoices!).mockResolvedValue({...settings,model:{...settings.model,defaults:mode==='keep'?['shared']:null,options:[{display:'Retail',exportValue:'shared'},{display:'Wholesale',exportValue:'shared'}]}});
 const v=render(<FormChoicesDialog {...p}/>);await v.findByLabelText('Display label');
 if(mode==='rows'){
  fireEvent.change(v.getByLabelText('Default answer action'),{target:{value:'rows'}});
  fireEvent.change(v.getByLabelText('Default answer rows'),{target:{value:'original-1'}});
 }
 const note=()=>v.getByRole('note',{name:'Default answer ambiguity'});
 expect(note()).toHaveTextContent('Resetting this field may select a different matching row.');
 expect(note()).toHaveTextContent('1. Retail');expect(note()).toHaveTextContent('2. Wholesale');
 fireEvent.change(v.getByLabelText('Option to edit'),{target:{value:'original-1'}});fireEvent.click(v.getByRole('button',{name:'Move up'}));
 expect(note()).toHaveTextContent('1. Wholesale');expect(note()).toHaveTextContent('2. Retail');
 fireEvent.change(v.getByLabelText('Display label'),{target:{value:'Wholesale renamed'}});expect(note()).toHaveTextContent('1. Wholesale renamed');
 await preview(v);expect(vi.mocked(p.adapter.setFormChoices!).mock.calls[0][1].edit.defaults).toEqual(mode==='keep'?{kind:'keep'}:{kind:'rows',value:[0]});
 fireEvent.change(v.getByLabelText('Stored value'),{target:{value:'wholesale'}});
 expect(v.queryByRole('note',{name:'Default answer ambiguity'})).not.toBeInTheDocument();
});

it('previews linked pages and applies the exact candidate once',async()=>{
 const p=setup(),v=render(<FormChoicesDialog {...p}/>);await edit(v);expect(v.getByText(/2 linked appearances/)).toBeVisible();await preview(v);
 expect(p.adapter.setFormChoices).toHaveBeenCalledWith('source',expect.objectContaining({expectedKey:'key',pageIndex:0,annotationIndex:2}));
 fireEvent.change(v.getByLabelText('Preview page'),{target:{value:'1'}});
 await waitFor(()=>expect(p.adapter.renderPage).toHaveBeenCalledWith('candidate',1,900));
 await waitFor(()=>expect(v.getByRole('button',{name:'Apply choices'})).toBeEnabled());fireEvent.click(v.getByRole('button',{name:'Apply choices'}));
 await waitFor(()=>expect(p.onApply).toHaveBeenCalledWith(result));expect(p.adapter.setFormChoices).toHaveBeenCalledTimes(1);
 v.unmount();expect(p.adapter.closeDocument).not.toHaveBeenCalled();
});
it('invalidates on draft changes and resets transient state without losing row focus on preview',async()=>{
 const p=setup(),v=render(<FormChoicesDialog {...p}/>);await edit(v);const input=v.getByLabelText('Display label');input.focus();await preview(v);expect(input).toHaveFocus();
 fireEvent.change(input,{target:{value:'Next'}});expect(v.getByRole('button',{name:'Apply choices'})).toBeDisabled();await waitFor(()=>expect(vi.mocked(p.adapter.closeDocument).mock.calls).toEqual([['candidate']]));
 fireEvent.click(v.getByRole('button',{name:'Reset choices'}));expect(v.getByLabelText('Display label')).toHaveValue('Alpha');expect(v.getByRole('button',{name:'Preview choices'})).toBeDisabled();v.unmount();expect(p.adapter.closeDocument).toHaveBeenCalledTimes(1);
});
it('discards a late candidate after Cancel through its producing adapter',async()=>{
 const p=setup(),job=deferred<DocumentInfo>();vi.mocked(p.adapter.setFormChoices!).mockReturnValue(job.promise);const v=render(<FormChoicesDialog {...p}/>);await edit(v);
 fireEvent.click(v.getByRole('button',{name:'Preview choices'}));fireEvent.click(v.getByRole('button',{name:'Cancel'}));v.unmount();
 await act(async()=>job.resolve(result));await waitFor(()=>expect(vi.mocked(p.adapter.closeDocument).mock.calls).toEqual([['candidate']]));expect(p.onApply).not.toHaveBeenCalled();
});
it('rejects borrowed responses without closing another tab, even if its pages are malformed',async()=>{
 const p=setup();p.isBorrowed.mockImplementation(id=>['source','another-tab'].includes(id));vi.mocked(p.adapter.setFormChoices!).mockResolvedValue({id:'another-tab',pages:null} as unknown as DocumentInfo);
 const v=render(<FormChoicesDialog {...p}/>);await edit(v);fireEvent.click(v.getByRole('button',{name:'Preview choices'}));await v.findByText(/new source version/);
 expect(v.getByRole('button',{name:'Apply choices'})).toBeDisabled();v.unmount();expect(p.adapter.closeDocument).not.toHaveBeenCalled();
});
it('closes malformed owned responses without assuming a pages array',async()=>{
 const p=setup();vi.mocked(p.adapter.setFormChoices!).mockResolvedValue({id:'bad',pages:null} as unknown as DocumentInfo);
 const v=render(<FormChoicesDialog {...p}/>);await edit(v);fireEvent.click(v.getByRole('button',{name:'Preview choices'}));await waitFor(()=>expect(vi.mocked(p.adapter.closeDocument).mock.calls).toEqual([['bad']]));expect(v.getByRole('button',{name:'Apply choices'})).toBeDisabled();
});
it('ignores out-of-order previews and disposes each superseded source once',async()=>{
 const p=setup(),first=deferred<DocumentInfo>(),second=deferred<DocumentInfo>();vi.mocked(p.adapter.setFormChoices!).mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise);
 const v=render(<FormChoicesDialog {...p}/>);await edit(v);fireEvent.click(v.getByRole('button',{name:'Preview choices'}));fireEvent.change(v.getByLabelText('Display label'),{target:{value:'Second'}});fireEvent.click(v.getByRole('button',{name:'Preview choices'}));
 await act(async()=>second.resolve({...result,id:'second'}));await waitFor(()=>expect(v.getByRole('button',{name:'Apply choices'})).toBeEnabled());
 await act(async()=>first.resolve({...result,id:'first'}));await waitFor(()=>expect(p.adapter.closeDocument).toHaveBeenCalledWith('first'));
 fireEvent.click(v.getByRole('button',{name:'Apply choices'}));await waitFor(()=>expect(p.onApply).toHaveBeenCalledWith(expect.objectContaining({id:'second'})));v.unmount();expect(p.adapter.closeDocument).toHaveBeenCalledTimes(1);
});
it('releases adapter-replaced requests using the original adapter',async()=>{
 const p=setup(),next=setup(),job=deferred<DocumentInfo>();vi.mocked(p.adapter.setFormChoices!).mockReturnValue(job.promise);
 const v=render(<FormChoicesDialog {...p}/>);await edit(v);fireEvent.click(v.getByRole('button',{name:'Preview choices'}));v.rerender(<FormChoicesDialog {...next}/>);
 await act(async()=>job.resolve(result));await waitFor(()=>expect(vi.mocked(p.adapter.closeDocument).mock.calls).toEqual([['candidate']]));expect(next.adapter.closeDocument).not.toHaveBeenCalled();expect(v.getByRole('button',{name:'Apply choices'})).toBeDisabled();
});
it('cleans up failed adoption and requires a fresh preview',async()=>{
 const p=setup();p.onApply.mockRejectedValue(new Error('The document changed'));const v=render(<FormChoicesDialog {...p}/>);await edit(v);await preview(v);fireEvent.click(v.getByRole('button',{name:'Apply choices'}));
 await v.findByText('The document changed');await waitFor(()=>expect(vi.mocked(p.adapter.closeDocument).mock.calls).toEqual([['candidate']]));expect(v.getByRole('button',{name:'Apply choices'})).toBeDisabled();
});
it('retains a newly borrowed candidate instead of closing a source adopted elsewhere',async()=>{
 const p=setup(),v=render(<FormChoicesDialog {...p}/>);await edit(v);await preview(v);p.isBorrowed.mockImplementation(id=>id==='source'||id==='candidate');
 fireEvent.change(v.getByLabelText('Display label'),{target:{value:'Next'}});v.unmount();await act(async()=>{});expect(p.adapter.closeDocument).not.toHaveBeenCalled();
});
it('edits the last of 2000 options without truncation and keeps its identity and focus on reorder',async()=>{
 const p=setup();vi.mocked(p.adapter.inspectFormChoices!).mockResolvedValue({...settings,model:{...settings.model,selection:{kind:'none'},defaults:null,options:Array.from({length:2000},(_,i)=>({display:`Row ${i}`,exportValue:`value-${i}`}))}});
 const v=render(<FormChoicesDialog {...p}/>);const list=await v.findByLabelText('Option to edit');expect(list.querySelectorAll('option')).toHaveLength(2000);
 fireEvent.change(list,{target:{value:'original-1999'}});fireEvent.change(v.getByLabelText('Display label'),{target:{value:'Last edited'}});
 const up=v.getByRole('button',{name:'Move up'});up.focus();fireEvent.click(up);expect(up).toHaveFocus();expect(list).toHaveValue('original-1999');expect(v.getByLabelText('Display label')).toHaveValue('Last edited');
 expect(v.getByRole('dialog').querySelectorAll('input')).toHaveLength(2);await preview(v);
 const request=vi.mocked(p.adapter.setFormChoices!).mock.calls[0][1];expect(request.edit.options).toHaveLength(2000);expect(request.edit.options[1998]).toEqual({sourceIndex:1999,display:'Last edited',exportValue:'value-1999'});
});
it('will not apply an old image while the changed page is still rendering',async()=>{
 const p=setup(),job=deferred<string>();vi.mocked(p.adapter.renderPagePlan!).mockImplementation(plan=>plan.sourceId==='candidate'?job.promise:Promise.resolve('data:image/png;base64,AA'));
 const v=render(<FormChoicesDialog {...p}/>);await edit(v);fireEvent.click(v.getByRole('button',{name:'Preview choices'}));await v.findByText('Rendering edited page…');expect(v.getByRole('button',{name:'Apply choices'})).toBeDisabled();
 await act(async()=>job.resolve('data:image/png;base64,BB'));await waitFor(()=>expect(v.getByRole('button',{name:'Apply choices'})).toBeEnabled());
});
