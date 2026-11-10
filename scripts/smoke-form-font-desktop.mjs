// Owned Windows/WebView acceptance using redistributable synthetic PDFs/fonts.
import {chromium,expect} from '@playwright/test';
import {execFile} from 'node:child_process';
import {promisify} from 'node:util';
import {readFileSync,writeFileSync,existsSync,renameSync} from 'node:fs';
import {resolve,basename} from 'node:path';
import {createHash} from 'node:crypto';
const root=resolve(process.env.FOLIO_FORM_FONT_DESKTOP_ROOT);
const record=JSON.parse(readFileSync(resolve(root,'profile/process.json'),'utf8').replace(/^\uFEFF/,''));
const browser=await chromium.connectOverCDP(`http://127.0.0.1:${record.port}`);
const page=browser.contexts()[0].pages().find(p=>!p.url().startsWith('devtools:'));page.setDefaultTimeout(30000);
const errors=[],checks=[],sources={},fonts={};page.on('pageerror',error=>errors.push(error.message));
const button=name=>page.getByRole('button',{name,exact:true});
const draft=()=>page.getByRole('textbox',{name:'Replacement text',exact:true});
const hash=path=>createHash('sha256').update(readFileSync(path)).digest('hex');
async function picker(action,path,title){await promisify(execFile)('pwsh',['-NoProfile','-File',resolve('scripts/set-native-dialog.ps1'),'-AppProcessId',String(record.pid),'-FilePath',path,'-Action',action,...(title?['-DialogTitle',title]:[]),...(action==='Open'?['-NativeFileName']:[])],{windowsHide:true,timeout:60000});}
async function open(path){await page.getByRole('button',{name:/^(Open|Open a PDF)$/}).click();await picker('Open',path);await expect(page.getByRole('tab',{name:basename(path),exact:true}).last()).toHaveAttribute('aria-selected','true');await expect(page.locator('.document-page image').first()).toBeVisible();}
async function save(name){const path=resolve(root,name),native=resolve(root,'Folio edited.pdf');if(existsSync(path)||existsSync(native))throw Error('Output already exists');await button('Save a copy').click();await picker('Save',native);await expect(page.locator('.statusbar')).toContainText('Saved a copy');renameSync(native,path);return path;}
async function select(){await button('Edit text').click();await page.locator('[data-text-path="1.0.0"]').first().click();await expect(draft()).toBeVisible();}
async function importFont(font){await button('Choose substitute font…').click();await button('Import font…').click();await picker('Open',font,'Import a font');await expect(button('Apply font')).toBeEnabled();await button('Apply font').click();await expect(page.getByLabel('Replacement font preview',{exact:true})).toContainText('Cafè edit');}
try {
 await page.setViewportSize({width:1280,height:900});
 for(const kind of ['simple','cid']) {
  const source=resolve(root,`${kind}-original.pdf`),font=resolve(root,`${kind}-substitute.ttf`);sources[kind]=hash(source);fonts[kind]=hash(font);await open(source);await select();
  await expect(draft()).toHaveValue('Café old');await expect(page.locator('.existing-text-font')).toContainText('DejaVu Serif');await expect(button('Choose substitute font…')).toBeEnabled();await expect(button('Edit paragraph…')).toHaveCount(0);
  await draft().fill('Cafè edit');await button('Apply changes').click();await expect(page.getByRole('alert')).toContainText('glyph');await expect(draft()).toHaveValue('Cafè edit');await page.screenshot({path:resolve(root,`${kind}-missing-glyph.png`)});
  await button('Choose substitute font…').click();await button('Cancel').click();await expect(draft()).toHaveValue('Cafè edit');
  await importFont(font);await button('Use original font').click();await expect(draft()).toHaveValue('Cafè edit');await expect(page.getByLabel('Replacement font preview',{exact:true})).toHaveCount(0);await importFont(font);
  await draft().fill('M'.repeat(80));await button('Apply changes').click();await expect(page.getByRole('alert')).toBeVisible();await expect(draft()).toHaveValue('M'.repeat(80));await expect(button('Use original font')).toBeEnabled();
  await draft().fill('Cafè edit');await page.setViewportSize({width:640,height:720});await expect(button('Apply changes')).toBeInViewport();await button('Use original font').scrollIntoViewIfNeeded();await expect(button('Use original font')).toBeInViewport();await expect(button('Apply changes')).toBeInViewport();await page.screenshot({path:resolve(root,`${kind}-preview-narrow.png`)});await page.setViewportSize({width:1280,height:900});
  await button('Apply changes').click();await expect(draft()).toHaveCount(0);renameSync(font,`${font}.unavailable`);
  await expect.poll(()=>page.evaluate(async id=>{try{await window.__TAURI_INTERNALS__.invoke('font_info',{id});return 'loaded';}catch(error){return String(error);}},fonts[kind])).toContain('not loaded');
  await button('Undo').click();await save(`${kind}-undo.pdf`);await button('Redo').click();const edited=await save(`${kind}-substituted.pdf`);
  await open(edited);await select();await expect(draft()).toHaveValue('Cafè edit');await expect(page.locator('.existing-text-font')).toContainText('DejaVu Serif');await expect(button('Use original font')).toHaveCount(0);
  await draft().fill('Cafè again');await button('Apply changes').click();await expect(draft()).toHaveCount(0);await save(`${kind}-reedited.pdf`);await page.screenshot({path:resolve(root,`${kind}-reedited.png`)});
  expect(hash(source)).toBe(sources[kind]);expect(hash(`${font}.unavailable`)).toBe(fonts[kind]);checks.push(`${kind}: missing-glyph draft, picker cancel/reset, import/preview, rejected substitute retry, narrow dialog, undo/redo, save/reopen and re-edit without font file or registry entry`);
 }
 expect(errors).toEqual([]);writeFileSync(resolve(root,'results.json'),JSON.stringify({passed:true,checks,sources,fonts,errors,exe:record},null,2));console.log('PASS form font Windows acceptance');
}catch(error){writeFileSync(resolve(root,'failure.json'),JSON.stringify({error:String(error.stack),checks,errors},null,2));await page.screenshot({path:resolve(root,'failure.png')}).catch(()=>{});throw error;}
finally{await browser.close();}
