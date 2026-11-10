// Documented repository Windows/WebView acceptance: actual native IPC and dialogs.
import {chromium,expect} from '@playwright/test';
import {execFile} from 'node:child_process';
import {promisify} from 'node:util';
import {copyFileSync,existsSync,mkdirSync,readFileSync,writeFileSync,readdirSync,statSync} from 'node:fs';
import {resolve} from 'node:path';
import {createHash} from 'node:crypto';
const run=resolve(process.env.FOLIO_RELEASE_RUN),exec=promisify(execFile);
const state=JSON.parse(readFileSync(resolve(run,'process.json'),'utf8').replace(/^\uFEFF/,''));
const corpus=resolve(run,'workflow');mkdirSync(corpus);
const source=resolve(corpus,'original.pdf');
copyFileSync(resolve('artifacts/occurrence-paragraph-20261024/corpus-final/paragraph-physical-true-r0-u1-source.pdf'),source);
const hash=p=>createHash('sha256').update(readFileSync(p)).digest('hex');
const originalHash=hash(source),errors=[];
const browser=await chromium.connectOverCDP(`http://127.0.0.1:${state.port}`);
const page=browser.contexts()[0].pages().find(p=>!p.url().startsWith('devtools:'));page.setDefaultTimeout(25000);
page.on('pageerror',e=>errors.push(String(e)));
async function dialog(title,path,action){await exec('pwsh',['-NoProfile','-File',resolve('scripts/set-release-dialog.ps1'),'-AppProcessId',String(state.pid),'-FilePath',path,'-Title',title,'-Action',action],{windowsHide:true,timeout:25000});}
async function save(name){const path=resolve(corpus,name);await page.getByRole('button',{name:'Save a copy',exact:true}).click();await dialog('Save a PDF copy',path,'Save');await expect.poll(()=>existsSync(path)).toBe(true);await expect(page.getByRole('button',{name:'Save a copy',exact:true})).toBeEnabled();return path;}
async function select(index){await page.getByRole('button',{name:`Page ${index} of 3`,exact:true}).click();await expect(page.getByRole('button',{name:'Go to page',exact:true})).toHaveText(`Page ${index} of 3`);}
async function choose(text){await select(2);await page.getByRole('button',{name:'Edit text',exact:true}).click();const target=page.locator('.page-canvas').nth(1).getByRole('button',{name:new RegExp(`^Edit text: ${text}`)}).first();await target.focus();await page.keyboard.press('Enter');await expect(page.getByRole('dialog',{name:'Edit existing text'})).toBeVisible();await page.getByRole('button',{name:'Edit paragraph…',exact:true}).click();await expect(page.getByRole('textbox',{name:'Paragraph text',exact:true})).toBeVisible();}
const editor=()=>page.getByRole('textbox',{name:'Paragraph text',exact:true});
async function type(text){await editor().focus();await page.keyboard.press('Control+a');await page.keyboard.insertText(text);await expect(editor()).toHaveText(text);}
async function preview(){await page.getByRole('button',{name:'Preview paragraph',exact:true}).click();await expect(page.getByAltText('Paragraph PDF preview')).toBeVisible();}
async function apply(text){await type(text);await page.getByRole('button',{name:'Fit area to text',exact:true}).click();await expect(page.getByRole('button',{name:'Preview paragraph',exact:true})).toBeEnabled();await preview();await page.getByRole('button',{name:'Apply paragraph',exact:true}).click();await expect(page.getByRole('dialog')).toHaveCount(0);}

try {
 await page.getByRole('button',{name:'Open a PDF',exact:true}).click();await dialog('Open a PDF',source,'Open');
 await expect(page.getByRole('tab',{name:'original.pdf',exact:true})).toBeVisible();
 await expect(page.locator('.page-canvas').first().locator('image')).toHaveCount(1);
 await page.getByRole('button',{name:'Rotate',exact:true}).click();
 await page.getByRole('button',{name:'Crop page…',exact:true}).click();await page.getByLabel('Left margin (pt)').fill('12');await page.getByLabel('Top margin (pt)').fill('8');
 await page.getByRole('button',{name:'Preview crop',exact:true}).click();await expect(page.getByRole('button',{name:'Apply crop',exact:true})).toBeEnabled();await page.getByRole('button',{name:'Apply crop',exact:true}).click();
 await page.getByRole('button',{name:'Duplicate',exact:true}).click();await save('baseline.pdf');
 await choose('BB');
 const initial=await editor().innerText();expect(initial).toContain('AA');expect(initial).toContain('BB');expect(initial).toContain('CC');
 const mark=editor().locator('[data-highlighted=true]');await expect(mark).toHaveText('BB');
 await mark.evaluate(el=>{el.closest('[role=textbox]').focus();const range=document.createRange();range.selectNodeContents(el);const selection=getSelection();selection.removeAllRanges();selection.addRange(range);document.dispatchEvent(new Event('selectionchange'));});
 await expect(page.getByText('Formatting applies to the selected text. Check the PDF preview for exact font appearance.',{exact:true})).toBeVisible();
 await page.getByRole('spinbutton',{name:'Font size (pt)',exact:true}).fill('14');await page.getByLabel('Text color',{exact:true}).fill('#993366');
 await editor().locator('[data-highlighted=true]').evaluate(el=>{el.closest('[role=textbox]').focus();const range=document.createRange();range.selectNodeContents(el);const selection=getSelection();selection.removeAllRanges();selection.addRange(range);document.dispatchEvent(new Event('selectionchange'));});
 await expect(page.getByRole('spinbutton',{name:'Font size (pt)',exact:true})).toHaveValue('14');await expect(page.getByLabel('Text color',{exact:true})).toHaveValue('#993366');
 await page.keyboard.insertText('BB DD EE');await expect(editor()).toHaveText(initial.replace('BB','BB DD EE'));await expect(editor().locator('[data-highlighted=true]')).toHaveText('BB DD EE');
 await page.getByRole('button',{name:'Fit area to text',exact:true}).click();await expect(page.getByRole('button',{name:'Preview paragraph',exact:true})).toBeEnabled();await preview();
 await page.screenshot({path:resolve(run,'mixed-preview.png')});await page.getByRole('button',{name:'Apply paragraph',exact:true}).click();await expect(page.getByRole('dialog')).toHaveCount(0);await save('edited.pdf');
 await page.getByRole('button',{name:'Undo',exact:true}).click();await save('undone.pdf');await page.getByRole('button',{name:'Redo',exact:true}).click();await save('redone.pdf');
 await page.getByRole('button',{name:'Open',exact:true}).click();await dialog('Open a PDF',resolve(corpus,'edited.pdf'),'Open');await expect(page.getByRole('tab',{name:'edited.pdf',exact:true,selected:true})).toBeVisible();
 await choose('BB');await expect(editor()).toHaveText(initial.replace('BB','BB DD EE'));await expect(editor().locator('[data-highlighted=true]')).toHaveText('BB DD EE');await preview();await page.getByRole('button',{name:'Apply paragraph',exact:true}).click();await expect(page.getByRole('dialog')).toHaveCount(0);await save('replayed.pdf');
 await page.getByRole('button',{name:'Fill forms',exact:true}).click();await expect(page.getByRole('textbox',{name:'shared',exact:true})).toHaveValue('Keep field');await page.getByRole('textbox',{name:'shared',exact:true}).fill('Mixed linked');await page.getByRole('button',{name:'Apply field',exact:true}).click();await expect(page.getByRole('button',{name:'Done',exact:true})).toBeEnabled();await page.getByRole('button',{name:'Done',exact:true}).click();
 for(const index of [1,3]){await select(index);await page.getByRole('button',{name:'Fill forms',exact:true}).click();await expect(page.getByRole('textbox',{name:'shared',exact:true})).toHaveValue('Mixed linked');await page.getByRole('button',{name:'Done',exact:true}).click();}
 await save('filled.pdf');
 expect(hash(source)).toBe(originalHash);expect(errors).toEqual([]);
 writeFileSync(resolve(run,'results.json'),JSON.stringify({passed:true,originalHash,errors,exe:state,checks:['mixed fonts/colors','verified highlight extension','per-style size/color','fit','rotated cropped duplicate','undo/redo','save/reopen','exact no-op replay','shared fields']},null,2),{flag:'wx'});
 console.log('PASS Windows mixed highlighted paragraph editing, replay and linked fields');
}catch(error){await page.screenshot({path:resolve(run,'failure.png')}).catch(()=>{});throw error;}
finally{await browser.close();}
