import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, it } from "vitest";
import { ShapedTextPreview } from "../src/components/ShapedTextPreview";
import type { NativeTextPreview } from "../src/editor/native-text-preview";

afterEach(cleanup);
const preview: NativeTextPreview = {
  version: 1, fontId: "bundled:fixture", text: "office العربية", width: 240, height: 320,
  rotation: 0, color: [0.2, 0.4, 0.6],
  outlines: [{ glyphId: 7, path: "M0 0L10 0L10 20Z" }],
  glyphs: [{ glyphId: 7, transform: [0.01, 0, 0, 0.01, 24, 280] },
    { glyphId: 7, transform: [0.01, 0, 0, 0.01, 36, 280] }],
};

it("renders native outlines and placements with native color, without browser shaping", () => {
  const { container } = render(<ShapedTextPreview preview={preview} />);
  expect(container.querySelectorAll("defs path")).toHaveLength(1);
  expect(container.querySelector("path")?.getAttribute("d")).toBe(preview.outlines[0].path);
  const uses = [...container.querySelectorAll("use")];
  expect(uses.map(node => node.getAttribute("transform"))).toEqual([
    "matrix(0.01 0 0 0.01 24 280)", "matrix(0.01 0 0 0.01 36 280)",
  ]);
  expect(container.querySelector("g")?.getAttribute("fill")).toBe("rgb(20% 40% 60%)");
  expect(container.querySelector("text, tspan, foreignObject")).toBeNull();
  expect(screen.getByRole("img", { name: preview.text })).toBeTruthy();
});

it.each([
  [0, "matrix(1 0 0 -1 0 320)", "0 0 240 320", 240, 320],
  [90, "matrix(0 1 1 0 0 0)", "0 0 320 240", 320, 240],
  [180, "matrix(-1 0 0 1 240 0)", "0 0 240 320", 240, 320],
  [270, "matrix(0 -1 -1 0 320 240)", "0 0 320 240", 320, 240],
] as const)("maps source PDF axes at %i degrees and scales once", (rotation, transform, viewBox, width, height) => {
  const { container } = render(<ShapedTextPreview preview={{ ...preview, rotation }} scale={1.5} />);
  const svg = container.querySelector("svg")!;
  expect(svg.getAttribute("viewBox")).toBe(viewBox);
  expect(Number(svg.getAttribute("width"))).toBe(width * 1.5);
  expect(Number(svg.getAttribute("height"))).toBe(height * 1.5);
  expect(container.querySelector("g")?.getAttribute("transform")).toBe(transform);
});

it("isolates outline references between mounted previews sharing glyph IDs", () => {
  const { container } = render(<><ShapedTextPreview preview={preview} /><ShapedTextPreview preview={preview} /></>);
  const svgs = [...container.querySelectorAll("svg")];
  const ids = svgs.map(svg => svg.querySelector("path")!.id);
  expect(new Set(ids).size).toBe(2);
  svgs.forEach((svg, index) => [...svg.querySelectorAll("use")].forEach(use => {
    expect(use.getAttribute("href")).toBe(`#${ids[index]}`);
  }));
});

it("shows an unavailable cue for an unsupported native version", () => {
  const { container } = render(<ShapedTextPreview preview={{ ...preview, version: 2 } as unknown as NativeTextPreview} />);
  expect(screen.getByRole("status").textContent).toContain("Text preview unavailable");
  expect(container.querySelector("svg")).toBeNull();
});
