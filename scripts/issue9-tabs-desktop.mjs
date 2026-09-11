// Compare packaged builds using the same corpus, window, and readiness condition.
// Start with corpus-start-desktop.ps1; FOLIO_APP_PID must identify that isolated app.
import { chromium, expect } from '@playwright/test';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { readFileSync, mkdirSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { createHash } from 'node:crypto';
const exec = promisify(execFile);
const pid = process.env.FOLIO_APP_PID;
if (!pid || !/^\d+$/.test(pid)) throw Error('Set FOLIO_APP_PID to an isolated test app.');
const corpus = resolve(process.env.FOLIO_CORPUS_DIR || 'artifacts/corpus');
const output = resolve(process.env.FOLIO_PROFILE_OUTPUT || 'artifacts/issue9-tabs');
mkdirSync(output, { recursive: true });
const names = ['dense-300-pages.pdf', 'scan-40-pages.pdf'];
const rates = (process.env.FOLIO_PROFILE_RATES || '1,4').split(',').map(Number);
if (!rates.length || rates.some(rate => !Number.isFinite(rate) || rate < 1)) throw Error('Invalid CPU throttle rates');
const manifest = JSON.parse(readFileSync(resolve(corpus, 'manifest.json'), 'utf8'));
const hashes = Object.fromEntries(names.map(name => {
 const hash = createHash('sha256').update(readFileSync(resolve(corpus, name))).digest('hex');
 expect(hash).toBe(manifest.fixtures.find(f => f.file === name)?.sha256);
 return [name, hash];
}));
const browser = await chromium.connectOverCDP(process.env.FOLIO_CDP_URL || 'http://127.0.0.1:9242');
const page = browser.contexts()[0].pages().find(p => !p.url().startsWith('devtools:'));
const cdp = await page.context().newCDPSession(page);
const errors = [], nativeRequests = [];
const startedUtc = new Date().toISOString();
page.on('pageerror', error => errors.push(error.message));
const ready = async name => {
 await expect(page.getByRole('tab', {name, exact:true})).toHaveAttribute('aria-selected', 'true');
 await expect(page.locator('.document-page').first().locator('.page-canvas image')).toHaveCount(1, {timeout:90000});
 if (name.startsWith('dense')) await expect.poll(() => page.locator('[data-pdf-character]').count(), {timeout:90000}).toBeGreaterThan(1000);
 await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
};
try {
 await cdp.send('Network.enable');
 await cdp.send('Performance.enable');
 cdp.on('Network.requestWillBeSent', event => {
  const url = new URL(event.request.url), command = decodeURIComponent(url.pathname.replace(/^\//, ''));
  if ((url.hostname === 'ipc.localhost' || url.protocol === 'ipc:') && ['render_page','page_text'].includes(command)) nativeRequests.push(command);
 });
 for (const name of names) {
  if (await page.getByRole('tab',{name,exact:true}).count()) {
   if (process.env.FOLIO_PROFILE_REUSE_TABS !== '1') throw Error('Expected fresh test tabs.');
   await page.getByRole('tab',{name,exact:true}).click();
  } else {
   await page.getByRole('button',{name:(await page.getByRole('tab').count()) ? 'Open' : 'Open a PDF',exact:true}).click();
   await exec('pwsh',['-NoProfile','-File',resolve('scripts/set-native-dialog.ps1'),'-AppProcessId',pid,'-FilePath',resolve(corpus,name),'-Action','Open'],{windowsHide:true,timeout:60000});
  }
  await ready(name);
 }
 // Require a positive control from this collector, including when tabs were pre-opened.
 // Choose a previously unvisited probe page for every repeat on the same app.
 const probeIndex = Number(process.env.FOLIO_PROFILE_PROBE_PAGE || 20);
 if (!Number.isSafeInteger(probeIndex) || probeIndex < 1 || probeIndex >= 300) throw Error('Probe page must be an unvisited index 1..299');
 await page.getByRole('tab',{name:names[0],exact:true}).click();
 const probe = page.locator('.document-page').nth(probeIndex);
 await probe.evaluate(el => el.scrollIntoView({block:'start'}));
 await expect(probe.locator('.page-canvas image')).toHaveCount(1,{timeout:90000});
 await expect.poll(() => probe.locator('[data-pdf-character]').count(),{timeout:90000}).toBeGreaterThan(1000);
 expect(nativeRequests).toContain('render_page');
 expect(nativeRequests).toContain('page_text');
 await page.locator('.document-page').first().evaluate(el => el.scrollIntoView({block:'start'}));
 await ready(names[0]);
 const expectedGlyphs = {[names[0]]:await page.locator('[data-pdf-character]').count()};
 await page.getByRole('tab',{name:names[1],exact:true}).click();
 await ready(names[1]);
 expectedGlyphs[names[1]] = await page.locator('[data-pdf-character]').count();
 const positiveControl = {probeIndex,observedCommands:[...new Set(nativeRequests)]};
 const viewport = await page.evaluate(() => ({width:innerWidth,height:innerHeight,devicePixelRatio}));
 const metrics = async () => Object.fromEntries((await cdp.send('Performance.getMetrics')).metrics.map(m => [m.name,m.value]));
 const results=[];
 for (const rate of rates) {
  await cdp.send('Emulation.setCPUThrottlingRate',{rate});
  const startRequests = nativeRequests.length, before = await metrics(), samples=[];
  for (let round=0;round<6;round++) for (const name of names) {
   const readyMs = await page.evaluate(async ({name,expectedCount}) => {
    const started = performance.now();
    const tab = [...document.querySelectorAll('[role=tab]')].find(e => e.getAttribute('aria-label') === name);
    tab.click();
    await new Promise((resolve,reject) => {
     const timeout = setTimeout(() => reject(Error('Tab readiness timed out')),30000);
     const check = () => {
      const glyphs=document.querySelectorAll('[data-pdf-character]');
      const textReady = glyphs.length === expectedCount && (!expectedCount || glyphs[glyphs.length-1].style.transform);
      if (tab.getAttribute('aria-selected')==='true' && textReady && document.querySelector('.document-page .page-canvas image')) requestAnimationFrame(() => requestAnimationFrame(() => {clearTimeout(timeout);resolve();}));
      else requestAnimationFrame(check);
     };
     requestAnimationFrame(check);
    });
    return performance.now()-started;
   },{name,expectedCount:expectedGlyphs[name]});
   samples.push({fixture:name,round,readyMs});
   console.log(JSON.stringify({cpuThrottle:rate,fixture:name,round,readyMs}));
  }
  const after=await metrics(), repeatNativeRequests=nativeRequests.slice(startRequests);
  expect(repeatNativeRequests).toEqual([]);
  results.push({cpuThrottle:rate,samples,repeatNativeRequests,metrics:Object.fromEntries(['LayoutCount','LayoutDuration','RecalcStyleCount','RecalcStyleDuration','ScriptDuration','TaskDuration'].map(k => [k,after[k]-before[k]]))});
 }
 expect(errors).toEqual([]);
 const result={passed:true,startedUtc,completedUtc:new Date().toISOString(),pid:Number(pid),hashes,viewport,expectedGlyphs,positiveControl,results,errors,method:'In-WebView click to selected tab DOM, raster image element, calibrated dense text (or no text for scan), then two animation frames. This measures DOM/text readiness, not image decode or presentation completion. Six switches per fixture per rate. CPU 4x is a simulation on the same host, not lower-memory hardware. Native render/text requests must remain zero during every warm group. No absolute timing threshold.'};
 writeFileSync(resolve(output,'tabs.json'),JSON.stringify(result,null,2));
 console.log(JSON.stringify(result,null,2));
} finally {
 await cdp.send('Emulation.setCPUThrottlingRate',{rate:1}).catch(()=>{});
 await cdp.detach();await browser.close();
}