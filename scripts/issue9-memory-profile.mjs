// Diagnostic only: run against an isolated, owned packaged app with a fresh profile.
import { chromium, expect } from '@playwright/test';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
const exec = promisify(execFile), pid = process.env.FOLIO_APP_PID;
if (!pid || !/^\d+$/.test(pid)) throw Error('Set owned FOLIO_APP_PID');
const output = resolve(process.env.FOLIO_CORPUS_UI_OUTPUT || 'artifacts/issue9-memory');
mkdirSync(output, { recursive:true });
const browser = await chromium.connectOverCDP(process.env.FOLIO_CDP_URL || 'http://127.0.0.1:9241');
const page = browser.contexts()[0].pages().find(p => !p.url().startsWith('devtools:'));
const cdp = await page.context().newCDPSession(page);
const resumeDense = process.env.FOLIO_MEMORY_RESUME === 'dense';
const denseOnly = process.env.FOLIO_MEMORY_WORKLOAD === 'dense';
const previous = resumeDense ? JSON.parse(readFileSync(resolve(output,'memory.json'),'utf8')) : {};
const snapshots = previous.snapshots || [], responses = previous.responses || [];
const method = { workload: denseOnly ? 'dense thumbnails only' : 'three scan scroll cycles, then dense thumbnails',
  forcedGarbageCollection: false, resumedDensePhase: resumeDense,
  note: 'Process-tree memory snapshots include allocator/GPU retention. PNG metadata retains no payload bytes. Dialog handling is outside scrolling measurements; dense-only opening settles for at least 10 seconds.' };
const idle = ms => new Promise(r => setTimeout(r, ms));
const frame = () => page.evaluate(() => new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r))));
async function snapshot(phase) {
  const {stdout} = await exec('pwsh', ['-NoProfile','-File',resolve('scripts/corpus-process-memory.ps1'),'-AppProcessId',pid], {windowsHide:true, timeout:30000});
  const metrics = Object.fromEntries((await cdp.send('Performance.getMetrics')).metrics.map(m => [m.name,m.value]));
  const state = await page.evaluate(() => ({...window.__imageDiagnostic, live: [...window.__imageDiagnostic.live.values()], thumbnails:document.querySelectorAll('.thumbnail-canvas').length, mounted:document.querySelectorAll('.page-canvas').length}));
  const result = {phase,memory:JSON.parse(stdout.replace(/^\uFEFF/,'')),metrics,state};
  snapshots.push(result); console.log(JSON.stringify({phase,privateMiB:result.memory.currentTreePrivateBytes/1048576,jsMiB:metrics.JSHeapUsedSize/1048576,liveBlobs:state.live.length,blobMiB:state.live.reduce((s,b)=>s+b.size,0)/1048576,thumbnails:state.thumbnails}));
  writeFileSync(resolve(output,'memory.json'),JSON.stringify({method,snapshots,responses},null,2));
  return result;
}
async function open(name) {
  await page.getByRole('button',{name:await page.getByRole('tab').count()?'Open':'Open a PDF',exact:true}).click();
  console.log('OPEN_DIALOG '+name);
  if (!process.env.FOLIO_DIALOG_MANUAL) await exec('pwsh',['-NoProfile','-File',resolve('scripts/set-native-dialog.ps1'),'-AppProcessId',pid,'-FilePath',resolve(process.env.FOLIO_CORPUS_DIR,name),'-Action','Open'],{windowsHide:true,timeout:60000});
  await expect(page.getByRole('tab',{name,exact:true})).toHaveAttribute('aria-selected','true',{timeout:180000});
  await expect(page.locator('.document-page').first().locator('.page-canvas image')).toHaveCount(1,{timeout:90000});
  await frame();
}
try {
  if (!resumeDense) await expect(page.getByRole('tab')).toHaveCount(0);
  await cdp.send('Performance.enable');
  await cdp.send('Network.enable',{maxTotalBufferSize:1000000,maxResourceBufferSize:100000});
  cdp.on('Network.responseReceived',event => {
    if(event.response.url.includes('render_page')) responses.push({url:event.response.url,mimeType:event.response.mimeType,headers:event.response.headers,encodedDataLength:event.response.encodedDataLength});
  });
  if (!resumeDense) {
  await page.evaluate(() => {
    const state = window.__imageDiagnostic = {live:new Map(),created:0,revoked:0};
    const create = URL.createObjectURL, revoke = URL.revokeObjectURL;
    URL.createObjectURL = function(blob){
      const url=create.call(this,blob), meta={size:blob.size,type:blob.type};
      state.live.set(url,meta);state.created++;
      if(blob.type==='image/png') void blob.slice(0,24).arrayBuffer().then(header=>{
        const view=new DataView(header);meta.signature=[...new Uint8Array(header).slice(0,8)];
        meta.width=view.getUint32(16);meta.height=view.getUint32(20);
      });
      return url;
    };
    URL.revokeObjectURL = function(url){state.live.delete(url);state.revoked++;return revoke.call(this,url);};
  });
  await snapshot('empty');
  if (!denseOnly) {
  await open('scan-40-pages.pdf'); await snapshot('scan-open');
  for(let round=0;round<3;round++) {
    for(let index=0;index<40;index++) {
      const slot=page.locator('.document-page').nth(index);
      await slot.evaluate(el=>el.scrollIntoView({block:'start'}));
      await expect(slot.locator('.page-canvas image')).toHaveCount(1,{timeout:30000});
      await frame();
    }
    await snapshot('scan-scroll-round-'+round); await idle(10000); await snapshot('scan-idle-round-'+round);
  }
  await page.getByRole('button',{name:'Close scan-40-pages.pdf',exact:true}).click();
  await idle(10000); await snapshot('scan-closed-idle');
  }
  await open('dense-300-pages.pdf');
  } else {
    if (!await page.evaluate(() => !!window.__imageDiagnostic)) throw Error('Cannot resume without original image instrumentation');
    await expect(page.getByRole('tab',{name:'dense-300-pages.pdf',exact:true})).toHaveAttribute('aria-selected','true');
    await expect(page.locator('.document-page').first().locator('.page-canvas image')).toHaveCount(1,{timeout:90000});
  }
  if (denseOnly) await idle(10000);
  await snapshot('dense-open');
  for(let index=0;index<300;index+=4) {
    const thumbnail=page.locator('.thumbnail-item').nth(index);
    await thumbnail.evaluate(el=>el.scrollIntoView({block:'start'}));
    await expect(thumbnail.locator('.thumbnail-canvas image')).toHaveCount(1,{timeout:30000});
    await frame();
  }
  const visited = await snapshot('dense-all-thumbnails');
  if (process.env.FOLIO_EXPECT_BOUNDED_THUMBNAILS) {
    expect(visited.state.live.length).toBeLessThanOrEqual(60);
    expect(visited.state.thumbnails).toBeLessThan(300);
  }
  await idle(10000); await snapshot('dense-thumbnails-idle');
  await page.getByRole('button',{name:'Close dense-300-pages.pdf',exact:true}).click();
  await idle(10000); const closed = await snapshot('all-closed-idle');
  expect(closed.state.live).toHaveLength(0);
} finally {await cdp.detach();await browser.close();}
