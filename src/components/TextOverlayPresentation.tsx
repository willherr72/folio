import type { TextOverlay } from "../editor/types";
import { textOverlayTransform } from "../editor/text-overlay-geometry";

/** Reopened text keeps its own orientation inside the editor page transform. */
export function TextOverlayPresentation({ overlay }: { overlay: TextOverlay }) {
  return <text x={overlay.x} y={overlay.y + overlay.fontSize} transform={textOverlayTransform(overlay)} fill={overlay.color} xmlSpace="preserve" style={{ whiteSpace: "pre" }} fontFamily="Arial, Helvetica, sans-serif" fontSize={overlay.fontSize}>
    {overlay.text.split("\n").map((line, index) => <tspan key={index} x={overlay.x} dy={index === 0 ? 0 : overlay.fontSize * 1.2}>{line || " "}</tspan>)}
  </text>;
}