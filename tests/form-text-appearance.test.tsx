import {act,cleanup,fireEvent,render,waitFor} from '@testing-library/react';
import {afterEach,expect,it,vi} from 'vitest';
import {FormTextAppearanceDialog} from '../src/components/FormTextAppearanceDialog';
import {FormFillEditor} from '../src/components/FormFillEditor';
import type {FolioAdapter} from '../src/editor/adapter';
import type {EditorDocument,FormField,PageContentReplacement,PagePlan} from '../src/editor/types';
const page:PagePlan={id:'p',sourceId:'source',pageIndex:0,width:300,height:400,rotation:90,overlays:[]};
const document={id:'d',name:'Document',pages:[page],selectedPageId:'p',selectedOverlayId:null} as EditorDocument;
const field:FormField={annotationIndex:2,name:'answer',label:'Answer',kind:'text',value:{kind:'text',text:'Keep'},options:[],selectedIndices:[],readOnly:false,required:true,multiline:false,password:false,multiSelect:false,editableChoice:false,reason:null,bounds:{x:20,y:30,width:100,height:30},visible:true};
const style={styleKey:'source-style',fontLabel:'Original serif',fontSize:20.4,color:'#ff0000',colorSpace:'cmyk',alignment:'left' as const};
const result:PageContentReplacement={document:{id:'candidate',name:'Styled',pages:[{width:300,height:400,overlays:[]}]},pageIndices:[0]};
function setup(){return {adapter:{renderPagePlan:vi.fn().mockResolvedValue('data:image/png;base64,AA'),closeDocument:vi.fn().mockResolvedValue(undefined),inspectFormTextAppearance:vi.fn().mockResolvedValue(style),setFormTextAppearance:vi.fn().mockResolvedValue(result),listFormFields:vi.fn().mockResolvedValue([field])} as unknown as FolioAdapter,document,page,field,sourceIds:['source','other-tab'],onApply:vi.fn().mockReturnValue(true),onCancel:vi.fn()};}
afterEach(()=>{cleanup();vi.restoreAllMocks();});
it('keeps physical size and original CMYK unchanged for alignment-only edits, transfers ownership on Apply',async()=>{
 const p=setup(),v=render(<FormTextAppearanceDialog {...p}/>);
 await v.findByText('Original serif'); expect(v.getByLabelText('Font size (pt)')).toHaveValue(20.4);
 expect(v.getByRole('button',{name:'Preview appearance'})).toBeDisabled();
 fireEvent.change(v.getByLabelText('Alignment'),{target:{value:'center'}});
 fireEvent.click(v.getByRole('button',{name:'Preview appearance'}));
 await waitFor(()=>expect(v.getByRole('button',{name:'Apply appearance'})).toBeEnabled());
 expect(p.adapter.setFormTextAppearance).toHaveBeenCalledWith('source',[0],0,{annotationIndex:2,expectedBounds:field.bounds,expectedValue:field.value,expectedStyleKey:'source-style',changes:{alignment:'center'}});
 fireEvent.click(v.getByRole('button',{name:'Apply appearance'}));fireEvent.click(v.getByRole('button',{name:'Apply appearance'}));v.unmount();
 expect(p.onApply).toHaveBeenCalledOnce();expect(p.adapter.closeDocument).not.toHaveBeenCalled();
});
it('validates blank and out-of-range size, resets strings, and sends explicit Auto fit and RGB',async()=>{
 const p=setup(),v=render(<FormTextAppearanceDialog {...p}/>);await v.findByText('Original serif');
 for(const size of ['', '-1','513']){fireEvent.change(v.getByLabelText('Font size (pt)'),{target:{value:size}});expect(v.getByRole('button',{name:'Preview appearance'})).toBeDisabled();}
 fireEvent.click(v.getByRole('button',{name:'Reset appearance'}));expect(v.getByLabelText('Font size (pt)')).toHaveValue(20.4);
 fireEvent.click(v.getByLabelText('Auto fit'));fireEvent.change(v.getByLabelText('Text color'),{target:{value:'custom'}});
 fireEvent.change(v.getByLabelText('RGB color'),{target:{value:'wrong'}});expect(v.getByRole('button',{name:'Preview appearance'})).toBeDisabled();
 fireEvent.change(v.getByLabelText('RGB color'),{target:{value:'#123456'}});fireEvent.click(v.getByRole('button',{name:'Preview appearance'}));
 await waitFor(()=>expect(v.getByRole('button',{name:'Apply appearance'})).toBeEnabled());
 expect(p.adapter.setFormTextAppearance).toHaveBeenLastCalledWith('source',[0],0,expect.objectContaining({changes:{fontSize:0,color:'#123456'}}));
 fireEvent.click(v.getByRole('button',{name:'Reset appearance'}));expect(v.getByRole('button',{name:'Apply appearance'})).toBeDisabled();await waitFor(()=>expect(p.adapter.closeDocument).toHaveBeenCalledWith('candidate'));
});
it('discards stale and closed preview results without adopting another tab source',async()=>{
 for(const mode of ['edit','close','borrowed']){
  const p=setup();let done!:(r:PageContentReplacement)=>void;vi.mocked(p.adapter.setFormTextAppearance!).mockImplementation(()=>new Promise(resolve=>{done=resolve;}));
  const v=render(<FormTextAppearanceDialog {...p}/>);await v.findByText('Original serif');fireEvent.change(v.getByLabelText('Alignment'),{target:{value:'right'}});fireEvent.click(v.getByRole('button',{name:'Preview appearance'}));
  if(mode==='edit')fireEvent.change(v.getByLabelText('Alignment'),{target:{value:'center'}});
  if(mode==='close'){fireEvent.click(v.getByRole('button',{name:'Cancel'}));v.unmount();}
  await act(async()=>done(mode==='borrowed'?{...result,document:{...result.document,id:'other-tab'}}:result));
  expect(p.onApply).not.toHaveBeenCalled();
  if(mode==='borrowed'){expect(p.adapter.closeDocument).not.toHaveBeenCalled();expect(v.getByRole('alert')).toHaveTextContent('new source');}
  else expect(p.adapter.closeDocument).toHaveBeenCalledWith('candidate');
  v.unmount();
 }
});
it('shows unsupported native inspection and never enables Preview',async()=>{
 const p=setup();vi.mocked(p.adapter.inspectFormTextAppearance!).mockRejectedValue(new Error('Saved artwork uses a different font.'));
 const v=render(<FormTextAppearanceDialog {...p}/>);expect(await v.findByRole('alert')).toHaveTextContent('different font');expect(v.getByRole('button',{name:'Preview appearance'})).toBeDisabled();
});
it('waits for the candidate raster and rejects stale rendering',async()=>{
 const p=setup();let done!:(v:string)=>void;vi.mocked(p.adapter.renderPagePlan!).mockImplementation((plan)=>plan.sourceId==='candidate'?new Promise(resolve=>{done=resolve;}):Promise.resolve('data:image/png;base64,AA'));
 const v=render(<FormTextAppearanceDialog {...p}/>);await v.findByText('Original serif');fireEvent.change(v.getByLabelText('Alignment'),{target:{value:'right'}});fireEvent.click(v.getByRole('button',{name:'Preview appearance'}));
 await waitFor(()=>expect(done).toBeDefined());expect(v.getByRole('button',{name:'Apply appearance'})).toBeDisabled();
 fireEvent.change(v.getByLabelText('Alignment'),{target:{value:'center'}});await act(async()=>done('data:image/png;base64,BB'));
 expect(v.getByRole('button',{name:'Apply appearance'})).toBeDisabled();await waitFor(()=>expect(p.adapter.closeDocument).toHaveBeenCalledWith('candidate'));
});
it('requires applying or resetting value drafts before styling',async()=>{
 const p=setup(),onTextAppearance=vi.fn(),v=render(<FormFillEditor page={page} adapter={p.adapter} pageNumber={1} preview={false} onApply={vi.fn()} onClose={vi.fn()} onTextAppearance={onTextAppearance}/>);
 const button=await v.findByRole('button',{name:'Text appearance…'});expect(button).toBeEnabled();fireEvent.click(button);expect(onTextAppearance).toHaveBeenCalledWith(field);
 fireEvent.change(v.getByRole('textbox'),{target:{value:'Draft'}});expect(button).toBeDisabled();
});
it('invalidates pending candidate ownership when the source props change',async()=>{
 const p=setup();let done!:(r:PageContentReplacement)=>void;vi.mocked(p.adapter.setFormTextAppearance!).mockImplementation(()=>new Promise(resolve=>{done=resolve;}));
 const v=render(<FormTextAppearanceDialog {...p}/>);await v.findByText('Original serif');fireEvent.change(v.getByLabelText('Alignment'),{target:{value:'right'}});fireEvent.click(v.getByRole('button',{name:'Preview appearance'}));
 const nextPage={...page,sourceId:'other-tab'};
 v.rerender(<FormTextAppearanceDialog {...p} page={nextPage} document={{...document,pages:[nextPage]}}/>);
 await waitFor(()=>expect(p.adapter.inspectFormTextAppearance).toHaveBeenLastCalledWith('other-tab',0,2));
 await act(async()=>done(result));
 await waitFor(()=>expect(p.adapter.closeDocument).toHaveBeenCalledWith('candidate'));
 expect(v.getByRole('button',{name:'Apply appearance'})).toBeDisabled();
});
it('retains candidate ownership after a declined Apply and closes it on cancellation',async()=>{
 const p=setup();p.onApply.mockReturnValue(false);const v=render(<FormTextAppearanceDialog {...p}/>);await v.findByText('Original serif');
 fireEvent.change(v.getByLabelText('Alignment'),{target:{value:'center'}});fireEvent.click(v.getByRole('button',{name:'Preview appearance'}));await waitFor(()=>expect(v.getByRole('button',{name:'Apply appearance'})).toBeEnabled());
 fireEvent.click(v.getByRole('button',{name:'Apply appearance'}));expect(v.getByRole('alert')).toHaveTextContent('document changed');expect(p.adapter.closeDocument).not.toHaveBeenCalled();
 fireEvent.click(v.getByRole('button',{name:'Cancel'}));await waitFor(()=>expect(p.adapter.closeDocument).toHaveBeenCalledWith('candidate'));
});
it('does not apply a candidate whose raster failed',async()=>{
 const p=setup();vi.mocked(p.adapter.renderPagePlan!).mockImplementation(plan=>plan.sourceId==='candidate'?Promise.reject(new Error('Raster failed')):Promise.resolve('data:image/png;base64,AA'));
 const v=render(<FormTextAppearanceDialog {...p}/>);await v.findByText('Original serif');fireEvent.change(v.getByLabelText('Alignment'),{target:{value:'center'}});fireEvent.click(v.getByRole('button',{name:'Preview appearance'}));
 expect(await v.findByRole('alert')).toHaveTextContent('Raster failed');expect(v.getByRole('button',{name:'Apply appearance'})).toBeDisabled();
});
it('disposes adopted candidates through their producing adapters across identity changes',async()=>{
 const a=setup(),b=setup();
 const v=render(<FormTextAppearanceDialog {...a}/>);await v.findByText('Original serif');fireEvent.change(v.getByLabelText('Alignment'),{target:{value:'right'}});fireEvent.click(v.getByRole('button',{name:'Preview appearance'}));await waitFor(()=>expect(v.getByRole('button',{name:'Apply appearance'})).toBeEnabled());
 v.rerender(<FormTextAppearanceDialog {...b}/>);
 await waitFor(()=>expect(a.adapter.closeDocument).toHaveBeenCalledWith('candidate'));
 expect(b.adapter.closeDocument).not.toHaveBeenCalled();
 await v.findByText('Original serif');fireEvent.change(v.getByLabelText('Alignment'),{target:{value:'right'}});fireEvent.click(v.getByRole('button',{name:'Preview appearance'}));await waitFor(()=>expect(v.getByRole('button',{name:'Apply appearance'})).toBeEnabled());v.unmount();
 await waitFor(()=>expect(b.adapter.closeDocument).toHaveBeenCalledWith('candidate'));expect(a.adapter.closeDocument).toHaveBeenCalledTimes(1);
});
