// Production WebView and exact owned native dialogs; synthetic practice PDF only.
import {chromium,expect} from '@playwright/test';
import {execFile} from 'node:child_process';import {promisify} from 'node:util';
import {copyFileSync,existsSync,readFileSync,writeFileSync,renameSync,readdirSync,statSync} from 'node:fs';import {resolve,basename} from 'node:path';import {createHash} from 'node:crypto';
const run=resolve(process.env.FOLIO_RELEASE_RUN),exec=promisify(execFile),checks=[];
let state=JSON.parse(readFileSync(resolve(run,'process.json'),'utf8').replace(/^\uFEFF/,''));
const hash=p=>createHash('sha256').update(readFileSync(p)).digest('hex');
const original=resolve(run,'Form practice.pdf'),saved=resolve(run,'Form edited.pdf');copyFileSync(resolve('examples/Edit one shared occurrence.pdf'),original);const originalHash=hash(original);
let browser=await chromium.connectOverCDP(`http://127.0.0.1:${state.port}`),page=browser.contexts()[0].pages().find(p=>!p.url().startsWith('devtools:'));page.setDefaultTimeout(20000);
const canvas=()=>page.locator('.page-canvas').first(),text=()=>canvas().locator('.pdf-text-layer').textContent();
async function dialog(title,path,action='Open'){await exec('pwsh',['-NoProfile','-File',resolve('scripts/set-release-dialog.ps1'),'-AppProcessId',String(state.pid),'-FilePath',path,'-Title',title,'-Action',action],{windowsHide:true,timeout:25000});}
async function open(path){await page.getByRole('button',{name:/^(Open|Open a PDF)$/}).click();await dialog('Open a PDF',path);await expect(page.getByRole('tab',{name:basename(path),exact:true})).toHaveAttribute('aria-selected','true');await expect(canvas().locator('image')).toHaveCount(1);}
async function edit(from,to){await page.getByRole('button',{name:'Edit text',exact:true}).click();await canvas().getByRole('button',{name:`Edit text: ${from}`,exact:true}).first().click();await page.getByRole('textbox',{name:'Replacement text',exact:true}).fill(to);await page.getByRole('button',{name:'Apply changes',exact:true}).click();await expect(page.getByRole('dialog')).toHaveCount(0);await expect.poll(text).toContain(to);}
try{
 await open(original);
 await expect.poll(text).toContain('Shared words');
 const initialSharedCount=(await text()).split('Shared words').length-1;expect(initialSharedCount).toBeGreaterThanOrEqual(2);
 await page.getByRole('button',{name:'Edit text',exact:true}).click();
 await canvas().getByRole('button',{name:'Edit text: Shared words',exact:true}).first().click();
 await expect(page.getByText(/Only this occurrence will change/)).toBeVisible();
 await expect(page.getByRole('button',{name:'Edit together…',exact:true})).toHaveCount(0);
 await page.getByRole('textbox',{name:'Replacement text',exact:true}).fill('New shared words');
 await page.screenshot({path:resolve(run,'form-modal.png')});
 await page.getByRole('button',{name:'Apply changes',exact:true}).click();
 await expect(page.getByRole('dialog')).toHaveCount(0);
 await expect.poll(text).toContain('New shared words');
 await expect.poll(async()=>(await text()).split('Shared words').length-1).toBe(initialSharedCount-1);
 await page.getByRole('button',{name:'Undo',exact:true}).click();await expect.poll(async()=>(await text()).split('Shared words').length-1).toBe(initialSharedCount);
 await page.getByRole('button',{name:'Redo',exact:true}).click();await expect.poll(text).toContain('New shared words');
 checks.push('explicit nested occurrence, longer replacement, unchanged sibling, undo/redo');
 await page.getByRole('button',{name:'Find',exact:true}).click();await page.getByRole('searchbox',{name:'Find in document'}).fill('New shared words');await expect(page.locator('.search-count')).toContainText('1 of 1');await page.getByRole('button',{name:'Close search',exact:true}).click();checks.push('edited form source searchable');
 await page.getByRole('button',{name:'Save a copy',exact:true}).click();await dialog('Save a PDF copy',saved,'Save');await expect.poll(()=>existsSync(saved)).toBe(true);expect(hash(original)).toBe(originalHash);
 await page.getByRole('button',{name:'Close Form practice.pdf',exact:true}).click();renameSync(original,resolve(run,'Original moved.pdf'));await open(saved);await expect.poll(text).toContain('New shared words');checks.push('native save/reopen with original moved and unchanged');
 const editStarted=Date.now();await edit('New shared words','Recovered words');
 const recovery=resolve(run,'data/recovery');await expect.poll(()=>{try{return readdirSync(recovery).filter(n=>/^checkpoint-.*\.json$/.test(n)).some(n=>{const path=resolve(recovery,n);return statSync(path).mtimeMs>editStarted&&JSON.parse(readFileSync(path,'utf8')).workspace.tabs.some(t=>t.dirty);});}catch{return false;}},{timeout:20000}).toBe(true);
 await page.screenshot({path:resolve(run,'form-edited.png')});await browser.close();
 await exec('pwsh',['-NoProfile','-Command',"$state=Get-Content -LiteralPath '"+resolve(run,'process.json').replaceAll("'","''")+"' -Raw|ConvertFrom-Json;$owned=Get-Process -Id $state.pid -ErrorAction Stop;if($owned.Path -ne $state.binary){throw 'Unexpected process'};Stop-Process -Id $owned.Id -Force"],{windowsHide:true,timeout:15000});
 renameSync(saved,resolve(run,'Saved moved.pdf'));await exec('pwsh',['-NoProfile','-File',resolve('scripts/start-release-test.ps1'),'-Binary',state.binary,'-RunRoot',run,'-Port',String(state.port)],{windowsHide:true,timeout:35000});state=JSON.parse(readFileSync(resolve(run,'process.json'),'utf8').replace(/^\uFEFF/,''));
 browser=await chromium.connectOverCDP(`http://127.0.0.1:${state.port}`);page=browser.contexts()[0].pages().find(p=>!p.url().startsWith('devtools:'));page.setDefaultTimeout(20000);
 await expect(page.getByRole('dialog',{name:'Restore your workspace?'})).toBeVisible();await page.getByRole('button',{name:'Restore workspace',exact:true}).click();await expect.poll(text).toContain('Recovered words');checks.push('crash recovery restores unsaved form source with both original and saved paths unavailable');
 await page.screenshot({path:resolve(run,'form-recovered.png')});writeFileSync(resolve(run,'results.json'),JSON.stringify({passed:true,binarySha256:state.sha256,originalSha256:originalHash,checks},null,2));console.log('PASS',checks);
}finally{await browser.close();}
