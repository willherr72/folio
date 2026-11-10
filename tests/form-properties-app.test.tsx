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
 vi.spyOn(nativeAdapter,'closeDocument').mockResolvedValue();
 vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,'inspectFormProperties').mockResolvedValue({expectedKey:'key',name:'answer',fullName:'answer',label:'Answer',readOnly:false,required:false,linkedWidgets:2,renameReason:null});
 vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,'listFormFields').mockImplementation(async id=>[{annotationIndex:0,name:'answer',label:'Answer',kind:'text',value:{kind:'text',text:'Keep'},options:[],selectedIndices:[],readOnly:id==='candidate',required:id==='candidate',multiline:false,password:false,multiSelect:false,editableChoice:false,reason:id==='candidate'?'Read only':null,bounds:{x:20,y:30,width:100,height:30},visible:true}]);
 vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,'setFormProperties').mockResolvedValue({id:'candidate',name:'Updated.pdf',pages:[size,size]});
});
afterEach(()=>{cleanup();clearSearchTextCache(['source','candidate','other']);vi.restoreAllMocks();delete (window as any).__TAURI_INTERNALS__;});
it('updates every shared source occurrence with one Undo step and keeps the form editor open',async()=>{
 render(<App initialDemo={false}/>);fireEvent.click(screen.getByRole('button',{name:'Open a PDF'}));await screen.findByRole('tab',{name:'Original.pdf'});
 fireEvent.click(screen.getByRole('button',{name:'Duplicate'}));const before=JSON.parse(screen.getByTestId('plans').textContent!);
 fireEvent.click(screen.getByRole('button',{name:'Fill forms'}));fireEvent.click(await screen.findByRole('button',{name:'Field properties…'}));
 await screen.findByDisplayValue('answer');expect(screen.getByRole('button',{name:'Undo'})).toBeDisabled();
 fireEvent.click(screen.getByLabelText('Read only'));fireEvent.click(screen.getByLabelText('Required'));fireEvent.click(screen.getByRole('button',{name:'Apply properties'}));
 await waitFor(()=>expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
 expect(nativeAdapter.setFormProperties).toHaveBeenCalledWith('source',{pageIndex:0,annotationIndex:0,expectedKey:'key',changes:{readOnly:true,required:true}});
 const after=JSON.parse(screen.getByTestId('plans').textContent!);expect(after.map((p:any)=>[p.sourceId,p.pageIndex])).toEqual([['candidate',0],['candidate',0],['candidate',1]]);
 await waitFor(()=>expect(screen.getByDisplayValue('Keep')).toBeDisabled());expect(screen.getByRole('button',{name:'Field properties…'})).toBeEnabled();
 fireEvent.click(screen.getByRole('button',{name:'Done'}));fireEvent.click(screen.getByRole('button',{name:'Undo'}));expect(JSON.parse(screen.getByTestId('plans').textContent!)).toEqual(before);
 fireEvent.click(screen.getByRole('button',{name:'Redo'}));expect(JSON.parse(screen.getByTestId('plans').textContent!)).toEqual(after);expect(nativeAdapter.closeDocument).not.toHaveBeenCalledWith('candidate');
});
it('rejects another tab source as a property result without closing or adopting it',async()=>{
 vi.mocked(nativeAdapter.openPdf).mockResolvedValueOnce({id:'source',name:'Original.pdf',pages:[size,size]}).mockResolvedValueOnce({id:'other',name:'Other.pdf',pages:[size,size]});
 vi.mocked(nativeAdapter.setFormProperties!).mockResolvedValue({id:'other',name:'Other.pdf',pages:[size,size]});
 render(<App initialDemo={false}/>);fireEvent.click(screen.getByRole('button',{name:'Open a PDF'}));await screen.findByRole('tab',{name:'Original.pdf'});fireEvent.click(screen.getByRole('button',{name:'Open'}));await screen.findByRole('tab',{name:'Other.pdf'});fireEvent.click(screen.getByRole('tab',{name:'Original.pdf'}));
 const before=screen.getByTestId('plans').textContent;fireEvent.click(screen.getByRole('button',{name:'Fill forms'}));fireEvent.click(await screen.findByRole('button',{name:'Field properties…'}));await screen.findByDisplayValue('answer');fireEvent.click(screen.getByLabelText('Required'));fireEvent.click(screen.getByRole('button',{name:'Apply properties'}));
 expect(await screen.findByRole('alert')).toHaveTextContent('new source version');expect(screen.getByTestId('plans').textContent).toBe(before);expect(nativeAdapter.closeDocument).not.toHaveBeenCalledWith('other');
});
