// Repository Windows/WebView2 acceptance, restricted to an owned Folio process.
import {chromium,expect} from '@playwright/test';
import {execFile} from 'node:child_process';
import {promisify} from 'node:util';
import {readFileSync,writeFileSync,existsSync,renameSync} from 'node:fs';
import {resolve,basename} from 'node:path';
import {createHash} from 'node:crypto';
const root=resolve(process.env.FOLIO_FORM_FLOW_DESKTOP_ROOT);
const record=JSON.parse(readFileSync(resolve(root,'profile/process.json'),'utf8').replace(/^\uFEFF/,''));
const browser=await chromium.connectOverCDP(`http://127.0.0.1:${record.port}`);
const page=browser.contexts()[0].pages().find(p=>!p.url().startsWith('devtools:'));
page.setDefaultTimeout(30000);
const errors=[],checks=[],sources={};page.on('pageerror',error=>errors.push(error.message));
const button=name=>page.getByRole('button',{name,exact:true});
const draft=()=>page.getByRole('textbox',{name:'Replacement text',exact:true});
const hash=path=>createHash('sha256').update(readFileSync(path)).digest('hex');
async function picker(action,path,title){await promisify(execFile)('pwsh',['-NoProfile','-File',resolve('artifacts/set-multifont-dialog.ps1'),'-AppProcessId',String(record.pid),'-FilePath',path,'-Action',action,...(title?['-DialogTitle',title]:[]),...(action==='Open'?['-NativeFileName']:[])],{windowsHide:true,timeout:60000});}
async function open(path){await page.getByRole('button',{name:/^(Open|Open a PDF)$/}).click();await picker('Open',path);await expect(page.getByRole('tab',{name:basename(path),exact:true}).last()).toHaveAttribute('aria-selected','true');await expect(page.locator('.document-page image').first()).toBeVisible();}
async function save(name){const path=resolve(root,name),native=resolve(root,'Folio edited.pdf');if(existsSync(path)||existsSync(native))throw Error('Output already exists');await button('Save a copy').click();await picker('Save',native);await expect(page.locator('.statusbar')).toContainText('Saved a copy');renameSync(native,path);return path;}
async function select(path){await button('Edit text').click();const target=page.locator(`[data-text-path="${path}"]`).first();await expect(target).toBeVisible();await target.scrollIntoViewIfNeeded();await target.click();await expect(draft()).toBeVisible();}
try {
 await page.setViewportSize({width:1280,height:900});
 for(const [prefix,target,old,replacement] of [
  ['reportlab-draw-string','0.1','Second sentence','Second longer sentence'],
  ['reportlab-text-lines','0.0','First sentence','First longer sentence'],
  ['simple-form-shear-90','1.0.1','Café old','Cafe'],
 ]) {
  const source=resolve(root,`${prefix}-original.pdf`);sources[prefix]=hash(source);await open(source);await select(target);await expect(draft()).toHaveValue(old);
  await page.setViewportSize({width:640,height:720});await draft().fill(replacement);await button('Apply changes').scrollIntoViewIfNeeded();await expect(button('Apply changes')).toBeInViewport();await page.screenshot({path:resolve(root,`${prefix}-dialog.png`)});await page.setViewportSize({width:1280,height:900});
  await button('Apply changes').click();await expect(draft()).toHaveCount(0);await button('Undo').click();await save(`${prefix}-undo.pdf`);await button('Redo').click();const edited=await save(`${prefix}-edited.pdf`);
  await open(edited);await select(target);await expect(draft()).toHaveValue(replacement);await draft().fill(old);await button('Apply changes').click();await save(`${prefix}-restored.pdf`);
  expect(hash(source)).toBe(sources[prefix]);checks.push(`${prefix}: actual first/later line hit target, narrow dialog, edit/undo/redo/save/reopen/restore`);
 }
 const prefix='cid-double',target='1.0.1',source=resolve(root,`${prefix}-original.pdf`),font=resolve(root,'Substitute.ttf');sources[prefix]=hash(source);const fontHash=hash(font);
 await open(source);await select(target);await expect(draft()).toHaveValue('Café old');await draft().fill('Cafè edit');await button('Choose substitute font…').click();await button('Import font…').click();await picker('Open',font,'Import a font');await expect(button('Apply font')).toBeEnabled();await button('Apply font').click();await button('Apply changes').click();await expect(draft()).toHaveCount(0);
 await button('Undo').click();await save(`${prefix}-undo.pdf`);await button('Redo').click();const edited=await save(`${prefix}-edited.pdf`);await open(edited);await select(target);await expect(draft()).toHaveValue('Cafè edit');await button('Cancel').click();
 await select('1.0.0');await expect(draft()).toHaveValue('Café old');await button('Cancel').click();
 expect(hash(source)).toBe(sources[prefix]);expect(hash(font)).toBe(fontHash);checks.push('Identity-H double-quote later line substitutes independently; preceding line, shared copy and spacing retained through undo/redo/save/reopen');
 await page.screenshot({path:resolve(root,'accepted.png')});expect(errors).toEqual([]);
 writeFileSync(resolve(root,'results.json'),JSON.stringify({passed:true,checks,sources,fontHash,errors,exe:record},null,2));console.log('PASS reusable text font and line flow Windows acceptance');
}catch(error){writeFileSync(resolve(root,'failure.json'),JSON.stringify({error:String(error.stack),checks,errors},null,2));await page.screenshot({path:resolve(root,'failure.png')}).catch(()=>{});throw error;}
finally{await browser.close();}
