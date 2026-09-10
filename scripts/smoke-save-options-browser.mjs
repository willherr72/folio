import { chromium, expect } from "@playwright/test";
import { mkdirSync, writeFileSync } from "node:fs";
const origin = process.env.FOLIO_SAVE_OPTIONS_TEST_ORIGIN ?? "http://127.0.0.1:1427";
const browser = await chromium.launch({ headless: true });
const directory = "artifacts/save-options-review"; mkdirSync(directory, { recursive: true });
const results = [];
try {
  for (const width of [1280, 900]) for (const theme of ["light", "dark"]) {
    const context = await browser.newContext({ viewport: { width, height: 800 } });
    await context.addInitScript(theme => localStorage.setItem("folio.preferences.v1", JSON.stringify({ theme, viewMode: "continuous", defaultZoom: 90, penColor: "#2D2A26", penWidth: 2 })), theme);
    const page = await context.newPage();
    await page.goto(`${origin}/tests/fixtures/tab-performance.html?characters=0`);
    await page.getByRole("button", { name: "Open a PDF", exact: true }).click();
    await page.getByRole("tab", { name: "PDF1.pdf", exact: true }).waitFor();
    await expect(page.locator("html")).toHaveAttribute("data-theme", theme);
    const trigger = page.getByRole("button", { name: "Save options", exact: true });
    // At 900px the toolbar must scroll to expose this trigger. Its queued
    // scroll event must keep the first opening intact, without a second click.
    await trigger.click();
    await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
    const menu = page.getByRole("menu", { name: "Save options", exact: true }); await expect(menu).toBeVisible();
    const bounds = await menu.boundingBox();
    expect(bounds.x).toBeGreaterThanOrEqual(0); expect(bounds.x + bounds.width).toBeLessThanOrEqual(width); expect(bounds.y + bounds.height).toBeLessThanOrEqual(800);
    const unoccluded = await menu.evaluate(element => {
      const box = element.getBoundingClientRect();
      return [[box.left + 10, box.top + 10], [box.right - 10, box.bottom - 10]].every(([x, y]) => element.contains(document.elementFromPoint(x, y)));
    });
    expect(unoccluded).toBe(true);
    await expect(page.getByRole("menuitem", { name: "Save editable PDF", exact: true })).toBeFocused();
    await page.keyboard.press("ArrowDown"); await expect(page.getByRole("menuitem", { name: "Flatten text and ink", exact: true })).toBeFocused();
    await page.screenshot({ path: `${directory}/${width}-${theme}.png` });
    await page.keyboard.press("Escape"); await expect(trigger).toBeFocused();
    await trigger.click(); await page.keyboard.press("Tab"); await expect(menu).toHaveCount(0); await expect(page.getByRole("button", { name: "Page 1 of 1", exact: true })).toBeFocused();
    await trigger.click(); await page.keyboard.press("Shift+Tab"); await expect(menu).toHaveCount(0); await expect(page.getByRole("button", { name: "Save a copy", exact: true })).toBeFocused();
    results.push({ width, theme, bounds, unoccluded, firstOpenStable: true, keyboardOrderCorrect: true });
    console.log(`Save options ${width}px ${theme}: unclipped first opening, arrows/Escape and adjacent Tab order passed`);
    await context.close();
  }
  writeFileSync(`${directory}/results.json`, JSON.stringify(results, null, 2));
} finally { await browser.close(); }