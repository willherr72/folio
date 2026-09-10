import type { AnnotationRect, PageText, PdfTextCharacter, TextOverlay } from "./types";

/** Rotate a local text box about its top-left anchor in normalized page units. */
export function rotateTextRect(overlay: TextOverlay, rect: AnnotationRect): AnnotationRect {
  const { x, y, width, height } = rect;
  switch (overlay.rotation ?? 0) {
    case 90: return { x: overlay.x - y - height, y: overlay.y + x, width: height, height: width };
    case 180: return { x: overlay.x - x - width, y: overlay.y - y - height, width, height };
    case 270: return { x: overlay.x + y, y: overlay.y - x - width, width: height, height: width };
    default: return { x: overlay.x + x, y: overlay.y + y, width, height };
  }
}

export function textOverlayTransform(overlay: TextOverlay): string | undefined {
  return overlay.rotation ? `rotate(${overlay.rotation} ${overlay.x} ${overlay.y})` : undefined;
}

export function textOverlayBounds(overlay: TextOverlay): AnnotationRect {
  const lines = overlay.text.split("\n");
  return rotateTextRect(overlay, {
    x: -4, y: -3,
    width: Math.max(36, ...lines.map(line => Array.from(line).length * overlay.fontSize * .56)) + 8,
    height: Math.max(overlay.fontSize * 1.2, lines.length * overlay.fontSize * 1.2) + 5,
  });
}

/** Added text has approximate font advances, consistently rotated before page zoom. */
export function textOverlayCharacters(overlay: TextOverlay): PageText {
  const characters: PdfTextCharacter[] = [];
  let column = 0, line = 0;
  for (const text of overlay.text) {
    const rect = rotateTextRect(overlay, { x: column * overlay.fontSize * .56, y: line * overlay.fontSize * 1.2, width: text === "\n" ? 0 : overlay.fontSize * .56, height: overlay.fontSize * 1.2 });
    characters.push({ text, ...rect });
    if (text === "\n") { line++; column = 0; } else column++;
  }
  return { characters, intrinsicRotation: overlay.rotation ?? 0 };
}