import type { InkOverlay, Rotation } from "./types";

export interface Point { x: number; y: number }

export function displayDimensions(width: number, height: number, rotation: Rotation) {
  return rotation === 90 || rotation === 270
    ? { width: height, height: width }
    : { width, height };
}

export function toDisplayPoint(point: Point, width: number, height: number, rotation: Rotation): Point {
  switch (rotation) {
    case 90: return { x: height - point.y, y: point.x };
    case 180: return { x: width - point.x, y: height - point.y };
    case 270: return { x: point.y, y: width - point.x };
    default: return { ...point };
  }
}

export function fromDisplayPoint(point: Point, width: number, height: number, rotation: Rotation): Point {
  switch (rotation) {
    case 90: return { x: point.y, y: height - point.x };
    case 180: return { x: width - point.x, y: height - point.y };
    case 270: return { x: width - point.y, y: point.x };
    default: return { ...point };
  }
}

export function pageTransform(width: number, height: number, rotation: Rotation): string {
  switch (rotation) {
    case 90: return `translate(${height} 0) rotate(90)`;
    case 180: return `translate(${width} ${height}) rotate(180)`;
    case 270: return `translate(0 ${width}) rotate(270)`;
    default: return "";
  }
}

export function placeInkPaths(
  paths: Point[][],
  requestedOrigin: Point,
  pageWidth: number,
  pageHeight: number,
  maxWidth = 180,
  maxHeight = 72,
): Point[][] {
  const points = paths.flat();
  if (!points.length) return [];
  const minX = Math.min(...points.map((point) => point.x));
  const maxX = Math.max(...points.map((point) => point.x));
  const minY = Math.min(...points.map((point) => point.y));
  const maxY = Math.max(...points.map((point) => point.y));
  const sourceWidth = Math.max(1, maxX - minX);
  const sourceHeight = Math.max(1, maxY - minY);
  const scale = Math.min(0.55, maxWidth / sourceWidth, maxHeight / sourceHeight, pageWidth / sourceWidth, pageHeight / sourceHeight);
  const placedWidth = (maxX - minX) * scale;
  const placedHeight = (maxY - minY) * scale;
  const originX = Math.max(0, Math.min(pageWidth - placedWidth, requestedOrigin.x));
  const originY = Math.max(0, Math.min(pageHeight - placedHeight, requestedOrigin.y));
  return paths.map((path) => path.map((point) => ({
    x: originX + (point.x - minX) * scale,
    y: originY + (point.y - minY) * scale,
  })));
}
export function clientPointToPage(
  clientX: number,
  clientY: number,
  rect: Pick<DOMRect, "left" | "top" | "width" | "height">,
  width: number,
  height: number,
  rotation: Rotation,
): Point {
  const display = displayDimensions(width, height, rotation);
  const displayed = {
    x: (clientX - rect.left) * display.width / rect.width,
    y: (clientY - rect.top) * display.height / rect.height,
  };
  const original = fromDisplayPoint(displayed, width, height, rotation);
  return {
    x: Math.max(0, Math.min(width, original.x)),
    y: Math.max(0, Math.min(height, original.y)),
  };
}
/** Proportional resize around the ink origin, constrained to the source page. */
export function resizeInk(ink: InkOverlay, requestedWidth: number, pageWidth: number, pageHeight: number): InkOverlay {
  if (!Number.isFinite(requestedWidth) || requestedWidth <= 0) return ink;
  const points = ink.paths.flat();
  if (!points.length) return ink;
  const box = points.reduce((b,p) => ({left: Math.min(b.left,p.x),top: Math.min(b.top,p.y),right: Math.max(b.right,p.x),bottom: Math.max(b.bottom,p.y)}), {left:Infinity,top:Infinity,right:-Infinity,bottom:-Infinity});
  const width = box.right-box.left, height = box.bottom-box.top;
  if (width <= 0 || box.left >= pageWidth || box.top >= pageHeight) return ink;
  const originX = Math.max(0, box.left), originY = Math.max(0, box.top);
  const scale = Math.min(requestedWidth/width, (pageWidth-originX)/width, height>0 ? (pageHeight-originY)/height : Infinity);
  if (!Number.isFinite(scale) || scale<=0 || (scale===1 && originX===box.left && originY===box.top)) return ink;
  return {...ink, paths:ink.paths.map(path=>path.map(point=>({x:originX+(point.x-box.left)*scale,y:originY+(point.y-box.top)*scale})))};
}
