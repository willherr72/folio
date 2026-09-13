import { chromium, expect } from "@playwright/test";
import { mkdir, readdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { inkTolerance } from "./preview-ink-comparison.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const origin = process.env.FOLIO_SHARED_PREVIEW_TEST_ORIGIN ?? "http://127.0.0.1:1427";
const input = path.join(root, "artifacts/shaped-text/shared-preview-native");
const output = path.join(root, "artifacts/shaped-text/shared-preview-browser");
const stems = (await readdir(input)).filter(name => name.endsWith(".preview.json")).map(name => name.replace(/\.preview\.json$/, "")).sort();
expect(stems).toHaveLength(32);
expect(stems).toEqual(["ligatures-on", "ligatures-off", "composed", "decomposed", "two-axis-marks", "arabic", "indic", "supplementary"]
  .flatMap(name => [0, 90, 180, 270].map(rotation => `${name}-${rotation}`)).sort());
await mkdir(output, { recursive: true });
const browser = await chromium.launch({ headless: true });
const cases = [], errors = [];
try {
  const page = await browser.newPage({ viewport: { width: 2200, height: 1600 }, deviceScaleFactor: 1 });
  page.on("pageerror", error => errors.push(error.message));
  for (const stem of stems) for (const zoom of [150, 300]) {
    const native = JSON.parse(await readFile(path.join(input, `${stem}.preview.json`), "utf8"));
    await page.goto(`${origin}/tests/fixtures/shaped-text-preview.html?case=${encodeURIComponent(stem)}&zoom=${zoom}`);
    const svg = page.locator("#native-preview svg"), pdfium = page.locator("#pdfium-preview");
    await expect(svg).toBeVisible();
    await expect(svg.locator("text, tspan, foreignObject")).toHaveCount(0);
    await expect(svg.locator("use")).toHaveCount(native.glyphs.length);
    await pdfium.evaluate(async img => { await img.decode(); });
    const png = await svg.screenshot({ path: path.join(output, `${stem}-${zoom}.browser.png`) });
    const result = await page.evaluate(async ({ png }) => {
      const { compareInk } = await import("/scripts/preview-ink-comparison.mjs");
      const reference = document.querySelector("#pdfium-preview");
      const actual = new Image(); actual.src = `data:image/png;base64,${png}`; await actual.decode();
      const width = reference.naturalWidth, height = reference.naturalHeight;
      if (actual.naturalWidth !== width || actual.naturalHeight !== height) return { pass: false, reason: "Raster dimensions differ", actual: [actual.naturalWidth, actual.naturalHeight], expected: [width, height] };
      const canvas = document.createElement("canvas"); canvas.width = width; canvas.height = height;
      const context = canvas.getContext("2d", { willReadFrequently: true });
      const pixels = image => { context.clearRect(0, 0, width, height); context.drawImage(image, 0, 0); return context.getImageData(0, 0, width, height).data; };
      return { width, height, ...compareInk(width, height, pixels(actual), pixels(reference)) };
    }, { png: png.toString("base64") });
    await page.screenshot({ path: path.join(output, `${stem}-${zoom}.comparison.png`), fullPage: true });
    cases.push({ stem, zoom, rotation: native.rotation, text: native.text, fontId: native.fontId, ...result });
    console.log(`${result.pass ? "PASS" : "FAIL"} ${stem} ${zoom}% ${JSON.stringify(result)}`);
  }
  await writeFile(path.join(output, "results.json"), JSON.stringify({ description: "Browser SVG versus PDFium: geometry comparison with explicit rasterizer tolerance; not pixel identity", inkTolerance, errors, cases }, null, 2));
  expect(errors).toEqual([]);
  expect(cases).toHaveLength(64);
  expect(cases.filter(item => !item.pass)).toEqual([]);
} finally { await browser.close(); }
