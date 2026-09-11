import { chromium, expect } from "@playwright/test";
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";

// Diagnostic fixture profiler. Each round switches to PDF1 and back to PDF2.
// FOLIO_PROFILE_URL may include query parameters; explicit character counts win.
const url = new URL(process.env.FOLIO_PROFILE_URL || "http://127.0.0.1:1428/tests/fixtures/tab-performance.html");
const characterCount = positiveInteger("FOLIO_PROFILE_CHARACTERS", url.searchParams.get("characters") ?? 6000);
const rounds = positiveInteger("FOLIO_PROFILE_ROUNDS", 2);
const readinessTimeout = positiveInteger("FOLIO_PROFILE_TIMEOUT_MS", 15000);
const label = process.env.FOLIO_PROFILE_LABEL || "baseline";
const outputDirectory = "artifacts/text-profile";
url.searchParams.set("characters", String(characterCount));
mkdirSync(outputDirectory, { recursive: true });

function positiveInteger(name, fallback) {
  const value = Number(process.env[name] ?? fallback);
  if (!Number.isSafeInteger(value) || value <= 0) throw new Error(`${name} must be a positive integer`);
  return value;
}

const browser = await chromium.launch({ headless: true });
try {
  const page = await browser.newPage({ viewport: { width: 1440, height: 1050 } });
  await page.goto(url.href);
  for (const name of ["Open a PDF", "Open"]) {
    await page.getByRole("button", { name, exact: true }).click();
    await expect(page.locator("[data-pdf-character]")).toHaveCount(characterCount, { timeout: readinessTimeout });
    await page.waitForFunction(() => document.querySelector(".page-canvas image"), undefined, { timeout: readinessTimeout });
  }

  const cdp = await page.context().newCDPSession(page);
  const readMetrics = async () => Object.fromEntries(
    (await cdp.send("Performance.getMetrics")).metrics.map(({ name, value }) => [name, value]),
  );
  const readWork = () => page.evaluate(() => ({ ...window.tabPerf }));
  const workBefore = await readWork();
  expect(workBefore.renders).toBeGreaterThan(0);
  expect(workBefore.textRequests).toBe(2);
  await cdp.send("Performance.enable");
  await cdp.send("Profiler.enable");
  await cdp.send("Profiler.start");
  const metricsBefore = await readMetrics();
  const times = [];

  for (let round = 0; round < rounds; round++) {
    for (const name of ["PDF1.pdf", "PDF2.pdf"]) {
      const elapsed = await page.evaluate(async ({ name, characterCount, readinessTimeout }) => {
        const tab = [...document.querySelectorAll('[role="tab"]')].find(element => element.getAttribute("aria-label") === name);
        if (!tab) throw new Error(`Missing tab: ${name}`);
        const start = performance.now();
        tab.click();
        await new Promise((resolve, reject) => {
          let frame;
          const timeout = setTimeout(() => {
            cancelAnimationFrame(frame);
            reject(new Error(`Timed out waiting for ${name} after ${readinessTimeout} ms`));
          }, readinessTimeout);
          const ready = () => {
            const glyphs = document.querySelectorAll("[data-pdf-character]");
            if (tab.getAttribute("aria-selected") === "true" && glyphs.length === characterCount
                && glyphs[characterCount - 1].style.transform && document.querySelector(".page-canvas image")) {
              frame = requestAnimationFrame(() => {
                frame = requestAnimationFrame(() => { clearTimeout(timeout); resolve(); });
              });
            } else frame = requestAnimationFrame(ready);
          };
          frame = requestAnimationFrame(ready);
        });
        return performance.now() - start;
      }, { name, characterCount, readinessTimeout });
      times.push(elapsed);
      await expect(page.locator("[data-pdf-character]")).toHaveCount(characterCount);
    }
  }

  const metricsAfter = await readMetrics();
  const { profile } = await cdp.send("Profiler.stop");
  const workAfter = await readWork();
  const repeatWork = {
    renders: workAfter.renders - workBefore.renders,
    textRequests: workAfter.textRequests - workBefore.textRequests,
  };
  writeFileSync(join(outputDirectory, `${label}.cpuprofile`), JSON.stringify(profile));

  const durations = new Map();
  for (let index = 0; index < (profile.samples?.length ?? 0); index++) {
    const id = profile.samples[index];
    durations.set(id, (durations.get(id) ?? 0) + profile.timeDeltas[index]);
  }
  const hottest = profile.nodes
    .map(node => ({ ...node.callFrame, ms: (durations.get(node.id) ?? 0) / 1000 }))
    .sort((a, b) => b.ms - a.ms)
    .slice(0, 25);
  const result = {
    url: url.href,
    characterCount,
    rounds,
    times,
    repeatWork,
    metrics: Object.fromEntries(Object.keys(metricsAfter)
      .filter(key => /Duration|Count/.test(key))
      .map(key => [key, metricsAfter[key] - metricsBefore[key]])),
    hottest,
  };
  writeFileSync(join(outputDirectory, `${label}.json`), JSON.stringify(result, null, 2));
  console.log(JSON.stringify(result, null, 2));
  expect(repeatWork, "Warm tab switches must reuse both raster and source text caches").toEqual({ renders: 0, textRequests: 0 });
} finally {
  await browser.close();
}
