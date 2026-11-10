// Actual Windows input, native preview and save/reopen, with authored PDFs only.
import {chromium,expect} from '@playwright/test';
import {execFile} from 'node:child_process';import {promisify} from 'node:util';
import {readFileSync,writeFileSync,existsSync,copyFileSync,renameSync} from 'node:fs';import {resolve,basename} from 'node:path';import {createHash} from 'node:crypto';
const root=resolve(process.env.FOLIO_HIGHLIGHT_DESKTOP_ROOT||'artifacts/highlight-editor-20261024/desktop');
const record=JSON.parse(readFileSync(resolve(root,'profile/process.json'),'utf8').replace(/^\uFEFF/,''));
const browser=await chromium.connectOverCDP(`http://127.0.0.1:${record.port}`),page=browser.contexts()[0].pages().find(p=>!p.url().startsWith('devtools:'));
page.setDefaultTimeout(30000);const checks=[],errors=[],outputs=[];page.on('pageerror',e=>errors.push(e.message));
const button=name=>page.getByRole('button',{name,exact:true}),field=name=>page.getByRole('spinbutton',{name,exact:true}),editor=page.getByRole('textbox',{name:'Paragraph text',exact:true});
const sha=path=>createHash('sha256').update(readFileSync(path)).digest('hex');
async function picker(action,path){await promisify(execFile)('pwsh',['-NoProfile','-File',resolve('scripts/set-native-dialog.ps1'),'-AppProcessId',String(record.pid),'-FilePath',path,'-Action',action,...(action==='Open'?['-NativeFileName']:[])],{windowsHide:true,timeout:60000});}
async function open(path){await page.getByRole('button',{name:/^(Open|Open a PDF)$/}).click();await picker('Open',path);await expect(page.getByRole('tab',{name:basename(path),exact:true}).last()).toHaveAttribute('aria-selected','true');await expect(page.locator('.document-page image').first()).toBeVisible();}
async function save(name){const native=resolve(root,'Folio edited.pdf'),output=resolve(root,name);if(existsSync(native)||existsSync(output))throw Error('Preserve prior output');await button('Save a copy').click();await picker('Save',native);await expect(page.locator('.statusbar')).toContainText('Saved a copy');renameSync(native,output);return output;}
async function paragraph(text){await button('Edit text').click();await button(`Edit text: ${text}`).first().click();await button('Edit paragraph…').click();await expect(editor).toBeVisible();}
async function preview(){await button('Preview paragraph').click();await expect(page.getByAltText('Paragraph PDF preview')).toBeVisible();}
async function bounds(){return Promise.all(['Left (pt)','Top (pt)','Area width (pt)','Area height (pt)'].map(label=>field(label).inputValue()));}
try{
 await page.setViewportSize({width:1280,height:900});
 for(const [rotation,b] of [[0,[40,60,50,200]],[90,[300,40,200,50]],[180,[390,300,50,200]],[270,[60,390,200,50]]]){
  const original=resolve(`artifacts/highlight-publication-20261024/native-acceptance/source-${rotation}.pdf`),source=resolve(root,`Highlight-${rotation}.pdf`);if(existsSync(source))throw Error('Preserve fixture copy');copyFileSync(original,source);const sourceHash=sha(source);
  await open(source);await paragraph('AA BB');await expect(editor.locator('[data-highlighted=true]')).toHaveText('AA BB');await expect(editor.locator('[data-highlighted=true]')).toHaveCSS('background-color','rgb(255, 255, 0)');
  await editor.focus();await page.keyboard.press('Control+a');await page.keyboard.type('AA BB CC DD');await page.keyboard.press('Enter');await page.keyboard.press('Enter');await page.keyboard.type('EE FF');await expect(editor).toHaveText('AA BB CC DD\n\nEE FF');
  await page.keyboard.press('Control+z');await expect(editor).toHaveText('AA BB CC DD\n\n');await page.keyboard.press('Control+y');await expect(editor.locator('[data-highlighted=true]')).toHaveText('AA BB CC DD\n\nEE FF');
  for(const [i,label] of ['Left (pt)','Top (pt)','Area width (pt)','Area height (pt)'].entries())await field(label).fill(String(b[i]));
  await preview();const savedBounds=await bounds();await page.screenshot({path:resolve(root,`preview-${rotation}.png`)});await button('Apply paragraph').click();
  await button('Undo').click();await button('Edit text').click();await expect(button('Edit text: AA BB').first()).toBeVisible();await button('Redo').click();
  const output=await save(`Edited-${rotation}.pdf`);await open(output);await paragraph('AA BB ');await expect(editor).toHaveText('AA BB CC DD\n\nEE FF');await expect(editor.locator('[data-highlighted=true]')).toHaveText('AA BB CC DD\n\nEE FF');await expect.poll(bounds).toEqual(savedBounds);await preview();await button('Apply paragraph').click();const repeated=await save(`Reedited-${rotation}.pdf`);
  expect(sha(source)).toBe(sourceHash);expect(sha(original)).toBe(sourceHash);outputs.push({rotation,source,sourceHash,output,repeated,savedBounds});checks.push(`rotation ${rotation}: highlighted typing/history, native preview, apply/undo/redo, save/reopen and no-op re-edit`);
 }
 expect(errors).toEqual([]);writeFileSync(resolve(root,'results.json'),JSON.stringify({passed:true,checks,errors,outputs,exe:record},null,2));console.log(JSON.stringify({passed:true,checks}));
}catch(error){writeFileSync(resolve(root,'failure.json'),JSON.stringify({error:String(error.stack),checks,errors,outputs},null,2));await page.screenshot({path:resolve(root,'failure.png')}).catch(()=>{});throw error;}finally{await browser.close();}
