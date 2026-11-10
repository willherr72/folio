import {cleanup,fireEvent,render,screen,waitFor,act} from '@testing-library/react';
import {beforeEach,afterEach,it,expect,vi} from 'vitest';
import {App} from '../src/App';
import {nativeAdapter} from '../src/editor/adapter';
import {clearSearchTextCache} from '../src/editor/search';
import {clearRenderCache} from '../src/components/PageView';
const back=()=>screen.getByRole('button',{name:'Back to previous view'});
const forward=()=>screen.getByRole('button',{name:'Forward to next view'});
function rect(left:number,top:number,width:number,height:number):DOMRect{return {left,top,width,height,right:left+width,bottom:top+height,x:left,y:top,toJSON(){}};}
beforeEach(()=>{
 localStorage.clear();
 vi.spyOn(nativeAdapter,'openPdf').mockResolvedValue({id:'nav-source',name:'Navigation.pdf',pages:Array.from({length:3},()=>({width:600,height:1000})),bookmarks:[{id:'end',title:'Ending',level:0,destination:{kind:'array',value:[{kind:'page',value:'2'},{kind:'name',value:[70,105,116]}]}}]});
 vi.spyOn(nativeAdapter,'renderPage').mockResolvedValue('data:image/png;base64,');
 vi.spyOn(nativeAdapter,'closeDocument').mockResolvedValue();
 vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,'getPageText').mockResolvedValue({characters:[..."Needle"].map((text,i)=>({text,x:40+i*6,y:300,width:6,height:12}))});
 vi.spyOn(HTMLElement.prototype,'getBoundingClientRect').mockImplementation(function(this:HTMLElement){
  if(this.classList.contains('document-viewport'))return rect(0,0,500,400);
  if(this.classList.contains('document-page')){
   const host=this.closest('.document-viewport')!;let top=38;
   for(const slot of host.querySelectorAll<HTMLElement>('.document-page')){if(slot===this)break;top+=parseFloat(slot.style.height)+24;}
   return rect(42-host.scrollLeft,top-host.scrollTop,parseFloat(this.style.width),parseFloat(this.style.height));
  }
  return rect(0,0,0,0);
 });
});
afterEach(()=>{cleanup();clearSearchTextCache(['nav-source','nav-other']);clearRenderCache(['nav-source','nav-other']);vi.restoreAllMocks();});
async function open(){render(<App initialDemo={false}/>);fireEvent.click(screen.getByRole('button',{name:'Open a PDF'}));await screen.findByRole('tab',{name:'Navigation.pdf'});}
it('returns after bookmark and page jumps with reading position/zoom, without editing undo or dirty changes',async()=>{
 await open();expect(back()).toBeDisabled();
 const host=screen.getByRole('main',{name:'Document'});host.scrollTop=280;fireEvent.scroll(host);
 fireEvent.click(screen.getByRole('button',{name:'Bookmarks'}));fireEvent.click(screen.getByRole('treeitem',{name:/Ending/}));
 expect(screen.getByText('Page 3 of 3')).toBeInTheDocument();expect(back()).toBeEnabled();
 fireEvent.click(screen.getByRole('button',{name:'Zoom in'}));fireEvent.click(back());
 expect(screen.getByText('Page 1 of 3')).toBeInTheDocument();expect(host.scrollTop).toBeCloseTo(280);expect(screen.getByTitle(/Reset zoom/)).toHaveTextContent('90%');
 expect(screen.getByRole('button',{name:'Undo'})).toBeDisabled();expect(screen.queryByLabelText('Unsaved changes')).not.toBeInTheDocument();
 fireEvent.keyDown(window,{key:'ArrowRight',altKey:true});expect(screen.getByText('Page 3 of 3')).toBeInTheDocument();expect(screen.getByTitle(/Reset zoom/)).toHaveTextContent('100%');
 fireEvent.keyDown(window,{key:'ArrowLeft',altKey:true});fireEvent.click(screen.getByRole('button',{name:'Next page'}));expect(screen.getByText('Page 2 of 3')).toBeInTheDocument();expect(forward()).toBeDisabled();
});
it('isolates tabs, skips deleted pages, and leaves edit undo usable',async()=>{
 await open();fireEvent.click(screen.getByRole('button',{name:'Next page'}));fireEvent.click(screen.getByRole('button',{name:'Next page'}));
 vi.mocked(nativeAdapter.openPdf).mockResolvedValue({id:'nav-other',name:'Other.pdf',pages:[{width:600,height:1000}]});
 fireEvent.click(screen.getByRole('button',{name:'Open'}));await screen.findByRole('tab',{name:'Other.pdf'});expect(back()).toBeDisabled();
 fireEvent.click(screen.getByRole('tab',{name:'Navigation.pdf'}));expect(back()).toBeEnabled();fireEvent.click(back());
 expect(screen.getByText('Page 2 of 3')).toBeInTheDocument();fireEvent.click(screen.getByRole('button',{name:'Delete page'}));
 expect(screen.getByText(/Page \d of 2/)).toBeInTheDocument();fireEvent.click(forward());expect(screen.getByText('Page 2 of 2')).toBeInTheDocument();
 fireEvent.click(back());expect(screen.getByText('Page 1 of 2')).toBeInTheDocument();
 fireEvent.click(screen.getByRole('button',{name:'Undo'}));expect(screen.getByText(/Page \d of 3/)).toBeInTheDocument();
});
it('dismisses Find on return, and does not intercept typing or modal shortcuts',async()=>{
 await open();fireEvent.click(screen.getByRole('button',{name:'Next page'}));
 fireEvent.click(screen.getByRole('button',{name:'Find'}));const input=screen.getByRole('searchbox');
 fireEvent.keyDown(input,{key:'ArrowLeft',altKey:true});expect(screen.getByText('Page 2 of 3')).toBeInTheDocument();
 fireEvent.change(input,{target:{value:'Needle'}});await screen.findByText('1 of 3');
 fireEvent.click(back());expect(screen.queryByRole('searchbox')).not.toBeInTheDocument();expect(screen.getByText('Page 2 of 3')).toBeInTheDocument();
 await act(async()=>{await new Promise(resolve=>setTimeout(resolve,250));});expect(screen.getByText('Page 2 of 3')).toBeInTheDocument();
 fireEvent.click(screen.getByRole('button',{name:'Go to page'}));expect(back()).toBeDisabled();fireEvent.keyDown(window,{key:'ArrowLeft',altKey:true});expect(screen.getByText('Page 2 of 3')).toBeInTheDocument();
});
it('does not repeat an already followed search result after a modal or tab switch',async()=>{
 await open();fireEvent.click(screen.getByRole('button',{name:'Find'}));fireEvent.change(screen.getByRole('searchbox'),{target:{value:'Needle'}});await screen.findByText('1 of 3');
 const host=screen.getByRole('main',{name:'Document'});host.scrollTop=480;fireEvent.scroll(host);
 fireEvent.click(screen.getByRole('button',{name:'Settings'}));fireEvent.click(screen.getByRole('button',{name:'Done'}));
 expect(host.scrollTop).toBe(480);
 vi.mocked(nativeAdapter.openPdf).mockResolvedValue({id:'nav-other',name:'Other.pdf',pages:[{width:600,height:1000}]});
 fireEvent.click(screen.getByRole('button',{name:'Open'}));await screen.findByRole('tab',{name:'Other.pdf'});
 fireEvent.click(screen.getByRole('tab',{name:'Navigation.pdf'}));expect(screen.getByRole('main',{name:'Document'}).scrollTop).toBe(480);
});
it('does not create a Back entry for an unchanged view spanning several short pages',async()=>{
 vi.mocked(nativeAdapter.openPdf).mockResolvedValue({id:'nav-source',name:'Navigation.pdf',pages:Array.from({length:4},()=>({width:600,height:100}))});
 await open();const host=screen.getByRole('main',{name:'Document'});expect(host.scrollTop).toBe(0);
 fireEvent.click(screen.getByRole('button',{name:'Page 1 of 4'}));expect(host.scrollTop).toBe(0);expect(back()).toBeDisabled();
});
it('follows a query re-entered after a no-match edit, without replaying modal/tab returns',async()=>{
 await open();fireEvent.click(screen.getByRole('button',{name:'Find'}));const input=screen.getByRole('searchbox');
 fireEvent.change(input,{target:{value:'Needle'}});await screen.findByText('1 of 3');
 const host=screen.getByRole('main',{name:'Document'});host.scrollTop=1600;fireEvent.scroll(host);
 fireEvent.change(input,{target:{value:'Needlex'}});await screen.findByText('No matches');
 fireEvent.change(input,{target:{value:'Needle'}});await screen.findByText('1 of 3');
 expect(screen.getByText('Page 1 of 3')).toBeInTheDocument();expect(host.scrollTop).toBeLessThan(500);
});
it('unavailable history shortcuts preserve Find and the active editing tool',async()=>{
 await open();fireEvent.click(screen.getByRole('button',{name:'Find'}));expect(back()).toBeDisabled();
 fireEvent.keyDown(window,{key:'ArrowLeft',altKey:true});expect(screen.getByRole('searchbox')).toBeInTheDocument();
 fireEvent.click(screen.getByRole('button',{name:'Close search'}));fireEvent.click(screen.getByRole('button',{name:'Draw'}));
 fireEvent.keyDown(window,{key:'ArrowRight',altKey:true});expect(screen.getByRole('button',{name:'Draw'})).toHaveClass('active');
});
