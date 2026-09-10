import { chromium, expect } from "@playwright/test";
const origin = process.env.FOLIO_TEXT_ROTATION_TEST_ORIGIN ?? "http://127.0.0.1:1427";
const browser = await chromium.launch({ headless: true });
try {
  const page = await browser.newPage({ viewport: { width: 1100, height: 700 } });
  const errors = []; page.on("pageerror", error => errors.push(error.message));
  for (const zoom of [90, 170]) for (const textRotation of [0, 90, 180, 270]) for (const pageRotation of [0, 90, 180, 270]) {
    await page.goto(`${origin}/tests/fixtures/text-rotation.html?textRotation=${textRotation}&pageRotation=${pageRotation}&zoom=${zoom}`);
    const text = page.locator('.page-canvas [data-overlay] text'); await expect(text).toBeVisible();
    const lines = text.locator("tspan"); const matches = page.locator(".search-highlight"); await expect(matches).toHaveCount(2);
    const selection = await page.locator(".selection-box").boundingBox();
    for (let index = 0; index < 2; index++) {
      const glyphs = await lines.nth(index).boundingBox(); const match = await matches.nth(index).boundingBox();
      const allowance = 24 * zoom / 100 * .35; // Existing added-text advances are approximate.
      expect(Math.abs(glyphs.x + glyphs.width / 2 - match.x - match.width / 2)).toBeLessThan(allowance);
      expect(Math.abs(glyphs.y + glyphs.height / 2 - match.y - match.height / 2)).toBeLessThan(allowance);
      expect(glyphs.x).toBeGreaterThanOrEqual(selection.x - 1); expect(glyphs.y).toBeGreaterThanOrEqual(selection.y - 1);
      expect(glyphs.x + glyphs.width).toBeLessThanOrEqual(selection.x + selection.width + 1); expect(glyphs.y + glyphs.height).toBeLessThanOrEqual(selection.y + selection.height + 1);
    }
    const viewport = await page.getByRole("main", { name: "Document" }).boundingBox(); const active = await matches.first().boundingBox();
    expect(active.x).toBeGreaterThanOrEqual(viewport.x); expect(active.y).toBeGreaterThanOrEqual(viewport.y);
    expect(active.x + active.width).toBeLessThanOrEqual(viewport.x + viewport.width); expect(active.y + active.height).toBeLessThanOrEqual(viewport.y + viewport.height);
    const thumbnailText = page.locator(".thumbnail-canvas text"); await expect(thumbnailText.locator("tspan")).toHaveCount(2);
    expect(await thumbnailText.getAttribute("transform")).toBe(await text.getAttribute("transform"));
    const first = await lines.first().boundingBox();
    await page.mouse.move(first.x + first.width / 2, first.y + first.height / 2); await page.mouse.down(); await page.mouse.move(first.x + first.width / 2 + 17, first.y + first.height / 2 + 34, { steps: 8 }); await page.mouse.up();
    const saved = JSON.parse(await page.getByLabel("Text state").textContent());
    const dx = 17 * 100 / zoom, dy = 34 * 100 / zoom;
    const shift = pageRotation === 90 ? [dy, -dx] : pageRotation === 180 ? [-dx, -dy] : pageRotation === 270 ? [-dy, dx] : [dx, dy];
    expect(saved.x).toBeCloseTo(240 + shift[0], 1); expect(saved.y).toBeCloseTo(280 + shift[1], 1); expect(saved.rotation).toBe(textRotation);
    console.log(`Reopened text ${textRotation}°, editor ${pageRotation}°, ${zoom}%: search/selection/thumbnail geometry and native drag passed`);
  }
  expect(errors).toEqual([]);
} finally { await browser.close(); }