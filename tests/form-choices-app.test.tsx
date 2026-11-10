import {cleanup,fireEvent,render,screen,waitFor} from '@testing-library/react';
import {afterEach,beforeEach,expect,it,vi} from 'vitest';
import {App} from '../src/App';
import {nativeAdapter} from '../src/editor/adapter';
import {clearSearchTextCache} from '../src/editor/search';
vi.mock('../src/editor/useWorkspaceRecovery',()=>({useWorkspaceRecovery:()=>({ready:true,checkpoint:vi.fn().mockResolvedValue(undefined)})}));
vi.mock('../src/components/DocumentViewport',()=>({DocumentViewport:(props:any)=><output data-testid="plans">{JSON.stringify(props.pages)}</output>}));
const size={width:300,height:400};
beforeEach(()=>{
 localStorage.clear();Object.defineProperty(window,'__TAURI_INTERNALS__',{value:{invoke:vi.fn().mockResolvedValue(0),transformCallback:()=>0},configurable:true});
 vi.spyOn(nativeAdapter,'openPdf').mockResolvedValue({id:'source',name:'Original.pdf',pages:[size,size]});
 vi.spyOn(nativeAdapter,'renderPage').mockResolvedValue('data:image/png;base64,AA');
 vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,'renderPagePlan').mockResolvedValue('data:image/png;base64,AA');
 vi.spyOn(nativeAdapter,'closeDocument').mockResolvedValue();
 vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,'inspectFormChoices').mockResolvedValue({expectedKey:'key',name:'answer',widgets:[{pageIndex:0,annotationIndex:0},{pageIndex:1,annotationIndex:0}],model:{options:[{display:'Alpha',exportValue:'a'},{display:'Beta',exportValue:'b'}],selection:{kind:'rows',value:[1]},defaults:['a'],combo:true,editable:false,multiple:false,sorted:false,topIndex:0}});
 vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,'listFormFields').mockImplementation(async id=>[{annotationIndex:0,name:'answer',label:'Answer',kind:'combo',value:{kind:'choice',indices:[1]},options:[id==='candidate'?'Changed':'Alpha','Beta'],selectedIndices:[1],readOnly:false,required:false,multiline:false,password:false,multiSelect:false,editableChoice:false,reason:null,bounds:{x:20,y:30,width:100,height:30},visible:true}]);
 vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,'setFormChoices').mockResolvedValue({id:'candidate',name:'Updated.pdf',pages:[size,size]});
});
afterEach(()=>{cleanup();clearSearchTextCache(['source','candidate','other']);vi.restoreAllMocks();delete (window as any).__TAURI_INTERNALS__;});
async function open(){render(<App initialDemo={false}/>);fireEvent.click(screen.getByRole('button',{name:'Open a PDF'}));await screen.findByRole('tab',{name:'Original.pdf'});}
async function edit(){fireEvent.click(screen.getByRole('button',{name:'Fill forms'}));fireEvent.click(await screen.findByRole('button',{name:'Edit choices…'}));fireEvent.change(await screen.findByLabelText('Display label'),{target:{value:'Changed'}});}
it('adopts the preview for every source occurrence with one Undo/Redo step',async()=>{
 await open();fireEvent.click(screen.getByRole('button',{name:'Duplicate'}));const before=JSON.parse(screen.getByTestId('plans').textContent!);await edit();
 expect(screen.getByRole('button',{name:'Undo'})).toBeDisabled();fireEvent.click(screen.getByRole('button',{name:'Preview choices'}));await waitFor(()=>expect(screen.getByRole('button',{name:'Apply choices'})).toBeEnabled());
 expect(screen.getByTestId('plans').textContent).toBe(JSON.stringify(before));fireEvent.click(screen.getByRole('button',{name:'Apply choices'}));await waitFor(()=>expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
 const after=JSON.parse(screen.getByTestId('plans').textContent!);expect(after.map((p:any)=>[p.sourceId,p.pageIndex])).toEqual([['candidate',0],['candidate',0],['candidate',1]]);expect(nativeAdapter.setFormChoices).toHaveBeenCalledTimes(1);
 expect(nativeAdapter.setFormChoices).toHaveBeenCalledWith('source',expect.objectContaining({pageIndex:0,annotationIndex:0,expectedKey:'key',edit:expect.objectContaining({selection:{kind:'keep'},defaults:{kind:'keep'}})}));
 fireEvent.click(screen.getByRole('button',{name:'Done'}));fireEvent.click(screen.getByRole('button',{name:'Undo'}));expect(JSON.parse(screen.getByTestId('plans').textContent!)).toEqual(before);
 fireEvent.click(screen.getByRole('button',{name:'Redo'}));expect(JSON.parse(screen.getByTestId('plans').textContent!)).toEqual(after);expect(nativeAdapter.closeDocument).not.toHaveBeenCalledWith('candidate');
});
it('Cancel disposes the candidate and leaves document history untouched',async()=>{
 await open();const before=screen.getByTestId('plans').textContent;await edit();fireEvent.click(screen.getByRole('button',{name:'Preview choices'}));await waitFor(()=>expect(screen.getByRole('button',{name:'Apply choices'})).toBeEnabled());fireEvent.click(screen.getByRole('button',{name:'Cancel'}));
 await waitFor(()=>expect(nativeAdapter.closeDocument).toHaveBeenCalledWith('candidate'));expect(screen.getByTestId('plans').textContent).toBe(before);expect(screen.getByRole('button',{name:'Undo'})).toBeDisabled();
});
it('does not adopt or close another tab source returned as a preview',async()=>{
 vi.mocked(nativeAdapter.openPdf).mockResolvedValueOnce({id:'source',name:'Original.pdf',pages:[size,size]}).mockResolvedValueOnce({id:'other',name:'Other.pdf',pages:[size,size]});vi.mocked(nativeAdapter.setFormChoices!).mockResolvedValue({id:'other',name:'Other.pdf',pages:[size,size]});
 await open();fireEvent.click(screen.getByRole('button',{name:'Open'}));await screen.findByRole('tab',{name:'Other.pdf'});fireEvent.click(screen.getByRole('tab',{name:'Original.pdf'}));const before=screen.getByTestId('plans').textContent;await edit();fireEvent.click(screen.getByRole('button',{name:'Preview choices'}));
 expect(await screen.findByRole('alert')).toHaveTextContent('new source version');expect(screen.getByTestId('plans').textContent).toBe(before);expect(nativeAdapter.closeDocument).not.toHaveBeenCalledWith('other');
});
it('requires applying or resetting a dirty fill answer before opening choices',async()=>{
 await open();fireEvent.click(screen.getByRole('button',{name:'Fill forms'}));const answer=await screen.findByLabelText('Answer');fireEvent.change(answer,{target:{value:'0'}});
 expect(screen.getByRole('button',{name:'Edit choices…'})).toBeDisabled();fireEvent.click(screen.getByRole('button',{name:'Reset draft'}));expect(screen.getByRole('button',{name:'Edit choices…'})).toBeEnabled();
});
