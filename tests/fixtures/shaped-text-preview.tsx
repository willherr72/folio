import { createRoot } from "react-dom/client";
import { ShapedTextPreview } from "../../src/components/ShapedTextPreview";
import type { NativeTextPreview } from "../../src/editor/native-text-preview";

const params = new URLSearchParams(location.search);
const stem = params.get("case") ?? "";
const zoom = Number(params.get("zoom") ?? "150");
const base = "/artifacts/shaped-text/shared-preview-native/";
const response = await fetch(`${base}${encodeURIComponent(stem)}.preview.json`);
if (!response.ok) throw new Error(`Missing native preview: ${response.status}`);
const preview: NativeTextPreview = await response.json();
createRoot(document.getElementById("root")!).render(<>
  <h1>{stem} — {zoom}%</h1>
  <div className="comparison">
    <figure><figcaption>Browser native outlines</figcaption><div id="native-preview"><ShapedTextPreview preview={preview} scale={zoom / 100} /></div></figure>
    <figure><figcaption>PDFium exported PDF</figcaption><img id="pdfium-preview" alt="PDFium exported PDF" src={`${base}${encodeURIComponent(stem)}.preview-${zoom}.png`} /></figure>
  </div>
</>);
