import { invoke } from "@tauri-apps/api/core";
import type { PagePlan } from "./types";
export interface PrintOptions { scale: "fit" | "actual"; orientation: "portrait" | "landscape" }
export const printPdf = (pages:PagePlan[],options:PrintOptions) => invoke<boolean>("print_pdf",{request:{pages},options});
export function selectPrintPages(pages: PagePlan[], mode: "all"|"current"|"range", currentPageId: string|null, range: string): PagePlan[] {
 if(!pages.length) throw new Error("There are no pages to print.");
 if(mode==="all")return pages;
 if(mode==="current"){const page=pages.find(page=>page.id===currentPageId);if(!page)throw new Error("Select a current page to print.");return [page];}
 const indices=new Set<number>();
 for(const part of range.split(",")){
  const match=/^\s*(\d+)\s*(?:-\s*(\d+)\s*)?$/.exec(part);
  if(!match)throw new Error("Enter page numbers or ranges, for example 1, 3-5.");
  const start=Number(match[1]),end=Number(match[2]??match[1]);
  if(start<1||end<start||end>pages.length)throw new Error("Page ranges must be between 1 and "+pages.length+".");
  for(let index=start-1;index<end;index++)indices.add(index);
 }
 return pages.filter((_,index)=>indices.has(index));
}
