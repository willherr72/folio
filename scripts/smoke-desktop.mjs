import { chromium, expect } from '@playwright/test';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { mkdirSync, readFileSync, writeFileSync, existsSync } from 'node:fs';
import { resolve } from 'node:path';

const execFileAsync = promisify(execFile);
const artifacts = resolve('artifacts/desktop-smoke');
mkdirSync(artifacts, { recursive: true });
const appProcessId = process.env.FOLIO_APP_PID;
if (!appProcessId) throw new Error('Set FOLIO_APP_PID to the Folio instance under test.');
const browser = await chromium.connectOverCDP(process.env.FOLIO_CDP_URL || 'http://127.0.0.1:9228');
const context = browser.contexts()[0];
const page = context.pages().find(page => !page.url().startsWith('devtools:')) || await context.newPage();
const errors = [];
page.on('pageerror', error => errors.push(error.message));
page.on('dialog', dialog => dialog.accept());
const source = resolve('examples/Welcome to Folio.pdf');
const outputDirectory = resolve(artifacts, 'output-' + Date.now());
mkdirSync(outputDirectory, { recursive: true });
const output = resolve(outputDirectory, 'Folio edited sample.pdf');
async function fileDialog(action, path) {
  const result = await execFileAsync('C:/Windows/System32/WindowsPowerShell/v1.0/powershell.exe', [
    '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', resolve('scripts/set-native-dialog.ps1'),
    '-AppProcessId', appProcessId, '-FilePath', path, '-Action', action,
  ], { timeout: 30000, windowsHide: true });
  console.log(result.stdout.trim());
}
try {
  const before = Date.now();
  await page.getByRole('button', { name: 'Open a PDF', exact: true }).click();
  await fileDialog('Open', source);
  await expect(page.getByText('Welcome to Folio.pdf', { exact: true })).toBeVisible({ timeout: 15000 });
  const canvas = page.locator('.page-stage .page-canvas');
  await expect(canvas.locator('image')).toHaveCount(1, { timeout: 15000 });
  const openAndRenderMs = Date.now() - before;
  await expect(page.locator('.page-error')).toHaveCount(0);
  await page.screenshot({ path: resolve(artifacts, '01-native-open.png'), fullPage: true });

  await page.getByRole('button', { name: 'Text', exact: true }).click();
  await canvas.click({ position: { x: 100, y: 285 } });
  await page.getByRole('textbox', { name: 'Content' }).fill('Built while you slept.');
  await page.getByRole('button', { name: 'Signature', exact: true }).click();
  const pad = page.getByLabel('Signature drawing area');
  const box = await pad.boundingBox();
  if (!box) throw new Error('Signature pad not visible');
  await page.mouse.move(box.x + 40, box.y + 80);
  await page.mouse.down();
  for (const [x,y] of [[75,35],[95,95],[120,45],[145,75],[200,55],[230,80]]) {
    await page.mouse.move(box.x+x, box.y+y, { steps: 4 });
  }
  await page.mouse.up();
  await page.getByRole('button', { name: 'Use signature' }).click();
  await canvas.click({ position: { x: 100, y: 455 } });
  await expect(canvas.locator('[data-overlay]')).toHaveCount(2);
  await canvas.click({ position: { x: 15, y: 15 } });
  await page.screenshot({ path: resolve(artifacts, '02-native-edited.png'), fullPage: true });

  await page.getByRole('button', { name: 'Duplicate', exact: true }).click();
  await expect(page.getByLabel('Page 2 of 4')).toHaveClass(/selected/);
  await page.getByRole('button', { name: 'Add PDF', exact: true }).click();
  await fileDialog('Open', source);
  await expect(page.getByLabel('Page 5 of 7')).toHaveClass(/selected/, { timeout: 15000 });
  await page.getByLabel('Page 1 of 7').click();
  await page.getByRole('button', { name: 'Save a copy', exact: true }).click();
  await fileDialog('Save', output);
  await expect(page.getByLabel('Unsaved changes')).toHaveCount(0, { timeout: 15000 });
  expect(existsSync(output)).toBe(true);
  expect(readFileSync(output).subarray(0,5).toString()).toBe('%PDF-');

  await page.getByRole('button', { name: 'Open', exact: true }).click();
  await fileDialog('Open', output);
  await expect(page.getByText('Folio edited sample.pdf', { exact: true })).toBeVisible({ timeout: 15000 });
  await expect(page.getByLabel('Page 1 of 7')).toHaveClass(/selected/);
  await expect(canvas.locator('image')).toHaveCount(1, { timeout: 15000 });
  await expect(canvas.locator('[data-overlay]')).toHaveCount(0);
  await expect(page.locator('.page-error')).toHaveCount(0);
  expect(errors).toEqual([]);
  await page.screenshot({ path: resolve(artifacts, '03-native-reopened.png'), fullPage: true });
  const report = { passed: true, source, output, openAndRenderMsIncludingDialogAutomation: openAndRenderMs, checks: ['native open dialog', 'native PDF render', 'text', 'drawn signature', 'duplicate annotated page', 'merge through native dialog', 'native save dialog', 'saved PDF file', 'native reopen of seven-page output'], consoleErrors: errors };
  writeFileSync(resolve(artifacts, 'results.json'), JSON.stringify(report, null, 2));
  console.log(JSON.stringify(report, null, 2));
} catch (error) {
  await page.screenshot({ path: resolve(artifacts, 'failure.png'), fullPage: true });
  throw error;
} finally {
  await browser.close();
}
