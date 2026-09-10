import React, { useState } from "react";
import { createRoot } from "react-dom/client";
import { PageView } from "../../src/components/PageView";
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
  const [tool, setTool] = useState<"select" | "draw" | "text" | "signature">("select");
  const [status, setStatus] = useState("");
  const page: PagePlan = { id: "browser", sourceId: "browser", pageIndex: 0, width, height, rotation: Number(params.get("rotation") ?? 0) as Rotation, overlays: [{ type: "text", id: "edit", text: "Edit", x: 30, y: 140, fontSize: 20, color: "black" }] };
  return <div style={{ padding: 20 }}><nav>{["select", "draw", "text", "signature"].map((name) => <button key={name} onClick={() => setTool(name as typeof tool)}>{name}</button>)}</nav><output>{status}</output><PageView adapter={adapter} page={page} pageNumber={1} zoom={Number(params.get("zoom") ?? 150)} tool={tool} selectedOverlayId={null} pendingSignature={tool === "signature" ? [[{ x: 0, y: 0 }, { x: 100, y: 30 }]] : null} onSelectOverlay={(id) => setStatus(id ?? "")} onAddText={() => setStatus("added text")} onPlaceSignature={() => setStatus("placed signature")} onDraw={() => setStatus("drew stroke")} onMoveOverlay={(_id, _x, _y, phase) => { if (phase === "move") setStatus("moved overlay"); }} /></div>;
}
createRoot(document.getElementById("root")!).render(<Fixture />);