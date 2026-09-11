import { chromium, expect } from '@playwright/test';
import { readFileSync, mkdirSync, writeFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
const native = JSON.parse(readFileSync('tests/fixtures/reader-selection/native.json'));
for (const specimen of native.cases) for (const [file, expected] of [[specimen.png, specimen.pngSha256], [specimen.layout, specimen.layoutSha256]]) {
  expect(createHash('sha256').update(readFileSync(`tests/fixtures/reader-selection/${file}`)).digest('hex')).toBe(expected);
}
const browser = await chromium.launch({ headless: true });
const results = [];
try {
  const context = await browser.newContext({ viewport: { width: 1400, height: 1100 }, permissions: ['clipboard-read', 'clipboard-write'] });
  const page = await context.newPage();
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  async function drag(locator, rotation, fraction = 1) {
    const box = await locator.boundingBox();
    expect(box).toBeTruthy();
    const start = rotation === 0 ? [box.x + .3, box.y + box.height / 2] : rotation === 90 ? [box.x + box.width / 2, box.y + .3] : rotation === 180 ? [box.x + box.width - .3, box.y + box.height / 2] : [box.x + box.width / 2, box.y + box.height - .3];
    const axis = rotation === 0 ? [box.width - .6, 0] : rotation === 90 ? [0, box.height - .6] : rotation === 180 ? [-box.width + .6, 0] : [0, -box.height + .6];
    await page.mouse.move(...start); await page.mouse.down();
    await page.mouse.move(start[0] + axis[0] * fraction, start[1] + axis[1] * fraction, { steps: 12 }); await page.mouse.up();
  }
  async function copy() {
    await page.evaluate(() => navigator.clipboard.writeText('UNTOUCHED'));
    await page.keyboard.press('Control+c');
    return page.evaluate(() => navigator.clipboard.readText());
  }
  for (const specimen of native.cases) for (const zoom of [1.5, 3]) {
    const result = { case: specimen.case, rotation: specimen.rotation, zoom };
    try {
      await page.goto(`${process.env.FOLIO_READER_ORIGIN || 'http://127.0.0.1:1420'}/tests/fixtures/reader-selection.html?case=${specimen.case}&rotation=${specimen.rotation}&zoom=${zoom}`);
      const value = specimen.case === 'ligatures-on' ? 'ff' : specimen.case === 'two-axis-marks' ? specimen.text : '𝐴';
      const target = page.locator('[data-pdf-character]').filter({ hasText: new RegExp(`^${value}$`, 'u') });
      await expect(target).toHaveCount(1);
      await drag(target, specimen.rotation);
      result.clipboard = await copy(); expect(result.clipboard).toBe(value);
      if (specimen.case === 'ligatures-on') {
        await page.evaluate(() => getSelection().removeAllRanges());
        await drag(target, specimen.rotation, .5);
        result.partialClipboard = await copy(); expect(result.partialClipboard).toBe('f');
      }
      await page.evaluate(() => getSelection().removeAllRanges());
      await page.getByRole('button', { name: 'Toggle highlight' }).click();
      await drag(target, specimen.rotation, specimen.case === 'ligatures-on' ? .5 : 1);
      const selected = specimen.pageText.characters.filter(c => value.includes(c.text) && c.text.trim());
      const x = Math.min(...selected.map(c => c.x)), y = Math.min(...selected.map(c => c.y));
      const expected = { x, y, width: Math.max(...selected.map(c => c.x + c.width)) - x, height: Math.max(...selected.map(c => c.y + c.height)) - y };
      await expect.poll(async () => JSON.parse(await page.locator('output').textContent()).length).toBe(1);
      result.highlights = JSON.parse(await page.locator('output').textContent());
      for (const key of ['x', 'y', 'width', 'height']) expect(result.highlights[0][key]).toBeCloseTo(expected[key], 5);
      result.passed = true;
    } catch (error) { result.passed = false; result.error = error.message; }
    results.push(result);
    console.log(JSON.stringify(result));
  }
  expect(errors).toEqual([]);
} finally {
  await browser.close();
  mkdirSync('artifacts/reader-selection', { recursive: true });
  writeFileSync('artifacts/reader-selection/browser.json', JSON.stringify(results, null, 2) + '\n');
}
expect(results.filter(result => !result.passed)).toEqual([]);
