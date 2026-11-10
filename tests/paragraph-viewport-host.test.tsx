import {cleanup,render,screen} from "@testing-library/react";
import {afterEach,expect,it,vi} from "vitest";
import {ParagraphViewportHost,useParagraphViewport} from "../src/components/ParagraphViewportHost";
import type {CanvasParagraphView} from "../src/editor/paragraph-canvas";

afterEach(cleanup);
const view=(sessionKey="first",pageId="page",url="original"):CanvasParagraphView=>({
 sessionKey,pageId,enabled:true,choosing:false,runs:[],selected:[],raster:{url,loading:false},
 onChange:vi.fn(),onToggle:vi.fn(),onGestureActive:vi.fn(),
});
function Publish({value}:{value:CanvasParagraphView}){useParagraphViewport(value);return null;}
function Viewport({value}:{value?:CanvasParagraphView}){return <output data-testid="viewport">{value?JSON.stringify({session:value.sessionKey,page:value.pageId,url:value.raster.url,enabled:value.enabled,loading:value.raster.loading}):"reading"}</output>;}
const shown=()=>JSON.parse(screen.getByTestId("viewport").textContent!);

it("keeps the viewport node while binding and clearing a session before exposing old paint",()=>{
 const first=view(),second=view("second","another-page","edited"),frames:(CanvasParagraphView|undefined)[]=[];
 const viewport=(value?:CanvasParagraphView)=>{frames.push(value);return <Viewport value={value}/>;};
 const app=render(<ParagraphViewportHost renderViewport={viewport}/>),node=screen.getByTestId("viewport");
 app.rerender(<ParagraphViewportHost session={first} renderViewport={viewport}><Publish value={first}/></ParagraphViewportHost>);
 expect(screen.getByTestId("viewport")).toBe(node);expect(shown().url).toBe("original");
 frames.length=0;
 app.rerender(<ParagraphViewportHost session={second} renderViewport={viewport}><Publish value={first}/></ParagraphViewportHost>);
 expect(shown()).toEqual({session:"second",page:"another-page",enabled:false,loading:true});
 expect(frames.every(frame=>frame?.sessionKey==="second"&&!frame.raster.url&&!frame.enabled)).toBe(true);
 app.rerender(<ParagraphViewportHost session={second} renderViewport={viewport}><Publish value={second}/></ParagraphViewportHost>);
 expect(shown().url).toBe("edited");expect(screen.getByTestId("viewport")).toBe(node);
 frames.length=0;
 app.rerender(<ParagraphViewportHost renderViewport={viewport}><Publish value={second}/></ParagraphViewportHost>);
 expect(screen.getByTestId("viewport")).toHaveTextContent("reading");expect(frames.every(frame=>frame===undefined)).toBe(true);
});

it("rejects a different page even when its session key matches",()=>{
 const active=view(),wrong=view("first","different-page","stale");
 render(<ParagraphViewportHost session={active} renderViewport={value=><Viewport value={value}/>}><Publish value={wrong}/></ParagraphViewportHost>);
 expect(shown()).toEqual({session:"first",page:"page",enabled:false,loading:true});
});

it("does not let a retiring owner clear a newer publication",()=>{
 const first=view(),second=view("first","page","newer"),viewport=(value?:CanvasParagraphView)=><Viewport value={value}/>;
 const app=render(<ParagraphViewportHost session={first} renderViewport={viewport}><Publish key="old" value={first}/><Publish key="new" value={second}/></ParagraphViewportHost>);
 expect(shown().url).toBe("newer");
 app.rerender(<ParagraphViewportHost session={first} renderViewport={viewport}><Publish key="new" value={second}/></ParagraphViewportHost>);
 expect(shown().url).toBe("newer");
 app.rerender(<ParagraphViewportHost session={first} renderViewport={viewport}/>);
 expect(shown()).toEqual({session:"first",page:"page",enabled:false,loading:true});
});
