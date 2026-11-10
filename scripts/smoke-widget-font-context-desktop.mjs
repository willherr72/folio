import {chromium,expect} from '@playwright/test';
import {readFileSync,writeFileSync} from 'node:fs';
import {resolve} from 'node:path';
import {execFile} from 'node:child_process';
import {promisify} from 'node:util';
import {createHash} from 'node:crypto';
const root=resolve(process.env.FOLIO_WIDGET_FONT_ROOT??'artifacts/widget-font-context-20261025/desktop');
const record=JSON.parse(readFileSync(resolve(root,'process.json'),'utf8').replace(/^\uFEFF/,''));
const browser=await chromium.connectOverCDP(`http://127.0.0.1:${record.port}`);
const page=browser.contexts()[0].pages().find(p=>!p.url().startsWith('devtools:'));page.setDefaultTimeout(30000);
const checks=[],errors=[];page.on('pageerror',e=>errors.push(e.message));
const button=name=>page.getByRole('button',{name,exact:true});
const sha=name=>createHash('sha256').update(readFileSync(resolve(root,name))).digest('hex');
async function picker(action,name){await promisify(execFile)('pwsh',['-NoProfile','-File',resolve('scripts/set-release-dialog.ps1'),'-AppProcessId',String(record.pid),'-Action',action,'-Title',action==='Open'?'Open a PDF':'Save a PDF copy','-FilePath',resolve(root,name)],{windowsHide:true,timeout:60000});}
async function open(name,first=false){await button(first?'Open a PDF':'Open').click();await picker('Open',name);await expect(page.getByRole('tab',{name,exact:true})).toHaveAttribute('aria-selected','true');}
async function edit(from,to){await button('Edit text').click();await page.getByRole('button',{name:`Edit text: ${from}`,exact:true}).click();await page.getByRole('textbox',{name:'Replacement text',exact:true}).fill(to);await button('Apply changes').click();await expect(page.getByRole('dialog')).toHaveCount(0);await expect(page.getByLabel('Unsaved changes')).toBeVisible();await button('Select').click();}
async function save(name){await button('Save a copy').click();await picker('Save',name);await expect(page.getByLabel('Unsaved changes')).toHaveCount(0);}
async function customer(value){await button('Fill forms').click();await page.getByRole('button',{name:'Select form field: Customer',exact:true}).click();await expect(page.getByRole('textbox',{name:'Customer',exact:true})).toHaveValue(value);}
try{
 await page.setViewportSize({width:1280,height:900});const sourceHash=sha('Text sample.pdf');
 await open('Text sample.pdf',true);await edit('First page','Edited page');
 await button('Undo').click();await expect(page.getByLabel('Unsaved changes')).toHaveCount(0);await button('Redo').click();await expect(page.getByLabel('Unsaved changes')).toBeVisible();
 await customer('Ada');await button('Done').click();await save('Edited.pdf');await page.screenshot({path:resolve(root,'edited.png')});checks.push('original missing-DR fixture: text replacement, undo/redo, retained Customer, saved copy');
 await open('Edited.pdf');await customer('Ada');await page.getByRole('textbox',{name:'Customer',exact:true}).fill('Grace');await button('Apply field').click();await expect(button('Apply field')).toBeDisabled();await expect(page.getByRole('combobox',{name:'Field',exact:true})).toBeEnabled();await button('Done').click();await save('Filled.pdf');checks.push('saved/reopened text edit retains native form filling');
 await open('Filled.pdf');await customer('Grace');await button('Done').click();await edit('Edited page','Final page');await save('Reedited.pdf');
 await open('Reedited.pdf');await customer('Grace');await page.screenshot({path:resolve(root,'reopened.png')});await button('Done').click();checks.push('filled value survives subsequent existing-text edit and second save/reopen');
 expect(sha('Text sample.pdf')).toBe(sourceHash);expect(errors).toEqual([]);
 writeFileSync(resolve(root,'results.json'),JSON.stringify({passed:true,checks,errors,sourceHash,outputs:['Edited.pdf','Filled.pdf','Reedited.pdf'].map(name=>({name,sha256:sha(name)})),exe:record},null,2));console.log('PASS Windows widget font context');
}catch(error){writeFileSync(resolve(root,'failure.json'),JSON.stringify({error:String(error.stack),checks,errors},null,2));await page.screenshot({path:resolve(root,'failure.png')}).catch(()=>{});throw error;}finally{await browser.close();}
