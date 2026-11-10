import {cleanup,fireEvent,render,screen,waitFor} from '@testing-library/react';
import {afterEach,beforeEach,expect,it,vi} from 'vitest';
import {StrictMode} from 'react';
import {TextRemovalDialog} from '../src/components/TextRemovalDialog';
import {captureTextRemoval} from '../src/editor/text-removal-workflow';
import {createSession} from '../src/editor/workspace';
import {createDemoAdapter,type FolioAdapter} from '../src/editor/adapter';
const size={width:500,height:300};
const rect={x:20,y:40,width:60,height:20};
const draw=vi.fn();
beforeEach(()=>{vi.spyOn(HTMLCanvasElement.prototype,'getContext').mockReturnValue({clearRect:vi.fn(),fillRect:draw} as unknown as CanvasRenderingContext2D);draw.mockClear();});
afterEach(()=>{cleanup();vi.restoreAllMocks();vi.useRealTimers();vi.unstubAllGlobals();});
function fixture(omissions:string[]=[]){
 const adapter=createDemoAdapter(),candidate={id:'candidate',name:'Removed.pdf',pages:[size,size],bookmarks:[]};
 Object.assign(adapter,{beginTextRemovalSession:vi.fn().mockResolvedValue('session'),prepareTextRemovalSource:vi.fn().mockResolvedValue({document:{...candidate,id:'working'},omittedMetadata:omissions}),previewSourceTextRemoval:vi.fn().mockResolvedValue({previewId:'preview',sourceId:'working',candidate,removedCharacters:6,affectedBlocks:1,expiresInSeconds:900,omittedMetadata:omissions,pages:[{pageIndex:0,...size,selectedGlyphCount:6,extent:rect}]}),textRemovalPreviewBounds:vi.fn().mockResolvedValue({pageIndex:0,offset:0,total:6,boxes:Array(6).fill(rect)}),applyTextRemovalPreview:vi.fn().mockResolvedValue(candidate),discardTextRemovalPreview:vi.fn().mockResolvedValue(undefined),releaseTextRemovalSession:vi.fn().mockResolvedValue(undefined),closeDocument:vi.fn().mockResolvedValue(undefined),renderPage:vi.fn().mockResolvedValue('data:image/png;base64,AA')});
 const tab=createSession({id:'original',name:'Original.pdf',pages:[size,size]},adapter,100);
 const props={request:captureTextRemoval(tab),isCurrent:()=>true,isBorrowed:(id:string)=>id==='original'||id==='foreign',onApply:vi.fn(()=>true),onCancel:vi.fn()};
 return {adapter:adapter as Required<FolioAdapter>,props,candidate};
}
async function prepared(){const image=await screen.findByAltText('Current edited page');fireEvent.load(image);return image;}
function mark(){for(const [label,value] of [['Left (pt)','20'],['Top (pt)','40'],['Width (pt)','60'],['Height (pt)','20']])fireEvent.change(screen.getByLabelText(label),{target:{value}});fireEvent.click(screen.getByRole('button',{name:'Add selection'}));}
async function preview(){fireEvent.click(screen.getByRole('button',{name:'Preview removal'}));const image=await screen.findByAltText('Page after text removal');fireEvent.load(image);await waitFor(()=>expect(draw).toHaveBeenCalled());}
it('marks with keyboard fields, renders exact preview bounds and applies once',async()=>{
 const {props,adapter,candidate}=fixture();render(<TextRemovalDialog {...props}/>);await prepared();mark();
 expect(screen.getByRole('button',{name:'Apply removal'})).toBeDisabled();await preview();
 await waitFor(()=>expect(screen.getByRole('button',{name:'Apply removal'})).toBeEnabled());
 expect(adapter.previewSourceTextRemoval).toHaveBeenCalledWith('session','working',[{pageIndex:0,rect}]);expect(draw).toHaveBeenCalledTimes(6);
 fireEvent.click(screen.getByRole('button',{name:'Apply removal'}));await waitFor(()=>expect(props.onApply).toHaveBeenCalledWith(candidate,expect.objectContaining({previewId:'preview'})));
 expect(adapter.closeDocument).not.toHaveBeenCalled();
});
it('requires omission acknowledgement and resets it after changing selections',async()=>{
 const {props,adapter}=fixture(['document information']);render(<TextRemovalDialog {...props}/>);await prepared();mark();await preview();
 expect(screen.getByText('document information')).toBeInTheDocument();expect(screen.getByRole('button',{name:'Apply removal'})).toBeDisabled();
 fireEvent.click(screen.getByRole('checkbox',{name:/understand/i}));await waitFor(()=>expect(screen.getByRole('button',{name:'Apply removal'})).toBeEnabled());
 fireEvent.click(screen.getByRole('button',{name:'Remove selection 1'}));expect(screen.getByRole('button',{name:'Apply removal'})).toBeDisabled();expect(screen.queryByRole('checkbox')).not.toBeInTheDocument();
 await waitFor(()=>expect(adapter.discardTextRemovalPreview).toHaveBeenCalledWith('session','preview'));
});
it('keeps unsupported working copies and selections intact while disabling Apply',async()=>{
 const {props,adapter}=fixture();vi.mocked(adapter.previewSourceTextRemoval).mockRejectedValue(new Error('This PDF content is unsupported.'));render(<TextRemovalDialog {...props}/>);await prepared();mark();fireEvent.click(screen.getByRole('button',{name:'Preview removal'}));
 expect(await screen.findByRole('alert')).toHaveTextContent('unsupported');expect(screen.getByRole('button',{name:'Remove selection 1'})).toBeInTheDocument();expect(screen.getByRole('button',{name:'Apply removal'})).toBeDisabled();expect(adapter.closeDocument).not.toHaveBeenCalled();
 fireEvent.click(screen.getByRole('button',{name:'Cancel'}));expect(props.onCancel).toHaveBeenCalled();await waitFor(()=>expect(adapter.releaseTextRemovalSession).toHaveBeenCalledWith('session'));
});
it('rejects empty numeric fields and handles source/candidate image failures',async()=>{
 const {props}=fixture();render(<TextRemovalDialog {...props}/>);const image=await prepared();
 fireEvent.change(screen.getByLabelText('Width (pt)'),{target:{value:''}});expect(screen.getByRole('button',{name:'Add selection'})).toBeDisabled();mark();await preview();
 fireEvent.error(screen.getByAltText('Page after text removal'));expect(screen.getByRole('button',{name:'Apply removal'})).toBeDisabled();
 fireEvent.error(image);expect(screen.getByRole('button',{name:'Add selection'})).toBeDisabled();
});
it('shows a live pointer rectangle, clamps it to the page and discards cancelled gestures',async()=>{
 vi.stubGlobal('PointerEvent',class extends MouseEvent{pointerId:number;constructor(type:string,init:PointerEventInit){super(type,init);this.pointerId=init.pointerId??1;}});
 const {props}=fixture();render(<TextRemovalDialog {...props}/>);await prepared();
 const svg=screen.getByLabelText('Select text to remove');vi.spyOn(svg,'getBoundingClientRect').mockReturnValue({x:0,y:0,left:0,top:0,right:500,bottom:300,width:500,height:300,toJSON:()=>({})});
 fireEvent.pointerDown(svg,{button:0,pointerId:1,clientX:20,clientY:40});fireEvent.pointerMove(svg,{pointerId:1,clientX:600,clientY:400});
 expect(screen.getByTestId('removal-drag')).toHaveAttribute('width','480');expect(screen.getByTestId('removal-drag')).toHaveAttribute('height','260');
 fireEvent.pointerCancel(svg,{pointerId:1});expect(screen.queryByTestId('removal-drag')).not.toBeInTheDocument();expect(screen.getByText('0 of 64 selections')).toBeInTheDocument();
 fireEvent.pointerDown(svg,{button:0,pointerId:2,clientX:20,clientY:40});fireEvent.pointerUp(svg,{pointerId:2,clientX:600,clientY:400});
 expect(screen.getByText('1 of 64 selections')).toBeInTheDocument();expect(screen.getByLabelText('Width (pt)')).toHaveValue(480);
});
it('streams large glyph selections in bounded chunks and refuses a malformed second chunk',async()=>{
 const {props,adapter}=fixture();const base=await adapter.previewSourceTextRemoval('session','working',[]);
 base.removedCharacters=1001;base.pages[0].selectedGlyphCount=1001;vi.mocked(adapter.previewSourceTextRemoval).mockResolvedValue(base);
 vi.mocked(adapter.textRemovalPreviewBounds).mockImplementation(async(_s,_p,_i,offset)=>({pageIndex:0,offset:offset===0?0:999,total:1001,boxes:Array(offset===0?1000:1).fill(rect)}));
 render(<TextRemovalDialog {...props}/>);await prepared();mark();fireEvent.click(screen.getByRole('button',{name:'Preview removal'}));
 await waitFor(()=>expect(adapter.textRemovalPreviewBounds).toHaveBeenCalledTimes(2));expect(adapter.textRemovalPreviewBounds).toHaveBeenLastCalledWith('session','preview',0,1000,1000);
 expect(await screen.findByRole('alert')).toHaveTextContent('invalid');expect(screen.getByRole('button',{name:'Apply removal'})).toBeDisabled();expect(draw).toHaveBeenCalledTimes(1000);
 fireEvent.click(screen.getByRole('button',{name:'Next page'}));await prepared();
 const nextCandidate=screen.queryByAltText('Page after text removal');if(nextCandidate)fireEvent.load(nextCandidate);
 await waitFor(()=>expect(screen.getByLabelText('Removal page')).toHaveValue(2));
 expect(screen.getByRole('button',{name:'Apply removal'})).toBeDisabled();expect(screen.queryByAltText('Page after text removal')).not.toBeInTheDocument();expect(adapter.applyTextRemovalPreview).not.toHaveBeenCalled();
});
it('refreshes an expired working copy from the captured plan and retains validated marks',async()=>{
 const {props,adapter}=fixture(['metadata']);render(<TextRemovalDialog {...props}/>);await prepared();mark();await preview();fireEvent.click(screen.getByRole('checkbox'));
 fireEvent.click(screen.getByRole('button',{name:'Refresh working copy'}));await waitFor(()=>expect(adapter.prepareTextRemovalSource).toHaveBeenCalledTimes(2));await prepared();
 expect(screen.queryByAltText('Page after text removal')).not.toBeInTheDocument();expect(screen.queryByRole('checkbox')).not.toBeInTheDocument();expect(screen.getByText('1 of 64 selections')).toBeInTheDocument();expect(screen.getByRole('button',{name:'Apply removal'})).toBeDisabled();expect(adapter.releaseTextRemovalSession).toHaveBeenCalledWith('session');
});
it('releases the abandoned StrictMode session and cancels without adopting late work',async()=>{
 const {props,adapter}=fixture();vi.mocked(adapter.beginTextRemovalSession).mockResolvedValueOnce('first').mockResolvedValue('second');
 const view=render(<StrictMode><TextRemovalDialog {...props}/></StrictMode>);await prepared();
 expect(adapter.releaseTextRemovalSession).toHaveBeenCalledWith('first');expect(adapter.prepareTextRemovalSource).toHaveBeenCalledTimes(1);
 view.unmount();await waitFor(()=>expect(adapter.releaseTextRemovalSession).toHaveBeenCalledWith('second'));expect(props.onApply).not.toHaveBeenCalled();
});
it('expires the displayed preview and disables Apply until a fresh preview',async()=>{
 const {props,adapter}=fixture();const base=await adapter.previewSourceTextRemoval('session','working',[]);base.expiresInSeconds=1;vi.mocked(adapter.previewSourceTextRemoval).mockResolvedValue(base);
 render(<TextRemovalDialog {...props}/>);await prepared();mark();await preview();await waitFor(()=>expect(screen.getByRole('button',{name:'Apply removal'})).toBeEnabled());
 await waitFor(()=>expect(screen.getByRole('button',{name:'Apply removal'})).toBeDisabled(),{timeout:2500});expect(screen.getByRole('alert')).toHaveTextContent('expired');
});
it('does not apply an old preview while a new pointer selection is in progress',async()=>{
 vi.stubGlobal('PointerEvent',class extends MouseEvent{pointerId:number;constructor(type:string,init:PointerEventInit){super(type,init);this.pointerId=init.pointerId??1;}});
 const {props}=fixture();render(<TextRemovalDialog {...props}/>);await prepared();mark();await preview();await waitFor(()=>expect(screen.getByRole('button',{name:'Apply removal'})).toBeEnabled());
 const svg=screen.getByLabelText('Select text to remove');vi.spyOn(svg,'getBoundingClientRect').mockReturnValue({x:0,y:0,left:0,top:0,right:500,bottom:300,width:500,height:300,toJSON:()=>({})});
 fireEvent.pointerDown(svg,{button:0,pointerId:1,clientX:20,clientY:40});expect(screen.getByRole('button',{name:'Apply removal'})).toBeDisabled();expect(screen.getByRole('button',{name:'Preview removal'})).toBeDisabled();
 fireEvent.pointerCancel(svg,{pointerId:1});expect(screen.getByRole('button',{name:'Apply removal'})).toBeEnabled();
});
