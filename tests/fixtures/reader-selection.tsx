import React, { useState } from "react";
import { createRoot } from "react-dom/client";
import { PdfTextLayer } from "../../src/components/PdfTextLayer";
import { createDemoAdapter } from "../../src/editor/adapter";
import type { AnnotationRect, PagePlan, PageText } from "../../src/editor/types";
import native from "./reader-selection/native.json";
const params = new URLSearchParams(location.search);
const specimen = native.cases.find(c => c.case === (params.get("case") ?? "ligatures-on") && c.rotation === Number(params.get("rotation") ?? 0))!;
const zoom = Number(params.get("zoom") ?? 2);
const page: PagePlan = { id: "native", sourceId: "native", pageIndex: 0, width: specimen.width, height: specimen.height, rotation: 0, overlays: [] };
const adapter = { ...createDemoAdapter(), getPageText: async () => specimen.pageText as PageText };
function Fixture() {
  const [highlighting, setHighlighting] = useState(false);
  const [rects, setRects] = useState<AnnotationRect[]>([]);
  return <>
    <button onClick={() => { setRects([]); setHighlighting(!highlighting); }}>Toggle highlight</button>
    <output aria-label="Highlight rectangles">{JSON.stringify(rects)}</output>
    <div className="document-viewport" style={{ margin: 20 }}>
      <svg width={page.width * zoom} height={page.height * zoom} viewBox={`0 0 ${page.width} ${page.height}`}>
        <image href={new URL(`./reader-selection/${specimen.png}`, import.meta.url).href} width={page.width} height={page.height} style={{ userSelect: "none" }} />
        <PdfTextLayer adapter={adapter} page={page} pageNumber={1} selectable highlighting={highlighting} onHighlight={setRects} />
        {rects.map((rect, i) => <rect key={i} {...rect} fill="yellow" opacity=".4" pointerEvents="none" />)}
      </svg>
    </div>
  </>;
}
createRoot(document.getElementById("root")!).render(<Fixture />);
