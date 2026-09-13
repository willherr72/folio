// Bounded acceptance against an isolated production WebView and owned native dialogs.
import {chromium,expect} from '@playwright/test';
import {execFile} from 'node:child_process';
import {promisify} from 'node:util';
import {copyFileSync,existsSync,readFileSync,writeFileSync,renameSync} from 'node:fs';
import {resolve,basename} from 'node:path';
import {createHash} from 'node:crypto';
const run=resolve(process.env.FOLIO_RELEASE_RUN||'artifacts/release-v0.10-desktop');
const processInfo=JSON.parse(readFileSync(resolve(run,'process.json'),'utf8').replace(/^\uFEFF/,''));
const exec=promisify(execFile), checks=[], errors=[];
const hash=p=>createHash('sha256').update(readFileSync(p)).digest('hex');
const original=resolve(run,'Original.pdf'),font=resolve(run,'Imported.ttf'),editable=resolve(run,'Editable.pdf'),flat=resolve(run,'Flattened.pdf');
copyFileSync(resolve('examples/Welcome to Folio.pdf'),original);
copyFileSync(resolve('tests/fixtures/corpus/fonts/DejaVuSerif.ttf'),font);
const sourceHash=hash(original);
const browser=await chromium.connectOverCDP(`http://127.0.0.1:${processInfo.port}`);
const context=browser.contexts()[0],page=context.pages().find(p=>!p.url().startsWith('devtools:'));
page.setDefaultTimeout(20000);page.on('pageerror',e=>errors.push(e.message));
const canvas=()=>page.locator('.document-page').first().locator('.page-canvas');
const content=()=>page.getByRole('textbox',{name:'Content',exact:true});
async function dialog(title,path,action='Open'){
 const result=await exec('pwsh',['-NoProfile','-File',resolve('scripts/set-release-dialog.ps1'),'-AppProcessId',String(processInfo.pid),'-FilePath',path,'-Action',action,'-Title',title],{windowsHide:true,timeout:25000});
 console.log(result.stdout.trim());
}
async function open(path){
 console.log('Open',basename(path));
 await page.getByRole('button',{name:/^(Open|Open a PDF)$/}).click();await dialog('Open a PDF',path);
 await expect(page.getByRole('tab',{name:basename(path),exact:true})).toHaveAttribute('aria-selected','true');
 await expect(canvas().locator('image')).toHaveCount(1);
}
async function ready(){await expect(canvas().locator('[data-overlay] use').first()).toBeAttached();await expect(canvas().locator('[data-overlay] text')).toHaveCount(0);}
try{
 await open(original);checks.push('native PDF open');
 await page.getByRole('button',{name:'Text',exact:true}).click();await canvas().click({position:{x:110,y:240}});
 await expect(content()).toBeFocused();await content().fill('office café');
 await page.getByRole('checkbox',{name:'Shaped text',exact:true}).check();
 await page.getByRole('button',{name:'More fonts…',exact:true}).click();
 await page.getByRole('button',{name:'Import font…',exact:true}).click();await dialog('Import a font',font);
 await expect(page.getByRole('button',{name:'Apply font',exact:true})).toBeEnabled();await page.getByRole('button',{name:'Apply font',exact:true}).click();await ready();
 checks.push('native font import and shaped outline preview');
 const ligated=await canvas().locator('[data-overlay] use').count();
 await page.getByRole('checkbox',{name:'Ligatures',exact:true}).uncheck();
 await expect.poll(()=>canvas().locator('[data-overlay] use').count()).toBeGreaterThan(ligated);
 await page.getByRole('checkbox',{name:'Ligatures',exact:true}).check();await expect.poll(()=>canvas().locator('[data-overlay] use').count()).toBe(ligated);
 checks.push('ligature controls change native glyphs');
 await content().fill('office café Ω');await ready();
 await page.getByRole('button',{name:'Undo',exact:true}).click();await expect(content()).toHaveValue('office café');
 await page.getByRole('button',{name:'Redo',exact:true}).click();await expect(content()).toHaveValue('office café Ω');checks.push('shaped text undo and redo');
 // Exercise the actual WebView composition path (not a physical Windows IME).
 await content().focus();await content().press('End');
 const cdp=await context.newCDPSession(page);
 await cdp.send('Input.imeSetComposition',{text:'Z',selectionStart:1,selectionEnd:1});
 await cdp.send('Input.insertText',{text:'Z'});
 await expect(content()).toHaveValue('office café ΩZ');await ready();
 await page.getByRole('button',{name:'Undo',exact:true}).click();await expect(content()).toHaveValue('office café Ω');checks.push('WebView composition commits as one undo step');
 await page.getByRole('button',{name:'Save a copy',exact:true}).click();await dialog('Save a PDF copy',editable,'Save');
 await expect.poll(()=>existsSync(editable)).toBe(true);await expect(page.locator('.statusbar')).toContainText('Saved a copy');
 await page.getByRole('button',{name:'Save options',exact:true}).click();await page.getByRole('menuitem',{name:'Flatten text and ink',exact:true}).click();await dialog('Save a PDF copy',flat,'Save');
 await expect.poll(()=>existsSync(flat)).toBe(true);await expect(page.getByRole('button',{name:'Save a copy',exact:true})).toBeEnabled();
 expect(hash(original)).toBe(sourceHash);renameSync(original,resolve(run,'Moved original.pdf'));renameSync(font,resolve(run,'Moved font.ttf'));checks.push('editable/flattened export preserves source');
 await page.getByRole('button',{name:'Close Original.pdf',exact:true}).click();await open(editable);await ready();
 await canvas().locator('[data-overlay]').click();await expect(content()).toHaveValue('office café Ω');
 await expect(page.getByRole('checkbox',{name:'Shaped text',exact:true})).toBeChecked();checks.push('editable reopen without source or imported font');
 await page.getByRole('button',{name:'Find',exact:true}).click();await page.getByRole('searchbox',{name:'Find in document'}).fill('office café Ω');await expect(page.locator('.search-count')).toContainText('1 of 1');await page.getByRole('button',{name:'Close search',exact:true}).click();checks.push('shaped annotation search');
 await page.screenshot({path:resolve(run,'editable.png')});
 await open(flat);await expect(canvas().locator('[data-overlay]')).toHaveCount(0);
 await expect.poll(()=>canvas().locator('.pdf-text-layer').textContent()).toContain('office café Ω');checks.push('flattened native reader text');
 await page.evaluate(()=>window.addEventListener('copy',event=>{window.__folioReleaseCopied=event.clipboardData.getData('text/plain');event.preventDefault();}));
 const glyphs=canvas().locator('[data-pdf-character]'),characters=await glyphs.allTextContents(),phrase='office café Ω';
 const offset=characters.join('').indexOf(phrase);let cursor=0,start=-1,end=-1;
 for(let i=0;i<characters.length;i++){if(cursor===offset)start=i;cursor+=characters[i].length;if(cursor===offset+phrase.length){end=i;break;}}
 expect(start).toBeGreaterThanOrEqual(0);expect(end).toBeGreaterThanOrEqual(start);
 const first=await glyphs.nth(start).boundingBox(),last=await glyphs.nth(end).boundingBox();
 await page.mouse.move(first.x+first.width*.25,first.y+first.height/2);await page.mouse.down();await page.mouse.move(last.x+last.width*.75,last.y+last.height/2,{steps:15});await page.mouse.up();await page.keyboard.press('Control+c');
 expect(await page.evaluate(()=>window.__folioReleaseCopied)).toBe(phrase);checks.push('flattened drag selection copies exact Unicode');
 await page.getByRole('button',{name:'Print',exact:true}).click();await expect(page.getByRole('dialog',{name:'Print document'})).toBeVisible();await page.getByRole('dialog',{name:'Print document'}).getByRole('button',{name:'Cancel',exact:true}).click();checks.push('print setup and cancel');
 await page.getByRole('tab',{name:'Editable.pdf',exact:true}).click();await ready();await canvas().locator('[data-overlay]').click();await content().fill('Recovery office café Ω');await ready();checks.push('tab return retains native preparation');
 await page.screenshot({path:resolve(run,'recovery-draft.png')});
 expect(errors).toEqual([]);
 writeFileSync(resolve(run,'results.json'),JSON.stringify({passed:true,binarySha256:processInfo.sha256,sourceSha256:sourceHash,editableSha256:hash(editable),flatSha256:hash(flat),checks,errors},null,2));console.log('PASS',checks);
}catch(e){await page.screenshot({path:resolve(run,'failure.png')}).catch(()=>{});writeFileSync(resolve(run,'failure.json'),JSON.stringify({error:String(e),checks,errors},null,2));throw e;}finally{await browser.close();}
