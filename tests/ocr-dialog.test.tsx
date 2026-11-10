import {act,cleanup,fireEvent,render,screen,waitFor} from '@testing-library/react';
import {afterEach,beforeEach,expect,it,vi} from 'vitest';
import {OcrDialog} from '../src/components/OcrDialog';
import {nativeAdapter} from '../src/editor/adapter';
import {startOcrSession} from '../src/editor/ocr/session';
import type {OcrPage} from '../src/editor/ocr/types';
vi.mock('../src/editor/ocr/session',()=>({startOcrSession:vi.fn()}));
const page:OcrPage={width:100,height:80,blocks:[{paragraphs:[{lines:[{words:[
 {text:'Native',confidence:98,bounds:{x:5,y:5,width:20,height:10}},
 {text:'cafe',confidence:64,bounds:{x:30,y:40,width:20,height:10}},
 {text:'final',confidence:96,bounds:{x:60,y:40,width:20,height:10}},
]}]}]}]};
const pages=[0,1].map(i=>({id:`p${i}`,sourceId:'source',pageIndex:i,width:200,height:160,rotation:0 as const,overlays:[]}));
const document={name:'Scan.pdf',pages:[...pages,{...pages[0],id:'copy'}],selectedPageId:'p0',selectedOverlayId:null};
function setup(owned:OcrPage|null=null){
 const png=new Uint8Array(33);png.set([137,80,78,71,13,10,26,10]);const v=new DataView(png.buffer);v.setUint32(8,13);png.set([73,72,68,82],12);v.setUint32(16,100);v.setUint32(20,80);
 const prepared={png,width:100,height:80,effectiveDpi:300,exclusions:owned?[]:[{x:14,y:6,width:2,height:8}],owned};
 const adapter={...nativeAdapter,prepareOcrPage:vi.fn().mockResolvedValue(prepared),
  applyOcr:vi.fn().mockResolvedValue({id:'candidate',name:'Scan',pages:[{width:200,height:160},{width:200,height:160}]}),closeDocument:vi.fn().mockResolvedValue(undefined)};
 const onAdopt=vi.fn().mockReturnValue(true),onClose=vi.fn();render(<OcrDialog document={document} adapter={adapter} knownSourceIds={()=>new Set(['source'])} onAdopt={onAdopt} onClose={onClose}/>);
 return {adapter,onAdopt,onClose,prepared};
}
beforeEach(()=>{vi.clearAllMocks();vi.stubGlobal('URL',class extends URL {static createObjectURL=vi.fn(()=> 'blob:preview');static revokeObjectURL=vi.fn();});
 vi.mocked(startOcrSession).mockResolvedValue({recognize:vi.fn().mockResolvedValue(page),dispose:vi.fn()});});
afterEach(()=>{cleanup();vi.unstubAllGlobals();});
it('explains local recognition and repeated copies, selects ranges and requires a language',async()=>{
 const {adapter}=setup();expect(screen.getByText(/copies of the same original page/i)).toBeInTheDocument();
 expect(screen.getByRole('combobox',{name:'Pages'})).toBeInTheDocument();
 fireEvent.change(screen.getByLabelText('Pages'),{target:{value:'range'}});fireEvent.change(screen.getByLabelText('Page range'),{target:{value:'1, 3'}});
 fireEvent.click(screen.getByLabelText('English'));expect(screen.getByRole('button',{name:'Start recognition'})).toBeDisabled();
 fireEvent.click(screen.getByLabelText('French'));fireEvent.click(screen.getByRole('button',{name:'Start recognition'}));
 await screen.findByRole('button',{name:'Apply searchable text'});expect(adapter.prepareOcrPage).toHaveBeenCalledTimes(1);
 expect(screen.getByRole('combobox',{name:'Review page'})).toBeInTheDocument();
 expect(startOcrSession).toHaveBeenCalledWith(['fra'],expect.anything());
});
it('reviews uncertain words, preserves existing text and applies corrections without changing scanned ink',async()=>{
 const {adapter,onAdopt,onClose}=setup();fireEvent.click(screen.getByRole('button',{name:'Start recognition'}));
 const input=await screen.findByLabelText('Recognized text');await waitFor(()=>expect(input).toHaveValue('cafe'));
 expect(screen.getByText(/scanned lettering stays unchanged/i)).toBeInTheDocument();
 fireEvent.change(input,{target:{value:'café'}});fireEvent.click(screen.getByRole('button',{name:'Save correction'}));
 fireEvent.click(screen.getByRole('button',{name:/final.*96/i}));fireEvent.click(screen.getByRole('button',{name:'Remove word'}));
 fireEvent.click(screen.getByRole('button',{name:'Apply searchable text'}));
 await waitFor(()=>expect(onAdopt).toHaveBeenCalledTimes(1));expect(onClose).toHaveBeenCalledTimes(1);
 const update=adapter.applyOcr.mock.calls[0][1][0];expect(update.page.blocks[0].paragraphs[0].lines[0].words.map((w:{text:string})=>w.text)).toEqual(['café']);
});
it('reviews an owned layer without recognizing and keeps Apply disabled until a correction',async()=>{
 setup(page);fireEvent.click(screen.getByRole('button',{name:'Start recognition'}));
 const input=await screen.findByLabelText('Recognized text');await waitFor(()=>expect(input).toHaveValue('cafe'));
 expect(startOcrSession).not.toHaveBeenCalled();expect(screen.getByRole('button',{name:'Apply searchable text'})).toBeDisabled();
 fireEvent.change(input,{target:{value:'corrected'}});fireEvent.click(screen.getByRole('button',{name:'Save correction'}));
 await waitFor(()=>expect(screen.getByRole('button',{name:'Apply searchable text'})).toBeEnabled());
});
it('locks multi-page review actions until the initial preview is ready, then saves the correction',async()=>{
 const {adapter,onAdopt,onClose,prepared}=setup(page);
 let release!:(value:typeof prepared)=>void;
 const preview=new Promise<typeof prepared>(resolve=>{release=resolve;});
 adapter.prepareOcrPage.mockResolvedValueOnce(prepared).mockResolvedValueOnce(prepared).mockReturnValueOnce(preview);
 fireEvent.change(screen.getByRole('combobox',{name:'Pages'}),{target:{value:'all'}});
 fireEvent.click(screen.getByRole('button',{name:'Start recognition'}));
 const input=await screen.findByLabelText('Recognized text');
 await waitFor(()=>expect(adapter.prepareOcrPage).toHaveBeenCalledTimes(3));
 expect(input).toBeDisabled();
 expect(screen.getByRole('combobox',{name:'Review page'})).toBeDisabled();
 expect(screen.getByRole('button',{name:'Start over'})).toBeDisabled();
 expect(screen.getByRole('button',{name:'Remove word'})).toBeDisabled();
 expect(screen.getByRole('button',{name:'Apply searchable text'})).toBeDisabled();
 expect(screen.getByRole('button',{name:/^Cancel$/})).toBeEnabled();
 await act(async()=>{release(prepared);await preview;});
 await waitFor(()=>expect(input).toBeEnabled());
 expect(startOcrSession).not.toHaveBeenCalled();
 fireEvent.change(input,{target:{value:'café'}});
 fireEvent.click(screen.getByRole('button',{name:'Save correction'}));
 await waitFor(()=>expect(screen.getByRole('button',{name:'Apply searchable text'})).toBeEnabled());
 fireEvent.click(screen.getByRole('button',{name:'Apply searchable text'}));
 await waitFor(()=>expect(onAdopt).toHaveBeenCalledTimes(1));
 expect(onClose).toHaveBeenCalledTimes(1);
 const update=adapter.applyOcr.mock.calls[0][1][0];
 expect(update.page.blocks[0].paragraphs[0].lines[0].words.map((word:{text:string})=>word.text)).toEqual(['Native','café','final']);
});
it('Escape cancels pending recognition and closes the dialog without native Apply',async()=>{
 let finish!:(value:OcrPage)=>void;const dispose=vi.fn();vi.mocked(startOcrSession).mockResolvedValue({recognize:vi.fn().mockImplementation(()=>new Promise(resolve=>{finish=resolve;})),dispose});
 const {adapter,onClose}=setup();fireEvent.click(screen.getByRole('button',{name:'Start recognition'}));await waitFor(()=>expect(finish).toBeTypeOf('function'));
 fireEvent.keyDown(screen.getByRole('dialog'),{key:'Escape'});expect(onClose).toHaveBeenCalledTimes(1);finish(page);
 await waitFor(()=>expect(dispose).toHaveBeenCalled());expect(adapter.applyOcr).not.toHaveBeenCalled();
});
