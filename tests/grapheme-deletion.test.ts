import { expect, it } from "vitest";
import { graphemeDeletionRange } from "../src/editor/grapheme-deletion";

it.each([
  ["a\u0301\u0323b", 3, 3, "backward", [0, 3]],
  ["a\u0301\u0323b", 0, 0, "forward", [0, 3]],
  ["a\u0301\u0323b", 1, 1, "backward", [0, 3]],
  ["a\u0301\u0323b", 2, 2, "forward", [0, 3]],
  ["x😀y", 3, 3, "backward", [1, 3]],
  ["x😀y", 2, 2, "forward", [1, 3]],
  ["x😀y", 2, 3, "backward", [1, 3]],
  ["x😀a\u0301y", 2, 4, "forward", [1, 5]],
  ["office", 3, 3, "backward", [2, 3]],
  ["office", 2, 2, "forward", [2, 3]],
  ["אב\u05b0 ג", 3, 3, "backward", [1, 3]],
  ["אב\u05b0 ג", 3, 3, "forward", [3, 4]],
  ["👩‍💻!", 5, 5, "backward", [0, 5]],
  ["🇺🇸!", 4, 4, "backward", [0, 4]],
  ["abc", 0, 0, "backward", null],
  ["abc", 3, 3, "forward", null],
  ["", 0, 0, "forward", null],
] as const)("expands logical UTF-16 range in %j at %i..%i going %s", (text, start, end, direction, expected) => {
  expect(graphemeDeletionRange(text, start, end, direction)).toEqual(expected);
});
