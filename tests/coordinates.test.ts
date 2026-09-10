import { describe, expect, it } from "vitest";
import { displayDimensions, fromDisplayPoint, pageTransform, placeInkPaths, toDisplayPoint } from "../src/editor/geometry";

describe("rotated page coordinates", () => {
  it.each([
    [0, { x: 80, y: 120 }, { x: 80, y: 120 }],
    [90, { x: 80, y: 120 }, { x: 672, y: 80 }],
    [180, { x: 80, y: 120 }, { x: 532, y: 672 }],
    [270, { x: 80, y: 120 }, { x: 120, y: 532 }],
  ] as const)("maps and inversely maps %s degree rotation", (rotation, original, displayed) => {
    expect(toDisplayPoint(original, 612, 792, rotation)).toEqual(displayed);
    expect(fromDisplayPoint(displayed, 612, 792, rotation)).toEqual(original);
  });

  it("swaps only display dimensions for quarter turns", () => {
    expect(displayDimensions(612, 792, 90)).toEqual({ width: 792, height: 612 });
    expect(displayDimensions(612, 792, 270)).toEqual({ width: 792, height: 612 });
    expect(displayDimensions(612, 792, 180)).toEqual({ width: 612, height: 792 });
  });

  it("returns the SVG transform matching clockwise visual rotation", () => {
    expect(pageTransform(612, 792, 90)).toBe("translate(792 0) rotate(90)");
    expect(pageTransform(612, 792, 270)).toBe("translate(0 612) rotate(270)");
  });

  it("translates a whole signature inward at the page edge without deforming it", () => {
    const placed = placeInkPaths([[{ x: 0, y: 0 }, { x: 20, y: 10 }, { x: 40, y: 20 }]], { x: 600, y: 785 }, 612, 792);

    expect(placed[0][0]).toEqual({ x: 590, y: 781 });
    expect(placed[0][1]).toEqual({ x: 601, y: 786.5 });
    expect(placed[0][2]).toEqual({ x: 612, y: 792 });
    expect(placed[0][1].x - placed[0][0].x).toBe(placed[0][2].x - placed[0][1].x);
  });
});