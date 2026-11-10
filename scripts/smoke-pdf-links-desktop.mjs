// Actual Windows/WebView Save a copy and reopen, using only synthetic PDFs.
import {chromium, expect} from '@playwright/test';
import {execFile} from 'node:child_process';
import {promisify} from 'node:util';
import {copyFileSync, existsSync, mkdirSync, readFileSync, writeFileSync} from 'node:fs';
import {resolve, basename} from 'node:path';
import {createHash} from 'node:crypto';

const root = resolve(process.env.FOLIO_LINK_ROOT);
const run = resolve(process.env.FOLIO_RELEASE_RUN);
const state = JSON.parse(readFileSync(resolve(run, 'process.json'), 'utf8').replace(/^\uFEFF/, ''));
const exec = promisify(execFile), errors = [], checks = [];
const hash = path => createHash('sha256').update(readFileSync(path)).digest('hex');
const browser = await chromium.connectOverCDP(`http://127.0.0.1:${state.port}`);
const page = browser.contexts()[0].pages().find(p => !p.url().startsWith('devtools:'));
page.setDefaultTimeout(20000);
page.on('pageerror', error => errors.push(String(error)));
async function dialog(title, path, action) {
  await exec('pwsh', ['-NoProfile', '-File', resolve('scripts/set-release-dialog.ps1'),
    '-AppProcessId', String(state.pid), '-FilePath', path, '-Title', title, '-Action', action],
  {windowsHide: true, timeout: 25000});
}
async function open(path) {
  await page.getByRole('button', {name: /^(Open|Open a PDF)$/}).click();
  await dialog('Open a PDF', path, 'Open');
  await expect(page.getByRole('tab', {name: basename(path), exact: true, selected: true})).toHaveCount(1);
  await expect(page.locator('.page-canvas').first().locator('image')).toHaveCount(1);
  await page.getByRole('button', {name: 'Select', exact: true}).click();
  await expect.poll(async () => (await page.locator('[data-pdf-character]').allTextContents()).join('')).toContain('Original page 0');
}
try {
  let previous = resolve(root, 'corpus-final/save-source.pdf');
  for (const name of ['first-save', 'second-save']) {
    const corpus = resolve(run, name);
    if (existsSync(corpus)) throw Error('Preserving previous acceptance evidence');
    mkdirSync(corpus);
    const source = resolve(corpus, 'save-source.pdf'), output = resolve(corpus, 'save-output.pdf');
    copyFileSync(previous, source);
    const sourceSha256 = hash(source);
    await open(source);
    await page.getByRole('button', {name: 'Save a copy', exact: true}).click();
    await dialog('Save a PDF copy', output, 'Save');
    await expect.poll(() => existsSync(output)).toBe(true);
    await open(output);
    await page.screenshot({path: resolve(run, `${name}.png`)});
    expect(hash(source)).toBe(sourceSha256);
    const {stdout} = await exec('python', ['scripts/verify-pdf-links.py', '--desktop', '--corpus', corpus,
      '--output', resolve(run, `${name}-independent`)], {windowsHide: true, timeout: 30000});
    checks.push({name, sourceSha256, savedSha256: hash(output), independent: JSON.parse(stdout)});
    previous = output;
  }
  expect(errors).toEqual([]);
  writeFileSync(resolve(run, 'results.json'), JSON.stringify({passed: true, checks, errors, exe: state,
    method: 'Native Open / Save a copy / reopen twice; independent navigation and exact raster validation'}, null, 2), {flag: 'wx'});
  console.log('PASS', checks.length, 'actual Windows save and reopen workflows');
} catch (error) {
  await page.screenshot({path: resolve(run, 'failure.png')}).catch(() => {});
  throw error;
} finally {
  await browser.close();
}
