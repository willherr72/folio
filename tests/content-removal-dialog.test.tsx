import {previewPng as png} from './fixtures/content-removal-preview-png';
import {cleanup,fireEvent,render,screen,waitFor} from '@testing-library/react';
import {afterEach,beforeEach,expect,it,vi} from 'vitest';
import {TextRemovalDialog} from '../src/components/TextRemovalDialog';
import {captureTextRemoval} from '../src/editor/text-removal-workflow';
import {createSession} from '../src/editor/workspace';
import {createDemoAdapter} from '../src/editor/adapter';
const size={width:500,height:300},rect={x:20,y:40,width:60,height:20};
let urls=0;
beforeEach(()=>{urls=0;vi.stubGlobal('URL',class extends URL {static createObjectURL=vi.fn(()=>`blob:content-preview-${++urls}`);static revokeObjectURL=vi.fn();});});
afterEach(()=>{cleanup();vi.restoreAllMocks();vi.unstubAllGlobals();});
function fixture(){
 const adapter=createDemoAdapter(),candidate={id:'candidate',name:'Removed.pdf',pages:[size,size],bookmarks:[]};
 Object.assign(adapter,{beginTextRemovalSession:vi.fn().mockResolvedValue('session'),prepareTextRemovalSource:vi.fn().mockResolvedValue({document:{...candidate,id:'working'},omittedMetadata:[]}),
  previewSourceTextRemoval:vi.fn(),textRemovalPreviewBounds:vi.fn(),previewSourceContentRemoval:vi.fn().mockResolvedValue({previewId:'preview',sourceId:'working',candidate,removedCharacters:0,removedPaths:1,affectedImages:1,expiresInSeconds:900,omittedMetadata:[],
   pages:[{pageIndex:0,...size,removedCharacters:0,removedPaths:1,affectedImages:0,removedImagePixels:0,extent:rect,extendsBeyondMarks:true},{pageIndex:1,...size,removedCharacters:0,removedPaths:0,affectedImages:1,removedImagePixels:256,extent:rect,extendsBeyondMarks:true}]}),
  renderContentRemoval:vi.fn().mockImplementation(async(_session,sourceId,previewId,pageIndex)=>({sourceId,previewId,candidateId:previewId?'candidate':null,pageIndex,pageWidth:500,pageHeight:300,width:900,height:540,sourcePng:png,candidatePng:previewId?png:null,disclosurePng:previewId?png:null})),
  applyTextRemovalPreview:vi.fn().mockResolvedValue(candidate),discardTextRemovalPreview:vi.fn().mockResolvedValue(undefined),releaseTextRemovalSession:vi.fn().mockResolvedValue(undefined),closeDocument:vi.fn(),renderPage:vi.fn().mockResolvedValue('data:image/png;base64,AA')});
 const tab=createSession({id:'original',name:'Original.pdf',pages:[size,size]},adapter,100);
 return {adapter:adapter as any,props:{request:captureTextRemoval(tab,'content'),isCurrent:()=>true,isBorrowed:(id:string)=>id==='original',onApply:vi.fn(()=>true),onCancel:vi.fn()}};
}
async function loadPage(after=false){
 if(after)await screen.findByAltText('Page after content removal');
 fireEvent.load(await screen.findByAltText('Current edited page'));
 if(after){fireEvent.load(await screen.findByAltText('Page after content removal'));fireEvent.load(await screen.findByAltText('Removal bounds and rendering influence'));}
}
async function prepare(){
 await loadPage();fireEvent.click(screen.getByRole('button',{name:'Add selection'}));
 fireEvent.click(screen.getByRole('button',{name:'Next page'}));await loadPage();fireEvent.click(screen.getByRole('button',{name:'Add selection'}));
 fireEvent.click(screen.getByRole('button',{name:'Preview removal'}));await loadPage(true);
}
it('discloses whole-content expansion and waits for all affected pages to load',async()=>{
 const {adapter,props}=fixture();render(<TextRemovalDialog {...props}/>);
 expect(screen.getByRole('dialog')).toHaveAccessibleName('Remove content');await prepare();
 expect(screen.getByRole('button',{name:'Apply removal'})).toBeDisabled();
 expect(screen.getByText(/1 graphic.*1 image/i)).toBeInTheDocument();
 fireEvent.click(screen.getByRole('checkbox',{name:/beyond/i}));
 fireEvent.click(screen.getByRole('button',{name:'Previous page'}));
 // A new page response must replace the old images before its load events count.
 await waitFor(()=>expect(adapter.renderContentRemoval).toHaveBeenLastCalledWith('session','working','preview',0,900));
 await waitFor(()=>expect(screen.getByAltText('Current edited page').getAttribute('src')).toMatch(/^blob:/));await loadPage(true);
 await waitFor(()=>expect(screen.getByRole('button',{name:'Apply removal'})).toBeEnabled());
 fireEvent.click(screen.getByRole('button',{name:'Apply removal'}));await waitFor(()=>expect(props.onApply).toHaveBeenCalledTimes(1));
 expect(adapter.previewSourceTextRemoval).not.toHaveBeenCalled();expect(adapter.renderPage).not.toHaveBeenCalled();
});
it('keeps Apply disabled when a disclosure image fails and releases every owned URL on Cancel',async()=>{
 const {adapter,props}=fixture();const view=render(<TextRemovalDialog {...props}/>);await prepare();
 fireEvent.error(screen.getByAltText('Removal bounds and rendering influence'));
 expect(screen.getByRole('button',{name:'Apply removal'})).toBeDisabled();
 fireEvent.click(screen.getByRole('button',{name:'Cancel'}));view.unmount();
 await waitFor(()=>expect(adapter.releaseTextRemovalSession).toHaveBeenCalledWith('session'));
 expect(adapter.applyTextRemovalPreview).not.toHaveBeenCalled();
 const created=vi.mocked(URL.createObjectURL).mock.results.map(r=>r.value);
 expect(new Set(vi.mocked(URL.revokeObjectURL).mock.calls.map(c=>c[0]))).toEqual(new Set(created));
});
it('requires the newly returned image to load even when refreshed transport identifiers repeat',async()=>{
 const {props}=fixture();render(<TextRemovalDialog {...props}/>);await loadPage();
 const old=screen.getByAltText('Current edited page').getAttribute('src');
 expect(screen.getByRole('button',{name:'Add selection'})).toBeEnabled();
 fireEvent.click(screen.getByRole('button',{name:'Refresh working copy'}));
 await waitFor(()=>expect(screen.getByAltText('Current edited page').getAttribute('src')).not.toBe(old));
 expect(screen.getByRole('button',{name:'Add selection'})).toBeDisabled();
 await loadPage();expect(screen.getByRole('button',{name:'Add selection'})).toBeEnabled();
});
