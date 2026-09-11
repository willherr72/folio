import { useCustomFont } from "../editor/use-font-resources";
import { customTextError } from "../editor/custom-fonts";
import type { TextOverlay } from "../editor/types";
import { textOverlayTransform } from "../editor/text-overlay-geometry";

import { textFont } from "../editor/text-fonts";

/** Reopened text keeps its own orientation inside the editor page transform. */
export function TextOverlayPresentation({ overlay }: { overlay: TextOverlay }) {
  const custom = useCustomFont(overlay.fontId);
  const problem = overlay.fontId ? custom?.status !== "ready" ? custom?.error ?? "Loading font…" : customTextError(custom.info!, overlay.text) : null;
  if (problem) return <text x={overlay.x} y={overlay.y + overlay.fontSize} transform={textOverlayTransform(overlay)} fill="var(--muted, #666)" fontSize={12} role="status">{custom?.status === "error" ? "Font unavailable" : custom?.status === "ready" ? "Unsupported character" : "Loading font…"}<title>{problem}</title></text>;
  const font = custom?.status === "ready" ? { family: custom.family, weight: 400, style: "normal" } : textFont(overlay.fontName);
  return <text x={overlay.x} y={overlay.y + overlay.fontSize} transform={textOverlayTransform(overlay)} fill={overlay.color} xmlSpace="preserve" style={{ whiteSpace: "pre", fontKerning: "none", fontVariantLigatures: "none", fontSynthesis: "none" }} fontFamily={font.family} fontWeight={font.weight} fontStyle={font.style} fontSize={overlay.fontSize}>
    {overlay.text.split("\n").map((line, index) => <tspan key={index} x={overlay.x} dy={index === 0 ? 0 : overlay.fontSize * 1.2}>{line || " "}</tspan>)}
  </text>;
}