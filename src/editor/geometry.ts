import type { Rotation } from "./types";

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