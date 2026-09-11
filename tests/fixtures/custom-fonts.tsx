import { fontApi, fontRevision, subscribeFonts, ensureCustomFont, retainCustomFonts } from "../../src/editor/custom-fonts";
const fontId = "c".repeat(64);
fontApi.info = async () => ({id: fontId, name: "DejaVu Serif", weight:400, italic:false, coverage:[[32,126],[233,233],[937,937]]});
fontApi.bytes = async () => { await new Promise(resolve => setTimeout(resolve, 200)); return (await fetch("/tests/fixtures/corpus/fonts/DejaVuSerif.ttf")).arrayBuffer(); };
fontApi.release = async () => {};
import React, { useRef, useState, useEffect, useSyncExternalStore } from "react";
import { createRoot } from "react-dom/client";
import { DocumentViewport } from "../../src/components/DocumentViewport";
import { Thumbnail } from "../../src/components/PageView";
import { createDemoAdapter } from "../../src/editor/adapter";
import { OverlayProperties } from "../../src/components/OverlayProperties";
import { findPageMatches } from "../../src/editor/search";
import type { PagePlan, Rotation, TextOverlay } from "../../src/editor/types";
import "../../src/styles.css";
const params = new URLSearchParams(location.search);
const initial: TextOverlay = { type: "text", id: "reopened-text", x: 240, y: 280, text: "AB café Ω WWW iii", fontId, fontSize: 24, color: "#000000", rotation: Number(params.get("textRotation") ?? 0) as Rotation };
const adapter = { ...createDemoAdapter(), renderPage: async () => "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='480' height='640'%3E%3Crect width='100%25' height='100%25' fill='white'/%3E%3C/svg%3E", getPageText: async () => ({ characters: [] }) };
function Fixture() {
  useSyncExternalStore(subscribeFonts, fontRevision);
  useEffect(() => { retainCustomFonts(new Set([fontId])); void ensureCustomFont(fontId); return () => retainCustomFonts(new Set()); }, []);
  const [overlay, setOverlay] = useState(initial);
  const drag = useRef<{ x: number; y: number; overlay: TextOverlay } | null>(null);
  const page: PagePlan = { id: "rotated-text-page", sourceId: "rotated-text", pageIndex: 0, width: 480, height: 640, rotation: Number(params.get("pageRotation") ?? 0) as Rotation, overlays: [overlay] };
  const matches = findPageMatches(page, { characters: [] }, "ab");
  return <div style={{ height: "100%", display: "flex", flexDirection: "column" }}>
    <output aria-label="Text state" style={{ display: "none" }}>{JSON.stringify(overlay)}</output>
    <DocumentViewport adapter={adapter} pages={[page]} selectedPageId={page.id} selectedOverlayId={overlay.id} zoom={Number(params.get("zoom") ?? 90)} onZoomChange={() => {}} viewMode="continuous" tool="select" penColor="#000000" penWidth={2} pendingSignature={null} interactionDisabled={false} navigationRequest={{ pageId: page.id, revision: 1, rect: matches[0].rects[0] }} searchMatches={matches} activeSearchMatchId={matches[0].id} onSelectPage={() => {}} onSelectOverlay={() => {}} onAddText={() => {}} onPlaceSignature={() => {}} onDraw={() => {}}
      onMoveOverlay={(_pageId, _id, x, y, phase) => {
        if (phase === "start") drag.current = { x, y, overlay };
        else if (phase === "move" && drag.current) setOverlay({ ...drag.current.overlay, x: drag.current.overlay.x + x - drag.current.x, y: drag.current.overlay.y + y - drag.current.y });
        else if (phase === "end") drag.current = null;
      }} />
    <aside style={{position:"fixed",right:10,top:10,width:200,background:"var(--panel)",padding:12}}><OverlayProperties overlay={overlay} onChange={update=>setOverlay(current=>update(current) as TextOverlay)} onDelete={()=>{}} autoEdit={false} onAutoEdited={()=>{}} pageWidth={480} pageHeight={640}/></aside>
    <aside style={{ position: "absolute", left: 10, top: 10, width: 100 }}><Thumbnail adapter={adapter} page={page} /></aside>
  </div>;
}
createRoot(document.getElementById("root")!).render(<Fixture />);