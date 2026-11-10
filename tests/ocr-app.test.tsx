import {act,cleanup,fireEvent,render,screen,waitFor} from '@testing-library/react';
import {afterEach,beforeEach,expect,it,vi} from 'vitest';
import {App} from '../src/App';
import {nativeAdapter} from '../src/editor/adapter';
import {startOcrSession} from '../src/editor/ocr/session';
import type {DocumentInfo} from '../src/editor/types';
vi.mock('../src/components/DocumentViewport',()=>({DocumentViewport:({pages}:{pages:{sourceId:string}[]})=><div data-testid="sources">{pages.map(p=>p.sourceId).join(',')}</div>}));
vi.mock('../src/editor/ocr/session',()=>({startOcrSession:vi.fn()}));
const info:DocumentInfo={id:'source',name:'Scan.pdf',pages:[{width:200,height:160},{width:200,height:160}]};
beforeEach(()=>{
 localStorage.clear();vi.spyOn(nativeAdapter,'openPdf').mockResolvedValue(info);vi.spyOn(nativeAdapter,'renderPage').mockResolvedValue('data:image/png;base64,');
 vi.spyOn(nativeAdapter,'exportPdf').mockResolvedValue('saved.pdf');vi.spyOn(nativeAdapter,'closeDocument').mockResolvedValue();
 vi.stubGlobal('URL',class extends URL {static createObjectURL=vi.fn(()=> 'blob:ocr');static revokeObjectURL=vi.fn();});
 const png=new Uint8Array(33);png.set([137,80,78,71,13,10,26,10]);const v=new DataView(png.buffer);v.setUint32(8,13);png.set([73,72,68,82],12);v.setUint32(16,100);v.setUint32(20,80);
 vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,'prepareOcrPage').mockResolvedValue({png,width:100,height:80,effectiveDpi:300,exclusions:[],owned:null});
 vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,'applyOcr').mockResolvedValue({...info,id:'recognized'});
 vi.mocked(startOcrSession).mockResolvedValue({recognize:vi.fn().mockResolvedValue({width:100,height:80,blocks:[{paragraphs:[{lines:[{words:[{text:'café',confidence:70,bounds:{x:10,y:10,width:30,height:10}}]}]}]}]}),dispose:vi.fn()});
});
afterEach(()=>{cleanup();vi.restoreAllMocks();vi.unstubAllGlobals();});
async function open(){const view=render(<App initialDemo={false}/>);fireEvent.click(screen.getByRole('button',{name:'Open a PDF'}));await screen.findByRole('tab',{name:'Scan.pdf'});return view;}
async function review(){fireEvent.click(screen.getByRole('button',{name:'Recognize text'}));fireEvent.click(screen.getByRole('button',{name:'Start recognition'}));await screen.findByLabelText('Recognized text');}
it('applies one source replacement, blocks conflicting edits, and retains both pages through undo, redo and save',async()=>{
 await open();await review();expect(screen.getByRole('button',{name:'Save a copy'})).toBeDisabled();expect(screen.getByRole('button',{name:'Undo'})).toBeDisabled();
 fireEvent.change(screen.getByLabelText('Recognized text'),{target:{value:'corrected'}});fireEvent.click(screen.getByRole('button',{name:'Save correction'}));fireEvent.click(screen.getByRole('button',{name:'Apply searchable text'}));
 await waitFor(()=>expect(screen.queryByRole('dialog')).not.toBeInTheDocument());expect(screen.getByTestId('sources')).toHaveTextContent('recognized,recognized');
 fireEvent.click(screen.getByRole('button',{name:'Undo'}));expect(screen.getByTestId('sources')).toHaveTextContent('source,source');
 fireEvent.click(screen.getByRole('button',{name:'Redo'}));fireEvent.click(screen.getByRole('button',{name:'Save a copy'}));await waitFor(()=>expect(nativeAdapter.exportPdf).toHaveBeenCalled());
 expect(vi.mocked(nativeAdapter.exportPdf).mock.calls[0][0].map(p=>p.sourceId)).toEqual(['recognized','recognized']);
});
it('closes a late candidate on app unmount',async()=>{
 let finish!:(value:DocumentInfo)=>void;vi.mocked(nativeAdapter.applyOcr!).mockReturnValue(new Promise(r=>{finish=r;}));const view=await open();await review();
 const apply=screen.getByRole('button',{name:'Apply searchable text'});await waitFor(()=>expect(apply).toBeEnabled());
 fireEvent.click(apply);await waitFor(()=>expect(nativeAdapter.applyOcr).toHaveBeenCalledOnce());view.unmount();await act(async()=>{finish({...info,id:'late'});});
 await waitFor(()=>expect(nativeAdapter.closeDocument).toHaveBeenCalledWith('late'));
});
it('leaves the source and correction draft intact on failure',async()=>{
 vi.mocked(nativeAdapter.applyOcr!).mockRejectedValue(new Error('Could not preserve this page'));await open();await review();const apply=screen.getByRole('button',{name:'Apply searchable text'});await waitFor(()=>expect(apply).toBeEnabled());fireEvent.click(apply);
 expect(await screen.findByRole('alert')).toHaveTextContent('Could not preserve this page');expect(screen.getByTestId('sources')).toHaveTextContent('source,source');expect(screen.getByLabelText('Recognized text')).toHaveValue('café');
});
it('does not offer recognition when copying PDF content is restricted',async()=>{
 vi.mocked(nativeAdapter.openPdf).mockResolvedValue({...info,protection:{documentId:'locked',owner:false,revision:4,permissions:{editContent:true,copyContent:false,annotate:true,fillForms:true,assemble:true,print:true,printHighQuality:true}}});
 await open();expect(screen.getByRole('button',{name:'Recognize text'})).toBeDisabled();expect(nativeAdapter.prepareOcrPage).not.toHaveBeenCalled();
});
