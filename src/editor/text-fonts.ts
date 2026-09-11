import { customFontState } from "./custom-fonts";
/** PDF standard Latin faces and their Windows preview fonts. */
export const TEXT_FONTS = [
  {"name": "Helvetica", "label": "Helvetica", "family": "Arial, Helvetica, sans-serif", "weight": 400, "style": "normal", "advance": 0.56},
  {"name": "Helvetica-Bold", "label": "Helvetica Bold", "family": "Arial, Helvetica, sans-serif", "weight": 700, "style": "normal", "advance": 0.56},
  {"name": "Helvetica-Oblique", "label": "Helvetica Italic", "family": "Arial, Helvetica, sans-serif", "weight": 400, "style": "italic", "advance": 0.56},
  {"name": "Helvetica-BoldOblique", "label": "Helvetica Bold Italic", "family": "Arial, Helvetica, sans-serif", "weight": 700, "style": "italic", "advance": 0.56},
  {"name": "Times-Roman", "label": "Times", "family": "\"Times New Roman\", Times, serif", "weight": 400, "style": "normal", "advance": 0.5},
  {"name": "Times-Bold", "label": "Times Bold", "family": "\"Times New Roman\", Times, serif", "weight": 700, "style": "normal", "advance": 0.5},
  {"name": "Times-Italic", "label": "Times Italic", "family": "\"Times New Roman\", Times, serif", "weight": 400, "style": "italic", "advance": 0.5},
  {"name": "Times-BoldItalic", "label": "Times Bold Italic", "family": "\"Times New Roman\", Times, serif", "weight": 700, "style": "italic", "advance": 0.5},
  {"name": "Courier", "label": "Courier", "family": "\"Courier New\", Courier, monospace", "weight": 400, "style": "normal", "advance": 0.6},
  {"name": "Courier-Bold", "label": "Courier Bold", "family": "\"Courier New\", Courier, monospace", "weight": 700, "style": "normal", "advance": 0.6},
  {"name": "Courier-Oblique", "label": "Courier Italic", "family": "\"Courier New\", Courier, monospace", "weight": 400, "style": "italic", "advance": 0.6},
  {"name": "Courier-BoldOblique", "label": "Courier Bold Italic", "family": "\"Courier New\", Courier, monospace", "weight": 700, "style": "italic", "advance": 0.6}
] as const;
export type TextFontName = typeof TEXT_FONTS[number]["name"];
export function textFont(name?: string) { return TEXT_FONTS.find(font => font.name === name) ?? TEXT_FONTS[0]; }

let context: CanvasRenderingContext2D | null | undefined;
const advances = new Map<string, number>();
/** Measure at a fixed size, caching glyph advances independently of annotation size. */
export function textAdvance(character: string, size: number, name?: TextFontName, fontId?: string): number {
  const custom = fontId ? customFontState(fontId) : undefined;
  const font = custom?.status === "ready" ? { name: fontId!, family: custom.family, weight: 400, style: "normal", advance: 0.56 } : textFont(name);
  // Do not cache fallback measurements under a custom font identifier.
  if (typeof CanvasRenderingContext2D === "undefined") return font.advance * size;
  if (context === undefined) context = document.createElement("canvas").getContext("2d");
  if (!context) return font.advance * size;
  const key = `${font.name}:${character}`;
  let width = advances.get(key);
  if (width === undefined) {
    context.font = `${font.style} ${font.weight} 100px ${font.family}`;
    width = context.measureText(character).width / 100;
    if (advances.size >= 4096) advances.clear();
    advances.set(key, width);
  }
  return width * size;
}
