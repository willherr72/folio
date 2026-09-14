import { useState } from "react";
import { createRoot } from "react-dom/client";
import { OverlayProperties } from "../../src/components/OverlayProperties";
import type { Overlay } from "../../src/editor/types";
const params = new URLSearchParams(location.search);
function Editor() {
  const [overlay, setOverlay] = useState<Overlay>({ type: "text", id: "test", x: 0, y: 0, fontSize: 20, color: "#123456", text: params.get("text") ?? "", shaping: params.has("legacy") ? undefined : { version: 1, direction: "auto", ligatures: true } });
  const [commits, setCommits] = useState(0);
  return <><OverlayProperties overlay={overlay} onChange={update => { setOverlay(update); setCommits(n => n + 1); }} onDelete={() => {}} autoEdit={false} onAutoEdited={() => {}} pageWidth={500} pageHeight={600}/><output id="commits">{commits}</output><output id="saved">{overlay.type === "text" ? overlay.text : ""}</output></>;
}
createRoot(document.getElementById("root")!).render(<Editor/>);
