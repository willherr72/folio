import {chromium,expect} from '@playwright/test';
import {execFile} from 'node:child_process';
import {promisify} from 'node:util';
import {existsSync,readFileSync,writeFileSync} from 'node:fs';
import {resolve} from 'node:path';
const run=resolve(process.env.FOLIO_RELEASE_RUN),exec=promisify(execFile);
const state=JSON.parse(readFileSync(resolve(run,'process.json'),'utf8').replace(/^\uFEFF/,''));
const browser=await chromium.connectOverCDP(`http://127.0.0.1:${state.port}`);
const page=browser.contexts()[0].pages().find(p=>!p.url().startsWith('devtools:'));page.setDefaultTimeout(25000);
const errors=[];page.on('pageerror',e=>errors.push(String(e)));
async function save(name){const path=resolve(run,name);await page.getByRole('button',{name:'Save a copy',exact:true}).click();await exec('pwsh',['-NoProfile','-File',resolve('scripts/set-release-dialog.ps1'),'-AppProcessId',String(state.pid),'-FilePath',path,'-Title','Save a PDF copy','-Action','Save'],{windowsHide:true,timeout:25000});await expect.poll(()=>existsSync(path)).toBe(true);await expect(page.getByRole('button',{name:'Save a copy',exact:true})).toBeEnabled();}
try {
 await expect(page.getByRole('dialog',{name:'Restore your workspace?'})).toBeVisible();await page.getByRole('button',{name:'Restore workspace',exact:true}).click();
 await expect(page.getByRole('tab',{name:'original.pdf',exact:true,selected:true})).toBeVisible();await expect(page.getByRole('tab')).toHaveCount(2);
 await page.getByRole('button',{name:'Find',exact:true}).click();await page.getByRole('searchbox').fill('Next text');await expect(page.locator('.search-count')).toHaveText('1 of 1');await page.keyboard.press('Escape');
 await save('recovered.pdf');await page.getByRole('tab',{name:'edited.pdf',exact:true}).click();
 await page.getByRole('button',{name:'Page 1 of 3',exact:true}).click();await page.getByRole('button',{name:'Fill forms',exact:true}).click();await expect(page.getByRole('textbox',{name:'shared',exact:true})).toHaveValue('Desktop linked');await page.getByRole('button',{name:'Done',exact:true}).click();
 await save('recovered-filled.pdf');await page.screenshot({path:resolve(run,'restored.png')});expect(errors).toEqual([]);
 writeFileSync(resolve(run,'results.json'),JSON.stringify({passed:true,restoredTabs:2,textSearch:true,sharedFieldValue:true,errors,exe:state},null,2),{flag:'wx'});console.log('PASS group-edit workspace recovery');
}catch(error){await page.screenshot({path:resolve(run,'failure.png')}).catch(()=>{});throw error;}
finally{await browser.close();}
