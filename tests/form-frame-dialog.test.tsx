import {act,cleanup,fireEvent,render,waitFor} from '@testing-library/react';
import {afterEach,expect,it,vi} from 'vitest';
import {FormFrameDialog} from '../src/components/FormFrameDialog';
import {FormFillEditor} from '../src/components/FormFillEditor';
import type {FolioAdapter} from '../src/editor/adapter';
import type {EditorDocument,FormField,PageContentReplacement,PagePlan} from '../src/editor/types';
const page:PagePlan={id:'p',sourceId:'source',pageIndex:0,width:300,height:400,rotation:90,overlays:[]};
const document={id:'d',name:'Document',pages:[page],selectedPageId:'p',selectedOverlayId:null} as EditorDocument;
const field:FormField={annotationIndex:2,name:'answer',label:'Answer',kind:'text',value:{kind:'text',text:'Keep'},options:[],selectedIndices:[],readOnly:false,required:true,multiline:false,password:false,multiSelect:false,editableChoice:false,reason:null,bounds:{x:20,y:30,width:100,height:30},visible:true};
const style={styleKey:'a'.repeat(64),width:0.5,style:'solid',border:{colorSpace:'cmyk',components:[1,0,0,0],preview:'#00ffff'},background:{colorSpace:'transparent',components:[],preview:null},supportedStyles:['solid','dashed','underline','beveled','inset']};
const result:PageContentReplacement={document:{id:'candidate',name:'Framed',pages:[{width:300,height:400,overlays:[]}]},pageIndices:[0]};
function setup(){return {adapter:{renderPagePlan:vi.fn().mockResolvedValue('data:image/png;base64,AA'),closeDocument:vi.fn().mockResolvedValue(undefined),inspectFormFrame:vi.fn().mockResolvedValue(style),setFormFrame:vi.fn().mockResolvedValue(result),listFormFields:vi.fn().mockResolvedValue([field])} as unknown as FolioAdapter,document,page,field,sourceIds:['source','other-tab'],onApply:vi.fn().mockReturnValue(true),onCancel:vi.fn()};}
afterEach(()=>{cleanup();vi.restoreAllMocks();});
async function edit(v:ReturnType<typeof render>){await v.findByLabelText('Border width (pt)');fireEvent.change(v.getByLabelText('Border width (pt)'),{target:{value:'2.25'}});}
async function prepare(v:ReturnType<typeof render>){await edit(v);fireEvent.click(v.getByRole('button',{name:'Preview appearance'}));await waitFor(()=>expect(v.getByRole('button',{name:'Apply appearance'})).toBeEnabled());}
it('previews only changed width and transfers the candidate exactly once',async()=>{
 const p=setup(),v=render(<FormFrameDialog {...p}/>);await v.findByLabelText('Border width (pt)');expect(v.getByRole('button',{name:'Preview appearance'})).toBeDisabled();await prepare(v);
 expect(p.adapter.setFormFrame).toHaveBeenCalledWith('source',[0],0,{annotationIndex:2,expectedBounds:field.bounds,expectedValue:field.value,expectedStyleKey:style.styleKey,changes:{width:2.25}});
 fireEvent.click(v.getByRole('button',{name:'Apply appearance'}));fireEvent.click(v.getByRole('button',{name:'Apply appearance'}));v.unmount();expect(p.onApply).toHaveBeenCalledOnce();expect(p.adapter.closeDocument).not.toHaveBeenCalled();
});
it('resets invalid width strings and supports separate transparent and RGB colors',async()=>{
 const p=setup(),v=render(<FormFrameDialog {...p}/>);await edit(v);
 fireEvent.change(v.getByLabelText('Border width (pt)'),{target:{value:''}});expect(v.getByRole('button',{name:'Preview appearance'})).toBeDisabled();
 fireEvent.click(v.getByRole('button',{name:'Reset appearance'}));expect(v.getByLabelText('Border width (pt)')).toHaveValue(0.5);
 fireEvent.change(v.getByLabelText('Border color'),{target:{value:'transparent'}});fireEvent.change(v.getByLabelText('Background'),{target:{value:'rgb'}});
 fireEvent.change(v.getByLabelText('Background RGB color'),{target:{value:'#123456'}});fireEvent.click(v.getByRole('button',{name:'Preview appearance'}));await waitFor(()=>expect(v.getByRole('button',{name:'Apply appearance'})).toBeEnabled());
 expect(p.adapter.setFormFrame).toHaveBeenLastCalledWith('source',[0],0,expect.objectContaining({changes:{border:{mode:'transparent'},background:{mode:'rgb',color:'#123456'}}}));
 fireEvent.click(v.getByRole('button',{name:'Reset appearance'}));expect(v.getByRole('button',{name:'Apply appearance'})).toBeDisabled();await waitFor(()=>expect(p.adapter.closeDocument).toHaveBeenCalledWith('candidate'));
});
it.each(['edit','cancel','unmount','source','adapter','borrowed'] as const)('discards pending candidates on %s without closing borrowed sources',async mode=>{
 const p=setup();let done!:(r:PageContentReplacement)=>void;vi.mocked(p.adapter.setFormFrame!).mockImplementation(()=>new Promise(resolve=>{done=resolve;}));const v=render(<FormFrameDialog {...p}/>);await edit(v);fireEvent.click(v.getByRole('button',{name:'Preview appearance'}));
 const other=setup();
 if(mode==='edit')fireEvent.change(v.getByLabelText('Border width (pt)'),{target:{value:'3'}});
 if(mode==='cancel')fireEvent.click(v.getByRole('button',{name:'Cancel'}));
 if(mode==='unmount')v.unmount();
 if(mode==='source'){const next={...page,sourceId:'other-tab'};v.rerender(<FormFrameDialog {...p} page={next} document={{...document,pages:[next]}}/>);}
 if(mode==='adapter')v.rerender(<FormFrameDialog {...p} adapter={other.adapter}/>);
 await act(async()=>done(mode==='borrowed'?{...result,document:{...result.document,id:'other-tab'}}:result));
 expect(p.onApply).not.toHaveBeenCalled();expect(other.adapter.closeDocument).not.toHaveBeenCalled();
 if(mode==='borrowed'){expect(p.adapter.closeDocument).not.toHaveBeenCalled();expect(v.getByRole('alert')).toHaveTextContent('new source');}
 else {await waitFor(()=>expect(p.adapter.closeDocument).toHaveBeenCalledWith('candidate'));expect(p.adapter.closeDocument).toHaveBeenCalledTimes(1);}
 v.unmount();
});
it('releases adopted candidates through their producing adapter and retains declined Apply ownership',async()=>{
 const p=setup(),other=setup(),v=render(<FormFrameDialog {...p}/>);await prepare(v);v.rerender(<FormFrameDialog {...other}/>);await waitFor(()=>expect(p.adapter.closeDocument).toHaveBeenCalledWith('candidate'));expect(p.adapter.closeDocument).toHaveBeenCalledTimes(1);
 other.onApply.mockReturnValue(false);await prepare(v);fireEvent.click(v.getByRole('button',{name:'Apply appearance'}));expect(v.getByRole('alert')).toHaveTextContent('document changed');expect(other.adapter.closeDocument).not.toHaveBeenCalled();v.unmount();await waitFor(()=>expect(other.adapter.closeDocument).toHaveBeenCalledWith('candidate'));expect(other.adapter.closeDocument).toHaveBeenCalledTimes(1);
});
it('waits for the current raster and never applies a failed raster',async()=>{
 const p=setup();let reject!:(e:Error)=>void;vi.mocked(p.adapter.renderPagePlan!).mockImplementation(plan=>plan.sourceId==='candidate'?new Promise((_,r)=>{reject=r;}):Promise.resolve('data:image/png;base64,AA'));
 const v=render(<FormFrameDialog {...p}/>);await edit(v);fireEvent.click(v.getByRole('button',{name:'Preview appearance'}));await waitFor(()=>expect(reject).toBeDefined());expect(v.getByRole('button',{name:'Apply appearance'})).toBeDisabled();await act(async()=>reject(new Error('Raster failed')));expect(v.getByRole('alert')).toHaveTextContent('Raster failed');expect(v.getByRole('button',{name:'Apply appearance'})).toBeDisabled();v.unmount();await waitFor(()=>expect(p.adapter.closeDocument).toHaveBeenCalledWith('candidate'));
});
it('offers only supported radio styles and refuses malformed inspection',async()=>{
 const p=setup();vi.mocked(p.adapter.inspectFormFrame!).mockResolvedValue({...style,style:null,supportedStyles:['solid','dashed']} as any);const v=render(<FormFrameDialog {...p}/>);await edit(v);expect(v.getByLabelText('Border style')).toHaveValue('original');expect(v.queryByRole('option',{name:'Underline'})).not.toBeInTheDocument();v.unmount();
 vi.mocked(p.adapter.inspectFormFrame!).mockResolvedValue({...style,width:NaN} as any);const w=render(<FormFrameDialog {...p}/>);expect(await w.findByRole('alert')).toHaveTextContent('invalid border');expect(w.getByRole('button',{name:'Preview appearance'})).toBeDisabled();
});
it('requires answer drafts to be applied or reset before changing the frame',async()=>{
 const p=setup(),onFrameAppearance=vi.fn(),v=render(<FormFillEditor page={page} adapter={p.adapter} pageNumber={1} preview={false} onApply={vi.fn()} onClose={vi.fn()} onFrameAppearance={onFrameAppearance}/>);
 const button=await v.findByRole('button',{name:'Border and background…'});fireEvent.click(button);expect(onFrameAppearance).toHaveBeenCalledWith(field);fireEvent.change(v.getByRole('textbox'),{target:{value:'Draft'}});expect(button).toBeDisabled();
});
it('does not adopt or close a source newly borrowed while Preview is pending',async()=>{
 const p=setup();let done!:(r:PageContentReplacement)=>void;let borrowed=false;
 vi.mocked(p.adapter.setFormFrame!).mockImplementation(()=>new Promise(resolve=>{done=resolve;}));
 const v=render(<FormFrameDialog {...p} isBorrowed={id=>borrowed&&id==='candidate'}/>);await edit(v);fireEvent.click(v.getByRole('button',{name:'Preview appearance'}));borrowed=true;
 await act(async()=>done(result));expect(v.getByRole('alert')).toHaveTextContent('new source');expect(v.getByRole('button',{name:'Apply appearance'})).toBeDisabled();v.unmount();expect(p.adapter.closeDocument).not.toHaveBeenCalled();
});
it('invalidates a successful but superseded candidate raster',async()=>{
 const p=setup();let done!:(s:string)=>void;
 vi.mocked(p.adapter.renderPagePlan!).mockImplementation(plan=>plan.sourceId==='candidate'?new Promise(resolve=>{done=resolve;}):Promise.resolve('data:image/png;base64,AA'));
 const v=render(<FormFrameDialog {...p}/>);await edit(v);fireEvent.click(v.getByRole('button',{name:'Preview appearance'}));await waitFor(()=>expect(done).toBeDefined());
 fireEvent.change(v.getByLabelText('Border width (pt)'),{target:{value:'3'}});await act(async()=>done('data:image/png;base64,BB'));expect(v.getByRole('button',{name:'Apply appearance'})).toBeDisabled();expect(p.onApply).not.toHaveBeenCalled();await waitFor(()=>expect(p.adapter.closeDocument).toHaveBeenCalledWith('candidate'));
});
it('offers frame editing for native dropdown fields',async()=>{
 const p=setup(),combo={...field,kind:'combo' as const,value:{kind:'choice' as const,indices:[0]},options:['One','Two']};
 vi.mocked(p.adapter.listFormFields!).mockResolvedValue([combo]);
 const onFrameAppearance=vi.fn(),v=render(<FormFillEditor page={page} adapter={p.adapter} pageNumber={1} preview={false} onApply={vi.fn()} onClose={vi.fn()} onFrameAppearance={onFrameAppearance}/>);
 fireEvent.click(await v.findByRole('button',{name:'Border and background…'}));expect(onFrameAppearance).toHaveBeenCalledWith(combo);
});
