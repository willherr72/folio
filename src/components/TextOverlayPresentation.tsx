import { useId } from "react";
import { useShapedText, type PreparedTextOverlay } from "../editor/shaped-text";
import { useCustomFont } from "../editor/use-font-resources";
import { customTextError } from "../editor/custom-fonts";
import type { TextOverlay } from "../editor/types";
import { textOverlayTransform } from "../editor/text-overlay-geometry";

import { textFont } from "../editor/text-fonts";

/** Reopened text keeps its own orientation inside the editor page transform. */
export function TextOverlayPresentation({ overlay }: { overlay: TextOverlay }) {
  const custom = useCustomFont(overlay.fontId);
  if (overlay.shaping) return <ShapedOverlayPresentation overlay={overlay}/>;
  const problem = overlay.fontId ? custom?.status !== "ready" ? custom?.error ?? "Loading font…" : customTextError(custom.info!, overlay.text) : null;
  if (problem) return <text x={overlay.x} y={overlay.y + overlay.fontSize} transform={textOverlayTransform(overlay)} fill="var(--muted, #666)" fontSize={12} role="status">{custom?.status === "error" ? "Font unavailable" : custom?.status === "ready" ? "Unsupported character" : "Loading font…"}<title>{problem}</title></text>;
  const font = custom?.status === "ready" ? { family: custom.family, weight: 400, style: "normal" } : textFont(overlay.fontName);
  return <text x={overlay.x} y={overlay.y + overlay.fontSize} transform={textOverlayTransform(overlay)} fill={overlay.color} xmlSpace="preserve" style={{ whiteSpace: "pre", fontKerning: "none", fontVariantLigatures: "none", fontSynthesis: "none" }} fontFamily={font.family} fontWeight={font.weight} fontStyle={font.style} fontSize={overlay.fontSize}>
    {overlay.text.split("\n").map((line, index) => <tspan key={index} x={overlay.x} dy={index === 0 ? 0 : overlay.fontSize * 1.2}>{line || " "}</tspan>)}
  </text>;
}
/** Native source PDF axes mapped to the editor baseline, without browser shaping. */
export function PreparedOverlayInk({result, fontSize}: {result: PreparedTextOverlay; fontSize: number}) {
  const id = useId();
  const {preview,origin} = result;
  return <g transform={`translate(${-origin[0]} ${fontSize+origin[1]}) scale(1 -1)`} fill={`rgb(${preview.color.map(c=>`${c*100}%`).join(" ")})`}>
    <defs>{preview.outlines.map(outline=><path key={outline.glyphId} id={`${id}-${outline.glyphId}`} d={outline.path}/>)}</defs>
    {preview.glyphs.map((glyph,index)=><use key={index} href={`#${id}-${glyph.glyphId}`} transform={`matrix(${glyph.transform.join(" ")})`}/>)}
  </g>;
}
function ShapedOverlayPresentation({overlay}: {overlay:TextOverlay}) {
  const state = useShapedText(overlay);
  if (state?.status !== "ready" || !state.result) return <text x={overlay.x} y={overlay.y+overlay.fontSize} transform={textOverlayTransform(overlay)} fill="var(--muted, #666)" fontSize={12} role="status">
    {state?.status === "error" ? "Text unavailable" : "Preparing text…"}<title>{state?.error}</title>
  </text>;
  return <g transform={textOverlayTransform(overlay)}><g transform={`translate(${overlay.x} ${overlay.y})`}><PreparedOverlayInk result={state.result} fontSize={overlay.fontSize}/></g></g>;
}
