// Owned Windows/WebView2 workflow from docs/corpus/README.md.
import {chromium,expect} from '@playwright/test';
import {execFile} from 'node:child_process';
import {promisify} from 'node:util';
import {readFileSync,writeFileSync,existsSync,renameSync,readdirSync} from 'node:fs';
import {resolve,basename} from 'node:path';
import {createHash} from 'node:crypto';
const dependent=!!process.env.FOLIO_DEPENDENT_DESKTOP_ROOT;
const root=resolve(process.env.FOLIO_DEPENDENT_DESKTOP_ROOT??process.env.FOLIO_INHERITANCE_DESKTOP_ROOT);
const record=JSON.parse(readFileSync(resolve(root,'profile/process.json'),'utf8').replace(/^\uFEFF/,''));
const browser=await chromium.connectOverCDP(`http://127.0.0.1:${record.port}`);
const page=browser.contexts()[0].pages().find(p=>!p.url().startsWith('devtools:'));
page.setDefaultTimeout(30000);
const errors=[],checks=[],sources={};page.on('pageerror',error=>errors.push(error.message));
const button=name=>page.getByRole('button',{name,exact:true});
const draft=()=>page.getByRole('textbox',{name:'Replacement text',exact:true});
const hash=path=>createHash('sha256').update(readFileSync(path)).digest('hex');
async function helper(script,args){return promisify(execFile)('pwsh',['-NoProfile','-File',resolve(script),'-AppProcessId',String(record.pid),...args],{windowsHide:true,timeout:60000});}
async function picker(action,path){await helper('artifacts/set-multifont-dialog.ps1',['-FilePath',path,'-Action',action,...(action==='Open'?['-NativeFileName']:[])]);}
async function open(path){await page.getByRole('button',{name:/^(Open|Open a PDF)$/}).click();await picker('Open',path);await expect(page.getByRole('tab',{name:basename(path),exact:true}).last()).toHaveAttribute('aria-selected','true');await expect(page.locator('.document-page image').first()).toBeVisible();}
async function save(name){const path=resolve(root,name),native=resolve(root,'Folio edited.pdf');if(existsSync(path)||existsSync(native))throw Error('Output already exists');await button('Save a copy').click();await picker('Save',native);await expect(page.locator('.statusbar')).toContainText('Saved a copy');renameSync(native,path);return path;}
async function select(){await button('Edit text').click();const target=page.locator('[data-text-path="1.0.0"]').first();await expect(target).toBeVisible();await target.scrollIntoViewIfNeeded();await target.click();await expect(draft()).toBeVisible();}
try {
 await page.setViewportSize({width:1280,height:900});
 for(const prefix of (dependent?['standard-0','simple-0','cid-0']:['standard-leading','simple-combined','cid-mirror'])) {
  const source=resolve(root,`${prefix}-source.pdf`),old=dependent||prefix.startsWith('standard')?'Cafe old':'Café old';sources[basename(source)]=hash(source);
  await open(source);await select();await expect(draft()).toHaveValue(old);await page.setViewportSize({width:640,height:720});await draft().fill('Cafe older');await button('Apply changes').scrollIntoViewIfNeeded();await expect(button('Apply changes')).toBeInViewport();await page.screenshot({path:resolve(root,`${prefix}-dialog.png`)});await page.setViewportSize({width:1280,height:900});
  await button('Apply changes').click();await expect(draft()).toHaveCount(0);await button('Undo').click();await save(`${prefix}-undo.pdf`);await button('Redo').click();const edited=await save(`${prefix}-edited.pdf`);
  await open(edited);await select();await expect(draft()).toHaveValue('Cafe older');await draft().fill(old);await button('Apply changes').click();await save(`${prefix}-restored.pdf`);
  expect(hash(source)).toBe(sources[basename(source)]);checks.push(`${prefix}: hit target, narrow dialog, edit/undo/redo/save/reopen/restore`);
 }
 const form=resolve(root,'Form.pdf');sources['Form.pdf']=hash(form);await open(form);await button('Fill forms').click();
 await button('Select form field: Full name').click();await page.getByRole('textbox',{name:'Full name',exact:true}).fill('Renderer acceptance');await button('Apply field').click();await expect(button('Apply field')).toBeDisabled();await button('Done').click();
 const filled=await save('filled.pdf');await open(filled);await button('Fill forms').click();await button('Select form field: Full name').click();await expect(page.getByRole('textbox',{name:'Full name',exact:true})).toHaveValue('Renderer acceptance');await page.screenshot({path:resolve(root,'form.png')});await button('Done').click();expect(hash(form)).toBe(sources['Form.pdf']);checks.push('Native form filling, rendered appearance, save and reopen');
 const comments=resolve(root,'Native comments.pdf');sources['Native comments.pdf']=hash(comments);await open(comments);
 await button('Page 2: Review this paragraph').click();await page.getByRole('textbox',{name:'Comment text'}).fill('Renderer acceptance café');await button('Apply comment').click();const annotated=await save('annotated.pdf');await open(annotated);await expect(button('Page 2: Renderer acceptance café')).toBeVisible();await expect(button('Page 1: Locked reference note')).toBeVisible();expect(hash(comments)).toBe(sources['Native comments.pdf']);checks.push('Native annotation edit/save/reopen retains locked note');
 await button('Save options').click();await page.getByRole('menuitem',{name:'Export images…',exact:true}).click();await page.getByRole('combobox',{name:'Pages',exact:true}).selectOption('all');await page.getByRole('combobox',{name:'Format',exact:true}).selectOption('png');await page.getByRole('combobox',{name:'Resolution',exact:true}).selectOption('150');await button('Choose folder and export…').click();await helper('artifacts/set-page-image-folder.ps1',['-Folder',resolve(root,'images')]);await expect(page.getByRole('dialog')).toHaveCount(0);await expect(page.locator('.statusbar')).toContainText('page image');
 const folders=readdirSync(resolve(root,'images'),{withFileTypes:true}).filter(x=>x.isDirectory());expect(folders).toHaveLength(1);const imageFolder=resolve(root,'images',folders[0].name);expect(readdirSync(imageFolder).sort()).toEqual(['page-0001.png','page-0002.png']);checks.push('Native two-page PNG image export');
 await button('Print').click();await expect(page.getByRole('dialog',{name:'Print document'})).toBeVisible();await button('Choose printer…').click();await helper('scripts/cancel-native-print.ps1',[]);await expect(page.getByRole('dialog')).toHaveCount(0);checks.push('Edited document reaches native Windows print dialog; cancellation, no physical print');
 await page.screenshot({path:resolve(root,'accepted.png')});expect(errors).toEqual([]);
 writeFileSync(resolve(root,'results.json'),JSON.stringify({passed:true,checks,sources,imageFolder,errors,exe:record},null,2));console.log(JSON.stringify({passed:true,checks}));
}catch(error){writeFileSync(resolve(root,'failure.json'),JSON.stringify({error:String(error.stack),checks,errors},null,2));await page.screenshot({path:resolve(root,'failure.png')}).catch(()=>{});throw error;}
finally{await browser.close();}
