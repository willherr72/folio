import { chromium, expect } from "@playwright/test";
import { mkdirSync } from "node:fs";
const origin = process.env.FOLIO_ANNOTATION_TEST_ORIGIN ?? "http://127.0.0.1:1426";
const browser = await chromium.launch({ headless: true });
mkdirSync("artifacts", { recursive: true });
try {
  const page = await browser.newPage();
  async function pixels(selector, path) {
    const screenshot = await page.locator(selector).screenshot({ path });
    return page.evaluate(async base64 => {
      const picture = new Image(); picture.src = `data:image/png;base64,${base64}`; await picture.decode();
      const canvas = document.createElement("canvas"); canvas.width = picture.width; canvas.height = picture.height;
      const context = canvas.getContext("2d"); context.drawImage(picture, 0, 0);
      const data = context.getImageData(0, 0, picture.width, picture.height).data;
      let black = 0, yellow = 0;
      for (let index = 0; index < data.length; index += 4) {
        if (data[index] < 60 && data[index + 1] < 60 && data[index + 2] < 60) black++;
        if (data[index] > 200 && data[index + 1] > 200 && data[index + 2] < 50) yellow++;
      }
      return { black, yellow };
    }, screenshot.toString("base64"));
  }
  for (const rotation of [0, 90]) {
    await page.goto(`${origin}/tests/fixtures/annotations.html?opacity=0&thumbnail=1&zoom=100&rotation=${rotation}`);
    await expect(page.locator(".page-canvas image")).toHaveCount(1); await expect(page.locator(".thumbnail-canvas image")).toHaveCount(1);
    const baseline = {};
    for (const name of ["page", "thumbnail"]) baseline[name] = await pixels(`.${name}-canvas`, `artifacts/highlight-${name}-${rotation}-baseline.png`);
    await page.goto(`${origin}/tests/fixtures/annotations.html?opacity=1&thumbnail=1&zoom=100&rotation=${rotation}`);
    await expect(page.locator(".page-canvas image")).toHaveCount(1); await expect(page.locator(".thumbnail-canvas image")).toHaveCount(1);
    for (const name of ["page", "thumbnail"]) {
      const highlighted = await pixels(`.${name}-canvas`, `artifacts/highlight-${name}-${rotation}-opaque.png`);
      expect(baseline[name].black, `${name} contains source text`).toBeGreaterThan(name === "thumbnail" ? 0 : 20);
      expect(highlighted.yellow, `${name} contains opaque highlight color`).toBeGreaterThan(1000);
      expect(highlighted.black, `${name} retains black source text under an opacity=1 highlight`).toBeGreaterThanOrEqual(baseline[name].black * .95);
      console.log(`${name} rotation${rotation}: opacity1 retains ${highlighted.black}/${baseline[name].black} black text pixels, ${highlighted.yellow} highlight pixels`);
    }
  }
} finally { await browser.close(); }