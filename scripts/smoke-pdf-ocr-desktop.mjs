// Uses only the recorded, isolated OCR desktop instance and generated fixtures.
import {chromium,expect} from '@playwright/test';
import {execFile} from 'node:child_process';
import {promisify} from 'node:util';
import {readFileSync,writeFileSync,existsSync,renameSync} from 'node:fs';
import {resolve,basename} from 'node:path';
import {createHash} from 'node:crypto';
const exec=promisify(execFile),dir=resolve('artifacts/pdf-ocr-desktop');
const record=JSON.parse(readFileSync('artifacts/owned-pdf-ocr-app.json','utf8').replace(/^\uFEFF/,''));
const browser=await chromium.connectOverCDP('http://127.0.0.1:9439');
const context=browser.contexts()[0],page=context.pages().find(p=>!p.url().startsWith('devtools:'));
page.setDefaultTimeout(15000);
const errors=[],external=[],workers=[];page.on('pageerror',e=>errors.push(e.message));page.on('worker',worker=>workers.push(worker.url()));
await context.route('**/*',route=>{const url=new URL(route.request().url());if(['127.0.0.1','localhost','tauri.localhost','ipc.localhost'].includes(url.hostname)||['blob:','data:'].includes(url.protocol))return route.continue();external.push(url.href);return route.abort();});
const button=name=>page.getByRole('button',{name,exact:true});
const text=()=>page.locator('.document-page').first().locator('.pdf-text-layer').textContent();
async function dialog(action,path){await exec('pwsh',['-NoProfile','-File',resolve('scripts/set-native-dialog.ps1'),'-AppProcessId',String(record.pid),'-FilePath',path,'-Action',action],{windowsHide:true,timeout:60000});}
async function open(path){const tab=page.getByRole('tab',{name:basename(path),exact:true}).last();if(await tab.count())await tab.click();else{await page.getByRole('button',{name:/^(Open|Open a PDF)$/}).click();await dialog('Open',path);}await expect(tab).toHaveAttribute('aria-selected','true',{timeout:30000});await expect(page.locator('.document-page').first().locator('image')).toHaveCount(1,{timeout:30000});}
async function save(name){const native=resolve(dir,'Folio edited.pdf'),output=resolve(dir,name);if(existsSync(native)||existsSync(output))throw Error('Expected unused generated output');await button('Save a copy').click();await dialog('Save',native);await expect(page.locator('.statusbar')).toContainText('Saved a copy',{timeout:30000});await expect.poll(()=>existsSync(native)).toBe(true);renameSync(native,output);return output;}
async function recognize(all=false){await button('Recognize text').click();if(all)await page.getByRole('combobox',{name:'Pages',exact:true}).selectOption('all');await button('Start recognition').click();await expect(page.getByRole('combobox',{name:'Review page',exact:true})).toBeVisible({timeout:120000});await expect(page.getByAltText('Scanned page preview')).toBeVisible();}
async function find(phrase,count){await button('Find').click();await page.getByRole('searchbox').fill(phrase);await expect(page.locator('.search-count')).toContainText(count,{timeout:15000});await button('Close search').click();}
const result={checks:[],errors,external,workers,exe:record};
try{
 if(await button('Discard recovery').isVisible())await button('Discard recovery').click();
 await open(resolve(dir,'Mixed scan.pdf'));
 await button('Comment').click();await page.locator('.document-page').first().locator('.page-canvas').click({position:{x:220,y:360}});
 await page.getByRole('textbox',{name:'Comment',exact:true}).fill('Review OCR result');await button('Select').click();
 result.baseline=await save('Before OCR.pdf');
 await page.screenshot({path:resolve(dir,'01-workspace.png')});
 await button('Recognize text').click();await button('Cancel').click();await expect(page.getByRole('dialog')).toHaveCount(0);result.checks.push('setup cancellation');
 await recognize(true);await page.screenshot({path:resolve(dir,'02-review.png')});
 console.log('REVIEW',await page.locator('.ocr-word-summary').innerText());
 const words=page.getByLabel('Recognized words');
 await expect(words.getByRole('button',{name:/^Native Existing text$/})).toBeVisible();
 await expect(words.getByRole('button',{name:/^header Existing text$/})).toBeVisible();
 await words.getByRole('button',{name:/^Invoice Score/}).click();
 await page.getByLabel('Recognized text',{exact:true}).fill('CorrectedInvoice');await button('Save correction').click();
 await page.getByRole('combobox',{name:'Review page',exact:true}).selectOption('1');await expect(words).toContainText('Second');
 await page.getByRole('combobox',{name:'Review page',exact:true}).selectOption('0');await expect(words).toContainText('CorrectedInvoice');
 await button('Apply searchable text').click();await expect(page.getByRole('dialog')).toHaveCount(0,{timeout:30000});
 await expect.poll(text).toContain('CorrectedInvoice');expect((await text()).match(/Native header/g)).toHaveLength(1);
 result.checks.push('two-page local recognition','native header excluded exactly once','review correction and page navigation','apply searchable text');
 await find('CorrectedInvoice','1 of 1');await find('Second scanned page','1 of 1');
 await button('Undo').click();await expect.poll(text).not.toContain('CorrectedInvoice');await find('CorrectedInvoice','No matches');
 await button('Redo').click();await expect.poll(text).toContain('CorrectedInvoice');result.checks.push('search and one-step undo/redo');
 await expect(button('Page 1: Review OCR result')).toBeVisible();
 const saved=await save('Searchable scan.pdf');result.saved=saved;
 await open(saved);await expect.poll(text).toContain('CorrectedInvoice');await expect(button('Page 1: Review OCR result')).toBeVisible();
 const workerCount=workers.length;await recognize();expect(workers.length).toBe(workerCount);
 await expect(words).toContainText('CorrectedInvoice');await expect(button('Apply searchable text')).toBeDisabled();
 await words.getByRole('button',{name:/^CorrectedInvoice Score/}).click();await page.getByLabel('Recognized text',{exact:true}).fill('ReopenedInvoice');await button('Save correction').click();
 await button('Apply searchable text').click();await expect(page.getByRole('dialog')).toHaveCount(0,{timeout:30000});await expect.poll(text).toContain('ReopenedInvoice');
 result.checks.push('save/reopen preserves comment and text','verified OCR review without worker','replace reopened OCR');
 result.corrected=await save('Corrected scan.pdf');
 await page.screenshot({path:resolve(dir,'03-corrected.png')});
 expect(errors).toEqual([]);expect(external).toEqual([]);
 result.passed=true;writeFileSync(resolve(dir,'desktop-results.json'),JSON.stringify(result,null,2));console.log(JSON.stringify(result));
}catch(error){writeFileSync(resolve(dir,'desktop-failure.json'),JSON.stringify({...result,error:String(error.stack)},null,2));await page.screenshot({path:resolve(dir,'desktop-failure.png')}).catch(()=>{});throw error;}
finally{await browser.close();}
