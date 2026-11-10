// Repository acceptance against an owned Windows WebView instance.
import {chromium,expect} from '@playwright/test';
import {readFileSync,writeFileSync} from 'node:fs';
import {resolve} from 'node:path';
import {execFile} from 'node:child_process';
import {promisify} from 'node:util';
import {createHash} from 'node:crypto';
const root=resolve(process.env.FOLIO_COMMENT_PAGE_ROOT??'artifacts/native-rotation-desktop'),exec=promisify(execFile);
const record=JSON.parse(readFileSync(resolve(root,'profile/process.json'),'utf8').replace(/^\uFEFF/,''));
const browser=await chromium.connectOverCDP(`http://127.0.0.1:${record.port}`),page=browser.contexts()[0].pages().find(p=>!p.url().startsWith('devtools:'));
page.setDefaultTimeout(15000);await page.setViewportSize({width:1280,height:820});const checks=[],errors=[];page.on('pageerror',e=>errors.push(e.message));
const button=name=>page.getByRole('button',{name,exact:true}),note=()=>button('PDF comment: Review this paragraph');
async function picker(action,name){await exec('pwsh',['-NoProfile','-File',resolve('scripts/set-native-dialog.ps1'),'-AppProcessId',String(record.pid),'-Action',action,'-FilePath',resolve(root,name),...(action==='Open'?['-NativeFileName']:[])],{windowsHide:true,timeout:60000});}
async function bounds(){return note().locator('rect').evaluate(r=>Object.fromEntries(['x','y','width','height'].map(k=>[k,Number(r.getAttribute(k))])));}
async function screenPoint(x,y){return note().locator('rect').evaluate((r,p)=>{const a=r.ownerSVGElement.createSVGPoint();a.x=p.x;a.y=p.y;const b=a.matrixTransform(r.getScreenCTM());return{x:b.x,y:b.y};},{x,y});}
async function expectBounds(value){await expect.poll(async()=>{const actual=await bounds();return Math.abs(actual.x-value.x)<.02&&Math.abs(actual.y-value.y)<.02;}).toBe(true);}
async function drag(from,to,cancel=false){await expect(page.locator('.page-stage').nth(1)).toHaveAttribute('aria-busy','false');await expect(note()).toHaveAttribute('aria-disabled','false');await note().scrollIntoViewIfNeeded();const a=await screenPoint(from.x+from.width/2,from.y+from.height/2),b=await screenPoint(to.x+from.width/2,to.y+from.height/2);await page.mouse.move(a.x,a.y);await page.mouse.down();await page.mouse.move(b.x,b.y,{steps:8});await expect(page.locator('.native-comment-draft')).toBeVisible();if(cancel)await page.keyboard.press('Escape');else await page.screenshot({path:resolve(root,'drag-preview.png')});await page.mouse.up();}
async function capture(name){
 await page.setViewportSize({width:1280,height:1100});
 for(let i=0;i<12;i++){const zoom=parseFloat(await page.locator('.zoom-value').textContent());if(zoom===100)break;await button(zoom<100?'Zoom in':'Zoom out').click();}
 await expect(page.locator('.zoom-value')).toHaveText('100%');await note().scrollIntoViewIfNeeded();await expect(page.locator('.page-stage').nth(1)).toHaveAttribute('aria-busy','false');
 const canvas=page.locator('.document-page').nth(1).locator('.page-canvas');
 await canvas.evaluate(svg=>{const host=svg.closest('.document-viewport');host.scrollTop+=svg.getBoundingClientRect().top-host.getBoundingClientRect().top-24;});
 await expect.poll(()=>canvas.evaluate(svg=>{const a=svg.getBoundingClientRect(),b=svg.closest('.document-viewport').getBoundingClientRect();return a.top>=b.top+20&&a.bottom<=b.bottom-20;})).toBe(true);
 const hit=await note().locator('rect').evaluate(r=>{const a=r.getBoundingClientRect(),svg=r.ownerSVGElement,b=svg.getBoundingClientRect(),v=svg.viewBox.baseVal;return{x:(a.left-b.left)*v.width/b.width,y:(a.top-b.top)*v.height/b.height,width:a.width*v.width/b.width,height:a.height*v.height/b.height};});
 await button('Select').click();await page.mouse.move(30,60);
 const encoded=await canvas.locator('image').evaluate(async image=>{const response=await fetch(image.getAttribute('href'));const bytes=new Uint8Array(await response.arrayBuffer());let data='';for(let i=0;i<bytes.length;i+=16384)data+=String.fromCharCode(...bytes.subarray(i,i+16384));return btoa(data);});
 writeFileSync(resolve(root,name+'-raster.png'),Buffer.from(encoded,'base64'));await canvas.screenshot({path:resolve(root,name+'-canvas.png')});return hit;
}
const sourceHash=createHash('sha256').update(readFileSync(resolve(root,'Page controls.pdf'))).digest('hex');
try{
 await button('Open a PDF').click();await picker('Open','Page controls.pdf');
 await button('Page 2: Review this paragraph').click();await button('Close').click();await note().scrollIntoViewIfNeeded();await expect(note()).toBeVisible();
 await expect(button('PDF comment: Hidden note')).toHaveCount(0);await expect(button('PDF comment: No-view note')).toHaveCount(0);await expect(button('Page 2: Hidden note')).toBeVisible();
 const original=await bounds();await drag(original,{x:original.x+30,y:original.y+25},true);await expectBounds(original);await expect(button('Undo')).toBeDisabled();checks.push('cancelled pointer draft has no history; hidden notes have no page targets');
 await button('Rotate').click();await expect(page.locator('.page-stage').nth(1)).toHaveAttribute('aria-busy','false');await note().scrollIntoViewIfNeeded();const rotatedOriginal=await bounds();
 const moved={...rotatedOriginal,x:rotatedOriginal.x+30,y:rotatedOriginal.y+40};await drag(rotatedOriginal,moved);await expectBounds(moved);await expect(note()).toBeFocused();
 await button('Undo').click();await expectBounds(rotatedOriginal);await button('Redo').click();await expectBounds(moved);checks.push('cropped/intrinsically rotated page plus workspace rotation; pointer move and undo/redo');
 await note().focus();await page.keyboard.press('ArrowRight');await page.keyboard.press('Shift+ArrowRight');await expectBounds(moved);await page.keyboard.press('Enter');const keyboard={...moved,y:moved.y-11};await expectBounds(keyboard);await expect(note()).toBeFocused();
 await page.keyboard.press('ArrowDown');await page.keyboard.press('Escape');await expectBounds(keyboard);await expect(page.locator('.native-comment-draft')).toHaveCount(0);checks.push('keyboard draft applies once and restores focus; Escape cancels');
 await note().dblclick();await expect(page.getByRole('dialog',{name:'PDF comment'})).toBeVisible();await expect(page.getByRole('textbox',{name:'Comment text'})).toHaveValue('Review this paragraph');const finalDisplay={x:Number(await page.getByRole('spinbutton',{name:'Left (pt)'}).inputValue()),y:Number(await page.getByRole('spinbutton',{name:'Top (pt)'}).inputValue())};await button('Close').click();
 const previewHit=await capture('before-save');await button('Save a copy').click();await picker('Save','Folio edited.pdf');await expect(page.getByLabel('Unsaved changes')).toHaveCount(0);await button('Open').click();await picker('Open','Folio edited.pdf');await expect(page.getByRole('tab',{name:'Folio edited.pdf',exact:true})).toHaveAttribute('aria-selected','true');await button('Page 2: Review this paragraph').click();await expect(page.getByRole('spinbutton',{name:'Left (pt)'})).toHaveValue(String(finalDisplay.x));await expect(page.getByRole('spinbutton',{name:'Top (pt)'})).toHaveValue(String(finalDisplay.y));await button('Close').click();await expect(note()).toBeVisible();await expect(button('PDF comment: Hidden note')).toHaveCount(0);checks.push('double-click edit and native save/reopen preserve displayed position');
 const reopenedHit=await capture('after-reopen');for(const key of ['x','y','width','height'])expect(Math.abs(previewHit[key]-reopenedHit[key])).toBeLessThan(.02);checks.push('final displayed NoRotate target agrees before and after saving');
 await page.screenshot({path:resolve(root,'reopened.png')});expect(errors).toEqual([]);expect(createHash('sha256').update(readFileSync(resolve(root,'Page controls.pdf'))).digest('hex')).toBe(sourceHash);
 writeFileSync(resolve(root,'results.json'),JSON.stringify({passed:true,checks,errors,sourceHash,original,rotatedOriginal,moved,keyboard,finalDisplay,previewHit,reopenedHit,executableSha256:record.sha256},null,2));console.log('PASS '+checks.join('; '));
}catch(error){writeFileSync(resolve(root,'failure.json'),JSON.stringify({checks,errors,error:String(error.stack)},null,2));await page.screenshot({path:resolve(root,'failure.png')}).catch(()=>{});throw error;}finally{await browser.close();}
