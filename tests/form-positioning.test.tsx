import {act,cleanup,fireEvent,render,waitFor} from '@testing-library/react';
import {afterEach,expect,it,vi} from 'vitest';
import {FormPositionDialog} from '../src/components/FormPositionDialog';
import {FormFillEditor} from '../src/components/FormFillEditor';
import type {FolioAdapter} from '../src/editor/adapter';
import type {EditorDocument,FormField,PageContentReplacement,PagePlan} from '../src/editor/types';
const page:PagePlan={id:'p',sourceId:'source',pageIndex:0,width:300,height:400,rotation:90,overlays:[],crop:{x:10,y:20,width:250,height:350}};
const document={id:'d',name:'Document',pages:[page],selectedPageId:'p',selectedOverlayId:null} as EditorDocument;
const field:FormField={annotationIndex:2,name:'answer',label:'Answer',kind:'text',value:{kind:'text',text:'Keep'},options:[],selectedIndices:[],readOnly:false,required:true,multiline:false,password:false,multiSelect:false,editableChoice:false,reason:null,bounds:{x:20,y:30,width:100,height:30},visible:true};
const result:PageContentReplacement={document:{id:'candidate',name:'Positioned',pages:[{width:300,height:400,overlays:[]}]},pageIndices:[0]};
function setup(){return {adapter:{renderPagePlan:vi.fn().mockResolvedValue('data:image/png;base64,AA'),closeDocument:vi.fn().mockResolvedValue(undefined),positionFormField:vi.fn().mockResolvedValue(result),listFormFields:vi.fn().mockResolvedValue([field])} as unknown as FolioAdapter,document,page,field,sourceIds:['source','other-tab'],onApply:vi.fn().mockReturnValue(true),onCancel:vi.fn()};}
afterEach(()=>{cleanup();vi.restoreAllMocks();});
it('moves in displayed coordinates, previews before Apply and invalidates previous candidates',async()=>{
 const p=setup(),v=render(<FormPositionDialog {...p}/>);const left=v.getByLabelText('Left (pt)') as HTMLInputElement;
 expect(left.value).toBe('340');expect(v.getByRole('button',{name:'Preview position'})).toBeDisabled();
 fireEvent.keyDown(v.getByRole('button',{name:'Move field'}),{key:'ArrowLeft',shiftKey:true});expect(left.value).toBe('330');
 fireEvent.click(v.getByRole('button',{name:'Preview position'}));await waitFor(()=>expect(v.getByRole('button',{name:'Apply position'})).toBeEnabled());
 expect(p.adapter.positionFormField).toHaveBeenCalledWith('source',[0],0,{annotationIndex:2,expectedBounds:field.bounds,expectedValue:field.value,bounds:{x:20,y:40,width:100,height:30},reflowText:true});
 fireEvent.change(v.getByLabelText('Width (pt)'),{target:{value:'40'}});expect(v.getByRole('button',{name:'Apply position'})).toBeDisabled();await waitFor(()=>expect(p.adapter.closeDocument).toHaveBeenCalledWith('candidate'));
 fireEvent.click(v.getByRole('button',{name:'Preview position'}));await waitFor(()=>expect(v.getByRole('button',{name:'Apply position'})).toBeEnabled());
 fireEvent.click(v.getByRole('button',{name:'Apply position'}));v.unmount();expect(p.onApply).toHaveBeenCalledOnce();expect(p.adapter.closeDocument).toHaveBeenCalledTimes(1);
});
it('rejects empty/outside-crop geometry and resets invalid numeric text',()=>{
 const p=setup(),v=render(<FormPositionDialog {...p}/>);fireEvent.change(v.getByLabelText('Left (pt)'),{target:{value:''}});expect(v.getByRole('button',{name:'Preview position'})).toBeDisabled();
 fireEvent.click(v.getByRole('button',{name:'Reset position'}));expect((v.getByLabelText('Left (pt)') as HTMLInputElement).value).toBe('340');
 fireEvent.change(v.getByLabelText('Left (pt)'),{target:{value:'0'}});expect(v.getByRole('button',{name:'Preview position'})).toBeDisabled();
});
it('discards stale and cancelled results but never closes borrowed sources',async()=>{
 for(const borrowed of [false,true]){
  const p=setup();let done!:(r:PageContentReplacement)=>void;vi.mocked(p.adapter.positionFormField!).mockImplementation(()=>new Promise(resolve=>{done=resolve;}));
  const v=render(<FormPositionDialog {...p}/>);fireEvent.keyDown(v.getByRole('button',{name:'Move field'}),{key:'ArrowLeft'});fireEvent.click(v.getByRole('button',{name:'Preview position'}));
  fireEvent.click(v.getByRole('button',{name:'Cancel'}));v.unmount();await act(async()=>done(borrowed?{...result,document:{...result.document,id:'other-tab'}}:result));
  expect(p.onApply).not.toHaveBeenCalled();if(borrowed)expect(p.adapter.closeDocument).not.toHaveBeenCalled();else expect(p.adapter.closeDocument).toHaveBeenCalledWith('candidate');
 }
});
it('requires applying or resetting a value draft before positioning',async()=>{
 const p=setup(),onPosition=vi.fn(),v=render(<FormFillEditor page={page} adapter={p.adapter} pageNumber={1} preview={false} onApply={vi.fn()} onClose={vi.fn()} onPosition={onPosition}/>);
 const button=await v.findByRole('button',{name:'Position and size…'});expect(button).toBeEnabled();fireEvent.click(button);expect(onPosition).toHaveBeenCalledWith(field);
 fireEvent.change(v.getByRole('textbox'),{target:{value:'Draft'}});expect(button).toBeDisabled();
});

it('defaults text resize to reflow and discards a pending preview when the mode changes',async()=>{
 const p=setup();let done!:(r:PageContentReplacement)=>void;vi.mocked(p.adapter.positionFormField!).mockImplementationOnce(()=>new Promise(resolve=>{done=resolve;}));
 const v=render(<FormPositionDialog {...p}/>);const mode=v.getByLabelText('When resizing');expect(mode).toHaveValue('reflow');
 fireEvent.change(v.getByLabelText('Width (pt)'),{target:{value:'40'}});fireEvent.click(v.getByRole('button',{name:'Preview position'}));
 expect(p.adapter.positionFormField).toHaveBeenLastCalledWith('source',[0],0,expect.objectContaining({reflowText:true}));
 fireEvent.change(mode,{target:{value:'scale'}});await act(async()=>done(result));
 expect(v.getByRole('button',{name:'Apply position'})).toBeDisabled();expect(p.adapter.closeDocument).toHaveBeenCalledWith('candidate');
 fireEvent.click(v.getByRole('button',{name:'Preview position'}));await waitFor(()=>expect(v.getByRole('button',{name:'Apply position'})).toBeEnabled());
 expect(vi.mocked(p.adapter.positionFormField!).mock.calls.at(-1)![3]).not.toHaveProperty('reflowText');
 fireEvent.change(mode,{target:{value:'reflow'}});expect(v.getByRole('button',{name:'Apply position'})).toBeDisabled();
});
it('keeps non-text field positioning on saved-appearance scaling',()=>{
 const p=setup(),v=render(<FormPositionDialog {...p} field={{...field,kind:'checkbox',value:{kind:'toggle',checked:false}}}/>);
 expect(v.queryByLabelText('When resizing')).not.toBeInTheDocument();
});
