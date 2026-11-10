// Repository desktop acceptance workflow, using an isolated owned WebView profile.
import {chromium,expect} from '@playwright/test';
import {readFileSync,writeFileSync} from 'node:fs';
import {resolve} from 'node:path';
import {execFile} from 'node:child_process';
import {promisify} from 'node:util';
import {createHash} from 'node:crypto';
const directory=resolve(process.env.FOLIO_COMMENT_ACCEPTANCE_ROOT ?? 'artifacts/comment-fidelity-desktop'),exec=promisify(execFile);
const record=JSON.parse(readFileSync(resolve(directory,process.env.FOLIO_COMMENTS_PROFILE||'profile','process.json'),'utf8').replace(/^\uFEFF/,''));
const original=createHash('sha256').update(readFileSync(resolve(directory,'Comment fidelity.pdf'))).digest('hex');
const browser=await chromium.connectOverCDP(`http://127.0.0.1:${record.port}`),page=browser.contexts()[0].pages().find(p=>!p.url().startsWith('devtools:'));
page.setDefaultTimeout(15000);await page.setViewportSize({width:1000,height:760});const errors=[],checks=[];page.on('pageerror',e=>errors.push(e.message));
const button=name=>page.getByRole('button',{name,exact:true});
async function picker(action,name){await exec('pwsh',['-NoProfile','-ExecutionPolicy','Bypass','-File',resolve('scripts/set-native-dialog.ps1'),'-AppProcessId',String(record.pid),'-Action',action,'-FilePath',resolve(directory,name),...(action==='Open'?['-NativeFileName']:[])],{windowsHide:true,timeout:60000});}
try{
 await button('Open a PDF').click();await picker('Open','Comment fidelity.pdf');
 await button('Page 1: Locked reference note').click();await expect(button('Delete comment')).toBeDisabled();await expect(button('Apply comment')).toBeDisabled();await button('Close').click();
 await button('Page 2: Review this paragraph').click();await expect(page.getByLabel('Comment color')).toBeDisabled();await page.getByRole('textbox',{name:'Comment text'}).fill('Moved café 日本語');await page.getByRole('spinbutton',{name:'Left (pt)'}).fill('80');await page.getByRole('spinbutton',{name:'Top (pt)'}).fill('100');await page.screenshot({path:resolve(directory,'position-dialog.png')});await button('Apply comment').click();await expect(page.getByRole('dialog')).toHaveCount(0);
 await button('Undo').click();await expect(button('Page 2: Review this paragraph')).toBeVisible();await button('Redo').click();await button('Page 2: Moved café 日本語').click();await expect(page.getByRole('spinbutton',{name:'Left (pt)'})).toHaveValue('80');await expect(page.getByRole('spinbutton',{name:'Top (pt)'})).toHaveValue('100');await button('Close').click();checks.push('standard custom-icon note text, movement and undo/redo');
 await button('Page 2: Color note').click();await page.getByLabel('Comment color').fill('#123456');await page.getByRole('textbox',{name:'Comment text'}).fill('Recolored note');await button('Apply comment').click();await expect(page.getByRole('dialog')).toHaveCount(0);checks.push('plain-icon color editing');
 await button('Page 2: Remove this note').click();await button('Delete comment').click();await expect(button('Page 2: Remove this note')).toHaveCount(0);await button('Undo').click();await expect(button('Page 2: Remove this note')).toBeVisible();await button('Redo').click();await expect(button('Page 2: Remove this note')).toHaveCount(0);checks.push('native deletion with undo/redo');
 await button('Save a copy').click();await picker('Save','Folio edited.pdf');await expect(page.getByLabel('Unsaved changes')).toHaveCount(0);await button('Open').click();await picker('Open','Folio edited.pdf');await expect(page.getByRole('tab',{name:'Folio edited.pdf',exact:true})).toHaveAttribute('aria-selected','true');
 await button('Page 2: Moved café 日本語').click();await expect(page.getByRole('spinbutton',{name:'Left (pt)'})).toHaveValue('80');await expect(page.getByRole('spinbutton',{name:'Top (pt)'})).toHaveValue('100');await button('Close').click();await button('Page 2: Recolored note').click();await expect(page.getByLabel('Comment color')).toHaveValue('#123456');await button('Close').click();await expect(button('Page 2: Remove this note')).toHaveCount(0);checks.push('native Save a copy and reopen');
 await page.screenshot({path:resolve(directory,'saved-review.png')});expect(errors).toEqual([]);expect(createHash('sha256').update(readFileSync(resolve(directory,'Comment fidelity.pdf'))).digest('hex')).toBe(original);
 writeFileSync(resolve(directory,'results.json'),JSON.stringify({passed:true,checks,errors,originalSha256:original,executableSha256:record.sha256},null,2));console.log('PASS '+checks.join('; '));
}catch(error){await page.screenshot({path:resolve(directory,'failure.png')}).catch(()=>{});throw error;}finally{await browser.close();}
