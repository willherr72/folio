import {drainImageAcquisitions} from "./image-resources";
import {releaseUnownedFont} from "./custom-fonts";
import type {DocumentInfo} from "./types";

/** An opened PDF owns imported resources even if it never joins workspace history. */
export async function discardOpenedDocument(document:DocumentInfo,close:(id:string)=>Promise<void>):Promise<void> {
 const fonts=new Set<string>();
 for(const page of document.pages)for(const overlay of page.overlays??[])
  if(overlay.type==="text"&&overlay.fontId)fonts.add(overlay.fontId);
 try {await drainImageAcquisitions();await close(document.id);}
 finally {await Promise.allSettled([...fonts].map(releaseUnownedFont));}
}
