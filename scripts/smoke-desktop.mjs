import { chromium, expect } from '@playwright/test';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { mkdirSync, readFileSync, writeFileSync, existsSync, copyFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { createHash } from 'node:crypto';

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
const browserPrompts = [];
page.on('dialog', dialog => { browserPrompts.push(dialog.type()); return dialog.accept(); });
const sample = resolve('examples/Welcome to Folio.pdf');
const outputDirectory = resolve(artifacts, 'output-' + Date.now());
mkdirSync(outputDirectory, { recursive: true });
const source = resolve(outputDirectory, 'Welcome to Folio.pdf');
copyFileSync(sample, source);
const output = resolve(outputDirectory, 'Folio edited.pdf');
const secondSource = resolve(outputDirectory, 'Second document.pdf');
// A cropped, intrinsically upside-down PDF exercises native caret orientation.
const stream='BT /F1 20 Tf 60 300 Td (Hello PDF) Tj 0 -30 Td (Second line) Tj ET';
const objects=[
  '<< /Type /Catalog /Pages 2 0 R >>',
  '<< /Type /Pages /Kids [3 0 R] /Count 1 >>',
  '<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 400] /CropBox [40 50 240 350] /Rotate 180 /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>',
  '<< /Length '+Buffer.byteLength(stream)+' >>\nstream\n'+stream+'\nendstream',
  '<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>',
];
let fixture='%PDF-1.7\n';
const offsets=[0];
for(let i=0;i<objects.length;i++){offsets.push(Buffer.byteLength(fixture));fixture+=(i+1)+' 0 obj\n'+objects[i]+'\nendobj\n';}
const xref=Buffer.byteLength(fixture);
fixture+='xref\n0 6\n0000000000 65535 f \n'+offsets.slice(1).map(n=>String(n).padStart(10,'0')+' 00000 n \n').join('');
fixture+='trailer\n<< /Size 6 /Root 1 0 R >>\nstartxref\n'+xref+'\n%%EOF\n';
writeFileSync(secondSource,fixture);
async function fileDialog(action, path) {
  const result = await execFileAsync('pwsh', [
    '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', resolve('scripts/set-native-dialog.ps1'),
    '-AppProcessId', appProcessId, '-FilePath', path, '-Action', action,
  ], { timeout: 60000, windowsHide: true });
  console.log(result.stdout.trim());
}
async function copyPdfText(canvas,phrase,rotation=0) {
  const glyphs=canvas.locator('[data-pdf-character]');
  await expect.poll(async()=>(await glyphs.allTextContents()).join('')).toContain(phrase);
  const chars=await glyphs.allTextContents();
  const offset=chars.join('').indexOf(phrase);
  let cursor=0,startIndex=-1,endIndex=-1;
  for(let i=0;i<chars.length;i++){
    if(cursor===offset)startIndex=i;
    cursor+=chars[i].length;
    if(cursor===offset+phrase.length){endIndex=i;break;}
  }
  const first=await glyphs.nth(startIndex).boundingBox(),last=await glyphs.nth(endIndex).boundingBox();
  const startX=rotation===180?first.x+first.width*.75:first.x+first.width*.25;
  const endX=rotation===180?last.x+last.width*.25:last.x+last.width*.75;
  await page.mouse.move(startX,first.y+first.height/2);
  await page.mouse.down();
  await page.mouse.move(endX,last.y+last.height/2,{steps:15});
  await page.mouse.up();
  await page.keyboard.press('Control+c');
  expect(await page.evaluate(()=>window.__folioCopied)).toBe(phrase);
}

try {
  const before = Date.now();
  await page.getByRole('button', { name: 'Open a PDF', exact: true }).click();
  await fileDialog('Open', source);
  await expect(page.getByText('Welcome to Folio.pdf', { exact: true })).toBeVisible({ timeout: 15000 });
  const canvas = page.locator('.document-page').first().locator('.page-canvas');
  await expect(canvas.locator('image')).toHaveCount(1, { timeout: 15000 });
  const openAndRenderMs = Date.now() - before;
  await expect(page.locator('.page-error')).toHaveCount(0);
  await expect(page.locator('[data-page-id]')).toHaveCount(3);
  await page.getByRole('button',{name:'Settings',exact:true}).click();
  await page.getByLabel('Theme',{exact:true}).selectOption('dark');
  await page.getByRole('button',{name:'Done',exact:true}).click();
  await expect(page.locator('html')).toHaveAttribute('data-theme','dark');
  await page.screenshot({ path: resolve(artifacts, '01-native-open.png'), fullPage: true });
  await page.evaluate(()=>{window.addEventListener('copy',event=>{window.__folioCopied=event.clipboardData.getData('text/plain');});});
  await copyPdfText(canvas,'Make it yours.');


  await page.getByRole('button', { name: 'Text', exact: true }).click();
  await canvas.click({ position: { x: 100, y: 285 } });
  await expect(page.getByRole('textbox', { name: 'Content' })).toBeFocused();
  await page.keyboard.type('Built while you slept.');
  await expect(page.getByRole('textbox', { name: 'Content' })).toHaveValue('Built while you slept.');
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
  const placementBox=await canvas.boundingBox();
  await page.mouse.move(placementBox.x+100,placementBox.y+455);
  await expect(canvas.locator('.signature-preview')).toBeVisible();
  const previewPoints=await canvas.locator('.signature-preview polyline').first().getAttribute('points');
  await page.mouse.click(placementBox.x+100,placementBox.y+455);
  await expect(canvas.locator('.signature-preview')).toHaveCount(0);
  expect(await canvas.locator('[data-overlay] polyline').first().getAttribute('points')).toBe(previewPoints);
  await expect(canvas.locator('[data-overlay]')).toHaveCount(2);
  await page.getByRole('button',{name:'Draw',exact:true}).click();
  const drawingBox = await canvas.boundingBox();
  await page.mouse.move(drawingBox.x+60,drawingBox.y+190);
  await page.mouse.down();
  for (const [x,y] of [[90,170],[125,195],[155,175],[185,193]]) await page.mouse.move(drawingBox.x+x,drawingBox.y+y,{steps:5});
  await page.mouse.up();
  await expect(canvas.locator('[data-overlay]')).toHaveCount(3);
  await page.getByRole('button',{name:'Select',exact:true}).click();
  await page.getByRole('button',{name:'Close Welcome to Folio.pdf',exact:true}).click();
  await expect(page.getByRole('dialog',{name:'Close document?'})).toBeVisible();
  await page.getByRole('button',{name:'Keep editing'}).click();
  await expect(canvas.locator('[data-overlay]')).toHaveCount(3);
  const firstZoom=await page.locator('.zoom-value').innerText();
  await page.getByLabel('Page 2 of 3',{exact:true}).click();
  const viewport=page.getByRole('main',{name:'Document',exact:true});
  const firstScroll=await viewport.evaluate(el=>el.scrollTop);
  await page.getByRole('button',{name:'Open',exact:true}).click();
  await fileDialog('Open',secondSource);
  await expect(page.getByRole('tab',{name:'Second document.pdf',exact:true})).toHaveAttribute('aria-selected','true');
  await expect(page.getByRole('tab')).toHaveCount(2);
  await copyPdfText(canvas,'Hello PDF\r\nSecond line',180);
  await expect(canvas.locator('[data-overlay]')).toHaveCount(0);
  await page.getByRole('button',{name:'Zoom in',exact:true}).click();
  await expect(page.locator('.zoom-value')).not.toHaveText(firstZoom);
  await page.screenshot({path:resolve(artifacts,'02-document-tabs.png'),fullPage:true});
  await execFileAsync('powershell',['-NoProfile','-ExecutionPolicy','Bypass','-File',resolve('scripts/request-native-close.ps1'),'-AppProcessId',appProcessId],{windowsHide:true});
  await expect(page.getByRole('dialog',{name:'Close Folio?'})).toContainText('unsaved changes in 1 document');
  await page.getByRole('button',{name:'Keep editing'}).click();
  await page.getByRole('tab',{name:'Welcome to Folio.pdf',exact:true}).click();
  await expect(page.locator('.zoom-value')).toHaveText(firstZoom);
  await expect.poll(()=>viewport.evaluate(el=>el.scrollTop)).toBeCloseTo(firstScroll,0);
  await page.getByRole('button',{name:'Close Second document.pdf',exact:true}).click();
  await expect(page.getByRole('tab')).toHaveCount(1);
  await page.getByLabel('Page 1 of 3',{exact:true}).click();
  await expect(canvas.locator('[data-overlay]')).toHaveCount(3);
  // Verify real Windows HTML drag handling, then undo back to the sample order.
  const firstThumb=page.getByLabel('Page 1 of 3',{exact:true});
  const lastThumb=page.getByLabel('Page 3 of 3',{exact:true});
  const firstBox=await firstThumb.boundingBox(),lastBox=await lastThumb.boundingBox();
  await page.mouse.move(lastBox.x+10,lastBox.y+lastBox.height/2);
  await page.mouse.down();
  await page.mouse.move(lastBox.x+20,lastBox.y+lastBox.height/2,{steps:3});
  await page.mouse.move(firstBox.x+40,firstBox.y+10,{steps:12});
  await expect(firstThumb).toHaveClass(/drop-before/);
  await page.mouse.up();
  await expect.poll(async () => (await page.locator('.document-page').first().boundingBox()).width).toBeCloseTo(712.8,0);
  await page.getByRole('button',{name:'Undo',exact:true}).click();
  await page.getByLabel('Page 1 of 3',{exact:true}).click();
  await canvas.click({ position: { x: 15, y: 15 } });
  await page.screenshot({ path: resolve(artifacts, '02-native-edited.png'), fullPage: true });

  await page.getByRole('button', { name: 'Duplicate', exact: true }).click();
  await expect(page.getByLabel('Page 2 of 4')).toHaveClass(/selected/);
  await page.getByRole('button', { name: 'Add PDF', exact: true }).click();
  await fileDialog('Open', source);
  await expect(page.getByLabel('Page 5 of 7')).toHaveClass(/selected/, { timeout: 15000 });
  await page.getByLabel('Page 1 of 7').click();
  await canvas.locator('[data-overlay] tspan').first().click();
  await page.getByRole('textbox', { name: 'Content' }).fill('A 中文 B');
  await page.getByRole('button', { name: 'Save a copy', exact: true }).click();
  await fileDialog('Save', output);
  await expect(page.locator('.status-error')).toContainText('U+4E2D', { timeout: 15000 });
  await expect(page.getByLabel('Unsaved changes')).toHaveCount(1);
  expect(existsSync(output)).toBe(false);
  await page.getByRole('textbox', { name: 'Content' }).fill('Built while you slept.');
  await page.getByRole('button', { name: 'Save a copy', exact: true }).click();
  await fileDialog('Save', output);
  await expect(page.getByLabel('Unsaved changes')).toHaveCount(0, { timeout: 15000 });
  expect(existsSync(output)).toBe(true);
  expect(readFileSync(output).subarray(0,5).toString()).toBe('%PDF-');

  await page.getByRole('button', { name: 'Open', exact: true }).click();
  await fileDialog('Open', output);
  await expect(page.getByText('Folio edited.pdf', { exact: true })).toBeVisible({ timeout: 15000 });
  await expect(page.getByLabel('Page 1 of 7')).toHaveClass(/selected/);
  await expect(canvas.locator('image')).toHaveCount(1, { timeout: 15000 });
  await expect(canvas.locator('[data-overlay]')).toHaveCount(3);
  await canvas.locator('[data-overlay] tspan').first().click();
  await expect(page.getByRole('textbox', { name: 'Content' })).toHaveValue('Built while you slept.');
  await expect(page.locator('.page-error')).toHaveCount(0);
  expect(errors).toEqual([]);
  expect(browserPrompts).toEqual([]);
  await page.screenshot({ path: resolve(artifacts, '03-native-reopened.png'), fullPage: true });
  await page.getByRole('button',{name:'Text',exact:true}).click();
  await canvas.click({position:{x:60,y:60}});
  await page.getByRole('tab',{name:'Welcome to Folio.pdf',exact:true}).click();
  await execFileAsync('powershell',['-NoProfile','-ExecutionPolicy','Bypass','-File',resolve('scripts/request-native-close.ps1'),'-AppProcessId',appProcessId],{windowsHide:true});
  await expect(page.getByRole('dialog',{name:'Close Folio?'})).toBeVisible();
  const closed = page.waitForEvent('close');
  await page.getByRole('button',{name:'Discard and close'}).click();
  await closed;
  const testedBinary=JSON.parse(readFileSync(resolve('artifacts/desktop-process.json'),'utf8').trim()).binary;
  const binarySha256=createHash('sha256').update(readFileSync(testedBinary)).digest('hex');
  const report = { passed: true, binarySha256, source, output, openAndRenderMsIncludingDialogAutomation: openAndRenderMs, checks: ['native open dialog', 'native PDF render', 'text', 'drawn signature', 'freehand drawing', 'real thumbnail drag and undo', 'dark settings', 'custom tab close modal', 'independent tabs and restored zoom/scroll', 'inactive dirty tab close protection', 'immediate text typing', 'signature ghost matches placed geometry', 'native PDF text mouse selection and copy including intrinsic180 crop', 'native close cancelled and confirmed', 'duplicate annotated page', 'merge through native dialog', 'unsupported text rejected without creating output', 'native save dialog', 'saved PDF file', 'native reopen of seven-page output'], consoleErrors: errors };
  writeFileSync(resolve(artifacts, 'results.json'), JSON.stringify(report, null, 2));
  console.log(JSON.stringify(report, null, 2));
} catch (error) {
  if (!page.isClosed()) await page.screenshot({ path: resolve(artifacts, 'failure.png'), fullPage: true });
  throw error;
} finally {
  await browser.close();
}
