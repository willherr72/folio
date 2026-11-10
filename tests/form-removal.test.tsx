import {act,cleanup,fireEvent,render,waitFor} from '@testing-library/react';
import {afterEach,expect,it,vi} from 'vitest';
import {FormRemovalDialog} from '../src/components/FormRemovalDialog';
import {applyFormRemoval,type FormRemovalInfo} from '../src/editor/form-removal';
import type {FolioAdapter} from '../src/editor/adapter';
import type {DocumentInfo,FormField,PagePlan} from '../src/editor/types';
const page:PagePlan={id:'p',sourceId:'source',pageIndex:0,width:300,height:400,rotation:0,overlays:[]};
const field:FormField={annotationIndex:2,name:'section.answer',label:'Answer',kind:'radio',value:{kind:'toggle',checked:false},options:[],selectedIndices:[],readOnly:false,required:false,multiline:false,password:false,multiSelect:false,editableChoice:false,reason:null,bounds:{x:20,y:30,width:100,height:30},visible:true};
const info:FormRemovalInfo={expectedKey:'key',fullName:'section.answer',linkedWidgets:2,pageIndices:[0,3],removalReason:null};
const result:DocumentInfo={id:'candidate',name:'Removed',pages:[{width:300,height:400}]};
function setup(){return {adapter:{inspectFormRemoval:vi.fn().mockResolvedValue(info),removeFormField:vi.fn().mockResolvedValue(result),closeDocument:vi.fn().mockResolvedValue(undefined)} as unknown as FolioAdapter,page,field,onApply:vi.fn().mockResolvedValue(undefined),onClose:vi.fn()};}
afterEach(cleanup);
it('explains entire linked radio field scope and requires explicit removal',async()=>{
 const p=setup(),v=render(<FormRemovalDialog {...p}/>);expect(v.getByRole('button',{name:'Remove field'})).toBeDisabled();
 await v.findByText('section.answer');expect(v.getByText(/2 linked appearances/)).toBeVisible();expect(v.getByText(/Source pages: 1, 4/)).toBeVisible();expect(v.getByText(/entire radio group/)).toBeVisible();expect(p.onApply).not.toHaveBeenCalled();
 fireEvent.click(v.getByRole('button',{name:'Remove field'}));await waitFor(()=>expect(p.onApply).toHaveBeenCalledWith({pageIndex:0,annotationIndex:2,expectedKey:'key'}));
});
it('allows cancel without touching the source and shows blocked removal reasons',async()=>{
 const p=setup();vi.mocked(p.adapter.inspectFormRemoval!).mockResolvedValue({...info,removalReason:'A script refers to this field.'});const v=render(<FormRemovalDialog {...p}/>);
 await v.findByText('A script refers to this field.');expect(v.getByRole('button',{name:'Remove field'})).toBeDisabled();fireEvent.click(v.getByRole('button',{name:'Cancel'}));expect(p.onClose).toHaveBeenCalledOnce();expect(p.onApply).not.toHaveBeenCalled();
});
it('disables duplicate submissions and dismissal during removal, then permits retry after failure',async()=>{
 const p=setup();let reject!:(e:Error)=>void;p.onApply.mockImplementation(()=>new Promise((_,r)=>{reject=r;}));const v=render(<FormRemovalDialog {...p}/>);await v.findByText('section.answer');
 fireEvent.click(v.getByRole('button',{name:'Remove field'}));fireEvent.click(v.getByRole('button',{name:'Remove field'}));fireEvent.click(v.getByRole('button',{name:'Cancel'}));expect(p.onApply).toHaveBeenCalledOnce();expect(p.onClose).not.toHaveBeenCalled();
 await act(async()=>reject(new Error('Field changed')));expect(v.getByRole('alert')).toHaveTextContent('Field changed');expect(v.getByRole('button',{name:'Remove field'})).toBeEnabled();
});
it('ignores a stale inspection after source replacement and shows inspection failure',async()=>{
 const p=setup();let done!:(i:FormRemovalInfo)=>void;vi.mocked(p.adapter.inspectFormRemoval!).mockImplementationOnce(()=>new Promise(r=>{done=r;})).mockRejectedValueOnce(new Error('New source unavailable'));
 const v=render(<FormRemovalDialog {...p}/>);await act(async()=>{});v.rerender(<FormRemovalDialog {...p} page={{...page,sourceId:'new'}}/>);await v.findByText('New source unavailable');await act(async()=>done(info));expect(v.queryByText('section.answer')).toBeNull();expect(v.getByRole('button',{name:'Remove field'})).toBeDisabled();
});
it('closes only unadopted new sources using the producing adapter',async()=>{
 for(const mode of ['stale','throw','borrowed','original','accepted']) {
  const p=setup();if(mode==='original')vi.mocked(p.adapter.removeFormField!).mockResolvedValue({...result,id:'source'});
  const commit=vi.fn(()=>{if(mode==='throw')throw new Error('Changed');return mode==='accepted';});
  const work=applyFormRemoval(p.adapter,'source',{pageIndex:0,annotationIndex:2,expectedKey:'key'},id=>mode==='borrowed'&&id==='candidate',commit);
  if(mode==='accepted')await work;else await expect(work).rejects.toThrow();
  if(mode==='stale'||mode==='throw')expect(p.adapter.closeDocument).toHaveBeenCalledWith('candidate');else expect(p.adapter.closeDocument).not.toHaveBeenCalled();
  if(mode==='borrowed'||mode==='original')expect(commit).not.toHaveBeenCalled();
 }
});
