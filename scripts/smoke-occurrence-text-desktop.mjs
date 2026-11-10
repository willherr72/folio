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
copyFileSync(resolve('artifacts/page-occurrence-native-20261024/corpus-final/r0-u1-s0-source.pdf'),source);
const hash=p=>createHash('sha256').update(readFileSync(p)).digest('hex');
const originalHash=hash(source),errors=[];
const browser=await chromium.connectOverCDP(`http://127.0.0.1:${state.port}`);
const page=browser.contexts()[0].pages().find(p=>!p.url().startsWith('devtools:'));page.setDefaultTimeout(25000);
page.on('pageerror',e=>errors.push(String(e)));
async function dialog(title,path,action){await exec('pwsh',['-NoProfile','-File',resolve('scripts/set-release-dialog.ps1'),'-AppProcessId',String(state.pid),'-FilePath',path,'-Title',title,'-Action',action],{windowsHide:true,timeout:25000});}
async function save(name){const path=resolve(corpus,name);await page.getByRole('button',{name:'Save a copy',exact:true}).click();await dialog('Save a PDF copy',path,'Save');await expect.poll(()=>existsSync(path)).toBe(true);await expect(page.getByRole('button',{name:'Save a copy',exact:true})).toBeEnabled();return path;}
async function select(index){await page.getByRole('button',{name:`Page ${index} of 3`,exact:true}).click();await expect(page.getByRole('button',{name:'Go to page',exact:true})).toHaveText(`Page ${index} of 3`);}
async function choose(text){await select(2);await page.getByRole('button',{name:'Edit text',exact:true}).click();const target=page.locator('.page-canvas').nth(1).getByRole('button',{name:`Edit text: ${text}`,exact:true});await target.focus();await page.keyboard.press('Enter');await expect(page.getByRole('dialog',{name:'Edit existing text'})).toBeVisible();}
async function apply(text){await page.getByRole('textbox',{name:'Replacement text',exact:true}).fill(text);await page.getByRole('button',{name:'Apply changes',exact:true}).click();await expect(page.getByRole('dialog')).toHaveCount(0);}
try {
 await page.getByRole('button',{name:'Open a PDF',exact:true}).click();await dialog('Open a PDF',source,'Open');
 await expect(page.getByRole('tab',{name:'original.pdf',exact:true})).toBeVisible();
 await expect(page.locator('.page-canvas').first().locator('image')).toHaveCount(1);
 await page.getByRole('button',{name:'Rotate',exact:true}).click();
 await page.getByRole('button',{name:'Crop page…',exact:true}).click();await page.getByLabel('Left margin (pt)').fill('12');await page.getByLabel('Top margin (pt)').fill('8');
 await page.getByRole('button',{name:'Preview crop',exact:true}).click();await expect(page.getByRole('button',{name:'Apply crop',exact:true})).toBeEnabled();await page.getByRole('button',{name:'Apply crop',exact:true}).click();
 await page.getByRole('button',{name:'Duplicate',exact:true}).click();await save('baseline.pdf');
 await choose('Old content');await page.getByRole('textbox',{name:'Replacement text',exact:true}).fill('Cancelled draft');await page.getByRole('button',{name:'Cancel',exact:true}).click();
 await choose('Old content');await page.getByRole('textbox',{name:'Replacement text',exact:true}).fill('X'.repeat(900));await page.getByRole('button',{name:'Apply changes',exact:true}).click();await expect(page.getByRole('alert')).toBeVisible();
 await expect(page.getByRole('textbox',{name:'Replacement text',exact:true})).toHaveValue('X'.repeat(900));
 await page.getByRole('textbox',{name:'Replacement text',exact:true}).fill('New content');
 await page.screenshot({path:resolve(run,'text-dialog.png')});
 await page.setViewportSize({width:600,height:650});await page.getByRole('button',{name:'Apply changes',exact:true}).scrollIntoViewIfNeeded();
 const layout=await page.getByRole('dialog').evaluate(e=>({width:e.clientWidth,scrollWidth:e.scrollWidth,viewport:window.innerWidth}));
 expect(layout.scrollWidth).toBeLessThanOrEqual(layout.width+1);expect(layout.width).toBeLessThan(layout.viewport);
 await page.screenshot({path:resolve(run,'text-narrow.png')});await page.setViewportSize({width:1280,height:820});
 await apply('New content');await save('first-edit.pdf');
 await choose('New content');await apply('Next content');
 await page.getByRole('button',{name:'Undo',exact:true}).click();await save('undo-reedit.pdf');
 await page.getByRole('button',{name:'Undo',exact:true}).click();await save('undone.pdf');
 await page.getByRole('button',{name:'Redo',exact:true}).click();await page.getByRole('button',{name:'Redo',exact:true}).click();await save('edited.pdf');
 await page.getByRole('button',{name:'Open',exact:true}).click();await dialog('Open a PDF',resolve(corpus,'edited.pdf'),'Open');await expect(page.getByRole('tab',{name:'edited.pdf',exact:true,selected:true})).toBeVisible();
 await page.getByRole('button',{name:'Find',exact:true}).click();await page.getByRole('searchbox').fill('Next content');await expect(page.locator('.search-count')).toHaveText('1 of 1');await page.keyboard.press('Escape');
 await select(2);await page.getByRole('button',{name:'Fill forms',exact:true}).click();
 await expect(page.getByRole('textbox',{name:'shared',exact:true})).toHaveValue('Keep field');await page.getByRole('textbox',{name:'shared',exact:true}).fill('Desktop linked');
 await page.getByRole('button',{name:'Apply field',exact:true}).click();await expect(page.getByRole('button',{name:'Done',exact:true})).toBeEnabled();await page.getByRole('button',{name:'Done',exact:true}).click();
 for(const index of [1,3]){await select(index);await page.getByRole('button',{name:'Fill forms',exact:true}).click();await expect(page.getByRole('textbox',{name:'shared',exact:true})).toHaveValue('Desktop linked');await page.getByRole('button',{name:'Done',exact:true}).click();}
 await save('filled.pdf');await page.screenshot({path:resolve(run,'text-and-shared-field.png')});
 const checkpointAfter=Date.now();await page.getByRole('tab',{name:'original.pdf',exact:true}).click();
 const recovery=resolve(run,'data/recovery/comment-format-3');
 await expect.poll(()=>{try{return readdirSync(recovery).filter(n=>/^checkpoint-.*\.json$/.test(n)).some(n=>statSync(resolve(recovery,n)).mtimeMs>=checkpointAfter);}catch{return false;}},{timeout:15000}).toBe(true);
 expect(hash(source)).toBe(originalHash);expect(errors).toEqual([]);
 writeFileSync(resolve(run,'results.json'),JSON.stringify({passed:true,originalHash,errors,exe:state,checks:['native shared-field text selection','keyboard selection','Cancel','rejection retains draft','narrow dialog','selected rotated/cropped duplicate','re-edit','two Undo/Redo steps','save/reopen/search','shared field updates all copies','recovery checkpoint']},null,2),{flag:'wx'});
 console.log('PASS Windows shared-field text edit, re-edit, undo, save/reopen and filling');
}catch(error){await page.screenshot({path:resolve(run,'failure.png')}).catch(()=>{});throw error;}
finally{await browser.close();}
