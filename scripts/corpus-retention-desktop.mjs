// Diagnostic snapshots after releasing all clean corpus tabs; never forces GC.
import { chromium, expect } from '@playwright/test';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
const exec = promisify(execFile), pid = process.env.FOLIO_APP_PID;
if (!pid || !/^\d+$/.test(pid)) throw Error('Set FOLIO_APP_PID to the owned corpus test instance.');
const browser = await chromium.connectOverCDP(process.env.FOLIO_CDP_URL || 'http://127.0.0.1:9239');
const page = browser.contexts()[0].pages().find(p => !p.url().startsWith('devtools:'));
const cdp = await page.context().newCDPSession(page);
const snapshots = [];
let closedAt = null;
async function snapshot(phase) {
  const { stdout } = await exec('pwsh', ['-NoProfile', '-File', resolve('scripts/corpus-process-memory.ps1'), '-AppProcessId', pid], { windowsHide: true, timeout: 30000 });
  const memory = JSON.parse(stdout.replace(/^\uFEFF/, ''));
  const metrics = Object.fromEntries((await cdp.send('Performance.getMetrics')).metrics.map(m => [m.name, m.value]));
  snapshots.push({ phase, elapsedSinceCloseMs: closedAt === null ? null : performance.now() - closedAt,
    memory, jsHeapUsedBytes: metrics.JSHeapUsedSize, jsHeapTotalBytes: metrics.JSHeapTotalSize,
    domNodes: metrics.Nodes, layoutObjects: metrics.LayoutObjects, openTabs: await page.getByRole('tab').count() });
}
try {
  await cdp.send('Performance.enable');
  await expect(page.getByLabel('Unsaved changes')).toHaveCount(0);
  const names = await page.getByRole('tab').evaluateAll(tabs => tabs.map(t => t.getAttribute('aria-label')));
  const allowed = ['dense-300-pages.pdf', 'scan-40-pages.pdf', 'dense-tables.pdf', 'embedded-font.pdf', 'mixed-geometry.pdf'];
  if (!names.length || names.some(name => !allowed.includes(name))) throw Error('Only the known clean corpus tabs may be closed.');
  await snapshot('before-closing-corpus-tabs');
  for (const name of names) await page.getByRole('button', { name: 'Close ' + name, exact: true }).click();
  await expect(page.getByRole('tab')).toHaveCount(0);
  await expect(page.getByRole('button', { name: 'Open a PDF', exact: true })).toBeVisible();
  closedAt = performance.now();
  await snapshot('all-tabs-closed');
  await new Promise(resolve => setTimeout(resolve, 10000));
  await snapshot('closed-idle-10s');
  const result = { passed: true, closedTabs: names, snapshots, forcedGarbageCollection: false,
    note: 'Natural idle snapshots. Process working sets include shared pages and allocator/GPU caches; these readings alone do not establish a leak.' };
  writeFileSync(resolve('artifacts/corpus-ui/retention.json'), JSON.stringify(result, null, 2));
  console.log(JSON.stringify(result, null, 2));
} finally { await cdp.detach(); await browser.close(); }
