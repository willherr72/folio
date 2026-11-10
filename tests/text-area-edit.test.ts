import { expect, it } from "vitest";
import { applyTextAreaEdit } from "../src/editor/text-area-edit";

it.each([
  ["A\r\n\r\nB\nC", "!A\n\nB\nC", "!A\r\n\r\nB\nC"],
  ["A\r\n\r\nB\nC", "A\n\n\nC", "A\r\n\r\n\nC"],
  ["A\r\nB\nC", "A\nB\nC", "A\r\nB\nC"],
  ["A\r\nB\nC", "A\nnew\nB\nC", "A\r\nnew\r\nB\nC"],
  ["A\r\nB\nC", "A\nB\nnew\nC", "A\r\nB\nnew\nC"],
  ["A\r\nB\nC", "", ""],
  ["A\rB\nC", "!A\nB\nC", "!A\rB\nC"],
  ["😀\r\nB", "😁\nB", "😁\r\nB"],
  ["A\nB", "A\nnew\nB", "A\nnew\nB"],
])("maps a textarea edit to its original source (%j → %j)", (source, edited, expected) => {
  expect(applyTextAreaEdit(source, edited)).toBe(expected);
});
