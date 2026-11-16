import {expect,it,vi} from "vitest";
import {awaitOrCancel} from "../src/editor/await-or-cancel";

function pending<T>(){
 let resolve!:(value:T)=>void,reject!:(error:unknown)=>void;
 const promise=new Promise<T>((yes,no)=>{resolve=yes;reject=no;});
 return {promise,resolve,reject};
}
it("preserves normal values and errors without disposing a transferred result",async()=>{
 const controller=new AbortController(),dispose=vi.fn();
 expect(await awaitOrCancel(Promise.resolve(42),controller.signal,dispose)).toEqual({cancelled:false,value:42});
 controller.abort();expect(dispose).not.toHaveBeenCalled();
 const error=new Error("open failed");
 await expect(awaitOrCancel(Promise.reject(error),new AbortController().signal,dispose)).rejects.toBe(error);
});
it("returns on cancellation before pending work or late disposal completes",async()=>{
 const work=pending<string>(),cleanup=pending<void>(),controller=new AbortController();
 const dispose=vi.fn(()=>cleanup.promise),result=awaitOrCancel(work.promise,controller.signal,dispose);
 controller.abort();expect(await result).toEqual({cancelled:true});
 work.resolve("late source");await vi.waitFor(()=>expect(dispose).toHaveBeenCalledWith("late source"));
 expect(dispose).toHaveBeenCalledTimes(1);cleanup.resolve();
});
it("disposes an already-resolved value when cancellation wins delivery",async()=>{
 const controller=new AbortController(),dispose=vi.fn();
 const result=awaitOrCancel(Promise.resolve("not installed"),controller.signal,dispose);
 controller.abort();expect(await result).toEqual({cancelled:true});
 await vi.waitFor(()=>expect(dispose).toHaveBeenCalledWith("not installed"));expect(dispose).toHaveBeenCalledTimes(1);
});
it("handles an already-aborted signal and consumes late failures",async()=>{
 const controller=new AbortController(),work=pending<string>(),dispose=vi.fn();controller.abort();
 expect(await awaitOrCancel(work.promise,controller.signal,dispose)).toEqual({cancelled:true});
 work.reject(new Error("late native rejection"));await Promise.resolve();await Promise.resolve();
 expect(dispose).not.toHaveBeenCalled();
});
it("does not turn a cancelled result into an unhandled cleanup rejection",async()=>{
 const controller=new AbortController(),work=pending<string>();
 const dispose=vi.fn(async()=>{throw new Error("worker already stopped");});
 const result=awaitOrCancel(work.promise,controller.signal,dispose);controller.abort();
 expect(await result).toEqual({cancelled:true});work.resolve("late");
 await vi.waitFor(()=>expect(dispose).toHaveBeenCalledTimes(1));
});
