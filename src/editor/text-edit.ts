import {commit} from "./model";
import type {DocumentInfo,PagePlan} from "./types";
import type {DocumentSession} from "./workspace";

/** Change only this page's immutable source; older versions remain owned for undo. */
export function commitTextReplacement(session:DocumentSession, expected:PagePlan, result:DocumentInfo):DocumentSession {
 const page=session.history.present.pages.find(page=>page.id===expected.id);
 if (!page || page.sourceId!==expected.sourceId || page.pageIndex!==expected.pageIndex) throw new Error("This page changed while editing. Select the text again.");
 if (result.pages.length!==1) throw new Error("The replacement must contain exactly one page.");
 const size=result.pages[0];
 if (!Number.isFinite(size.width)||!Number.isFinite(size.height)||Math.abs(size.width-page.width)>0.01||Math.abs(size.height-page.height)>0.01) throw new Error("The replacement page dimensions changed unexpectedly.");
 if (!result.id || session.sourceIds.includes(result.id)) throw new Error("The replacement must have a new source version.");
 return {...session,sourceIds:[...session.sourceIds,result.id],history:commit(session.history,current=>({
  ...current,selectedPageId:page.id,selectedOverlayId:null,
  pages:current.pages.map(item=>item.id===page.id?{...item,sourceId:result.id,pageIndex:0}:item),
 }))};
}
