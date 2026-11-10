import {beforeEach,describe,expect,it,vi} from "vitest";
const {invoke}=vi.hoisted(()=>({invoke:vi.fn()}));
vi.mock("@tauri-apps/api/core",()=>({invoke}));
const id="a".repeat(64);
const info={id,width:12,height:8,mimeType:"image/png",hasAlpha:true,bitsPerChannel:8,byteLength:100};
const registration=(token:string)=>({image:info,token});
function deferred<T>() {let resolve!:(value:T)=>void;let reject!:(error:Error)=>void;const promise=new Promise<T>((a,b)=>{resolve=a;reject=b;});return {promise,resolve,reject};}
beforeEach(()=>{vi.resetModules();invoke.mockReset();});

describe("independent image acquisition ownership",()=>{
 it("retains one workspace acquisition across repeated renders and releases only its token",async()=>{
  invoke.mockImplementation(async(command:string)=>command==="acquire_image"?registration("workspace"):undefined);
  const images=await import("../src/editor/image-resources");
  await images.retainImages(new Set([id]));await images.retainImages(new Set([id]));
  expect(invoke.mock.calls.filter(([command])=>command==="acquire_image")).toHaveLength(1);
  expect(images.imageResourceState(id)?.status).toBe("ready");
  await images.retainImages(new Set());await images.flushImageReleases();
  expect(invoke).toHaveBeenCalledWith("release_image",{token:"workspace"});
  expect(images.imageResourceState(id)).toBeUndefined();
 });
 it("a canceled old acquisition cannot release the newer owner of the same image",async()=>{
  const old=deferred<ReturnType<typeof registration>>(),fresh=deferred<ReturnType<typeof registration>>();let calls=0;
  invoke.mockImplementation((command:string)=>command==="acquire_image"?(calls++?fresh.promise:old.promise):Promise.resolve());
  const images=await import("../src/editor/image-resources");
  const first=images.retainImages(new Set([id]));await images.retainImages(new Set());
  const second=images.retainImages(new Set([id]));fresh.resolve(registration("new"));await second;
  old.resolve(registration("old"));await first;await images.drainImageAcquisitions();await images.flushImageReleases();
  expect(invoke).toHaveBeenCalledWith("release_image",{token:"old"});
  expect(invoke).not.toHaveBeenCalledWith("release_image",{token:"new"});
  expect(images.imageResourceState(id)?.status).toBe("ready");
  await images.retainImages(new Set());await images.flushImageReleases();
 });
 it("pending dialogs and workspace history have independent leases",async()=>{
  let count=0;invoke.mockImplementation(async(command:string)=>command==="acquire_image"?registration(`lease-${++count}`):undefined);
  const images=await import("../src/editor/image-resources");const pending=await images.acquireImageLease(id);
  await images.retainImages(new Set([id]));await pending.release();await pending.release();
  expect(invoke.mock.calls.filter(([command,args])=>command==="release_image"&&args.token==="lease-1")).toHaveLength(1);
  expect(invoke).not.toHaveBeenCalledWith("release_image",{token:"lease-2"});
  await images.retainImages(new Set());await images.flushImageReleases();
 });
 it("reports acquisition failures and supports an explicit retry",async()=>{
  let attempt=0;invoke.mockImplementation(async(command:string)=>{if(command==="acquire_image"){if(!attempt++)throw new Error("capacity");return registration("retry");}});
  const images=await import("../src/editor/image-resources");
  await expect(images.retainImages(new Set([id]))).rejects.toThrow("capacity");
  expect(images.imageResourceState(id)).toMatchObject({status:"error",error:"capacity"});
  await images.retainImages(new Set([id]));expect(images.imageResourceState(id)?.status).toBe("ready");
  await images.retainImages(new Set());await images.flushImageReleases();
 });
 it("drains pending acquisition before a source can close",async()=>{
  const pending=deferred<ReturnType<typeof registration>>();invoke.mockReturnValue(pending.promise);
  const images=await import("../src/editor/image-resources");const owner=images.retainImages(new Set([id]));
  let drained=false;const drain=images.drainImageAcquisitions().then(()=>{drained=true;});
  await Promise.resolve();expect(drained).toBe(false);
  pending.resolve(registration("held"));await owner;await drain;expect(drained).toBe(true);
  invoke.mockResolvedValue(undefined);await images.retainImages(new Set());await images.flushImageReleases();
 });
 it("imports an independently releasable pending image and handles picker cancellation",async()=>{
  invoke.mockResolvedValueOnce(null).mockResolvedValueOnce(registration("picked")).mockResolvedValue(undefined);
  const images=await import("../src/editor/image-resources");expect(await images.importImageLease()).toBeNull();
  const chosen=await images.importImageLease();expect(chosen?.registration.image).toEqual(info);
  await chosen!.release();expect(invoke).toHaveBeenCalledWith("release_image",{token:"picked"});
 });
 it("does not invoke the backend for invalid content identifiers",async()=>{
  const images=await import("../src/editor/image-resources");await expect(images.acquireImageLease("../image")).rejects.toThrow("identifier");expect(invoke).not.toHaveBeenCalled();
 });
 it("transfers the pending import into history without a second native acquisition",async()=>{
  invoke.mockImplementation(async(command:string)=>command==="import_image"?registration("picked"):undefined);
  const images=await import("../src/editor/image-resources");const picked=await images.importImageLease();
  images.adoptImageLease(picked!);await images.retainImages(new Set([id]));
  expect(invoke).not.toHaveBeenCalledWith("acquire_image",expect.anything());expect(invoke).not.toHaveBeenCalledWith("release_image",expect.anything());
  expect(images.imageResourceState(id)?.status).toBe("ready");
  await images.retainImages(new Set());await images.flushImageReleases();expect(invoke.mock.calls.filter(([name])=>name==="release_image")).toEqual([["release_image",{token:"picked"}]]);
 });
 it("releases a duplicate imported lease while keeping the workspace owner",async()=>{
  invoke.mockImplementation(async(command:string)=>command==="acquire_image"?registration("workspace"):command==="import_image"?registration("picked"):undefined);
  const images=await import("../src/editor/image-resources");await images.retainImages(new Set([id]));const picked=await images.importImageLease();
  images.adoptImageLease(picked!);await images.flushImageReleases();expect(invoke).toHaveBeenCalledWith("release_image",{token:"picked"});expect(invoke).not.toHaveBeenCalledWith("release_image",{token:"workspace"});
  await images.retainImages(new Set());await images.flushImageReleases();
 });
 it("adopts an import while an older workspace acquisition is still pending",async()=>{
  const old=deferred<ReturnType<typeof registration>>();invoke.mockImplementation((command:string)=>command==="acquire_image"?old.promise:Promise.resolve(command==="import_image"?registration("picked"):undefined));
  const images=await import("../src/editor/image-resources");const pending=images.retainImages(new Set([id]));images.adoptImageLease((await images.importImageLease())!);
  old.resolve(registration("old"));await pending;await images.flushImageReleases();
  expect(invoke).toHaveBeenCalledWith("release_image",{token:"old"});expect(invoke).not.toHaveBeenCalledWith("release_image",{token:"picked"});expect(images.imageResourceState(id)?.status).toBe("ready");
  await images.retainImages(new Set());await images.flushImageReleases();
 });
});
