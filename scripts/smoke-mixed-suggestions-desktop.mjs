// Connected authored mixed paragraph suggestions, native preview and saved replay.
import {chromium,expect} from '@playwright/test';
import {execFile} from 'node:child_process';import {promisify} from 'node:util';
import {readFileSync,writeFileSync,existsSync,renameSync} from 'node:fs';import {resolve,basename} from 'node:path';import {createHash} from 'node:crypto';
const root=resolve(process.env.FOLIO_MIXED_DESKTOP_ROOT||'artifacts/mixed-suggestions-20261024/desktop');
const record=JSON.parse(readFileSync(resolve(root,'profile/process.json'),'utf8').replace(/^\uFEFF/,''));
const browser=await chromium.connectOverCDP(`http://127.0.0.1:${record.port}`),page=browser.contexts()[0].pages().find(p=>!p.url().startsWith('devtools:'));
page.setDefaultTimeout(30000);const checks=[],errors=[],outputs=[];page.on('pageerror',e=>errors.push(e.message));
const button=name=>page.getByRole('button',{name,exact:true}),editor=page.getByRole('textbox',{name:'Paragraph text',exact:true});
const sha=path=>createHash('sha256').update(readFileSync(path)).digest('hex');
async function picker(action,path){await promisify(execFile)('pwsh',['-NoProfile','-File',resolve('scripts/set-native-dialog.ps1'),'-AppProcessId',String(record.pid),'-FilePath',path,'-Action',action,...(action==='Open'?['-NativeFileName']:[])],{windowsHide:true,timeout:60000});}
async function open(path){await page.getByRole('button',{name:/^(Open|Open a PDF)$/}).click();await picker('Open',path);await expect(page.getByRole('tab',{name:basename(path),exact:true}).last()).toHaveAttribute('aria-selected','true');await expect(page.locator('.document-page image').first()).toBeVisible();}
async function save(name){const native=resolve(root,'Folio edited.pdf'),output=resolve(root,name);if(existsSync(native)||existsSync(output))throw Error('Preserve prior output');await button('Save a copy').click();await picker('Save',native);await expect(page.locator('.statusbar')).toContainText('Saved a copy');renameSync(native,output);return output;}
async function paragraph(text){await button('Edit text').click();await button(`Edit text: ${text}`).first().click();await button('Edit paragraph…').click();await expect(editor).toBeVisible();}
async function preview(){await button('Preview paragraph').click();await expect(page.getByAltText('Paragraph PDF preview')).toBeVisible();}
try{
 await page.setViewportSize({width:1280,height:900});
 for(const rotation of [0,90,180,270]){
  const source=resolve(root,`Mixed-${rotation}.pdf`),sourceHash=sha(source);await open(source);
  for(const anchor of ['AA ','BB',' CC']){await paragraph(anchor);await expect(editor).toHaveText('AA BB CC');await expect(page.locator('.paragraph-editor-heading')).toContainText('3 paragraph pieces');await expect(editor.locator('[data-highlighted=true]')).toHaveText('BB');await button('Cancel').click();}
  await paragraph('BB');await button('Adjust selection').click();await expect(page.getByRole('checkbox',{checked:true})).toHaveCount(3);await button('Cancel adjustment').click();await expect(editor).toHaveText('AA BB CC');
  await editor.evaluate(el=>{el.focus();const marked=el.querySelector('[data-highlighted=true]').firstChild;getSelection().setBaseAndExtent(marked,0,marked,2);});await page.keyboard.type('DD');await expect(editor).toHaveText('AA DD CC');await expect(editor.locator('[data-highlighted=true]')).toHaveText('DD');
  await preview();await page.locator('.paragraph-body').evaluate(el=>el.scrollTop=0);await page.locator('.paragraph-page-scroll').evaluate(el=>el.scrollTop=0);await page.screenshot({path:resolve(root,`preview-${rotation}.png`)});await button('Apply paragraph').click();
  const output=await save(`Edited-${rotation}.pdf`);await open(output);await paragraph('DD');await expect(editor).toHaveText('AA DD CC');await expect(editor.locator('[data-highlighted=true]')).toHaveText('DD');await preview();await button('Apply paragraph').click();const repeated=await save(`Reedited-${rotation}.pdf`);
  expect(sha(source)).toBe(sourceHash);outputs.push({rotation,source,sourceHash,output,repeated});checks.push(`rotation ${rotation}: every source anchor suggests all three styles/pieces; adjustment, highlighted replacement, save/reopen and saved re-edit`);
 }
 expect(errors).toEqual([]);writeFileSync(resolve(root,'results.json'),JSON.stringify({passed:true,checks,errors,outputs,exe:record},null,2));console.log(JSON.stringify({passed:true,checks}));
}catch(error){writeFileSync(resolve(root,'failure.json'),JSON.stringify({error:String(error.stack),checks,errors,outputs},null,2));await page.screenshot({path:resolve(root,'failure.png')}).catch(()=>{});throw error;}finally{await browser.close();}
