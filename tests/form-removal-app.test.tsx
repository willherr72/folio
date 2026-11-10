import {act,cleanup,fireEvent,render,screen,waitFor} from '@testing-library/react';
import {afterEach,beforeEach,expect,it,vi} from 'vitest';
import {App} from '../src/App';
import {nativeAdapter} from '../src/editor/adapter';
import {clearSearchTextCache} from '../src/editor/search';
vi.mock('../src/editor/useWorkspaceRecovery',()=>({useWorkspaceRecovery:()=>({ready:true,checkpoint:vi.fn().mockResolvedValue(undefined)})}));
vi.mock('@tauri-apps/api/window',()=>({getCurrentWindow:()=>({onCloseRequested:vi.fn().mockResolvedValue(()=>{})})}));
vi.mock('../src/components/DocumentViewport',()=>({DocumentViewport:(props:any)=><output data-testid="plans">{JSON.stringify(props.pages)}</output>}));
const size={width:300,height:400};
beforeEach(()=>{
 localStorage.clear();Object.defineProperty(window,'__TAURI_INTERNALS__',{value:{invoke:vi.fn().mockResolvedValue(0),transformCallback:()=>0},configurable:true});
 vi.spyOn(nativeAdapter,'openPdf').mockResolvedValue({id:'source',name:'Original.pdf',pages:[size,size]});
 vi.spyOn(nativeAdapter,'renderPage').mockResolvedValue('data:image/png;base64,AA');
 vi.spyOn(nativeAdapter,'closeDocument').mockResolvedValue();
 vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,'inspectFormRemoval').mockResolvedValue({expectedKey:'key',fullName:'answer',linkedWidgets:2,pageIndices:[0,1],removalReason:null});
 vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,'listFormFields').mockImplementation(async id=>id==='candidate'?[]:[{annotationIndex:0,name:'answer',label:'Answer',kind:'text',value:{kind:'text',text:'Keep'},options:[],selectedIndices:[],readOnly:false,required:false,multiline:false,password:false,multiSelect:false,editableChoice:false,reason:null,bounds:{x:20,y:30,width:100,height:30},visible:true}]);
 vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,'removeFormField').mockResolvedValue({id:'candidate',name:'Updated.pdf',pages:[size,size]});
});
afterEach(()=>{cleanup();clearSearchTextCache(['source','candidate','other']);vi.restoreAllMocks();delete (window as any).__TAURI_INTERNALS__;});
async function open(){fireEvent.click(screen.getByRole('button',{name:'Open a PDF'}));await screen.findByRole('tab',{name:'Original.pdf'});}
async function dialog(){fireEvent.click(screen.getByRole('button',{name:'Fill forms'}));fireEvent.click(await screen.findByRole('button',{name:'Remove field…'}));await screen.findByText('answer');}
it('removes every shared occurrence in one Undo step and clears field selection',async()=>{
 render(<App initialDemo={false}/>);await open();fireEvent.click(screen.getByRole('button',{name:'Duplicate'}));const before=JSON.parse(screen.getByTestId('plans').textContent!);
 await dialog();expect(screen.getByRole('button',{name:'Undo'})).toBeDisabled();fireEvent.click(screen.getByRole('button',{name:'Remove field'}));await waitFor(()=>expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
 expect(nativeAdapter.removeFormField).toHaveBeenCalledWith('source',{pageIndex:0,annotationIndex:0,expectedKey:'key'});
 const after=JSON.parse(screen.getByTestId('plans').textContent!);expect(after.map((p:any)=>[p.sourceId,p.pageIndex])).toEqual([['candidate',0],['candidate',0],['candidate',1]]);
 expect(screen.queryByDisplayValue('Keep')).not.toBeInTheDocument();fireEvent.click(screen.getByRole('button',{name:'Undo'}));expect(JSON.parse(screen.getByTestId('plans').textContent!)).toEqual(before);
 fireEvent.click(screen.getByRole('button',{name:'Redo'}));expect(JSON.parse(screen.getByTestId('plans').textContent!)).toEqual(after);expect(nativeAdapter.closeDocument).not.toHaveBeenCalledWith('candidate');
});
it('never adopts or closes a removal response owned by another tab',async()=>{
 vi.mocked(nativeAdapter.openPdf).mockResolvedValueOnce({id:'source',name:'Original.pdf',pages:[size,size]}).mockResolvedValueOnce({id:'other',name:'Other.pdf',pages:[size,size]});
 vi.mocked(nativeAdapter.removeFormField!).mockResolvedValue({id:'other',name:'Other.pdf',pages:[size,size]});
 render(<App initialDemo={false}/>);await open();fireEvent.click(screen.getByRole('button',{name:'Open'}));await screen.findByRole('tab',{name:'Other.pdf'});fireEvent.click(screen.getByRole('tab',{name:'Original.pdf'}));const before=screen.getByTestId('plans').textContent;
 await dialog();fireEvent.click(screen.getByRole('button',{name:'Remove field'}));expect(await screen.findByRole('alert')).toHaveTextContent('new source version');expect(screen.getByTestId('plans').textContent).toBe(before);expect(nativeAdapter.closeDocument).not.toHaveBeenCalledWith('other');
});
it('blocks removal until a value draft is applied or reset',async()=>{
 render(<App initialDemo={false}/>);await open();fireEvent.click(screen.getByRole('button',{name:'Fill forms'}));await screen.findByRole('button',{name:'Remove field…'});
 fireEvent.change(screen.getByDisplayValue('Keep'),{target:{value:'Unsaved'}});expect(screen.getByRole('button',{name:'Remove field…'})).toBeDisabled();expect(nativeAdapter.inspectFormRemoval).not.toHaveBeenCalled();
});
it('does not expose field removal to a fill-only protected document',async()=>{
 vi.mocked(nativeAdapter.openPdf).mockResolvedValue({id:'source',name:'Original.pdf',pages:[size,size],protection:{documentId:'protected',revision:4,owner:false,permissions:{copyContent:true,editContent:false,annotate:false,fillForms:true,assemble:false,print:true,printHighQuality:true}}});
 render(<App initialDemo={false}/>);await open();fireEvent.click(screen.getByRole('button',{name:'Fill forms'}));await screen.findByDisplayValue('Keep');expect(screen.queryByRole('button',{name:'Remove field…'})).not.toBeInTheDocument();
});
it('releases a late native candidate when the app unmounts before removal finishes',async()=>{
 let done!:(value:{id:string;name:string;pages:typeof size[]})=>void;
 vi.mocked(nativeAdapter.removeFormField!).mockImplementation(()=>new Promise(resolve=>{done=resolve;}));
 const view=render(<App initialDemo={false}/>);await open();await dialog();fireEvent.click(screen.getByRole('button',{name:'Remove field'}));
 await waitFor(()=>expect(nativeAdapter.removeFormField).toHaveBeenCalledOnce());view.unmount();
 await act(async()=>done({id:'candidate',name:'Updated',pages:[size,size]}));
 await waitFor(()=>expect(nativeAdapter.closeDocument).toHaveBeenCalledWith('candidate'));
});
it('rejects an incomplete replacement page map and releases its candidate',async()=>{
 vi.mocked(nativeAdapter.removeFormField!).mockResolvedValue({id:'candidate',name:'Incomplete',pages:[size]});
 render(<App initialDemo={false}/>);await open();const before=screen.getByTestId('plans').textContent;await dialog();fireEvent.click(screen.getByRole('button',{name:'Remove field'}));
 expect(await screen.findByRole('alert')).toHaveTextContent('omitted a page');expect(screen.getByTestId('plans').textContent).toBe(before);expect(nativeAdapter.closeDocument).toHaveBeenCalledWith('candidate');
});
