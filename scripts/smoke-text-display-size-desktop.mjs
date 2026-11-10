import {chromium,expect} from '@playwright/test';
import {readFileSync,writeFileSync} from 'node:fs';
import {resolve} from 'node:path';
import {execFile} from 'node:child_process';
import {promisify} from 'node:util';
import {createHash} from 'node:crypto';
const root=resolve('artifacts/text-display-size-20261025/desktop');
const read=name=>JSON.parse(readFileSync(resolve(root,name),'utf8').replace(/^\uFEFF/,''));
const record=read('process.json'),inputs=read('inputs.json');
const browser=await chromium.connectOverCDP(`http://127.0.0.1:${record.port}`);
const page=browser.contexts()[0].pages().find(p=>!p.url().startsWith('devtools:'));page.setDefaultTimeout(30000);
const button=name=>page.getByRole('button',{name,exact:true});
const checks=[],errors=[];page.on('pageerror',e=>errors.push(e.message));
const sha=name=>createHash('sha256').update(readFileSync(resolve(root,name))).digest('hex');
async function picker(action,name){await promisify(execFile)('pwsh',['-NoProfile','-File',resolve('scripts/set-release-dialog.ps1'),'-AppProcessId',String(record.pid),'-Action',action,'-Title',action==='Open'?'Open a PDF':'Save a PDF copy','-FilePath',resolve(root,name)],{windowsHide:true,timeout:60000});}
async function open(name,first=false){await button(first?'Open a PDF':'Open').click();await picker('Open',name);await expect(page.getByRole('tab',{name,exact:true})).toHaveAttribute('aria-selected','true');await button('Edit text').click();}
async function inspect(text,size){await button(`Edit text: ${text}`).click();await expect(page.locator('.existing-text-font')).toHaveText(`Helvetica · ${size} pt`);}
try{
 await page.setViewportSize({width:1280,height:900});
 for(const [i,input] of inputs.entries()){
  const kind=input.name.replace('.pdf','');await open(input.name,i===0);await inspect('Old',input.size);
  if(kind==='group'){
   await button('Edit together…').click();await page.getByRole('checkbox',{name:'words',exact:true}).check();await button('Check selection').click();
   await expect(page.locator('.existing-text-font')).toHaveText(`Helvetica · ${input.size} pt`);
  }
  const replacement=kind==='group'?'New words':'New';await page.getByRole('textbox',{name:'Replacement text',exact:true}).fill(replacement);
  await page.screenshot({path:resolve(root,`${kind}-label.png`)});await button('Apply changes').click();await expect(page.getByRole('dialog')).toHaveCount(0);
  await button('Undo').click();await button('Redo').click();await button('Select').click();
  const output=`${kind}-edited.pdf`;await button('Save a copy').click();await picker('Save',output);await expect(page.getByLabel('Unsaved changes')).toHaveCount(0);
  await open(output);await inspect(replacement,input.size);await expect(button('Apply changes')).toBeDisabled();await button('Cancel').click();await button('Select').click();
  expect(sha(input.name)).toBe(input.sha256);checks.push({kind,size:input.size,output,sha256:sha(output),originalSha256:input.sha256});
 }
 expect(errors).toEqual([]);writeFileSync(resolve(root,'results.json'),JSON.stringify({passed:true,checks,errors,exe:record},null,2),{flag:'wx'});console.log('PASS direct, grouped and nested-form labels, edit, undo/redo, save/reopen');
}catch(error){writeFileSync(resolve(root,'failure.json'),JSON.stringify({error:String(error.stack),checks,errors},null,2));await page.screenshot({path:resolve(root,'failure.png')}).catch(()=>{});throw error;}finally{await browser.close();}
