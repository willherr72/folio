// Real packaged WebView benchmark. Requires an isolated test process owned by this run.
// Leaves the app open for further smoke tests; never prints or closes user documents.
import { chromium, expect } from '@playwright/test';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { readFileSync, mkdirSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { createHash } from 'node:crypto';
const exec = promisify(execFile);
const pid = process.env.FOLIO_APP_PID;
if (!pid || !/^\d+$/.test(pid)) throw Error('Set FOLIO_APP_PID to the isolated corpus test app.');
const corpus = resolve(process.env.FOLIO_CORPUS_DIR || 'artifacts/corpus');
const output = resolve(process.env.FOLIO_CORPUS_UI_OUTPUT || 'artifacts/corpus-ui');
mkdirSync(output, { recursive: true });
const names = ['dense-300-pages.pdf', 'scan-40-pages.pdf'];
const manifest = JSON.parse(readFileSync(resolve(corpus, 'manifest.json'), 'utf8'));
for (const name of names) {
  const actual = createHash('sha256').update(readFileSync(resolve(corpus, name))).digest('hex');
  if (actual !== manifest.fixtures.find(f => f.file === name)?.sha256) throw Error('Corpus checksum mismatch: ' + name);
}
const browser = await chromium.connectOverCDP(process.env.FOLIO_CDP_URL || 'http://127.0.0.1:9228');
const page = browser.contexts()[0].pages().find(p => !p.url().startsWith('devtools:'));
const errors = [];
page.on('pageerror', e => errors.push(e.message));
const cdp = await page.context().newCDPSession(page);
const frame = () => page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
try {
  await cdp.send('Performance.enable');
  await cdp.send('Network.enable');
  const nativeRequests = [];
  cdp.on('Network.requestWillBeSent', event => {
    const url = new URL(event.request.url);
    const command = decodeURIComponent(url.pathname.replace(/^\//, ''));
    if ((url.hostname === 'ipc.localhost' || url.protocol === 'ipc:') && ['render_page', 'page_text'].includes(command)) nativeRequests.push(command);
  });
  const opening = [], switching = [], scrolling = [], memory = [];
  const memorySnapshot = async phase => {
    const { stdout } = await exec("pwsh", ["-NoProfile", "-File", resolve("scripts/corpus-process-memory.ps1"), "-AppProcessId", pid], { windowsHide: true, timeout: 30000 });
    memory.push({ phase, ...JSON.parse(stdout.replace(/^\uFEFF/, "")) });
  };
  await memorySnapshot("before-opening");
  for (const name of names) {
    if (await page.getByRole('tab', { name, exact: true }).count()) throw Error('Use a fresh isolated profile: corpus tab already open.');
    await page.getByRole('button', { name: (await page.getByRole('tab').count()) === 0 ? 'Open a PDF' : 'Open', exact: true }).click();
    const begin = performance.now();
    await exec('pwsh', ['-NoProfile', '-File', resolve('scripts/set-native-dialog.ps1'), '-AppProcessId', pid, '-FilePath', resolve(corpus, name), '-Action', 'Open'], { windowsHide: true, timeout: 60000 });
    await expect(page.getByRole('tab', { name, exact: true })).toHaveAttribute('aria-selected', 'true', { timeout: 90000 });
    await expect(page.locator('.document-page').first().locator('.page-canvas image')).toHaveCount(1, { timeout: 90000 });
    if (name.startsWith('dense')) await expect.poll(() => page.locator('[data-pdf-character]').count(), { timeout: 90000 }).toBeGreaterThan(1000);
    await frame();
    opening.push({ fixture: name, dialogAndOpenToFirstPageMs: performance.now() - begin, mountedPages: await page.locator('.page-canvas').count(), textGlyphs: await page.locator('[data-pdf-character]').count() });
    await memorySnapshot('opened-' + name);
  }
  expect(nativeRequests).toContain('render_page');
  expect(nativeRequests).toContain('page_text');
  const beforeWarm = nativeRequests.length;
  for (let round = 0; round < 6; round++) for (const name of names) {
    const begin = performance.now();
    await page.getByRole('tab', { name, exact: true }).click();
    await expect(page.getByRole('tab', { name, exact: true })).toHaveAttribute('aria-selected', 'true');
    await expect(page.locator('.document-page').first().locator('.page-canvas image')).toHaveCount(1, { timeout: 30000 });
    if (name.startsWith('dense')) await expect.poll(() => page.locator('[data-pdf-character]').count()).toBeGreaterThan(1000);
    await frame();
    switching.push({ fixture: name, round, readyMs: performance.now() - begin });
  }
  const warmCacheRequests = nativeRequests.slice(beforeWarm);
  expect(warmCacheRequests).toEqual([]);
  await memorySnapshot('warm-switches-complete');
  for (const name of names) {
    await page.getByRole('tab', { name, exact: true }).click();
    for (const index of [1, 2, 3, 4, 5, 20, 0]) {
      const slot = page.locator('.document-page').nth(index);
      const begin = performance.now();
      await slot.evaluate(el => el.scrollIntoView({ block: 'start' }));
      await expect(slot.locator('.page-canvas image')).toHaveCount(1, { timeout: 30000 });
      await frame();
      const mountedPages = await page.locator('.page-canvas').count();
      expect(mountedPages).toBeLessThan(manifest.fixtures.find(f => f.file === name).pages);
      scrolling.push({ fixture: name, page: index + 1, readyMs: performance.now() - begin, mountedPages, textGlyphs: await page.locator('[data-pdf-character]').count() });
    }
  }
  await memorySnapshot("scrolling-complete");
  const metrics = Object.fromEntries((await cdp.send('Performance.getMetrics')).metrics.map(m => [m.name, m.value]));
  expect(errors).toEqual([]);
  await page.screenshot({ path: resolve(output, 'corpus-desktop.png') });
  const result = { passed: true, pid: Number(pid), opening, switching, scrolling, warmCacheRequests, memory, metrics, errors,
    method: 'Playwright wall-clock readiness includes automation polling and IPC. Open includes native dialog helper. Warm switches wait for raster/text and two animation frames. Native timings are measured separately. No absolute performance threshold asserted.' };
  writeFileSync(resolve(output, 'results.json'), JSON.stringify(result, null, 2));
  console.log(JSON.stringify(result, null, 2));
} catch (error) {
  writeFileSync(resolve(output, 'failure.txt'), String(error.stack));
  await page.screenshot({ path: resolve(output, 'failure.png') }).catch(() => {});
  throw error;
} finally {
  await cdp.detach(); await browser.close();
}
