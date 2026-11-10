import {act,cleanup,fireEvent,render,screen} from "@testing-library/react";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {useActiveTabVisibility} from "../src/editor/use-active-tab-visibility";
let observers:{callback:()=>void;observe:ReturnType<typeof vi.fn>;disconnect:ReturnType<typeof vi.fn>}[];
let width:number,offsets:number[],tabWidth:number;
function Harness({active="a",count=3,revision=0}:{active?:string|null;count?:number;revision?:number}){
 const ref=useActiveTabVisibility(active,count);
 return <><input aria-label="Editor" defaultValue="Keep my draft"/><div data-testid="strip" ref={ref}>
  {Array.from({length:count},(_,i)=><div key={i} data-index={i} className={`document-tab ${String.fromCharCode(97+i)===active?"active":""}`}><button>{String.fromCharCode(97+i)}</button><button>Close</button></div>)}
 </div><output>{revision}</output></>;
}
const rect=(left:number,width:number)=>({x:left,y:20,left,right:left+width,top:20,bottom:60,width,height:40,toJSON(){}});
beforeEach(()=>{
 width=200;offsets=[0,120,240];tabWidth=115;observers=[];
 vi.stubGlobal("ResizeObserver",class{callback:()=>void;observe=vi.fn();disconnect=vi.fn();constructor(callback:()=>void){this.callback=callback;observers.push(this);}});
 vi.spyOn(HTMLElement.prototype,"clientWidth","get").mockImplementation(function(this:HTMLElement){return this.dataset.testid==="strip"?width:0;});
 vi.spyOn(HTMLElement.prototype,"scrollWidth","get").mockImplementation(function(this:HTMLElement){return this.dataset.testid==="strip"?500:0;});
 vi.spyOn(HTMLElement.prototype,"getBoundingClientRect").mockImplementation(function(this:HTMLElement){const el=this;return el.dataset.testid==="strip"?rect(10,width):el.dataset.index!==undefined?rect(10+offsets[Number(el.dataset.index)]-(el.parentElement?.scrollLeft??0),tabWidth):rect(0,0);});
});
afterEach(()=>{cleanup();vi.restoreAllMocks();vi.unstubAllGlobals();});
it("reveals the full active wrapper on selection and resize without changing focus or vertical scroll",()=>{
 const view=render(<Harness/>);const strip=screen.getByTestId("strip");const editor=screen.getByRole("textbox");editor.focus();strip.scrollTop=9;
 view.rerender(<Harness active="c"/>);expect(strip.scrollLeft).toBe(155);expect(editor).toHaveFocus();expect(editor).toHaveValue("Keep my draft");expect(strip.scrollTop).toBe(9);
 width=150;act(()=>observers.at(-1)!.callback());expect(strip.scrollLeft).toBe(205);expect(editor).toHaveFocus();
 view.rerender(<Harness active="a"/>);expect(strip.scrollLeft).toBe(0);
});
it("leaves intentional manual scrolling alone during ordinary edits and ignores stale observers",()=>{
 const view=render(<Harness/>);const strip=screen.getByTestId("strip"),first=observers[0];strip.scrollLeft=200;fireEvent.scroll(strip);
 view.rerender(<Harness revision={1}/>);expect(strip.scrollLeft).toBe(200);expect(observers).toHaveLength(1);
 view.rerender(<Harness active="c"/>);const current=strip.scrollLeft;act(()=>first.callback());expect(strip.scrollLeft).toBe(current);expect(first.disconnect).toHaveBeenCalledOnce();
 view.unmount();expect(observers.at(-1)!.disconnect).toHaveBeenCalledOnce();
});
it("handles empty/restored workspaces, narrow strips and resize without ResizeObserver",()=>{
 vi.stubGlobal("ResizeObserver",undefined);const view=render(<Harness active={null} count={0}/>);view.rerender(<Harness active="c"/>);const strip=screen.getByTestId("strip");expect(strip.scrollLeft).toBe(155);
 width=80;fireEvent(window,new Event("resize"));expect(strip.scrollLeft).toBe(240);fireEvent(window,new Event("resize"));expect(strip.scrollLeft).toBe(240);
 width=0;strip.scrollLeft=100;fireEvent(window,new Event("resize"));expect(strip.scrollLeft).toBe(100);
 width=200;offsets=[0,120,160];view.rerender(<Harness active="c" count={4}/>);expect(strip.scrollLeft).toBe(100);
});
