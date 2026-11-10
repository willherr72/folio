// Repository acceptance against an owned Windows WebView instance.
import {chromium,expect} from '@playwright/test';
import {readFileSync,writeFileSync} from 'node:fs';
import {resolve} from 'node:path';
import {execFile} from 'node:child_process';
import {promisify} from 'node:util';
import {createHash} from 'node:crypto';
const root=resolve(process.env.FOLIO_COMMENT_PAGE_ROOT??'artifacts/comment-page-desktop'),exec=promisify(execFile);
const record=JSON.parse(readFileSync(resolve(root,'profile/process.json'),'utf8').replace(/^\uFEFF/,''));
const browser=await chromium.connectOverCDP(`http://127.0.0.1:${record.port}`),page=browser.contexts()[0].pages().find(p=>!p.url().startsWith('devtools:'));
page.setDefaultTimeout(15000);await page.setViewportSize({width:1280,height:820});const checks=[],errors=[];page.on('pageerror',e=>errors.push(e.message));
const button=name=>page.getByRole('button',{name,exact:true}),note=()=>button('PDF comment: Review this paragraph');
async function picker(action,name){await exec('pwsh',['-NoProfile','-File',resolve('scripts/set-native-dialog.ps1'),'-AppProcessId',String(record.pid),'-Action',action,'-FilePath',resolve(root,name),...(action==='Open'?['-NativeFileName']:[])],{windowsHide:true,timeout:60000});}
async function bounds(){return note().locator('rect').evaluate(r=>Object.fromEntries(['x','y','width','height'].map(k=>[k,Number(r.getAttribute(k))])));}
async function screenPoint(x,y){return note().locator('rect').evaluate((r,p)=>{const a=r.ownerSVGElement.createSVGPoint();a.x=p.x;a.y=p.y;const b=a.matrixTransform(r.getScreenCTM());return{x:b.x,y:b.y};},{x,y});}
async function expectBounds(value){await expect.poll(async()=>{const actual=await bounds();return Math.abs(actual.x-value.x)<.02&&Math.abs(actual.y-value.y)<.02;}).toBe(true);}
async function drag(from,to,cancel=false){await note().scrollIntoViewIfNeeded();const a=await screenPoint(from.x+from.width/2,from.y+from.height/2),b=await screenPoint(to.x+from.width/2,to.y+from.height/2);await page.mouse.move(a.x,a.y);await page.mouse.down();await page.mouse.move(b.x,b.y,{steps:8});await expect(page.locator('.native-comment-draft')).toBeVisible();if(cancel)await page.keyboard.press('Escape');else await page.screenshot({path:resolve(root,'drag-preview.png')});await page.mouse.up();}
const sourceHash=createHash('sha256').update(readFileSync(resolve(root,'Page controls.pdf'))).digest('hex');
try{
 await button('Open a PDF').click();await picker('Open','Page controls.pdf');
 await button('Page 2: Review this paragraph').click();await button('Close').click();await note().scrollIntoViewIfNeeded();await expect(note()).toBeVisible();
 await expect(button('PDF comment: Hidden note')).toHaveCount(0);await expect(button('PDF comment: No-view note')).toHaveCount(0);await expect(button('Page 2: Hidden note')).toBeVisible();
 const original=await bounds();await drag(original,{x:original.x+30,y:original.y+25},true);await expectBounds(original);await expect(button('Undo')).toBeDisabled();checks.push('cancelled pointer draft has no history; hidden notes have no page targets');
 await button('Rotate').click();await note().scrollIntoViewIfNeeded();const rotatedOriginal=await bounds();expect(rotatedOriginal).toEqual(original);
 const moved={...original,x:original.x+30,y:original.y+40};await drag(original,moved);await expectBounds(moved);await expect(note()).toBeFocused();
 await button('Undo').click();await expectBounds(original);await button('Redo').click();await expectBounds(moved);checks.push('cropped/intrinsically rotated page plus workspace rotation; pointer move and undo/redo');
 await note().focus();await page.keyboard.press('ArrowRight');await page.keyboard.press('Shift+ArrowRight');await expectBounds(moved);await page.keyboard.press('Enter');const keyboard={...moved,y:moved.y-11};await expectBounds(keyboard);await expect(note()).toBeFocused();
 await page.keyboard.press('ArrowDown');await page.keyboard.press('Escape');await expectBounds(keyboard);await expect(page.locator('.native-comment-draft')).toHaveCount(0);checks.push('keyboard draft applies once and restores focus; Escape cancels');
 await note().dblclick();await expect(page.getByRole('dialog',{name:'PDF comment'})).toBeVisible();await expect(page.getByRole('textbox',{name:'Comment text'})).toHaveValue('Review this paragraph');const finalDisplay={x:Number(await page.getByRole('spinbutton',{name:'Left (pt)'}).inputValue()),y:Number(await page.getByRole('spinbutton',{name:'Top (pt)'}).inputValue())};await button('Close').click();
 await button('Save a copy').click();await picker('Save','Folio edited.pdf');await expect(page.getByLabel('Unsaved changes')).toHaveCount(0);await button('Open').click();await picker('Open','Folio edited.pdf');await expect(page.getByRole('tab',{name:'Folio edited.pdf',exact:true})).toHaveAttribute('aria-selected','true');await button('Page 2: Review this paragraph').click();await expect(page.getByRole('spinbutton',{name:'Left (pt)'})).toHaveValue(String(finalDisplay.x));await expect(page.getByRole('spinbutton',{name:'Top (pt)'})).toHaveValue(String(finalDisplay.y));await button('Close').click();await expect(note()).toBeVisible();await expect(button('PDF comment: Hidden note')).toHaveCount(0);checks.push('double-click edit and native save/reopen preserve displayed position');
 await page.screenshot({path:resolve(root,'reopened.png')});expect(errors).toEqual([]);expect(createHash('sha256').update(readFileSync(resolve(root,'Page controls.pdf'))).digest('hex')).toBe(sourceHash);
 writeFileSync(resolve(root,'results.json'),JSON.stringify({passed:true,checks,errors,sourceHash,original,moved,keyboard,finalDisplay,executableSha256:record.sha256},null,2));console.log('PASS '+checks.join('; '));
}catch(error){writeFileSync(resolve(root,'failure.json'),JSON.stringify({checks,errors,error:String(error.stack)},null,2));await page.screenshot({path:resolve(root,'failure.png')}).catch(()=>{});throw error;}finally{await browser.close();}
