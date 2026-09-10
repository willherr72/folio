import { chromium, expect } from "@playwright/test";
const browser = await chromium.launch({ headless: true });
const origin = process.env.FOLIO_SEARCH_TEST_ORIGIN ?? "http://127.0.0.1:1434";
try {
  for (const zoom of [90, 170]) {
    const context = await browser.newContext({ viewport: { width: 1400, height: 900 } });
    await context.addInitScript((zoom) => localStorage.setItem("folio.preferences.v1", JSON.stringify({ theme: "light", viewMode: "continuous", defaultZoom: zoom, penColor: "#2D2A26", penWidth: 2 })), zoom);
    const page = await context.newPage();
    const errors = [];
    page.on("pageerror", (error) => errors.push(error.message));
    for (const rotation of [0, 90, 180, 270]) {
      await page.goto(`${origin}/?demo=1`);
      await expect(page.getByRole("tab", { name: "Folio welcome.pdf" })).toBeVisible();
      for (let step = 0; step < rotation / 90; step++) await page.getByRole("button", { name: "Rotate", exact: true }).click();
      await page.keyboard.press("Control+f");
      await page.getByRole("searchbox", { name: "Find in document" }).fill("local");
      await expect(page.getByText("1 of 4", { exact: true })).toBeVisible();
      const checkActive = async () => {
        const highlight = page.locator(".search-highlight.active");
        await expect(highlight).toHaveCount(1);
        await expect.poll(async () => {
          const hit = await highlight.boundingBox();
          const viewport = await page.getByRole("main", { name: "Document", exact: true }).boundingBox();
          return !!hit && hit.x >= viewport.x - 1 && hit.y >= viewport.y - 1 && hit.x + hit.width <= viewport.x + viewport.width + 1 && hit.y + hit.height <= viewport.y + viewport.height + 1;
        }).toBe(true);
      };
      await checkActive();
      await page.getByRole("searchbox").press("Shift+Enter");
      await expect(page.getByText("4 of 4", { exact: true })).toBeVisible();
      await checkActive();
      await page.getByRole("searchbox").press("Enter");
      await expect(page.getByText("1 of 4", { exact: true })).toBeVisible();
      await checkActive();
      await page.getByRole("searchbox").press("Escape");
      await expect(page.getByRole("searchbox")).toHaveCount(0);
      await expect(page.locator(".search-highlight")).toHaveCount(0);
      console.log(`Search rectangle navigation and wrap passed: rotation=${rotation}, zoom=${zoom}%`);
    }
    expect(errors).toEqual([]);
    await context.close();
  }
} finally { await browser.close(); }
