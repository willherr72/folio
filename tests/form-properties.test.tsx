import {act,cleanup,fireEvent,render,waitFor} from '@testing-library/react';
import {afterEach,expect,it,vi} from 'vitest';
import {FormPropertiesDialog} from '../src/components/FormPropertiesDialog';
import {FormFillEditor} from '../src/components/FormFillEditor';
import {formPropertiesRequest,propertiesDraft,applyFormProperties,type FormProperties} from '../src/editor/form-properties';
import type {FolioAdapter} from '../src/editor/adapter';
import type {FormField,PagePlan,DocumentInfo} from '../src/editor/types';
const page:PagePlan={id:'p',sourceId:'source',pageIndex:0,width:300,height:400,rotation:0,overlays:[]};
const field:FormField={annotationIndex:2,name:'section.answer',label:'Answer',kind:'text',value:{kind:'text',text:'Keep'},options:[],selectedIndices:[],readOnly:true,required:false,multiline:false,password:false,multiSelect:false,editableChoice:false,reason:'This field is read-only.',bounds:{x:20,y:30,width:100,height:30},visible:true};
const settings:FormProperties={expectedKey:'key',name:'answer',fullName:'section.answer',label:'Answer',readOnly:true,required:false,linkedWidgets:2,renameReason:null};
const result:DocumentInfo={id:'candidate',name:'Updated',pages:[{width:300,height:400,overlays:[]}]};
function setup(){return {adapter:{inspectFormProperties:vi.fn().mockResolvedValue(settings),setFormProperties:vi.fn().mockResolvedValue(result),closeDocument:vi.fn().mockResolvedValue(undefined),listFormFields:vi.fn().mockResolvedValue([field])} as unknown as FolioAdapter,page,field,onApply:vi.fn().mockResolvedValue(undefined),onClose:vi.fn()};}
afterEach(()=>{cleanup();vi.restoreAllMocks();});
it('sends changed-only settings and leaves oversized original names and labels untouched',()=>{
 const original={...settings,name:'x'.repeat(2000),label:'z'.repeat(5000)},draft=propertiesDraft(original);draft.required=true;
 expect(formPropertiesRequest(original,draft,0,2)).toEqual({pageIndex:0,annotationIndex:2,expectedKey:'key',changes:{required:true}});
 expect(formPropertiesRequest(settings,propertiesDraft(settings),0,2)).toBeNull();
 for(const name of ['','bad.name','bad\u202ename','é'.repeat(600)])expect(()=>formPropertiesRequest(settings,{...propertiesDraft(settings),name},0,2)).toThrow();
});
it('allows unlocking readonly fields, explains linked scope and resets drafts',async()=>{
 const p=setup(),v=render(<FormPropertiesDialog {...p}/>);await v.findByDisplayValue('answer');
 expect(v.getByText(/2 linked/)).toBeVisible();expect(v.getByLabelText('Read only')).toBeChecked();
 fireEvent.click(v.getByLabelText('Read only'));fireEvent.click(v.getByLabelText('Required'));
 fireEvent.click(v.getByRole('button',{name:'Reset properties'}));expect(v.getByLabelText('Read only')).toBeChecked();expect(v.getByRole('button',{name:'Apply properties'})).toBeDisabled();
 fireEvent.change(v.getByLabelText('Tooltip'),{target:{value:'New label'}});fireEvent.click(v.getByRole('button',{name:'Apply properties'}));
 await waitFor(()=>expect(p.onApply).toHaveBeenCalledWith({pageIndex:0,annotationIndex:2,expectedKey:'key',changes:{label:'New label'}}));
});
it('disables only renaming when scripts are present and preserves the reason',async()=>{
 const p=setup();vi.mocked(p.adapter.inspectFormProperties!).mockResolvedValue({...settings,renameReason:'Scripts depend on field names.'});const v=render(<FormPropertiesDialog {...p}/>);
 await v.findByText('Scripts depend on field names.');expect(v.getByLabelText('Field name')).toBeDisabled();expect(v.getByLabelText('Tooltip')).toBeEnabled();
 fireEvent.click(v.getByLabelText('Required'));expect(v.getByRole('button',{name:'Apply properties'})).toBeEnabled();
});
it('retains drafts after native failures and prevents duplicate Apply',async()=>{
 const p=setup();let reject!:(e:Error)=>void;p.onApply.mockImplementation(()=>new Promise((_,r)=>{reject=r;}));const v=render(<FormPropertiesDialog {...p}/>);await v.findByDisplayValue('answer');
 fireEvent.change(v.getByLabelText('Field name'),{target:{value:'new'}});fireEvent.click(v.getByRole('button',{name:'Apply properties'}));fireEvent.click(v.getByRole('button',{name:'Apply properties'}));expect(p.onApply).toHaveBeenCalledOnce();
 await act(async()=>reject(new Error('Name already exists')));expect(v.getByRole('alert')).toHaveTextContent('Name already exists');expect(v.getByLabelText('Field name')).toHaveValue('new');expect(v.getByRole('button',{name:'Apply properties'})).toBeEnabled();
});
it('ignores stale inspections after changing source and never applies stale settings',async()=>{
 const p=setup();let done!:(p:FormProperties)=>void;vi.mocked(p.adapter.inspectFormProperties!).mockImplementationOnce(()=>new Promise(resolve=>{done=resolve;}));const v=render(<FormPropertiesDialog {...p}/>);
 v.rerender(<FormPropertiesDialog {...p} page={{...page,sourceId:'new-source'}}/>);await v.findByDisplayValue('answer');await act(async()=>done({...settings,name:'stale'}));expect(v.queryByDisplayValue('stale')).toBeNull();
});
it('keeps properties accessible for readonly fields but blocks unsaved value drafts',async()=>{
 const p=setup(),onProperties=vi.fn(),v=render(<FormFillEditor page={page} adapter={p.adapter} pageNumber={1} preview={false} onApply={vi.fn()} onClose={vi.fn()} onProperties={onProperties}/>);
 const button=await v.findByRole('button',{name:'Field properties…'});expect(button).toBeEnabled();fireEvent.click(button);expect(onProperties).toHaveBeenCalledWith(field);
 v.unmount();vi.mocked(p.adapter.listFormFields!).mockResolvedValue([{...field,readOnly:false,reason:null}]);
 const editable=render(<FormFillEditor page={page} adapter={p.adapter} pageNumber={1} preview={false} onApply={vi.fn()} onClose={vi.fn()} onProperties={onProperties}/>);
 await editable.findByRole('button',{name:'Field properties…'});fireEvent.change(editable.getByRole('textbox'),{target:{value:'Draft'}});expect(editable.getByRole('button',{name:'Field properties…'})).toBeDisabled();
});
it('releases a completed old submission after the dialog receives a new source',async()=>{
 const p=setup();let done!:()=>void;p.onApply.mockImplementation(()=>new Promise<void>(resolve=>{done=resolve;}));const v=render(<FormPropertiesDialog {...p}/>);await v.findByDisplayValue('answer');
 fireEvent.click(v.getByLabelText('Required'));fireEvent.click(v.getByRole('button',{name:'Apply properties'}));
 v.rerender(<FormPropertiesDialog {...p} page={{...page,sourceId:'new-source'}}/>);await v.findByDisplayValue('answer');await act(async()=>done());
 expect(v.getByLabelText('Required')).toBeEnabled();fireEvent.click(v.getByLabelText('Required'));expect(v.getByRole('button',{name:'Apply properties'})).toBeEnabled();
});
it('uses producing adapter cleanup for stale results and never closes borrowed sources',async()=>{
 for(const mode of ['stale','throw','borrowed','accepted']){
  const p=setup();const commit=vi.fn(()=>{if(mode==='throw')throw new Error('stale document');return mode==='accepted';});
  const request=formPropertiesRequest(settings,{...propertiesDraft(settings),required:true},0,2)!;
  const work=applyFormProperties(p.adapter,'source',request,id=>mode==='borrowed'&&id==='candidate',commit);
  if(mode==='accepted')await work;else await expect(work).rejects.toThrow();
  if(mode==='stale'||mode==='throw')expect(p.adapter.closeDocument).toHaveBeenCalledWith('candidate');else expect(p.adapter.closeDocument).not.toHaveBeenCalled();
  if(mode==='borrowed')expect(commit).not.toHaveBeenCalled();
 }
});
