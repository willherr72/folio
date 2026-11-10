import {commit} from "./model";
import type {EditorDocument,SourceReplacement,PagePlan,ProtectionInfo,EditorTool} from "./types";
import type {DocumentSession} from "./workspace";

export type PdfPermission=keyof ProtectionInfo["permissions"];
export function pageAllows(page:PagePlan|null|undefined,right:PdfPermission):boolean {return page?.protection?.permissions[right]!==false;}
export function canArrangePages(pages:PagePlan[]):boolean {return pages.every(page=>pageAllows(page,"assemble"));}
export function canFlattenPages(pages:PagePlan[]):boolean {return pages.every(page=>pageAllows(page,"editContent")||!page.overlays.some(o=>o.type==="text"||o.type==="ink"||o.type==="image"));}
export function toolAllowed(page:PagePlan|null|undefined,tool:EditorTool):boolean {
 if(tool==="select")return true;
 if(tool==="form")return pageAllows(page,"fillForms");
 if(tool==="edit"||tool==="images")return pageAllows(page,"editContent");
 return pageAllows(page,"annotate")&&(tool!=="highlight"||pageAllows(page,"copyContent"));
}
export function describeRestrictions(page:PagePlan):string {
 if(!page.protection)return "";
 if(page.protection.owner)return "Owner access. Saved copies keep the original protection.";
 const labels:[PdfPermission,string][]=[["editContent","editing existing text"],["copyContent","copying text"],["annotate","changing annotations"],["fillForms","filling forms"],["assemble","arranging pages and bookmarks"],["print","printing"]];
 const denied=labels.filter(([right])=>!pageAllows(page,right)).map(([,label])=>label);
 return (denied.length?`This PDF restricts ${denied.join(", ")}.`:"The current password permits all supported actions.")+(pageAllows(page,"print")&&!pageAllows(page,"printHighQuality")?" Printing is limited to 150 dpi.":"");
}
/** UI guard for shortcuts and stale pointer callbacks; native policy is authoritative. */
export function allowsWorkspaceEdit(before:EditorDocument,after:EditorDocument):boolean {
 if(JSON.stringify(before.metadata)!==JSON.stringify(after.metadata)&&before.pages.some(page=>!pageAllows(page,'editContent')))return false;
 if(!canArrangePages(before.pages)) {
  if(before.pages.length!==after.pages.length||before.pages.some((page,i)=>page.id!==after.pages[i]?.id||page.rotation!==after.pages[i]?.rotation))return false;
  if(JSON.stringify(before.bookmarks)!==JSON.stringify(after.bookmarks)||before.bookmarkError!==after.bookmarkError)return false;
 }
 for(const page of before.pages) {
  if(!pageAllows(page,"annotate")) {
   const next=after.pages.find(p=>p.id===page.id);
   if(next&&JSON.stringify(page.overlays)!==JSON.stringify(next.overlays))return false;
  }
 }
 return true;
}

/** Rebind only the present version. Older native capabilities remain owned by undo. */
export function commitOwnerUnlock(session:DocumentSession,expected:EditorDocument,replacements:SourceReplacement[]):DocumentSession {
 if(session.history.present.pages!==expected.pages)throw new Error("The document changed while unlocking. Try again.");
 if(!replacements.length)throw new Error("No unlocked source was returned.");
 const versions=new Map(replacements.map(item=>[item.sourceId,item.document]));
 const ids=replacements.map(item=>item.document.id);
 if(versions.size!==replacements.length||new Set(ids).size!==ids.length||ids.some(id=>!id||session.sourceIds.includes(id)))throw new Error("Unlocked sources must have unique new versions.");
 const origin=replacements[0].document.protection?.documentId;
 if(!origin)throw new Error("Missing original protection identity.");
 const required=new Set(expected.pages.filter(page=>page.protection?.documentId===origin).map(page=>page.sourceId));
 if(required.size!==versions.size||[...versions.keys()].some(id=>!required.has(id)))throw new Error("The unlocked sources do not match this protected document.");
 for(const page of expected.pages.filter(page=>required.has(page.sourceId))) {
  const result=versions.get(page.sourceId)!,size=result.pages[page.pageIndex];
  if(!result.protection?.owner||result.protection.documentId!==origin)throw new Error("The source was not unlocked with its owner password.");
  if(!size||!Number.isFinite(size.width)||!Number.isFinite(size.height)||Math.abs(size.width-page.width)>0.01||Math.abs(size.height-page.height)>0.01)throw new Error("Unlocking changed or omitted a page.");
 }
 return {...session,sourceIds:[...session.sourceIds,...ids],history:commit(session.history,document=>({...document,selectedOverlayId:null,
  pages:document.pages.map(page=>{const result=versions.get(page.sourceId);return result?{...page,sourceId:result.id,contentSourceId:page.contentSourceId??page.sourceId,protection:result.protection}:page;}),
 }))};
}
