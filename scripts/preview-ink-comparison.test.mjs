import assert from "node:assert/strict";
import { test } from "node:test";
import { compareInk } from "./preview-ink-comparison.mjs";

function pixels(dx = 0, fade = 0) {
  const data = new Uint8ClampedArray(32 * 32 * 4).fill(255);
  for (let y = 5; y < 20; y++) for (let x = 6 + dx; x < 12 + dx; x++) {
    const i = (y * 32 + x) * 4;
    data[i] = data[i + 1] = data[i + 2] = fade;
  }
  return data;
}
test("accepts equal ink and minor rasterizer darkness variation", () => {
  const result = compareInk(32, 32, pixels(), pixels(0, 15));
  assert.equal(result.pass, true);
  assert.equal(result.boundsDelta, 0);
  assert.ok(result.centroidDelta < 1e-10);
});
test("rejects a shifted glyph even when its dimensions and ink area match", () => {
  assert.equal(compareInk(32, 32, pixels(), pixels(4)).pass, false);
});
test("rejects missing ink and two blank pages", () => {
  const white = new Uint8ClampedArray(32 * 32 * 4).fill(255);
  assert.equal(compareInk(32, 32, pixels(), white).pass, false);
  assert.equal(compareInk(32, 32, white, white).pass, false);
});
