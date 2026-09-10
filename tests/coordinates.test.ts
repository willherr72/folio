import { describe, expect, it } from "vitest";
import { displayDimensions, fromDisplayPoint, pageTransform, toDisplayPoint } from "../src/editor/geometry";

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
});
