import {cleanup,render,waitFor} from "@testing-library/react";
import {afterEach,expect,it,vi} from "vitest";
import {PageView,clearRenderCache} from "../src/components/PageView";
import {createDemoAdapter} from "../src/editor/adapter";
import type {CanvasParagraphView} from "../src/editor/paragraph-canvas";
import type {PagePlan} from "../src/editor/types";

const page:PagePlan={id:"page",sourceId:"raster-identity-source",pageIndex:0,width:300,height:400,rotation:0,overlays:[]};
afterEach(()=>{cleanup();clearRenderCache([page.sourceId]);vi.restoreAllMocks();});
it("keeps the mounted image across paragraph loading, ready, error and source restoration without stale paint",async()=>{
 const adapter={...createDemoAdapter(),renderPage:vi.fn().mockResolvedValue("data:image/png;base64,original"),getPageText:vi.fn().mockResolvedValue({characters:[]})};
 const props={adapter,page,pageNumber:1,zoom:100,tool:"select" as const,selectedOverlayId:null,pendingSignature:null,onSelectOverlay:vi.fn(),onAddText:vi.fn(),onPlaceSignature:vi.fn(),onMoveOverlay:vi.fn()};
 const view=render(<PageView {...props}/>);
 await waitFor(()=>expect(view.container.querySelector("image")).toHaveAttribute("href","data:image/png;base64,original"));
 const image=view.container.querySelector("image"),svg=view.container.querySelector("svg");
 const pending:CanvasParagraphView={pageId:page.id,sessionKey:"edit",runs:[],selected:[],enabled:false,choosing:false,raster:{loading:true},onChange:vi.fn(),onToggle:vi.fn(),onGestureActive:vi.fn()};
 view.rerender(<PageView {...props} canvasParagraph={pending}/>);
 expect(view.container.querySelector("image")).toBe(image);
 expect(image).not.toHaveAttribute("href");expect(view.container.querySelector(".page-stage")).toHaveAttribute("aria-busy","true");
 const ready={...pending,raster:{url:"data:image/png;base64,edited",loading:false}};
 view.rerender(<PageView {...props} canvasParagraph={ready}/>);
 expect(view.container.querySelector("image")).toBe(image);expect(image).toHaveAttribute("href","data:image/png;base64,edited");
 view.rerender(<PageView {...props} canvasParagraph={{...pending,raster:{error:"Rendering refused",loading:false}}}/>);
 expect(view.getByRole("alert")).toHaveTextContent("Rendering refused");expect(view.container.querySelector("image")).toBe(image);expect(image).not.toHaveAttribute("href");
 view.rerender(<PageView {...props}/>);
 await waitFor(()=>expect(image).toHaveAttribute("href","data:image/png;base64,original"));
 expect(view.container.querySelector("image")).toBe(image);expect(view.container.querySelector("svg")).toBe(svg);
});
