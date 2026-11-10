// Repository Windows/WebView acceptance: real native listing, dialogs and edits.
import {chromium,expect} from '@playwright/test';
import {execFile} from 'node:child_process';
import {promisify} from 'node:util';
import {readFileSync,writeFileSync,copyFileSync,mkdirSync,existsSync} from 'node:fs';
import {resolve} from 'node:path';
import {createHash} from 'node:crypto';
const run=resolve(process.env.FOLIO_RELEASE_RUN),exec=promisify(execFile);
const state=JSON.parse(readFileSync(resolve(run,'process.json'),'utf8').replace(/^\uFEFF/,''));
const dir=resolve(run,'workflow');mkdirSync(dir);
const browser=await chromium.connectOverCDP(`http://127.0.0.1:${state.port}`);
const page=browser.contexts()[0].pages().find(p=>!p.url().startsWith('devtools:'));page.setDefaultTimeout(30000);
const errors=[],results=[];page.on('pageerror',e=>errors.push(String(e)));
const hash=path=>createHash('sha256').update(readFileSync(path)).digest('hex');
async function dialog(title,path,action){await exec('pwsh',['-NoProfile','-File',resolve('scripts/set-release-dialog.ps1'),'-AppProcessId',String(state.pid),'-FilePath',path,'-Title',title,'-Action',action],{windowsHide:true,timeout:25000});}
try{
 for(const count of [100,400,1600])for(const kind of ['top','forms']){
  const name=`dense-${count}-${kind}.pdf`,path=resolve(dir,name);
  copyFileSync(resolve('artifacts/edit-discovery-scale-20261024/corpus',name),path);const originalHash=hash(path);
  await page.getByRole('button',{name:results.length?'Open':'Open a PDF',exact:true}).click();await dialog('Open a PDF',path,'Open');
  await expect(page.getByRole('tab',{name,selected:true,exact:true})).toBeVisible();
  await expect(page.locator('.page-canvas').locator('image')).toHaveCount(1);
  const start=performance.now();await page.getByRole('button',{name:'Edit text',exact:true}).click();
  const runs=page.locator(kind==='forms'?'[data-text-path]':'[data-text-object]');
  await expect(runs).toHaveCount(count);const discoveryMs=performance.now()-start;
  await expect(page.locator('.existing-text-run.unsupported')).toHaveCount(0);
  await runs.first().focus();await page.keyboard.press('Enter');await expect(page.getByRole('dialog',{name:'Edit existing text'})).toBeVisible();
  expect(await page.getByRole('textbox',{name:'Replacement text',exact:true}).inputValue()).toBe('Old');
  await expect(page.getByRole('button',{name:'Apply changes',exact:true})).toBeDisabled();
  await page.getByRole('button',{name:'Cancel',exact:true}).click();
  if(count===1600)await page.screenshot({path:resolve(run,`${kind}-1600.png`)});
  if(count===100){
   await runs.first().focus();await page.keyboard.press('Enter');await page.getByRole('textbox',{name:'Replacement text',exact:true}).fill('New');
   await page.getByRole('button',{name:'Apply changes',exact:true}).click();await expect(page.getByRole('dialog')).toHaveCount(0);
   await page.getByRole('button',{name:'Save a copy',exact:true}).click();const output=resolve(dir,`${kind}-edited.pdf`);await dialog('Save a PDF copy',output,'Save');await expect.poll(()=>existsSync(output)).toBe(true);
   await expect(page.getByRole('button',{name:'Save a copy',exact:true})).toBeEnabled();
   await page.getByRole('button',{name:'Undo',exact:true}).click();
   await page.getByRole('button',{name:'Edit text',exact:true}).click();
   await runs.first().focus();await page.keyboard.press('Enter');
   await expect(page.getByRole('textbox',{name:'Replacement text',exact:true})).toHaveValue('Old');
   await page.getByRole('button',{name:'Cancel',exact:true}).click();
   await page.getByRole('button',{name:'Open',exact:true}).click();await dialog('Open a PDF',output,'Open');
   await expect(page.getByRole('tab',{name:`${kind}-edited.pdf`,selected:true,exact:true})).toBeVisible();
   await page.getByRole('button',{name:'Edit text',exact:true}).click();
   await expect(runs).toHaveCount(count);await runs.first().focus();await page.keyboard.press('Enter');
   await expect(page.getByRole('textbox',{name:'Replacement text',exact:true})).toHaveValue('New');
   await expect(page.getByRole('button',{name:'Apply changes',exact:true})).toBeDisabled();
   await page.getByRole('button',{name:'Cancel',exact:true}).click();
  }
  expect(hash(path)).toBe(originalHash);results.push({name,count,kind,discoveryMs,originalHash});
  await page.getByRole('button',{name:'Select',exact:true}).click();
 }
 expect(errors).toEqual([]);writeFileSync(resolve(run,'results.json'),JSON.stringify({passed:true,results,errors,measurement:'Edit text click through candidate DOM readiness, includes Playwright/IPC/polling; single cold sample per file',exe:state},null,2),{flag:'wx'});
 console.log('PASS dense Windows discovery, keyboard selection and editing');
}catch(e){await page.screenshot({path:resolve(run,'failure.png')}).catch(()=>{});throw e;}finally{await browser.close();}
