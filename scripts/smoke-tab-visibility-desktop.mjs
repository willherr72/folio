import {chromium,expect} from '@playwright/test';
import {execFile} from 'node:child_process';
import {promisify} from 'node:util';
import {readFileSync,writeFileSync,existsSync} from 'node:fs';
import {resolve,basename} from 'node:path';
import {createHash} from 'node:crypto';
const root=resolve(process.env.FOLIO_TAB_DESKTOP_ROOT);
const record=JSON.parse(readFileSync(resolve(root,'profile/process.json'),'utf8').replace(/^\uFEFF/,''));
const browser=await chromium.connectOverCDP(`http://127.0.0.1:${record.port}`);
const page=browser.contexts()[0].pages().find(p=>!p.url().startsWith('devtools:'));page.setDefaultTimeout(30000);
const checks=[],errors=[],sources={};page.on('pageerror',error=>errors.push(error.message));
const button=name=>page.getByRole('button',{name,exact:true});
const hash=path=>createHash('sha256').update(readFileSync(path)).digest('hex');
const name=i=>`Document${String(i).padStart(2,'0')}.pdf`;
async function picker(action,path){await promisify(execFile)('pwsh',['-NoProfile','-File',resolve('scripts/set-native-dialog.ps1'),'-AppProcessId',String(record.pid),'-FilePath',path,'-Action',action,...(action==='Open'?['-NativeFileName']:[])],{windowsHide:true,timeout:60000});}
async function open(path){await page.getByRole('button',{name:/^(Open|Open a PDF)$/}).click();await picker('Open',path);await expect(page.getByRole('tab',{name:basename(path),exact:true}).last()).toHaveAttribute('aria-selected','true');await expect(page.locator('.document-page image').first()).toBeVisible();}
async function bounds(){return page.evaluate(()=>{const strip=document.querySelector('.document-tabs'),active=strip.querySelector('.document-tab.active'),s=strip.getBoundingClientRect(),a=active.getBoundingClientRect();return {left:s.left+strip.clientLeft,right:s.left+strip.clientLeft+strip.clientWidth,activeLeft:a.left,activeRight:a.right,scrollLeft:strip.scrollLeft,active:active.querySelector('[role=tab]').getAttribute('aria-label')};});}
async function visible(){await expect.poll(async()=>{const b=await bounds();return b.activeLeft>=b.left-1&&b.activeRight<=b.right+1;}).toBe(true);}
async function select(i){await page.evaluate(label=>Array.from(document.querySelectorAll('[role=tab]')).find(el=>el.getAttribute('aria-label')===label).click(),name(i));await visible();}
try {
 await page.setViewportSize({width:1920,height:1000});
 for(let i=1;i<=12;i++){const path=resolve(root,name(i));sources[path]=hash(path);await open(path);await visible();}
 await page.setViewportSize({width:640,height:720});await visible();await page.screenshot({path:resolve(root,'narrow-last-tab.png')});checks.push('twelve native PDFs: new active tab and narrow resize reveal full label/close wrapper');
 await select(1);await page.getByRole('tab',{name:name(1),exact:true}).focus();await page.keyboard.press('End');await visible();await expect(page.getByRole('tab',{name:name(12),exact:true})).toBeFocused();await page.keyboard.press('Home');await visible();
 await page.keyboard.press('Control+Shift+Tab');await visible();expect((await bounds()).active).toBe(name(12));await page.keyboard.press('Control+Tab');await visible();expect((await bounds()).active).toBe(name(1));checks.push('first/last selection and Ctrl+Tab wrapping');
 await page.setViewportSize({width:1280,height:900});await button('Rotate').click();await button('Zoom in').click();
 const path=resolve(root,name(13));sources[path]=hash(path);await open(path);await visible();await select(1);await expect(page.getByText('90° clockwise',{exact:true})).toBeVisible();await expect(button('100%')).toBeVisible();await button('Undo').click();await expect(page.getByText('90° clockwise',{exact:true})).toHaveCount(0);await button('Redo').click();await expect(page.getByText('90° clockwise',{exact:true})).toBeVisible();checks.push('opening and switching retain per-document zoom and undo/redo history');
 await page.evaluate(()=>{const strip=document.querySelector('.document-tabs');strip.scrollLeft=strip.scrollWidth;});const manual=(await bounds()).scrollLeft;await button('Zoom in').click();expect((await bounds()).scrollLeft).toBe(manual);checks.push('manual strip scroll survives ordinary document update');
 await select(13);await page.evaluate(label=>Array.from(document.querySelectorAll('.tab-close')).find(el=>el.getAttribute('aria-label')===`Close ${label}`).click(),name(2));await expect(page.getByRole('tab',{name:name(2),exact:true})).toHaveCount(0);await visible();checks.push('closing inactive tab retains visible active tab');
 await button('Text').click();await page.locator('.page-canvas').first().click({position:{x:100,y:200}});const content=page.getByRole('textbox',{name:'Content',exact:true});await expect(content).toBeFocused();await content.fill('Focus stays here');const scroll=await page.locator('.document-viewport').evaluate(el=>({left:el.scrollLeft,top:el.scrollTop}));
 await page.setViewportSize({width:1100,height:900});await visible();await expect(content).toBeFocused();await expect(content).toHaveValue('Focus stays here');expect(await page.locator('.document-viewport').evaluate(el=>({left:el.scrollLeft,top:el.scrollTop}))).toEqual(scroll);await page.screenshot({path:resolve(root,'editor-focus-after-resize.png')});checks.push('resize preserves focused text editor, draft and page viewport while properties remain visible');
 const output=resolve(root,'Folio edited.pdf');if(existsSync(output))throw Error('Preserving existing output');await button('Save a copy').click();await picker('Save',output);await expect(page.locator('.statusbar')).toContainText('Saved a copy');await open(output);await visible();await expect(page.getByRole('button',{name:'Text: Focus stays here',exact:true})).toBeVisible();checks.push('save/reopen retains text edit with many open documents');
 for(const [path,digest] of Object.entries(sources))expect(hash(path)).toBe(digest);expect(errors).toEqual([]);
 writeFileSync(resolve(root,'results.json'),JSON.stringify({passed:true,checks,errors,sources,outputSha256:hash(output),bounds:await bounds(),exe:record},null,2));console.log(JSON.stringify({passed:true,checks}));
}catch(error){writeFileSync(resolve(root,'failure.json'),JSON.stringify({error:String(error.stack),checks,errors,bounds:await bounds().catch(()=>null)},null,2));await page.screenshot({path:resolve(root,'failure.png')}).catch(()=>{});throw error;}
finally{await browser.close();}
