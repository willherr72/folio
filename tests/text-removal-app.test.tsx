import {cleanup,fireEvent,render,screen,waitFor} from '@testing-library/react';
import {afterEach,beforeEach,expect,it,vi} from 'vitest';
import {App} from '../src/App';
import {nativeAdapter,type FolioAdapter} from '../src/editor/adapter';
const adapter=nativeAdapter as Required<FolioAdapter>;
import {clearSearchTextCache} from '../src/editor/search';
import {previewPng} from './fixtures/content-removal-preview-png';
vi.mock('../src/editor/useWorkspaceRecovery',()=>({useWorkspaceRecovery:()=>({ready:true,checkpoint:vi.fn().mockResolvedValue(undefined)})}));
vi.mock('../src/components/DocumentViewport',()=>({DocumentViewport:(props:any)=><output data-testid="plans">{JSON.stringify(props.pages)}</output>}));
const size={width:500,height:300},rect={x:20,y:40,width:60,height:20};
beforeEach(()=>{
 localStorage.clear();Object.defineProperty(window,'__TAURI_INTERNALS__',{value:{invoke:vi.fn().mockResolvedValue(0),transformCallback:()=>0},configurable:true});
 vi.spyOn(HTMLCanvasElement.prototype,'getContext').mockReturnValue({clearRect:vi.fn(),fillRect:vi.fn()} as unknown as CanvasRenderingContext2D);
 vi.spyOn(adapter,'openPdf').mockResolvedValueOnce({id:'source',name:'Original.pdf',pages:[size,size]}).mockResolvedValue({id:'other',name:'Other.pdf',pages:[size]});
 vi.spyOn(adapter,'renderPage').mockResolvedValue('data:image/png;base64,AA');
 vi.spyOn(adapter,'closeDocument').mockResolvedValue();
 vi.spyOn(adapter,'beginTextRemovalSession').mockResolvedValue('session');
 vi.spyOn(adapter,'prepareTextRemovalSource').mockImplementation(async(_session,pages)=>({document:{id:'working',name:'Working.pdf',pages:pages.map(p=>({width:p.width,height:p.height})),bookmarks:[]},omittedMetadata:[]}));
 vi.spyOn(adapter,'previewSourceTextRemoval').mockImplementation(async(_session,_source,marks)=>({previewId:'preview',sourceId:'working',candidate:{id:'candidate',name:'Removed.pdf',pages:[size,size,size],bookmarks:[]},removedCharacters:6,affectedBlocks:1,expiresInSeconds:900,omittedMetadata:[],pages:[{pageIndex:marks[0].pageIndex,...size,selectedGlyphCount:6,extent:rect}]}));
 vi.spyOn(adapter,'textRemovalPreviewBounds').mockImplementation(async(_s,_p,index)=>({pageIndex:index,offset:0,total:6,boxes:Array(6).fill(rect)}));
 vi.spyOn(adapter,'applyTextRemovalPreview').mockResolvedValue({id:'candidate',name:'Removed.pdf',pages:[size,size,size],bookmarks:[]});
 vi.spyOn(adapter,'discardTextRemovalPreview').mockResolvedValue();vi.spyOn(adapter,'releaseTextRemovalSession').mockResolvedValue();
 vi.spyOn(adapter,'startPdfSave').mockReturnValue({result:Promise.resolve('copy.pdf'),cancel:vi.fn().mockResolvedValue(false)});
});
afterEach(()=>{cleanup();clearSearchTextCache(['source','other','candidate']);vi.restoreAllMocks();delete(window as any).__TAURI_INTERNALS__;});
const plans=()=>JSON.parse(screen.getByTestId('plans').textContent!);
async function open(){render(<App initialDemo={false}/>);fireEvent.click(screen.getByRole('button',{name:'Open a PDF'}));await screen.findByRole('tab',{name:'Original.pdf'});fireEvent.click(screen.getByRole('button',{name:'Duplicate'}));}
async function preview(){fireEvent.click(screen.getByRole('button',{name:'Remove text…'}));fireEvent.load(await screen.findByAltText('Current edited page'));fireEvent.click(screen.getByRole('button',{name:'Add selection'}));fireEvent.click(screen.getByRole('button',{name:'Preview removal'}));fireEvent.load(await screen.findByAltText('Page after text removal'));await waitFor(()=>expect(screen.getByRole('button',{name:'Apply removal'})).toBeEnabled());}
it('offers combined removal explicitly and adopts it through the same duplicate-aware Undo transaction',async()=>{
 let url=0;vi.stubGlobal('URL',class extends URL {static createObjectURL=vi.fn(()=>`blob:app-removal-${++url}`);static revokeObjectURL=vi.fn();});
 vi.spyOn(adapter,'previewSourceContentRemoval').mockResolvedValue({previewId:'content-preview',sourceId:'working',candidate:{id:'candidate',name:'Removed.pdf',pages:[size,size,size],bookmarks:[]},removedCharacters:0,removedPaths:1,affectedImages:0,expiresInSeconds:900,omittedMetadata:[],pages:[{pageIndex:1,...size,removedCharacters:0,removedPaths:1,affectedImages:0,removedImagePixels:0,extendsBeyondMarks:true,extent:rect}]});
 vi.spyOn(adapter,'renderContentRemoval').mockImplementation(async(_s,sourceId,previewId,pageIndex)=>({sourceId,previewId,candidateId:previewId?'candidate':null,pageIndex,pageWidth:500,pageHeight:300,width:900,height:540,sourcePng:previewPng,candidatePng:previewId?previewPng:null,disclosurePng:previewId?previewPng:null}));
 try{
  await open();const before=plans();fireEvent.click(screen.getByRole('button',{name:'Remove content…'}));
  expect(screen.getByRole('dialog')).toHaveAccessibleName('Remove content');
  fireEvent.load(await screen.findByAltText('Current edited page'));fireEvent.click(screen.getByRole('button',{name:'Add selection'}));fireEvent.click(screen.getByRole('button',{name:'Preview removal'}));
  fireEvent.load(await screen.findByAltText('Page after content removal'));fireEvent.load(screen.getByAltText('Current edited page'));fireEvent.load(screen.getByAltText('Removal bounds and rendering influence'));
  fireEvent.click(screen.getByRole('checkbox',{name:/beyond/i}));await waitFor(()=>expect(screen.getByRole('button',{name:'Apply removal'})).toBeEnabled());
  fireEvent.click(screen.getByRole('button',{name:'Apply removal'}));await waitFor(()=>expect(plans().every((p:any)=>p.sourceId==='candidate')).toBe(true));
  fireEvent.click(screen.getByRole('button',{name:'Undo'}));expect(plans()).toEqual(before);
  fireEvent.click(screen.getByRole('button',{name:'Redo'}));expect(plans().every((p:any)=>p.sourceId==='candidate')).toBe(true);
  expect(adapter.previewSourceTextRemoval).not.toHaveBeenCalled();
 }finally{cleanup();vi.unstubAllGlobals();}
});
it('adopts one duplicate-aware complete source, supports exact Undo/Redo and saves the candidate plans',async()=>{
 await open();const before=plans();await preview();
 expect(nativeAdapter.prepareTextRemovalSource).toHaveBeenCalledWith('session',before,undefined);
 expect(nativeAdapter.previewSourceTextRemoval).toHaveBeenCalledWith('session','working',[{pageIndex:1,rect}]);
 expect(screen.getByRole('button',{name:'Undo'})).toBeDisabled();
 fireEvent.click(screen.getByRole('button',{name:'Apply removal'}));await waitFor(()=>expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
 const after=plans();expect(after.map((p:any)=>p.id)).toEqual(before.map((p:any)=>p.id));expect(after.map((p:any)=>[p.sourceId,p.pageIndex])).toEqual([['candidate',0],['candidate',1],['candidate',2]]);
 expect(nativeAdapter.closeDocument).not.toHaveBeenCalledWith('source');expect(nativeAdapter.closeDocument).not.toHaveBeenCalledWith('candidate');
 fireEvent.click(screen.getByRole('button',{name:'Undo'}));expect(plans()).toEqual(before);fireEvent.click(screen.getByRole('button',{name:'Redo'}));expect(plans()).toEqual(after);
 fireEvent.click(screen.getByRole('button',{name:'Save a copy'}));await waitFor(()=>expect(nativeAdapter.startPdfSave).toHaveBeenCalledWith(after,{bookmarks:[]},expect.any(Function)));
});
it('cancels an in-flight Apply, protects a newly opened tab and closes only the orphan candidate',async()=>{
 await open();const before=plans();await preview();let resolve!:(value:any)=>void;
 vi.mocked(nativeAdapter.applyTextRemovalPreview!).mockReturnValue(new Promise(done=>{resolve=done;}));
 fireEvent.click(screen.getByRole('button',{name:'Apply removal'}));fireEvent.click(screen.getByRole('button',{name:'Cancel'}));
 expect(plans()).toEqual(before);fireEvent.click(screen.getByRole('button',{name:'Open PDF in new tab'}));await screen.findByRole('tab',{name:'Other.pdf'});const other=plans();
 resolve({id:'candidate',name:'Removed.pdf',pages:[size,size,size],bookmarks:[]});await waitFor(()=>expect(nativeAdapter.closeDocument).toHaveBeenCalledWith('candidate'));
 expect(plans()).toEqual(other);expect(nativeAdapter.closeDocument).not.toHaveBeenCalledWith('source');expect(nativeAdapter.closeDocument).not.toHaveBeenCalledWith('other');
});
