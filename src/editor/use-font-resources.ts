import { useEffect, useLayoutEffect, useRef, useSyncExternalStore } from "react";
import type { Workspace } from "./workspace";
import { customFontState, ensureCustomFont, fontRevision, retainCustomFonts, subscribeFonts } from "./custom-fonts";

export function workspaceFontIds(workspace: Workspace): Set<string> {
  const ids = new Set<string>();
  for (const tab of workspace.tabs) {
    if (tab.adapter.kind !== "native") continue;
    for (const document of [...tab.history.past, tab.history.present, ...tab.history.future])
      for (const page of document.pages) for (const overlay of page.overlays)
        if (overlay.type === "text" && overlay.fontId) ids.add(overlay.fontId);
  }
  return ids;
}
export function useWorkspaceFonts(workspace: Workspace) {
  useSyncExternalStore(subscribeFonts, fontRevision);
  const alive = useRef(false);
  useLayoutEffect(() => {
    const ids = workspaceFontIds(workspace);
    retainCustomFonts(ids);
    for (const id of ids) void ensureCustomFont(id).catch(() => {});
  }, [workspace]);
  useEffect(() => {
    alive.current = true;
    return () => { alive.current = false; queueMicrotask(() => { if (!alive.current) retainCustomFonts(new Set()); }); };
  }, []);
}
export function useCustomFont(id?: string) {
  useSyncExternalStore(subscribeFonts, fontRevision);
  return id ? customFontState(id) : undefined;
}
