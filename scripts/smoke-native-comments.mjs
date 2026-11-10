// Uses the repository's documented Playwright desktop acceptance workflow.
import {chromium,expect} from '@playwright/test';
import {mkdirSync,readFileSync,writeFileSync} from 'node:fs';
import {resolve} from 'node:path';
import {execFile} from 'node:child_process';
import {promisify} from 'node:util';
import {createHash} from 'node:crypto';
const desktop=process.env.FOLIO_COMMENTS_DESKTOP==='1',exec=promisify(execFile);
const directory=resolve(desktop?'artifacts/native-comments-desktop':'artifacts/native-comments-browser');mkdirSync(directory,{recursive:true});
const record=desktop?JSON.parse(readFileSync(resolve(directory,process.env.FOLIO_COMMENTS_PROFILE||'profile','process.json'),'utf8').replace(/^\uFEFF/,'')):null;
const original=desktop?createHash('sha256').update(readFileSync(resolve(directory,'Native comments.pdf'))).digest('hex'):null;
const browser=desktop?await chromium.connectOverCDP(`http://127.0.0.1:${record.port}`):await chromium.launch({headless:true});
const page=desktop?browser.contexts()[0].pages().find(p=>!p.url().startsWith('devtools:')):await browser.newPage();
page.setDefaultTimeout(15000);await page.setViewportSize({width:1000,height:700});const errors=[],checks=[];page.on('pageerror',e=>errors.push(e.message));
const button=name=>page.getByRole('button',{name,exact:true});
async function picker(action,name){await exec('pwsh',['-NoProfile','-ExecutionPolicy','Bypass','-File',resolve('scripts/set-native-dialog.ps1'),'-AppProcessId',String(record.pid),'-Action',action,'-FilePath',resolve(directory,name),...(action==='Open'?['-NativeFileName']:[])],{windowsHide:true,timeout:60000});}
try{
 if(!desktop)await page.goto('http://127.0.0.1:1437/tests/fixtures/native-comments.html');
 await button('Open a PDF').click();if(desktop)await picker('Open','Native comments.pdf');
 await button('Page 1: Locked reference note').click();await expect(page.getByRole('textbox',{name:'Comment text'})).toHaveAttribute('readonly','');await expect(button('Apply comment')).toBeDisabled();await button('Close').click();checks.push('locked note is readable');
 await button('Page 2: Review this paragraph').click();await expect(button('Go to page')).toHaveText('Page 2 of 2');
 await expect(page.getByRole('dialog')).toContainText('Reviewer');await expect(page.getByRole('dialog')).toContainText('Check wording');
 await page.getByRole('textbox',{name:'Comment text'}).fill('Updated café 日本語\nSecond line');await page.screenshot({path:resolve(directory,'comment-dialog.png')});await button('Apply comment').click();
 await expect(page.getByRole('dialog')).toHaveCount(0);await expect(button('Page 2: Updated café 日本語\nSecond line')).toBeVisible();checks.push('native note navigation and Unicode edit');
 await button('Undo').click();await expect(button('Page 2: Review this paragraph')).toBeVisible();await button('Redo').click();await expect(button('Page 2: Updated café 日本語\nSecond line')).toBeVisible();checks.push('undo and redo');
 if(desktop){
  await button('Save a copy').click();await picker('Save','Folio edited.pdf');await expect(page.getByLabel('Unsaved changes')).toHaveCount(0);
  await button('Open').click();await picker('Open','Folio edited.pdf');await expect(page.getByRole('tab',{name:'Folio edited.pdf',exact:true})).toHaveAttribute('aria-selected','true');
  await expect(button('Page 2: Updated café 日本語\nSecond line')).toBeVisible();await expect(button('Page 1: Locked reference note')).toBeVisible();checks.push('native Save a copy and reopen');
  expect(createHash('sha256').update(readFileSync(resolve(directory,'Native comments.pdf'))).digest('hex')).toBe(original);
 }
 await page.screenshot({path:resolve(directory,'review.png')});expect(errors).toEqual([]);
 writeFileSync(resolve(directory,'results.json'),JSON.stringify({passed:true,desktop,checks,errors,originalSha256:original,executableSha256:record?.sha256},null,2));console.log('PASS '+checks.join('; '));
}catch(error){await page.screenshot({path:resolve(directory,'failure.png')}).catch(()=>{});throw error;}finally{await browser.close();}
