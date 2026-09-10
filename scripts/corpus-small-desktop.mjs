// Real source-PDF selection/copy and mixed-geometry checks in the packaged app.
import { chromium, expect } from '@playwright/test';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { createHash } from 'node:crypto';
const exec = promisify(execFile);
const pid = process.env.FOLIO_APP_PID;
if (!pid || !/^\d+$/.test(pid)) throw Error('Set FOLIO_APP_PID to the isolated test app.');
const directory = resolve('tests/fixtures/corpus');
const output = resolve('artifacts/corpus-ui'); mkdirSync(output, { recursive: true });
const manifest = JSON.parse(readFileSync(resolve(directory, 'manifest.json'), 'utf8'));
const browser = await chromium.connectOverCDP(process.env.FOLIO_CDP_URL || 'http://127.0.0.1:9239');
const page = browser.contexts()[0].pages().find(p => !p.url().startsWith('devtools:'));
const errors = []; page.on('pageerror', e => errors.push(e.message));
const copies = [], geometry = [];
async function open(name) {
  const path = resolve(directory, name);
  const fixture = manifest.fixtures.find(f => f.file === name);
  expect(createHash('sha256').update(readFileSync(path)).digest('hex')).toBe(fixture.sha256);
  await page.getByRole('button', { name: (await page.getByRole('tab').count()) ? 'Open' : 'Open a PDF', exact: true }).click();
  await exec('pwsh', ['-NoProfile', '-File', resolve('scripts/set-native-dialog.ps1'), '-AppProcessId', pid, '-FilePath', path, '-Action', 'Open'], { windowsHide: true, timeout: 60000 });
  await expect(page.getByRole('tab', { name, exact: true })).toHaveAttribute('aria-selected', 'true');
  await expect(page.locator('.document-page').first().locator('.page-canvas image')).toHaveCount(1);
  await page.getByRole('button', { name: 'Select', exact: true }).click();
  return fixture;
}
async function copyPhrase(name, phrase) {
  const glyphs = page.locator('.document-page').first().locator('[data-pdf-character]');
  await expect.poll(async () => (await glyphs.allTextContents()).join('')).toContain(phrase);
  const chars = await glyphs.allTextContents();
  const offset = chars.join('').indexOf(phrase);
  let cursor = 0, start = -1, end = -1;
  for (let index = 0; index < chars.length; index++) {
    if (cursor === offset) start = index;
    cursor += chars[index].length;
    if (cursor === offset + phrase.length) { end = index; break; }
  }
  expect(start).toBeGreaterThanOrEqual(0); expect(end).toBeGreaterThanOrEqual(start);
  await glyphs.nth(start).scrollIntoViewIfNeeded();
  await glyphs.nth(end).scrollIntoViewIfNeeded();
  const a = await glyphs.nth(start).boundingBox(), b = await glyphs.nth(end).boundingBox();
  await page.evaluate(() => { window.__folioCorpusCopied = null; });
  await page.mouse.move(a.x + a.width * .25, a.y + a.height / 2);
  await page.mouse.down();
  await page.mouse.move(b.x + b.width * .75, b.y + b.height / 2, { steps: 15 });
  await page.mouse.up();
  await page.keyboard.press('Control+c');
  const actual = await page.evaluate(() => window.__folioCorpusCopied);
  expect(actual).toBe(phrase);
  copies.push({ fixture: name, expected: phrase, copied: actual, passed: true });
}
try {
  await page.evaluate(() => {
    window.__folioCorpusCopyObserver = event => { window.__folioCorpusCopied = event.clipboardData.getData('text/plain'); };
    window.addEventListener('copy', window.__folioCorpusCopyObserver);
  });
  await open('dense-tables.pdf');
  await copyPhrase('dense-tables.pdf', '001-01-01');
  await open('embedded-font.pdf');
  for (const phrase of ['café naïve Straße Ångström', 'Ελληνικά αβγδε Ω', 'Пример текста', '∑ ∫ √ ∞ ≠ ≤ ≥']) await copyPhrase('embedded-font.pdf', phrase);
  await page.screenshot({ path: resolve(output, 'embedded-unicode-copy.png') });
  const mixed = await open('mixed-geometry.pdf');
  await copyPhrase('mixed-geometry.pdf', 'FOLIO GEOMETRY PAGE 0001');
  for (let index = 0; index < mixed.pages; index++) {
    const slot = page.locator('.document-page').nth(index);
    await slot.evaluate(el => el.scrollIntoView({ block: 'start' }));
    const canvas = slot.locator('.page-canvas');
    await expect(canvas.locator('image')).toHaveCount(1);
    const viewBox = (await canvas.getAttribute('viewBox')).split(/\s+/).map(Number);
    expect(viewBox).toEqual([0, 0, ...mixed.dimensions[index]]);
    geometry.push({ page: index + 1, rotation: mixed.settings.rotations[index], viewBox, passed: true });
  }
  expect(errors).toEqual([]);
  await page.screenshot({ path: resolve(output, 'mixed-geometry.png') });
  const result = { passed: true, copies, geometry, errors, method: 'Actual native PDF glyph mouse selection with quarter-glyph endpoints and Control+C; observed text/plain copy event payload. No document edits.' };
  writeFileSync(resolve(output, 'small-results.json'), JSON.stringify(result, null, 2));
  console.log(JSON.stringify(result, null, 2));
} catch (error) {
  await page.screenshot({ path: resolve(output, 'small-failure.png') }).catch(() => {});
  writeFileSync(resolve(output, 'small-failure.txt'), String(error.stack));
  throw error;
} finally {
  await page.evaluate(() => { window.removeEventListener('copy', window.__folioCorpusCopyObserver); delete window.__folioCorpusCopyObserver; delete window.__folioCorpusCopied; }).catch(() => {});
  await browser.close();
}
