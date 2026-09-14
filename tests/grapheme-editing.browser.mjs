import { chromium, expect } from "@playwright/test";
const origin = process.env.FOLIO_SHARED_PREVIEW_TEST_ORIGIN ?? "http://127.0.0.1:1438";
const browser = await chromium.launch({ headless: true });
try {
  const page = await browser.newPage();
  const errors = [];
  page.on("pageerror", error => errors.push(error.message));
  const cases = [
    ["marks-backward", "a\u0301\u0323b", 3, 3, "Backspace", "b", 0],
    ["marks-forward", "a\u0301\u0323b", 0, 0, "Delete", "b", 0],
    ["inside-marks", "a\u0301\u0323b", 1, 1, "Backspace", "b", 0],
    ["supplementary", "x😀y", 3, 3, "Backspace", "xy", 1],
    ["partial-selection", "x😀a\u0301y", 2, 4, "Delete", "xy", 1],
    ["ligature-interior", "office", 3, 3, "Backspace", "ofice", 2],
    ["rtl-mark", "אב\u05b0 ג", 3, 3, "Backspace", "א ג", 1],
    ["rtl-space", "אב\u05b0 ג", 3, 3, "Delete", "אב\u05b0ג", 3],
    ["preserve-decomposed", "xa\u0301\u0323", 1, 1, "Backspace", "a\u0301\u0323", 0],
  ];
  for (const [name, text, start, end, key, result, caret] of cases) {
    await page.goto(`${origin}/tests/fixtures/grapheme-editor.html?text=${encodeURIComponent(text)}`);
    const input = page.getByRole("textbox", { name: "Content" });
    await input.focus();
    await input.evaluate((node, { start, end }) => node.setSelectionRange(start, end, "backward"), { start, end });
    await input.press(key);
    await expect(input).toHaveValue(result);
    await expect(page.locator("#saved")).toHaveText(result);
    await expect(page.locator("#commits")).toHaveText("1");
    expect(await input.evaluate(node => [node.selectionStart, node.selectionEnd])).toEqual([caret, caret]);
    await input.press("Control+z");
    await expect(input).toHaveValue(text);
    await expect(page.locator("#commits")).toHaveText("2");
    await input.press("Control+y");
    await expect(input).toHaveValue(result);
    await expect(page.locator("#commits")).toHaveText("3");
    console.log(`PASS ${name}: exact text, caret, one commit, native undo/redo`);
  }
  const cdp = await page.context().newCDPSession(page);
  for (const [command, start] of [["deleteBackward", 3], ["deleteForward", 0]]) {
    const text = "a\u0301\u0323b";
    await page.goto(`${origin}/tests/fixtures/grapheme-editor.html?text=${encodeURIComponent(text)}`);
    const input = page.getByRole("textbox", { name: "Content" });
    await input.focus();
    await input.evaluate((node, start) => node.setSelectionRange(start, start), start);
    // Unidentified bypasses keydown preparation; the trusted browser edit reaches beforeinput.
    await cdp.send("Input.dispatchKeyEvent", { type: "rawKeyDown", key: "Unidentified", commands: [command] });
    await expect(input).toHaveValue("b");
    await expect(page.locator("#saved")).toHaveText("b");
    await expect(page.locator("#commits")).toHaveText("1");
    expect(await input.evaluate(node => [node.selectionStart, node.selectionEnd])).toEqual([0, 0]);
    await input.press("Control+z");
    await expect(input).toHaveValue(text);
    await expect(page.locator("#commits")).toHaveText("2");
    await input.press("Control+y");
    await expect(input).toHaveValue("b");
    await expect(page.locator("#commits")).toHaveText("3");
    console.log(`PASS trusted ${command} beforeinput: exact text, caret, one commit, native undo/redo`);
  }
  await cdp.detach();
  expect(errors).toEqual([]);
  console.log(`PASS ${cases.length + 2} cases; Chromium ${browser.version()}; Intl.Segmenter ${await page.evaluate(() => typeof Intl.Segmenter)}`);
} finally { await browser.close(); }
