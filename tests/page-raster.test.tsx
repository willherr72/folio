import {act,cleanup,render,fireEvent} from "@testing-library/react";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {createRasterPainter} from "../src/editor/pageRaster";
import {PageRaster} from "../src/components/PageRaster";

const {Blob:NodeBlob}=await vi.importActual<{Blob:typeof Blob}>("node:buffer");

const deferred=<T,>()=>{let resolve!:(value:T)=>void,reject!:(error:unknown)=>void;const promise=new Promise<T>((a,b)=>{resolve=a;reject=b;});return {promise,resolve,reject};};
function png(width=2,height=3){
 const bytes=new Uint8Array(33);bytes.set([137,80,78,71,13,10,26,10,0,0,0,13,73,72,68,82]);
 const view=new DataView(bytes.buffer);view.setUint32(16,width);view.setUint32(20,height);bytes[24]=8;bytes[25]=6;return bytes;
}
function response(chunks:Uint8Array[]=[png()]){
 let index=0;const cancel=vi.fn(async()=>{}),releaseLock=vi.fn();
 return {ok:true,headers:{get:()=>null},body:{getReader:()=>({read:async()=>index<chunks.length?{done:false,value:chunks[index++]}:{done:true,value:undefined},cancel,releaseLock})},cancel,releaseLock};
}
const tick=async()=>{await act(async()=>{await new Promise(resolve=>setTimeout(resolve,0));});};
const bitmap=(width=2,height=3)=>({width,height,close:vi.fn()});
let draw:ReturnType<typeof vi.fn>,clear:ReturnType<typeof vi.fn>,fetchMock:ReturnType<typeof vi.fn>,decode:ReturnType<typeof vi.fn>;
beforeEach(()=>{
 draw=vi.fn();clear=vi.fn();fetchMock=vi.fn(async()=>response());decode=vi.fn(async()=>bitmap());
 vi.stubGlobal("Blob",NodeBlob);vi.stubGlobal("fetch",fetchMock);vi.stubGlobal("createImageBitmap",decode);
 vi.spyOn(HTMLCanvasElement.prototype,"getContext").mockReturnValue({drawImage:draw,clearRect:clear} as unknown as CanvasRenderingContext2D);
 vi.stubGlobal("URL",{createObjectURL:vi.fn(),revokeObjectURL:vi.fn()});
});
afterEach(()=>{cleanup();vi.restoreAllMocks();vi.unstubAllGlobals();});

it("serializes decodes, closes stale A-B-A results and paints only the latest request",async()=>{
 const first=deferred<ReturnType<typeof bitmap>>();decode.mockImplementationOnce(()=>first.promise);
 const canvas=document.createElement("canvas"),notify=vi.fn(),painter=createRasterPainter(canvas,notify);
 painter.setSource("blob:a");await tick();expect(decode).toHaveBeenCalledTimes(1);
 painter.setSource("blob:b");painter.setSource("blob:a");await tick();expect(decode).toHaveBeenCalledTimes(1);
 const stale=bitmap();first.resolve(stale);await tick();await tick();
 expect(stale.close).toHaveBeenCalledOnce();expect(decode).toHaveBeenCalledTimes(2);expect(draw).toHaveBeenCalledTimes(1);
 expect(notify.mock.calls.at(-1)?.[0]).toEqual({url:"blob:a",kind:"canvas"});
 expect(URL.revokeObjectURL).not.toHaveBeenCalled();painter.dispose();
});
it("aborts fetch and closes a late bitmap after unmount without painting or reporting",async()=>{
 const pending=deferred<ReturnType<typeof bitmap>>();decode.mockReturnValue(pending.promise);
 const canvas=document.createElement("canvas"),notify=vi.fn(),painter=createRasterPainter(canvas,notify);
 painter.setSource("blob:a");await tick();const signal=fetchMock.mock.calls[0][1].signal;
 painter.dispose();const count=notify.mock.calls.length;const late=bitmap();pending.resolve(late);await tick();
 expect(signal.aborted).toBe(true);expect(late.close).toHaveBeenCalledOnce();expect(draw).not.toHaveBeenCalled();
 expect(notify).toHaveBeenCalledTimes(count);expect(canvas.width*canvas.height).toBe(0);
});
it.each([[0,3],[16385,1],[4097,4096]])("refuses unsafe PNG dimensions %s x %s before bitmap decode",async(width,height)=>{
 fetchMock.mockResolvedValue(response([png(width,height)]));const notify=vi.fn(),painter=createRasterPainter(document.createElement("canvas"),notify);
 painter.setSource("blob:large");await tick();expect(decode).not.toHaveBeenCalled();expect(notify.mock.calls.at(-1)?.[0].kind).toBe("fallback");painter.dispose();
});
it("bounds streamed encoded bytes, cancels the reader and falls back before decode",async()=>{
 const result=response([new Uint8Array(32*1024*1024),new Uint8Array(1)]);fetchMock.mockResolvedValue(result);
 const notify=vi.fn(),painter=createRasterPainter(document.createElement("canvas"),notify);painter.setSource("blob:large");await tick();
 expect(result.cancel).toHaveBeenCalled();expect(result.releaseLock).toHaveBeenCalled();expect(decode).not.toHaveBeenCalled();
 expect(notify.mock.calls.at(-1)?.[0].kind).toBe("fallback");painter.dispose();
});
it("rejects non-PNG and mismatched decoded dimensions, closing the bitmap",async()=>{
 const notify=vi.fn(),painter=createRasterPainter(document.createElement("canvas"),notify);
 fetchMock.mockResolvedValueOnce(response([new Uint8Array(33)]));painter.setSource("blob:jpeg");await tick();expect(decode).not.toHaveBeenCalled();
 const wrong=bitmap(20,30);decode.mockResolvedValueOnce(wrong);painter.setSource("blob:wrong");await tick();
 expect(wrong.close).toHaveBeenCalledOnce();expect(draw).not.toHaveBeenCalled();expect(notify.mock.calls.at(-1)?.[0].kind).toBe("fallback");painter.dispose();
});
it("reuses backing dimensions, clears transparent remnants and avoids borrowed URL revocation",async()=>{
 const canvas=document.createElement("canvas"),width=vi.spyOn(canvas,"width","set"),height=vi.spyOn(canvas,"height","set");
 const painter=createRasterPainter(canvas,vi.fn());painter.setSource("blob:a");await tick();painter.setSource("blob:b");await tick();painter.setSource("blob:b");await tick();
 expect(draw).toHaveBeenCalledTimes(2);expect(clear).toHaveBeenCalledTimes(2);expect(width).toHaveBeenCalledTimes(1);expect(height).toHaveBeenCalledTimes(1);
 expect(clear.mock.invocationCallOrder[1]).toBeLessThan(draw.mock.invocationCallOrder[1]);painter.dispose();expect(URL.revokeObjectURL).not.toHaveBeenCalled();
});
it("keeps a stable pointer-transparent surface, geometry and focus while hiding pending pixels",async()=>{
 const onStatus=vi.fn();const view=render(<><input aria-label="Draft"/><svg><PageRaster url="blob:a" width={100} height={200} transform="rotate(90)" onStatus={onStatus}/><rect data-testid="overlay"/></svg></>);
 await tick();const canvas=view.container.querySelector("canvas")!,host=view.container.querySelector("foreignObject")!;
 expect(host).toHaveAttribute("width","100");expect(host).toHaveAttribute("transform","rotate(90)");expect(host).toHaveStyle({pointerEvents:"none"});
 const input=view.getByRole("textbox");input.focus();const pending=deferred<ReturnType<typeof bitmap>>();decode.mockReturnValueOnce(pending.promise);
 view.rerender(<><input aria-label="Draft"/><svg><PageRaster url="blob:b" width={110} height={220} onStatus={onStatus}/><rect data-testid="overlay"/></svg></>);
 expect(view.container.querySelector("canvas")).toBe(canvas);expect(host).toHaveStyle({visibility:"hidden"});expect(input).toHaveFocus();
 await tick();pending.resolve(bitmap());await tick();expect(host).toHaveStyle({visibility:"visible"});expect(host).not.toHaveAttribute("transform");
 expect(onStatus.mock.calls.at(-1)?.[0]).toEqual({url:"blob:b",phase:"ready",mode:"canvas"});
});
it("uses SVG fallback with load/error status and recovers on a new source",async()=>{
 const onStatus=vi.fn(),view=render(<svg><PageRaster url="data:image/svg+xml,test" width={100} height={200} onStatus={onStatus}/></svg>);
 const image=view.container.querySelector("image")!;expect(image).toHaveAttribute("preserveAspectRatio","none");expect(fetchMock).not.toHaveBeenCalled();
 fireEvent.load(image);expect(onStatus.mock.calls.at(-1)?.[0]).toEqual({url:"data:image/svg+xml,test",phase:"ready",mode:"svg"});
 fireEvent.error(image);expect(onStatus.mock.calls.at(-1)?.[0].phase).toBe("error");
 view.rerender(<svg><PageRaster url="blob:b" width={100} height={200} onStatus={onStatus}/></svg>);await tick();
 expect(view.container.querySelector("image")).toBeNull();expect(onStatus.mock.calls.at(-1)?.[0]).toEqual({url:"blob:b",phase:"ready",mode:"canvas"});
 view.rerender(<svg><PageRaster width={100} height={200} onStatus={onStatus}/></svg>);expect(view.container.querySelector("foreignObject")).toHaveStyle({visibility:"hidden"});expect(onStatus.mock.calls.at(-1)?.[0].phase).toBe("empty");
});

it("does not decode when cancelled during fetch and starts only the newest source",async()=>{
 const first=deferred<ReturnType<typeof response>>();fetchMock.mockReturnValueOnce(first.promise);
 const notify=vi.fn(),painter=createRasterPainter(document.createElement("canvas"),notify);
 painter.setSource("blob:a");painter.setSource("blob:b");expect(fetchMock.mock.calls[0][1].signal.aborted).toBe(true);
 first.resolve(response());await tick();await tick();expect(decode).toHaveBeenCalledTimes(1);expect(draw).toHaveBeenCalledTimes(1);
 expect(notify.mock.calls.at(-1)?.[0]).toEqual({url:"blob:b",kind:"canvas"});painter.dispose();
});
it("falls back without decoding when the context or bitmap API is unavailable",async()=>{
 vi.mocked(HTMLCanvasElement.prototype.getContext).mockReturnValueOnce(null);
 const notify=vi.fn(),painter=createRasterPainter(document.createElement("canvas"),notify);painter.setSource("blob:a");
 expect(fetchMock).not.toHaveBeenCalled();expect(notify.mock.calls.at(-1)?.[0].kind).toBe("fallback");painter.dispose();
 vi.stubGlobal("createImageBitmap",undefined);const second=createRasterPainter(document.createElement("canvas"),notify);second.setSource("blob:b");
 expect(fetchMock).not.toHaveBeenCalled();expect(notify.mock.calls.at(-1)?.[0].kind).toBe("fallback");second.dispose();
});
it("closes a failed paint and recovers from decode rejection on the next source",async()=>{
 const value=bitmap();decode.mockRejectedValueOnce(new Error("corrupt PNG")).mockResolvedValueOnce(value);draw.mockImplementationOnce(()=>{throw new Error("lost surface");});
 const notify=vi.fn(),painter=createRasterPainter(document.createElement("canvas"),notify);painter.setSource("blob:a");await tick();
 expect(notify.mock.calls.at(-1)?.[0].kind).toBe("fallback");painter.setSource("blob:b");await tick();
 expect(value.close).toHaveBeenCalledOnce();expect(notify.mock.calls.at(-1)?.[0].kind).toBe("fallback");
 painter.setSource("blob:c");await tick();expect(notify.mock.calls.at(-1)?.[0].kind).toBe("canvas");painter.dispose();
});
it("rejects an oversized declared length before reading and releases an errored stream",async()=>{
 const cancel=vi.fn(async()=>{}),read=vi.fn();fetchMock.mockResolvedValueOnce({ok:true,headers:{get:()=>String(32*1024*1024+1)},body:{cancel,getReader:()=>({read})}});
 const notify=vi.fn(),painter=createRasterPainter(document.createElement("canvas"),notify);painter.setSource("blob:a");await tick();
 expect(cancel).toHaveBeenCalledOnce();expect(read).not.toHaveBeenCalled();
 const releaseLock=vi.fn();fetchMock.mockResolvedValueOnce({ok:true,headers:{get:()=>null},body:{getReader:()=>({read:async()=>{throw new Error("stream lost");},cancel,releaseLock})}});
 painter.setSource("blob:b");await tick();expect(releaseLock).toHaveBeenCalledOnce();expect(decode).not.toHaveBeenCalled();painter.dispose();
});

it("keeps every intermediate surface within budget when changing tall and wide frames",async()=>{
 const canvas=document.createElement("canvas"),sizes:number[]=[];
 for(const axis of ["width","height"] as const){
  const setter=Object.getOwnPropertyDescriptor(HTMLCanvasElement.prototype,axis)!.set!;
  vi.spyOn(canvas,axis,"set").mockImplementation(value=>{setter.call(canvas,value);sizes.push(canvas.width*canvas.height);});
 }
 fetchMock.mockResolvedValueOnce(response([png(1024,16384)])).mockResolvedValueOnce(response([png(16384,1024)]));
 decode.mockResolvedValueOnce(bitmap(1024,16384)).mockResolvedValueOnce(bitmap(16384,1024));
 const painter=createRasterPainter(canvas,vi.fn());painter.setSource("blob:tall");await tick();painter.setSource("blob:wide");await tick();
 expect(canvas.width).toBe(16384);expect(canvas.height).toBe(1024);expect(Math.max(...sizes)).toBeLessThanOrEqual(16_777_216);painter.dispose();
});
