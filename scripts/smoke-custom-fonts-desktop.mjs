// Run against an isolated, owned production app; submit printed native dialogs separately.
import {chromium,expect} from '@playwright/test';
import {existsSync,readFileSync,writeFileSync,renameSync} from 'node:fs';
import {resolve} from 'node:path';import {createHash} from 'node:crypto';
const dir=resolve('artifacts/custom-font-desktop'), original=resolve(dir,'Original.pdf'), imported=resolve(dir,'Imported.ttf'), saved=resolve(dir,'Editable.pdf'), flat=resolve(dir,'Flattened.pdf');
const hash=path=>createHash('sha256').update(readFileSync(path)).digest('hex');
const browser=await chromium.connectOverCDP(process.env.FOLIO_CDP_URL||'http://127.0.0.1:9246');
const page=browser.contexts()[0].pages().find(p=>!p.url().startsWith('devtools:')), errors=[];page.on('pageerror',e=>errors.push(e.message));
const canvas=()=>page.locator('.document-page').first().locator('.page-canvas');
async function open(path,name){await page.getByRole('button',{name:/^(Open|Open a PDF)$/}).click();console.log('DIALOG Open a PDF => '+path);await expect(page.getByRole('tab',{name,exact:true})).toHaveAttribute('aria-selected','true',{timeout:180000});await expect(canvas().locator('image')).toHaveCount(1,{timeout:30000});}
try{
 const originalHash=hash(original), fontHash=hash(imported), text='Custom café Ω Ж';
 await open(original,'Original.pdf');
 await page.getByRole('button',{name:'Text',exact:true}).click();await canvas().click({position:{x:110,y:240}});
 await page.getByRole('textbox',{name:'Content',exact:true}).fill(text);
 await page.getByRole('button',{name:'More fonts…',exact:true}).click();
 await page.getByRole('searchbox',{name:'Search fonts'}).fill('Arial');
 await page.getByRole('dialog').getByRole('button',{name:'Arial Italic',exact:true}).click({timeout:60000});
 await page.getByRole('button',{name:'Apply font',exact:true}).click();
 await expect(page.getByRole('dialog')).toHaveCount(0,{timeout:30000});
 await expect(canvas().locator('[data-overlay] text')).toHaveAttribute('font-family',/^FolioFont_/);
 await expect(page.getByRole('combobox',{name:'Font'})).toHaveValue('custom');
 await page.getByRole('button',{name:'Undo',exact:true}).click();await expect(page.getByRole('combobox',{name:'Font'})).toHaveValue('Helvetica');
 await page.getByRole('button',{name:'Redo',exact:true}).click();await expect(page.getByRole('combobox',{name:'Font'})).toHaveValue('custom');
 await page.getByRole('button',{name:'More fonts…',exact:true}).click();await page.getByRole('button',{name:'Import font…',exact:true}).click();console.log('DIALOG Import a font => '+imported);
 await expect(page.getByRole('button',{name:'Apply font',exact:true})).toBeEnabled({timeout:180000});await page.getByRole('button',{name:'Apply font',exact:true}).click();
 await expect(page.getByRole('dialog')).toHaveCount(0);
 await expect(canvas().locator('[data-overlay] text')).toHaveAttribute('font-family','FolioFont_'+fontHash);
 await expect(page.getByRole('combobox',{name:'Font'}).locator('option:checked')).toHaveText('DejaVu Serif');
 await page.getByRole('textbox',{name:'Content'}).fill('Missing 🙂');await expect(page.getByRole('alert')).toContainText('U+1F642');
 await page.getByRole('textbox',{name:'Content'}).fill(text);await expect(page.getByRole('alert')).toHaveCount(0);
 await page.getByRole('button',{name:'Save a copy',exact:true}).click();console.log('DIALOG Save a PDF copy => '+saved);
 await expect.poll(()=>existsSync(saved),{timeout:180000}).toBe(true);await expect(page.locator('.statusbar')).toContainText('Saved a copy');
 await page.getByRole('button',{name:'Save options',exact:true}).click();await page.getByRole('menuitem',{name:'Flatten text and ink',exact:true}).click();console.log('DIALOG Save a PDF copy => '+flat);
 await expect.poll(()=>existsSync(flat),{timeout:180000}).toBe(true);await expect(page.getByRole('button',{name:'Save a copy',exact:true})).toBeEnabled();
 expect(hash(original)).toBe(originalHash);renameSync(original,resolve(dir,'Moved original.pdf'));renameSync(imported,resolve(dir,'Moved font.ttf'));
 await page.getByRole('button',{name:'Close Original.pdf',exact:true}).click();await open(saved,'Editable.pdf');
 await expect(canvas().locator('[data-overlay] text')).toHaveAttribute('font-family','FolioFont_'+fontHash);
 await canvas().locator('[data-overlay]').click();await expect(page.getByRole('textbox',{name:'Content'})).toHaveValue(text);await expect(page.getByRole('combobox',{name:'Font'}).locator('option:checked')).toHaveText('DejaVu Serif');
 await page.getByRole('button',{name:'Find',exact:true}).click();await page.getByRole('searchbox').fill('café');await expect(page.locator('.search-count')).toContainText('1 of 1');await page.getByRole('button',{name:'Close search',exact:true}).click();
 await page.screenshot({path:resolve(dir,'reopened.png')});
 await open(flat,'Flattened.pdf');await expect(canvas().locator('[data-overlay]')).toHaveCount(0);
 await expect.poll(()=>canvas().locator('.pdf-text-layer').textContent()).toContain(text);
 expect(errors).toEqual([]);writeFileSync(resolve(dir,'results.json'),JSON.stringify({passed:true,originalSha256:originalHash,fontSha256:fontHash,editableSha256:hash(saved),flattenedSha256:hash(flat),checks:['installed Arial Italic picker','font undo/redo','local import','exact font family','missing glyph message','editable and flat export','original unchanged','reopen without original PDF/font','custom font controls restored','search','flattened selectable text'],errors},null,2));console.log('PASS custom-font desktop');
}finally{await browser.close();}
