import { documentToPages, type FolioAdapter } from "./adapter";
import { createHistory, planDigest, uniqueId } from "./model";
import type { DocumentInfo, EditorDocument, History } from "./types";
export interface ScrollPosition { top: number; left: number }
export interface DocumentSession {
  id: string;
  adapter: FolioAdapter;
  sourceIds: string[];
  history: History<EditorDocument>;
  savedDigest: string;
  zoom: number;
  navigationRequest: {pageId: string; revision: number; rect?:{x:number;y:number;width:number;height:number}} | null;
}
export interface Workspace { tabs: DocumentSession[]; activeId: string | null }
export function emptyWorkspace(): Workspace { return { tabs: [], activeId: null }; }
export function createSession(info: DocumentInfo, adapter: FolioAdapter, zoom: number): DocumentSession {
  const pages = documentToPages(info);
  const document: EditorDocument = {name: info.name, pages, selectedPageId:pages[0]?.id??null, selectedOverlayId:null};
  return { id:uniqueId("document"), adapter, sourceIds:[info.id], history:createHistory(document), savedDigest:planDigest(document), zoom,
    navigationRequest: pages[0] ? {pageId:pages[0].id,revision:1} : null };
}
export function addSession(workspace: Workspace, session: DocumentSession): Workspace {
  return {tabs:[...workspace.tabs,session],activeId:session.id};
}
export function updateSession(workspace: Workspace, id: string | null, update: (session: DocumentSession)=>DocumentSession): Workspace {
  let changed = false;
  const tabs = workspace.tabs.map(tab=>{if(tab.id!==id)return tab; const next=update(tab); if(next!==tab)changed=true; return next;});
  return changed ? {...workspace,tabs} : workspace;
}
export function removeSession(workspace: Workspace, id: string): Workspace {
  const index = workspace.tabs.findIndex(tab=>tab.id===id);
  if(index<0)return workspace;
  const tabs = workspace.tabs.filter(tab=>tab.id!==id);
  return {tabs,activeId:workspace.activeId===id ? tabs[Math.min(index,tabs.length-1)]?.id??null : workspace.activeId};
}
