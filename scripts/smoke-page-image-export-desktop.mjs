// Owned Windows WebView2 acceptance, following the repository's native desktop workflow.
import {chromium,expect} from '@playwright/test';
import {execFile} from 'node:child_process';import {promisify} from 'node:util';
import {readFileSync,writeFileSync,readdirSync,existsSync,renameSync} from 'node:fs';import {resolve,basename} from 'node:path';import {createHash} from 'node:crypto';
const root=resolve(process.env.FOLIO_IMAGE_EXPORT_DESKTOP_ROOT||'artifacts/page-image-export-20261024/desktop-accepted');
const record=JSON.parse(readFileSync(resolve(root,'profile/process.json'),'utf8').replace(/^\uFEFF/,''));
const browser=await chromium.connectOverCDP(`http://127.0.0.1:${record.port}`),page=browser.contexts()[0].pages().find(p=>!p.url().startsWith('devtools:'));
page.setDefaultTimeout(30000);const errors=[],checks=[],outputs=[];page.on('pageerror',e=>errors.push(e.message));
const button=name=>page.getByRole('button',{name,exact:true}),sha=path=>createHash('sha256').update(readFileSync(path)).digest('hex');
async function picker(action,path){await promisify(execFile)('pwsh',['-NoProfile','-File',resolve('artifacts/set-multifont-dialog.ps1'),'-AppProcessId',String(record.pid),'-FilePath',path,'-Action',action,...(action==='Open'?['-NativeFileName']:[])],{windowsHide:true,timeout:60000});}
async function folder(path,cancel=false){await promisify(execFile)('pwsh',['-NoProfile','-File',resolve('artifacts/set-page-image-folder.ps1'),'-AppProcessId',String(record.pid),'-Folder',path,...(cancel?['-Cancel']:[])],{windowsHide:true,timeout:60000});}
async function open(path){await page.getByRole('button',{name:/^(Open|Open a PDF)$/}).click();await picker('Open',path);await expect(page.getByRole('tab',{name:basename(path),exact:true}).last()).toHaveAttribute('aria-selected','true');await expect(page.locator('.document-page image').first()).toBeVisible();}
async function dialog(){await button('Save options').click();await page.getByRole('menuitem',{name:'Export images…',exact:true}).click();await expect(page.getByRole('dialog')).toBeVisible();}
const folders=path=>readdirSync(path,{withFileTypes:true}).filter(x=>x.isDirectory()).map(x=>x.name);
async function exportImages(format,dpi,mode='all',range=''){
 await dialog();await page.getByRole('combobox',{name:'Pages',exact:true}).selectOption(mode);if(mode==='range')await page.getByLabel('Page range',{exact:true}).fill(range);
 await page.getByRole('combobox',{name:'Format',exact:true}).selectOption(format);await page.getByRole('combobox',{name:'Resolution',exact:true}).selectOption(String(dpi));
 if(format==='jpeg'){
  await page.getByLabel('JPEG quality',{exact:true}).fill('90');await page.setViewportSize({width:520,height:740});
  expect(await page.getByRole('dialog').evaluate(el=>el.scrollWidth<=el.clientWidth+1)).toBeTruthy();await button('Choose folder and export…').scrollIntoViewIfNeeded();await page.screenshot({path:resolve(root,'export-narrow.png')});await page.setViewportSize({width:1280,height:900});
 }
 await page.getByRole('combobox',{name:'Pages',exact:true}).focus();await page.keyboard.press('Tab');await expect(mode==='range'?page.getByLabel('Page range',{exact:true}):page.getByRole('combobox',{name:'Format',exact:true})).toBeFocused();
 await page.screenshot({path:resolve(root,`export-${format}.png`)});const target=resolve(root,'outputs'),before=folders(target);
 await button('Choose folder and export…').click();await folder(target);await expect(page.getByRole('dialog')).toHaveCount(0);await expect(page.locator('.statusbar')).toContainText('page image');
 const added=folders(target).filter(x=>!before.includes(x));expect(added).toHaveLength(1);const path=resolve(target,added[0]),files=readdirSync(path);expect(files.every(x=>x.endsWith(format==='png'?'.png':'.jpg'))).toBeTruthy();outputs.push({format,dpi,folder:path,files});
}
try{
 await page.setViewportSize({width:1280,height:900});const source=resolve(root,'Image export sample.pdf'),sourceHash=sha(source);await open(source);await button('Select').click();
 await button('Crop page…').click();for(const[side,value]of[['Left',15],['Top',20],['Right',25],['Bottom',30]])await page.getByLabel(`${side} margin (pt)`,{exact:true}).fill(String(value));await button('Preview crop').click();await expect(button('Apply crop')).toBeEnabled();await button('Apply crop').click();
 await button('Rotate').click();await button('Edit text').click();await page.getByRole('button',{name:'Edit text: Retained content',exact:true}).first().click();await page.getByLabel('Replacement text',{exact:true}).fill('Retained');await button('Apply changes').click();await expect(page.getByRole('dialog')).toHaveCount(0);
 await exportImages('png',150);expect(outputs[0].files.sort()).toEqual(['page-0001.png','page-0002.png']);await expect(page.getByLabel('Unsaved changes')).toBeVisible();checks.push('PNG batch retains crop, rotation, existing text edit and dirty state');
 await exportImages('jpeg',300,'range','2');expect(outputs[1].files).toEqual(['page-0002.jpg']);await expect(page.getByLabel('Unsaved changes')).toBeVisible();checks.push('JPEG selected original page number, real DPI, narrow layout and keyboard order');
 const before=folders(resolve(root,'outputs'));await dialog();await button('Choose folder and export…').click();await folder(resolve(root,'outputs'),true);await expect(page.getByRole('status')).toContainText('Export cancelled');await button('Cancel').click();expect(folders(resolve(root,'outputs'))).toEqual(before);checks.push('Native folder picker cancellation produces no output');
 // The native folder picker also updates Windows' remembered save directory.
 const expected=resolve(root,'outputs/Folio edited.pdf');if(existsSync(expected))throw Error('Preserve oracle');await button('Save a copy').click();await picker('Save',expected);await expect(page.locator('.statusbar')).toContainText('Saved a copy');renameSync(expected,resolve(root,'Expected.pdf'));
 expect(sha(source)).toBe(sourceHash);expect(readFileSync(resolve(root,'outputs/existing.txt'),'utf8')).toBe('preserve this file');
 await open(resolve(root,'Cancel sample.pdf'));await button('Select').click();for(let i=0;i<12;i++)await button('Duplicate').click();
 await dialog();await page.getByRole('combobox',{name:'Pages',exact:true}).selectOption('all');await page.getByRole('combobox',{name:'Resolution',exact:true}).selectOption('300');await button('Choose folder and export…').click();await folder(resolve(root,'cancel-outputs'));
 await expect(page.getByRole('status')).toContainText(/of 13 pages exported/);await button('Cancel export').click();await expect(page.getByRole('status')).toContainText('Export cancelled');expect(folders(resolve(root,'cancel-outputs'))).toHaveLength(0);await button('Cancel').click();checks.push('Cancelling a running 13-page export removes its owned partial folder');
 expect(errors).toEqual([]);writeFileSync(resolve(root,'results.json'),JSON.stringify({passed:true,checks,outputs,source,sourceHash,oracle:resolve(root,'Expected.pdf'),errors,exe:record},null,2));console.log(JSON.stringify({passed:true,checks}));
}catch(error){writeFileSync(resolve(root,'failure.json'),JSON.stringify({error:String(error.stack),checks,outputs,errors},null,2));await page.screenshot({path:resolve(root,'failure.png')}).catch(()=>{});throw error;}finally{await browser.close();}
