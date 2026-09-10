import { chromium, expect } from "@playwright/test";
const origin = process.env.FOLIO_ANNOTATION_TEST_ORIGIN ?? "http://127.0.0.1:1426";
const browser = await chromium.launch({ headless: true });
try {
  const context = await browser.newContext({ permissions: ["clipboard-read", "clipboard-write"] });
  const page = await context.newPage();
  const failures = []; page.on("pageerror", error => failures.push(error.message));
  for (const zoom of [90, 170]) for (const intrinsic of [0, 90, 180, 270]) for (const rotation of [0, 90, 180, 270]) {
    const direction = (intrinsic + rotation) % 360;
    await page.goto(`${origin}/tests/fixtures/annotations.html?rotation=${rotation}&intrinsic=${intrinsic}&zoom=${zoom}`);
    const glyphs = page.locator("[data-pdf-character]"); await expect(glyphs).toHaveCount(21);
    const first = await glyphs.first().boundingBox(), last = await glyphs.last().boundingBox();
    // Aim within each glyph's first/last quarter; subpixel transformed edges can
    // resolve to the opposite caret in Chromium at fractional zoom.
    const start = direction === 0 ? [first.x + first.width * .25, first.y + first.height / 2] : direction === 90 ? [first.x + first.width / 2, first.y + first.height * .25] : direction === 180 ? [first.x + first.width * .75, first.y + first.height / 2] : [first.x + first.width / 2, first.y + first.height * .75];
    const end = direction === 0 ? [last.x + last.width * .75, last.y + last.height / 2] : direction === 90 ? [last.x + last.width / 2, last.y + last.height * .75] : direction === 180 ? [last.x + last.width * .25, last.y + last.height / 2] : [last.x + last.width / 2, last.y + last.height * .25];
    const sourceRects = [{ x: 30, y: 40, width: 108, height: 20 }, { x: 30, y: 75, width: 132, height: 20 }];
    const expected = sourceRects.map(rect => {
      const { x, y, width: w, height: h } = rect;
      return intrinsic === 90 ? { x: 320 - y - h, y: x, width: h, height: w } : intrinsic === 180 ? { x: 240 - x - w, y: 320 - y - h, width: w, height: h } : intrinsic === 270 ? { x: y, y: 240 - x - w, width: h, height: w } : rect;
    });
    for (const reverse of [false, true]) {
      await page.mouse.move(...(reverse ? end : start)); await page.mouse.down(); await page.mouse.move(...(reverse ? start : end), { steps: 15 }); await page.mouse.up();
      await expect(page.locator(".annotation-highlight")).toHaveCount(reverse ? 2 : 1);
      const overlays = JSON.parse(await page.getByLabel("Annotations").textContent());
      expect(overlays.at(-1).rects, `intrinsic=${intrinsic} rotation=${rotation} zoom=${zoom} reverse=${reverse}`).toEqual(expected);
    }
    await page.getByRole("button", { name: "select", exact: true }).click();
    await page.mouse.move(...start); await page.mouse.down(); await page.mouse.move(...end, { steps: 15 }); await page.mouse.up(); await page.keyboard.press("Control+c");
    expect(await page.evaluate(() => navigator.clipboard.readText())).toBe("Hello PDF\r\nSecond line");
    expect(await page.getByLabel("Selection state").textContent()).not.toBe("moved overlay");
    console.log(`Highlight source rects and preserved copy: intrinsic ${intrinsic}, editor ${rotation}, ${zoom}% zoom`);
  }
  for (const destination of [1, 4]) {
    await page.goto(`${origin}/tests/fixtures/annotations-multipage.html`);
    const first = page.locator('[data-page-id="multi-0"] [data-pdf-character]').first(); await first.waitFor();
    const start = await first.boundingBox();
    await page.mouse.move(start.x + start.width * .25, start.y + start.height / 2); await page.mouse.down();
    if (destination > 1) {
      await page.mouse.move(start.x + 35, start.y + start.height / 2, { steps: 5 });
      await page.getByRole("main", { name: "Document" }).evaluate((host, index) => { host.scrollTop = index * 312; }, destination);
    }
    const last = page.locator(`[data-page-id="multi-${destination}"] [data-pdf-character]`).last(); await last.waitFor();
    const end = await last.boundingBox();
    await page.mouse.move(end.x + end.width * .75, end.y + end.height / 2, { steps: 20 }); await page.mouse.up();
    await expect.poll(async () => JSON.parse(await page.getByLabel("Annotations").textContent()).length).toBe(destination + 1);
    const changes = JSON.parse(await page.getByLabel("Annotations").textContent());
    expect(changes.map(change => change.pageId)).toEqual(Array.from({ length: destination + 1 }, (_, index) => `multi-${index}`));
    for (const change of changes) expect(change.rects).toEqual([{ x: 30, y: 40, width: 108, height: 20 }, { x: 30, y: 75, width: 132, height: 20 }]);
    console.log(`Native cross-page mouse selection committed ${destination + 1} pages, including pages loaded during scrolling`);
  }
  expect(failures).toEqual([]);
} finally { await browser.close(); }