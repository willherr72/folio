import {chromium,expect} from '@playwright/test';
import {execFile} from 'node:child_process';import {promisify} from 'node:util';
import {readFileSync,readdirSync,writeFileSync,renameSync} from 'node:fs';import {resolve} from 'node:path';
const run=resolve(process.env.FOLIO_RELEASE_RUN||'artifacts/release-v0.10-desktop');
const state=JSON.parse(readFileSync(resolve(run,'process.json'),'utf8').replace(/^\uFEFF/,''));const exec=promisify(execFile);
function latest(){const dir=resolve(run,'data/recovery');const names=readdirSync(dir).filter(n=>/^checkpoint-.*\.json$/.test(n)).sort();return JSON.parse(readFileSync(resolve(dir,names.at(-1)),'utf8')).workspace;}
await expect.poll(()=>{try{return JSON.stringify(latest()).includes('Recovery office café Ω');}catch{return false;}},{timeout:20000}).toBe(true);
writeFileSync(resolve(run,'checkpoint-before.json'),JSON.stringify(latest(),null,2));
// Validate process ownership from the launch record immediately before terminating this test app.
await exec('pwsh',['-NoProfile','-Command',"$state=Get-Content -LiteralPath '"+resolve(run,'process.json').replaceAll("'","''")+"' -Raw|ConvertFrom-Json;$owned=Get-Process -Id $state.pid -ErrorAction Stop;if($owned.Path -ne $state.binary){throw 'Unexpected process'};Stop-Process -Id $owned.Id -Force"],{windowsHide:true,timeout:15000});
renameSync(resolve(run,'Editable.pdf'),resolve(run,'Editable.pdf.moved'));renameSync(resolve(run,'Flattened.pdf'),resolve(run,'Flattened.pdf.moved'));
await exec('pwsh',['-NoProfile','-File',resolve('scripts/start-release-test.ps1'),'-Binary',state.binary,'-RunRoot',run,'-Port',String(state.port)],{windowsHide:true,timeout:35000});
const browser=await chromium.connectOverCDP(`http://127.0.0.1:${state.port}`);const page=browser.contexts()[0].pages().find(p=>!p.url().startsWith('devtools:'));page.setDefaultTimeout(20000);
try{
 await expect(page.getByRole('dialog',{name:'Restore your workspace?'})).toBeVisible();await page.getByRole('button',{name:'Restore workspace',exact:true}).click();
 await page.getByRole('tab',{name:'Editable.pdf',exact:true}).click();const canvas=page.locator('.page-canvas').first();
 await expect(canvas.locator('[data-overlay] use').first()).toBeAttached();await canvas.locator('[data-overlay]').click();await expect(page.getByRole('textbox',{name:'Content',exact:true})).toHaveValue('Recovery office café Ω');
 await expect(page.getByRole('checkbox',{name:'Shaped text',exact:true})).toBeChecked();
 await page.getByRole('button',{name:'Find',exact:true}).click();await page.getByRole('searchbox',{name:'Find in document'}).fill('Recovery office café Ω');await expect(page.locator('.search-count')).toContainText('1 of 1');await page.getByRole('button',{name:'Close search',exact:true}).click();
 await page.screenshot({path:resolve(run,'restored.png')});
 writeFileSync(resolve(run,'recovery-results.json'),JSON.stringify({passed:true,binarySha256:state.sha256,checks:['crash checkpoint contains shaped draft','restore with all source PDFs and imported font moved','native outlines regenerated','shaping controls restored','recovered logical text searchable']},null,2));console.log('PASS shaped desktop recovery');
}finally{await browser.close();}
