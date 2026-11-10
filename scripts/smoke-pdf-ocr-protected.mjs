import {chromium,expect} from '@playwright/test';
import {execFile} from 'node:child_process';import {promisify} from 'node:util';
import {readFileSync,writeFileSync,existsSync,renameSync,copyFileSync} from 'node:fs';import {resolve,basename} from 'node:path';
const exec=promisify(execFile),dir=resolve('artifacts/pdf-ocr-desktop');
const record=JSON.parse(readFileSync('artifacts/owned-pdf-ocr-app.json','utf8').replace(/^\uFEFF/,''));
const browser=await chromium.connectOverCDP('http://127.0.0.1:9439'),context=browser.contexts()[0],page=context.pages().find(p=>!p.url().startsWith('devtools:'));
page.setDefaultTimeout(15000);const errors=[],external=[],checks=[],workers=[];
page.on('pageerror',e=>errors.push(e.message));page.on('worker',w=>workers.push(w.url()));
await context.route('**/*',route=>{const u=new URL(route.request().url());if(['127.0.0.1','localhost','tauri.localhost','ipc.localhost'].includes(u.hostname)||['blob:','data:'].includes(u.protocol))return route.continue();external.push(u.href);return route.abort();});
const button=name=>page.getByRole('button',{name,exact:true});
async function dialog(action,path){const args=['-NoProfile','-File',resolve('scripts/set-native-dialog.ps1'),'-AppProcessId',String(record.pid),'-FilePath',path,'-Action',action];try{await exec('pwsh',args,{windowsHide:true,timeout:60000});}catch(e){if(!String(e.stderr).includes('Could not submit'))throw e;console.log('Retrying owned native dialog after accessibility timeout');await exec('pwsh',args,{windowsHide:true,timeout:60000});}}
async function open(path,password){await page.getByRole('button',{name:/^(Open|Open a PDF)$/}).click();await dialog('Open',path);if(password){await page.getByLabel('PDF password',{exact:true}).fill(password);await button('Open PDF').click();}await expect(page.getByRole('tab',{name:basename(path),exact:true}).last()).toHaveAttribute('aria-selected','true',{timeout:30000});}
async function recognize(){await button('Recognize text').click();await button('Start recognition').click();await expect(page.getByRole('combobox',{name:'Review page',exact:true})).toBeVisible({timeout:120000});}
try{
 await open(resolve(dir,'Protected scan.pdf'),'reader');await expect(button('Recognize text')).toBeDisabled();checks.push('restricted reader cannot recognize');
 await button('Unlock editing').click();await page.getByLabel('Owner password',{exact:true}).fill('owner');await page.getByRole('dialog').getByRole('button',{name:'Unlock editing',exact:true}).click();await expect(page.getByRole('dialog')).toHaveCount(0);
 // Cancellation while a real worker is loading/recognizing must leave no dirty OCR.
 await button('Recognize text').click();const count=workers.length;await button('Start recognition').click();await expect.poll(()=>workers.length,{timeout:15000}).toBeGreaterThan(count);await button('Cancel').click();await expect(page.getByRole('dialog')).toHaveCount(0);await button('Undo').click();await expect(button('Recognize text')).toBeDisabled();await button('Redo').click();await expect(button('Recognize text')).toBeEnabled();checks.push('cancel running recognition leaves only the prior owner-unlock history step');
 await recognize();await page.getByLabel('Recognized words').getByRole('button',{name:/^Invoice Score/}).click();await page.getByLabel('Recognized text',{exact:true}).fill('ProtectedInvoice');await button('Save correction').click();
 await button('Apply searchable text').click();await expect(page.getByRole('dialog')).toHaveCount(0,{timeout:30000});
 await button('Find').click();await page.getByRole('searchbox').fill('ProtectedInvoice');await expect(page.locator('.search-count')).toContainText('1 of 1');await button('Close search').click();
 const native=resolve(dir,'Folio edited.pdf'),saved=resolve(dir,'Protected searchable.pdf');if(existsSync(native)||existsSync(saved))throw Error('Expected fresh protected output');
 await button('Save a copy').click();await dialog('Save',native);await expect(page.locator('.statusbar')).toContainText('Saved a copy',{timeout:30000});renameSync(native,saved);
 await open(saved,'reader');await expect(button('Recognize text')).toBeDisabled();await button('Find').click();await page.getByRole('searchbox').fill('ProtectedInvoice');await expect(page.locator('.search-count')).toContainText('1 of 1');await button('Close search').click();checks.push('owner OCR then encrypted save/reopen retains reader permissions and search');
 const hashes=JSON.parse(readFileSync(resolve(dir,'original-hashes.json'),'utf8').replace(/^\uFEFF/,''));const original=Object.keys(hashes).find(p=>p.includes('attachments'));
 const quiz=resolve(dir,'Quiz.pdf');if(!existsSync(quiz))copyFileSync(original,quiz);await open(quiz);
 await recognize();const summary=await page.locator('.ocr-word-summary').innerText();console.log('QUIZ',summary);writeFileSync(resolve(dir,'quiz-review.txt'),await page.getByRole('dialog').innerText());await page.screenshot({path:resolve(dir,'04-quiz-review.png')});
 await expect(button('Apply searchable text')).toBeDisabled();await button('Cancel').click();checks.push('already-native quiz has no new OCR and stays unchanged');
 expect(errors).toEqual([]);expect(external).toEqual([]);writeFileSync(resolve(dir,'protected-results.json'),JSON.stringify({passed:true,checks,errors,external,workers,exe:record},null,2));console.log(JSON.stringify({passed:true,checks}));
}catch(e){writeFileSync(resolve(dir,'protected-failure.json'),JSON.stringify({checks,errors,external,error:String(e.stack)},null,2));await page.screenshot({path:resolve(dir,'protected-failure.png')}).catch(()=>{});throw e;}finally{await browser.close();}
