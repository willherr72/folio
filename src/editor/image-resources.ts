import {invoke} from "@tauri-apps/api/core";

export interface ImageInfo {
  id:string;width:number;height:number;mimeType:string;hasAlpha:boolean;bitsPerChannel:number;byteLength:number;
}
export interface ImageRegistration {image:ImageInfo;token:string}
export interface ImageLease {registration:ImageRegistration;release():Promise<void>}
export interface ImageResourceState {status:"loading"|"ready"|"error";info?:ImageInfo;error?:string}
export const imageApi={
  importImage:()=>invoke<ImageRegistration|null>("import_image"),
  acquire:(id:string)=>invoke<ImageRegistration>("acquire_image",{id}),
  release:(token:string)=>invoke<void>("release_image",{token}),
  info:(id:string)=>invoke<ImageInfo>("image_info",{id}),
};
const acquisitions=new Set<Promise<unknown>>();
const releases=new Set<Promise<void>>();
function track<T>(set:Set<Promise<unknown>>,work:Promise<T>):Promise<T> {
  set.add(work);void work.then(()=>set.delete(work),()=>set.delete(work));return work;
}
function lease(registration:ImageRegistration):ImageLease {
  let released:Promise<void>|undefined;
  return {registration,release:()=>{
    if(!released) {
      released=imageApi.release(registration.token);
      releases.add(released);void released.then(()=>releases.delete(released!),()=>releases.delete(released!));
    }
    return released;
  }};
}
export function acquireImageLease(id:string):Promise<ImageLease> {
  if(!/^[a-f0-9]{64}$/.test(id))return Promise.reject(new Error("Invalid image resource identifier."));
  return track(acquisitions,imageApi.acquire(id).then(lease));
}
export function importImageLease():Promise<ImageLease|null> {
  return track(acquisitions,imageApi.importImage().then(value=>value?lease(value):null));
}
export async function drainImageAcquisitions() {
  while(acquisitions.size)await Promise.allSettled([...acquisitions]);
}
export async function flushImageReleases() {
  while(releases.size)await Promise.allSettled([...releases]);
}

interface Entry {state:ImageResourceState;promise:Promise<void>;lease?:ImageLease}
const entries=new Map<string,Entry>();
const listeners=new Set<()=>void>();
let revision=0;
function emit(){revision++;listeners.forEach(listener=>listener());}
export const subscribeImages=(listener:()=>void)=>{listeners.add(listener);return()=>{listeners.delete(listener);};};
export const imageRevision=()=>revision;
export const imageResourceState=(id:string)=>entries.get(id)?.state;

/** Consume a pending import's lease when placing it. The caller must relinquish
 * ownership after this call. A duplicate workspace owner makes it redundant;
 * otherwise the next workspace retention pass owns its lifetime. */
export function adoptImageLease(owned:ImageLease):void {
  const info=owned.registration.image,existing=entries.get(info.id);
  if(existing?.lease===owned)return;
  if(existing?.state.status==="ready"&&existing.lease){void owned.release().catch(()=>{});return;}
  if(existing?.lease)void existing.lease.release().catch(()=>{});
  entries.set(info.id,{state:{status:"ready",info},lease:owned,promise:Promise.resolve()});
  // An older pending acquire will see that its entry was replaced and release
  // only its own token when it eventually resolves.
  emit();
}

/** One workspace lease per asset, independent of sources and pending dialogs.
 * Old late responses release their own native token; they cannot delete a new owner.
 */
export function retainImages(ids:ReadonlySet<string>):Promise<void> {
  for(const [id,entry] of entries)if(!ids.has(id)) {
    entries.delete(id);if(entry.lease)void entry.lease.release().catch(()=>{});emit();
  }
  const pending:Promise<void>[]=[];
  for(const id of ids) {
    let entry=entries.get(id);
    if(!entry||entry.state.status==="error") {
      const created:Entry={state:{status:"loading"},promise:undefined!};entries.set(id,created);
      created.promise=acquireImageLease(id).then(async owned=>{
        if(entries.get(id)!==created){await owned.release();return;}
        created.lease=owned;created.state={status:"ready",info:owned.registration.image};emit();
      }).catch(error=>{
        if(entries.get(id)===created){created.state={status:"error",error:error instanceof Error?error.message:String(error)};emit();}
        throw error;
      });
      // A source-close barrier includes the ownership handoff and stale-response release.
      track(acquisitions,created.promise);
      entry=created;emit();
    }
    pending.push(entry.promise);
  }
  return Promise.all(pending).then(()=>{});
}
