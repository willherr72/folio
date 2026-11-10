// Repository-native WebView workflow; uses an owned app and synthetic PDF only.
import {chromium, expect} from '@playwright/test';
import {execFile} from 'node:child_process';
import {promisify} from 'node:util';
import {copyFileSync, existsSync, readFileSync, writeFileSync} from 'node:fs';
import {resolve, basename} from 'node:path';
import {createHash} from 'node:crypto';

const root = resolve(process.env.FOLIO_GROUP_BOUNDARY_ROOT);
const run = resolve(process.env.FOLIO_RELEASE_RUN);
const state = JSON.parse(readFileSync(resolve(run, 'process.json'), 'utf8').replace(/^\uFEFF/, ''));
const exec = promisify(execFile), checks = [], errors = [];
const hash = path => createHash('sha256').update(readFileSync(path)).digest('hex');
const original = resolve(run, 'Boundary practice.pdf');
const unchanged = resolve(run, 'Unchanged.pdf'), edited = resolve(run, 'Edited.pdf');
if ([original, unchanged, edited].some(existsSync)) throw Error('Preserving prior desktop evidence');
copyFileSync(resolve(root, 'embedded-corrected/split-2-original.pdf'), original);
const sourceHash = hash(original);
const browser = await chromium.connectOverCDP(`http://127.0.0.1:${state.port}`);
const page = browser.contexts()[0].pages().find(p => !p.url().startsWith('devtools:'));
page.setDefaultTimeout(20000);
page.on('pageerror', error => errors.push(String(error)));
const canvas = () => page.locator('.page-canvas').first();
const text = () => canvas().locator('.pdf-text-layer').textContent();
async function nativeDialog(title, path, action = 'Open') {
  await exec('pwsh', ['-NoProfile', '-File', resolve('scripts/set-release-dialog.ps1'),
    '-AppProcessId', String(state.pid), '-FilePath', path, '-Title', title, '-Action', action],
  {windowsHide: true, timeout: 25000});
}
async function open(path) {
  await page.getByRole('button', {name: /^(Open|Open a PDF)$/}).click();
  await nativeDialog('Open a PDF', path);
  await expect(page.getByRole('tab', {name: basename(path), exact: true})).toHaveAttribute('aria-selected', 'true');
  await expect(canvas().locator('image')).toHaveCount(1);
}
async function save(path) {
  await page.getByRole('button', {name: 'Save a copy', exact: true}).click();
  await nativeDialog('Save a PDF copy', path, 'Save');
  await expect.poll(() => existsSync(path)).toBe(true);
}
async function selectGroup() {
  await page.getByRole('button', {name: 'Edit text', exact: true}).click();
  await page.getByRole('button', {name: 'Edit text: Ca', exact: true}).click();
  await page.getByRole('button', {name: 'Edit together…', exact: true}).click();
  await page.getByRole('checkbox', {name: /^fé old/}).check();
  await page.getByRole('button', {name: 'Check selection', exact: true}).click();
  await expect(page.getByRole('textbox', {name: 'Replacement text', exact: true})).toHaveValue('Café old');
}
try {
  await open(original);
  await selectGroup();
  await expect(page.getByRole('button', {name: 'Apply changes', exact: true})).toBeDisabled();
  await page.screenshot({path: resolve(run, 'group-checked.png')});
  await page.getByRole('button', {name: 'Cancel', exact: true}).click();
  await save(unchanged);
  checks.push('Formerly refused embedded-font boundary checks successfully; unchanged draft remains unapplied');
  await selectGroup();
  await page.getByRole('textbox', {name: 'Replacement text', exact: true}).fill('Edit café');
  await page.getByRole('button', {name: 'Apply changes', exact: true}).click();
  await expect(page.getByRole('dialog')).toHaveCount(0);
  await expect.poll(text).toContain('Edit café');
  await page.getByRole('button', {name: 'Undo', exact: true}).click();
  await expect.poll(text).toContain('Café old');
  await page.getByRole('button', {name: 'Redo', exact: true}).click();
  await expect.poll(text).toContain('Edit café');
  checks.push('Longer grouped replacement, undo and redo');
  await save(edited);
  await open(edited);
  await expect.poll(text).toContain('Edit café');
  await expect.poll(text).toContain('Neighbor');
  await page.screenshot({path: resolve(run, 'group-reopened.png')});
  expect(hash(original)).toBe(sourceHash);
  expect(errors).toEqual([]);
  checks.push('Native save/reopen preserves replacement, neighbor and original file');
  writeFileSync(resolve(run, 'results.json'), JSON.stringify({passed: true, checks, errors,
    exe: state, originalSha256: sourceHash, unchangedSha256: hash(unchanged), editedSha256: hash(edited)}, null, 2));
  console.log('PASS', checks);
} catch (error) {
  await page.screenshot({path: resolve(run, 'failure.png')}).catch(() => {});
  throw error;
} finally {
  await browser.close();
}
