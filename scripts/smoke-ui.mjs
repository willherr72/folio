import { chromium, expect } from '@playwright/test';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';

const artifacts = resolve('artifacts/ui-smoke');
mkdirSync(artifacts, { recursive: true });
const browser = await chromium.launch({ headless: true });
const page = await browser.newPage({ viewport: { width: 1440, height: 1050 }, deviceScaleFactor: 1 });
const errors = [];
page.on('pageerror', error => errors.push(error.message));
page.on('dialog', dialog => dialog.accept());
try {
  await page.goto(process.env.FOLIO_UI_URL || 'http://127.0.0.1:1420/?demo=1');
  await expect(page.getByText('Folio welcome.pdf', { exact: true })).toBeVisible();
  const canvas = page.locator('.page-stage .page-canvas');
  await expect(canvas.locator('image')).toHaveCount(1);
  await page.screenshot({ path: resolve(artifacts, '01-workspace.png'), fullPage: true });

  await page.getByRole('button', { name: 'Text', exact: true }).click();
  await canvas.click({ position: { x: 100, y: 265 } });
  await page.getByRole('textbox', { name: 'Content' }).fill('Morning review\nFolio is working.');
  await expect(canvas.locator('[data-overlay]')).toHaveCount(1);

  await page.getByRole('button', { name: 'Signature', exact: true }).click();
  const pad = page.getByLabel('Signature drawing area');
  const box = await pad.boundingBox();
  if (!box) throw new Error('Signature pad not visible');
  await page.mouse.move(box.x + 45, box.y + 80);
  await page.mouse.down();
  for (const [x, y] of [[75,35],[95,95],[120,45],[145,75],[200,55],[230,80]]) {
    await page.mouse.move(box.x+x, box.y+y, { steps: 4 });
  }
  await page.mouse.up();
  await page.getByRole('button', { name: 'Use signature' }).click();
  await canvas.click({ position: { x: 115, y: 435 } });
  await expect(canvas.locator('[data-overlay]')).toHaveCount(2);

  await canvas.click({ position: { x: 20, y: 20 } });
  await page.getByRole('button', { name: 'Rotate', exact: true }).click();
  await expect(canvas).toHaveAttribute('viewBox', '0 0 792 612');
  await page.getByRole('button', { name: 'Move down', exact: true }).click();
  await expect(page.getByLabel('Page 2 of 3')).toHaveClass(/selected/);

  await page.getByRole('button', { name: 'Duplicate', exact: true }).click();
  await expect(page.getByLabel('Page 3 of 4')).toHaveClass(/selected/);
  await page.getByRole('button', { name: 'Delete page', exact: true }).click();
  await expect(page.getByLabel('Page 3 of 3')).toBeVisible();
  await page.getByRole('button', { name: 'Undo', exact: true }).click();
  await expect(page.getByLabel('Page 3 of 4')).toHaveClass(/selected/);
  await page.getByRole('button', { name: 'Redo', exact: true }).click();
  await expect(page.getByLabel('Page 3 of 3')).toBeVisible();
  await page.getByLabel('Page 2 of 3').click();

  const downloadPromise = page.waitForEvent('download');
  await page.getByRole('button', { name: 'Export demo plan', exact: true }).click();
  const download = await downloadPromise;
  const planPath = resolve(artifacts, download.suggestedFilename());
  await download.saveAs(planPath);
  const plan = JSON.parse(readFileSync(planPath, 'utf8'));
  expect(plan.format).toBe('folio-demo-edit-plan');
  expect(plan.pages.map(item => item.pageIndex)).toEqual([1, 0, 2]);
  expect(plan.pages[1].rotation).toBe(90);
  expect(plan.pages[1].overlays).toHaveLength(2);
  expect(plan.pages[1].overlays[0].text).toBe('Morning review\nFolio is working.');
  expect(plan.pages[1].overlays[1].type).toBe('ink');
  expect(plan.pages[1].overlays[1].paths[0].length).toBeGreaterThan(6);
  await expect(page.getByLabel('Unsaved changes')).toHaveCount(0);
  expect(errors).toEqual([]);
  await page.screenshot({ path: resolve(artifacts, '02-edited.png'), fullPage: true });
  const report = { passed: true, checks: ['demo open', 'text placement and editing', 'signature drawing and placement', 'rotation', 'page reorder', 'duplicate/delete', 'undo/redo', 'exported edit plan contents', 'saved-state indicator'], consoleErrors: errors };
  writeFileSync(resolve(artifacts, 'results.json'), JSON.stringify(report, null, 2));
  console.log(JSON.stringify(report, null, 2));
} catch (error) {
  await page.screenshot({ path: resolve(artifacts, 'failure.png'), fullPage: true });
  throw error;
} finally {
  await browser.close();
}
