// Owned Windows/WebView acceptance. Inputs are synthetic, redistributable fixtures.
import {chromium,expect} from '@playwright/test';
import {execFile} from 'node:child_process';
import {promisify} from 'node:util';
import {readFileSync,writeFileSync,existsSync,renameSync} from 'node:fs';
import {resolve,basename} from 'node:path';
import {createHash} from 'node:crypto';
const root=resolve(process.env.FOLIO_EMBEDDED_FORM_DESKTOP_ROOT);
const record=JSON.parse(readFileSync(resolve(root,'profile/process.json'),'utf8').replace(/^\uFEFF/,''));
const browser=await chromium.connectOverCDP(`http://127.0.0.1:${record.port}`);
const page=browser.contexts()[0].pages().find(p=>!p.url().startsWith('devtools:'));page.setDefaultTimeout(30000);
const errors=[],checks=[];page.on('pageerror',error=>errors.push(error.message));
const button=name=>page.getByRole('button',{name,exact:true});
const draft=()=>page.getByRole('textbox',{name:'Replacement text',exact:true});
const hash=path=>createHash('sha256').update(readFileSync(path)).digest('hex');
async function picker(action,path){await promisify(execFile)('pwsh',['-NoProfile','-File',resolve('scripts/set-native-dialog.ps1'),'-AppProcessId',String(record.pid),'-FilePath',path,'-Action',action,...(action==='Open'?['-NativeFileName']:[])],{windowsHide:true,timeout:60000});}
async function open(path){await page.getByRole('button',{name:/^(Open|Open a PDF)$/}).click();await picker('Open',path);await expect(page.getByRole('tab',{name:basename(path),exact:true}).last()).toHaveAttribute('aria-selected','true');await expect(page.locator('.document-page image').first()).toBeVisible();}
async function save(name){const path=resolve(root,name),native=resolve(root,'Folio edited.pdf');if(existsSync(path)||existsSync(native))throw Error('Output already exists');await button('Save a copy').click();await picker('Save',native);await expect(page.locator('.statusbar')).toContainText('Saved a copy');renameSync(native,path);return path;}
async function select(){await button('Edit text').click();await page.locator('[data-text-path="1.0.0"]').first().click();await expect(draft()).toBeVisible();}
try {
 await page.setViewportSize({width:1280,height:900});
 const sources={};
 for(const kind of ['simple','cid']) {
  const source=resolve(root,`${kind}-original.pdf`);sources[kind]=hash(source);await open(source);
  await save(`${kind}-before.pdf`);await select();
  await expect(draft()).toHaveValue('Café old');await expect(page.locator('.existing-text-font')).toContainText('DejaVu Serif');
  await expect(page.getByText('Font substitution is not available for this occurrence.')).toBeVisible();
  await expect(button('Choose substitute font…')).toHaveCount(0);await expect(button('Edit paragraph…')).toHaveCount(0);
  await draft().fill('Cafè old');await button('Apply changes').click();await expect(page.getByRole('alert')).toContainText('glyph');await expect(draft()).toHaveValue('Cafè old');
  await page.screenshot({path:resolve(root,`${kind}-missing-glyph.png`)});
  await draft().fill('Café edit');await button('Apply changes').click();await expect(draft()).toHaveCount(0);
  await button('Undo').click();await save(`${kind}-undo.pdf`);await button('Redo').click();
  const edited=await save(`${kind}-edited.pdf`);await open(edited);await select();await expect(draft()).toHaveValue('Café edit');
  await expect(page.locator('.existing-text-font')).toContainText('DejaVu Serif');
  await page.setViewportSize({width:640,height:720});await expect(button('Apply changes')).toBeInViewport();await page.screenshot({path:resolve(root,`${kind}-reopened-narrow.png`)});await page.setViewportSize({width:1280,height:900});
  await draft().fill('Café old');await button('Apply changes').click();await save(`${kind}-restored.pdf`);
  expect(hash(source)).toBe(sources[kind]);checks.push(`${kind}: readable label, Unicode, missing-glyph draft, undo/redo, save/reopen, narrow dialog, re-edit`);
 }
 expect(errors).toEqual([]);writeFileSync(resolve(root,'results.json'),JSON.stringify({passed:true,checks,sources,errors,exe:record},null,2));console.log('PASS embedded form Windows acceptance');
}catch(error){writeFileSync(resolve(root,'failure.json'),JSON.stringify({error:String(error.stack),checks,errors},null,2));await page.screenshot({path:resolve(root,'failure.png')}).catch(()=>{});throw error;}
finally{await browser.close();}
