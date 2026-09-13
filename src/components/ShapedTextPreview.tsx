import { useId } from "react";
import type { NativeTextPreview } from "../editor/native-text-preview";

/** Renders trusted native output; never asks browser fonts to shape source text. */
export function ShapedTextPreview({ preview, scale = 1 }: { preview: NativeTextPreview; scale?: number }) {
  const prefix = useId();
  if (preview.version !== 1) return <span role="status">Text preview unavailable (unsupported version).</span>;
  const { width, height, rotation } = preview;
  const sideways = rotation === 90 || rotation === 270;
  const displayWidth = sideways ? height : width;
  const displayHeight = sideways ? width : height;
  // The native placements use bottom-left PDF axes, before source page rotation.
  const transform = {
    0: `matrix(1 0 0 -1 0 ${height})`,
    90: "matrix(0 1 1 0 0 0)",
    180: `matrix(-1 0 0 1 ${width} 0)`,
    270: `matrix(0 -1 -1 0 ${height} ${width})`,
  }[rotation];
  const glyphId = (id: number) => `${prefix}-glyph-${id}`;
  return <svg xmlns="http://www.w3.org/2000/svg" role="img" aria-label={preview.text}
    viewBox={`0 0 ${displayWidth} ${displayHeight}`} width={displayWidth * scale} height={displayHeight * scale}>
    <defs>{preview.outlines.map(outline => <path key={outline.glyphId} id={glyphId(outline.glyphId)} d={outline.path} />)}</defs>
    <g transform={transform} fill={`rgb(${preview.color.map(channel => `${channel * 100}%`).join(" ")})`}>
      {preview.glyphs.map((glyph, index) => <use key={index} href={`#${glyphId(glyph.glyphId)}`} transform={`matrix(${glyph.transform.join(" ")})`} />)}
    </g>
  </svg>;
}
