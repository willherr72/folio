import React, { useState } from "react";
import { createRoot } from "react-dom/client";
import { DocumentViewport } from "../../src/components/DocumentViewport";
import { createDemoAdapter } from "../../src/editor/adapter";
import type { PagePlan, Rotation } from "../../src/editor/types";
import "../../src/styles.css";
const params = new URLSearchParams(location.search);
const intrinsicRotation = Number(params.get("intrinsic") ?? 0) as Rotation;
const width = intrinsicRotation % 180 ? 320 : 240, height = intrinsicRotation % 180 ? 240 : 320;
const lines = ["Hello PDF", "Second line"];
const characters = lines.flatMap((line, row) => [...line].map((text, index) => ({ text, x: 30 + index * 12, y: 40 + row * 35, width: 12, height: 20 })).concat(row === 0 ? [{ text: "\r\n", x: 138, y: 40, width: 0, height: 0 }] : []));
const normalizedCharacters = characters.map(character => {
  const { x, y, width: w, height: h } = character;
  switch (intrinsicRotation) {
    case 90: return { ...character, x: 320 - y - h, y: x, width: h, height: w };
    case 180: return { ...character, x: 240 - x - w, y: 320 - y - h };
    case 270: return { ...character, x: y, y: 240 - x - w, width: h, height: w };
    default: return character;
  }
});
const intrinsicTransform = { 0: "", 90: "translate(320 0) rotate(90)", 180: "translate(240 320) rotate(180)", 270: "translate(0 240) rotate(270)" }[intrinsicRotation];
const picture = `<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}"><rect width="100%" height="100%" fill="white"/><g transform="${intrinsicTransform}">${lines.map((line, row) => `<text x="30" y="${57 + row * 35}" font-family="monospace" font-size="20">${line}</text>`).join("")}</g></svg>`;
const adapter = { ...createDemoAdapter(), getPageText: async () => ({ characters: normalizedCharacters, intrinsicRotation }), renderPage: async () => `data:image/svg+xml,${encodeURIComponent(picture)}` };
function Fixture() {
 const [pages, setPages] = useState<PagePlan[]>(() => Array.from({ length: 8 }, (_, index) => ({ id: `multi-${index}`, sourceId: "multi", pageIndex: index, width, height, rotation: 0, overlays: [] })));
 const [selected, setSelected] = useState("multi-0");
 const [changes, setChanges] = useState<Array<{ pageId: string; rects: unknown }>>([]);
 return <div style={{ height: "100%", display: "flex", flexDirection: "column" }}><output style={{ display: "none" }} aria-label="Annotations">{JSON.stringify(changes)}</output><DocumentViewport adapter={adapter} pages={pages} selectedPageId={selected} selectedOverlayId={null} zoom={90} onZoomChange={() => {}} viewMode="continuous" tool="highlight" penColor="#ffff00" penWidth={2} pendingSignature={null} interactionDisabled={false} navigationRequest={null} onSelectPage={setSelected} onSelectOverlay={() => {}} onAddText={() => {}} onPlaceSignature={() => {}} onMoveOverlay={() => {}} onDraw={() => {}}
 onHighlight={(pageId, rects) => { setChanges(value => [...value, { pageId, rects }]); setPages(value => value.map(page => page.id === pageId ? { ...page, overlays: [...page.overlays, { type: "highlight", id: `mark-${pageId}`, rects, color: "#ffff00" }] } : page)); }} /></div>;
}
createRoot(document.getElementById("root")!).render(<Fixture />);
