// Repository-native Windows/WebView acceptance using an owned app and public PDFs.
import {chromium, expect} from '@playwright/test';
import {execFile} from 'node:child_process';
import {promisify} from 'node:util';
import {copyFileSync, existsSync, readFileSync, writeFileSync} from 'node:fs';
import {resolve, basename} from 'node:path';
import {createHash} from 'node:crypto';

const root = resolve(process.env.FOLIO_READER_ROOT);
const run = resolve(process.env.FOLIO_RELEASE_RUN);
const state = JSON.parse(readFileSync(resolve(run, 'process.json'), 'utf8').replace(/^\uFEFF/, ''));
const exec = promisify(execFile), checks = [], errors = [];
const hash = path => createHash('sha256').update(readFileSync(path)).digest('hex');
const browser = await chromium.connectOverCDP(`http://127.0.0.1:${state.port}`);
const page = browser.contexts()[0].pages().find(p => !p.url().startsWith('devtools:'));
page.setDefaultTimeout(20000);
page.on('pageerror', error => errors.push(String(error)));
async function nativeDialog(title, path, action = 'Open') {
  await exec('pwsh', ['-NoProfile', '-File', resolve('scripts/set-release-dialog.ps1'),
    '-AppProcessId', String(state.pid), '-FilePath', path, '-Title', title, '-Action', action],
  {windowsHide: true, timeout: 25000});
}
async function open(path) {
  await page.getByRole('button', {name: /^(Open|Open a PDF)$/}).click();
  await nativeDialog('Open a PDF', path);
  await expect(page.getByRole('tab', {name: basename(path), exact: true})).toHaveAttribute('aria-selected', 'true');
  await expect(page.locator('.page-canvas').first().locator('image')).toHaveCount(1);
  await page.getByRole('button', {name: 'Select', exact: true}).click();
}
const point = (box, rotation, fraction) => rotation === 0
  ? [box.x + box.width * fraction, box.y + box.height / 2]
  : rotation === 90 ? [box.x + box.width / 2, box.y + box.height * fraction]
  : rotation === 180 ? [box.x + box.width * (1 - fraction), box.y + box.height / 2]
  : [box.x + box.width / 2, box.y + box.height * (1 - fraction)];
async function copyAndFind(rotation) {
  const phrase = 'Café old';
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
  expect(start).toBeGreaterThanOrEqual(0);
  expect(end).toBeGreaterThanOrEqual(start);
  await glyphs.nth(start).scrollIntoViewIfNeeded();
  await glyphs.nth(end).scrollIntoViewIfNeeded();
  const a = await glyphs.nth(start).boundingBox(), b = await glyphs.nth(end).boundingBox();
  expect(a).toBeTruthy(); expect(b).toBeTruthy();
  await page.evaluate(() => { window.__folioReaderCopied = null; });
  await page.mouse.move(...point(a, rotation, .25));
  await page.mouse.down();
  await page.mouse.move(...point(b, rotation, .75), {steps: 15});
  await page.mouse.up();
  await page.keyboard.press('Control+c');
  const copied = await page.evaluate(() => window.__folioReaderCopied);
  expect(copied).toBe(phrase);
  await page.keyboard.press('Control+f');
  await page.getByRole('searchbox', {name: 'Find in document'}).fill(phrase);
  await expect(page.getByText('1 of 1', {exact: true})).toBeVisible();
  const highlight = page.locator('.search-highlight.active');
  await expect(highlight).toHaveCount(1);
  const hit = await highlight.boundingBox();
  const canvas = await page.locator('.page-canvas').first().boundingBox();
  expect(hit.x).toBeGreaterThanOrEqual(canvas.x - 1);
  expect(hit.y).toBeGreaterThanOrEqual(canvas.y - 1);
  expect(hit.x + hit.width).toBeLessThanOrEqual(canvas.x + canvas.width + 1);
  expect(hit.y + hit.height).toBeLessThanOrEqual(canvas.y + canvas.height + 1);
  await page.screenshot({path: resolve(run, `search-${rotation}-${checks.length}.png`)});
  await page.getByRole('searchbox', {name: 'Find in document'}).press('Escape');
  return {copied, matches: 1, highlightInsidePage: true};
}
try {
  await page.evaluate(() => {
    window.__folioReaderCopyObserver = event => { window.__folioReaderCopied = event.clipboardData.getData('text/plain'); };
    window.addEventListener('copy', window.__folioReaderCopyObserver);
  });
  for (const rotation of [0, 90, 180, 270]) {
    const original = resolve(run, `Reading ${rotation}.pdf`), saved = resolve(run, `Saved ${rotation}.pdf`);
    if ([original, saved].some(existsSync)) throw Error('Preserving previous desktop evidence');
    copyFileSync(resolve(root, `native/r${rotation}-s1-source.pdf`), original);
    const originalSha256 = hash(original);
    await open(original);
    const source = await copyAndFind(rotation);
    await page.getByRole('button', {name: 'Save a copy', exact: true}).click();
    await nativeDialog('Save a PDF copy', saved, 'Save');
    await expect.poll(() => existsSync(saved)).toBe(true);
    await open(saved);
    const reopened = await copyAndFind(rotation);
    expect(hash(original)).toBe(originalSha256);
    checks.push({rotation, source, reopened, originalSha256, savedSha256: hash(saved)});
  }
  expect(errors).toEqual([]);
  writeFileSync(resolve(run, 'results.json'), JSON.stringify({passed: true, checks, errors, exe: state,
    method: 'Actual mouse selection and Control+C text/plain event payload; Find highlight; native save/reopen'}, null, 2));
  console.log('PASS', checks.length, 'rotations with original and reopened copy/search');
} catch (error) {
  await page.screenshot({path: resolve(run, 'failure.png')}).catch(() => {});
  throw error;
} finally {
  await page.evaluate(() => { window.removeEventListener('copy', window.__folioReaderCopyObserver); }).catch(() => {});
  await browser.close();
}
