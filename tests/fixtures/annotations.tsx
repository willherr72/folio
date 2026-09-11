import React, { useState } from "react";
import { createRoot } from "react-dom/client";
import { PageView, Thumbnail } from "../../src/components/PageView";
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
  const [tool, setTool] = useState<"select" | "highlight" | "comment">("highlight");
  const [overlays, setOverlays] = useState<PagePlan["overlays"]>(() => params.has("opacity") ? [{ type: "highlight", id: "imported", color: "#ffff00", opacity: Number(params.get("opacity")), rects: [{ x: 0, y: 0, width, height }] }] : []);
  const [selected, setSelected] = useState<string | null>(null);
  const [status, setStatus] = useState("");
  const [mount, setMount] = useState(0);
  const page: PagePlan = { id: "browser", sourceId: "browser", pageIndex: 0, width, height, rotation: Number(params.get("rotation") ?? 0) as Rotation, overlays };
  return <div style={{ padding: 20 }}><nav>{["select", "highlight", "comment"].map(name => <button key={name} onClick={() => setTool(name as typeof tool)}>{name}</button>)}<button onClick={() => setMount(value => value + 1)}>Remount page</button></nav><output style={{ display: "block", height: 24 }} aria-label="Selection state">{status}</output><output style={{ display: "none" }} aria-label="Annotations">{JSON.stringify(overlays)}</output><PageView key={mount} adapter={adapter} page={page} pageNumber={1} zoom={Number(params.get("zoom") ?? 150)} tool={tool} selectedOverlayId={selected} pendingSignature={null}
    onSelectOverlay={id => { setSelected(id); setStatus(id ?? ""); }} onAddText={() => {}} onPlaceSignature={() => {}}
    onHighlight={rects => setOverlays(value => [...value, { type: "highlight", id: `highlight-${value.length}`, rects, color: "#ffff00" }])}
    onAddComment={point => setOverlays(value => [...value, { type: "comment", id: `comment-${value.length}`, ...point, text: "Review note", color: "#ffff00" }])}
    onMoveOverlay={(_id, _x, _y, phase) => { if (phase === "move") setStatus("moved overlay"); }} />{params.has("thumbnail") && <aside style={{ position: "absolute", left: 20, top: 100, width: 120 }}><Thumbnail adapter={adapter} page={page} /></aside>}</div>;
}
createRoot(document.getElementById("root")!).render(<Fixture />);
