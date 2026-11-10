import {chromium,expect} from '@playwright/test';
import {readFileSync,writeFileSync} from 'node:fs';
import {resolve} from 'node:path';
import {execFile} from 'node:child_process';
import {promisify} from 'node:util';
import {createHash} from 'node:crypto';
const root=resolve(process.env.FOLIO_CANVAS_FORM_ROOT??'artifacts/canvas-form-filling-20261024/desktop');
const read=name=>JSON.parse(readFileSync(resolve(root,name),'utf8').replace(/^\uFEFF/,''));
const record=read('process.json'),browser=await chromium.connectOverCDP(`http://127.0.0.1:${record.port}`);
const page=browser.contexts()[0].pages().find(p=>!p.url().startsWith('devtools:'));page.setDefaultTimeout(30000);
const errors=[],checks=[],geometry=[];page.on('pageerror',e=>errors.push(e.message));
const button=name=>page.getByRole('button',{name,exact:true});
const owner=n=>page.getByLabel(`Page ${n} container`,{exact:true});
const target=(name,n=1)=>owner(n).getByRole('button',{name:`Select form field: ${name}`,exact:true});
async function picker(action,name){await promisify(execFile)('pwsh',['-NoProfile','-File',resolve('scripts/set-native-dialog.ps1'),'-AppProcessId',String(record.pid),'-Action',action,'-FilePath',resolve(root,name),...(action==='Open'?['-NativeFileName']:[])],{windowsHide:true,timeout:60000});}
async function choose(name,n=1){await target(name,n).scrollIntoViewIfNeeded();await target(name,n).click();}
async function apply(){await button('Apply field').click();await expect(page.getByRole('combobox',{name:'Field',exact:true})).toBeEnabled();await expect(button('Apply field')).toBeDisabled();}
async function capture(name){
 await button('Full-page form overview').click();const modal=page.getByRole('dialog',{name:'Fill forms',exact:true});
 const marker=modal.getByRole('button',{name:'Select form field: Read only marker',exact:true});await expect(marker).toBeVisible();
 const entry=await marker.evaluate(el=>({bounds:Object.fromEntries(['x','y','width','height'].map(k=>[k,Number(el.getAttribute(k))])),viewBox:el.ownerSVGElement.getAttribute('viewBox')}));
 const bytes=await modal.getByAltText('Form page preview').evaluate(async img=>Array.from(new Uint8Array(await(await fetch(img.src)).arrayBuffer())));
 writeFileSync(resolve(root,`${name}-native.png`),Buffer.from(bytes));geometry.push({name,...entry});await modal.getByRole('button',{name:'Done',exact:true}).click();
 await page.screenshot({path:resolve(root,`${name}.png`)});
}
try{
 await page.setViewportSize({width:1280,height:900});await button('Open a PDF').click();await picker('Open','Form.pdf');await button('Fill forms').click();
 await expect(page.getByRole('dialog',{name:'Fill forms'})).toHaveCount(0);await expect(page.getByRole('textbox',{name:'Full name',exact:true})).toBeVisible();
 await capture('intrinsic-rotation');await expect(target('Hidden field')).toHaveCount(0);
 await choose('Read only marker');await expect(page.getByRole('textbox',{name:'Read only marker'})).toBeDisabled();
 await choose('Full name');const name=page.getByRole('textbox',{name:'Full name',exact:true});await expect(name).toBeFocused();await name.fill('Retain this draft');
 await button('Zoom in').click();await button('Next page').click();await expect(button('Go to page')).toHaveText('Page 2 of 2');await expect(name).toHaveValue('Retain this draft');await expect(button('Save a copy')).toBeDisabled();
 await expect(target('Full name',2)).toHaveAttribute('aria-disabled','true');await button('Show selected field').click();await expect(button('Go to page')).toHaveText('Page 1 of 2');
 await expect(target('Full name')).toHaveAttribute('aria-disabled','false');await target('Full name').focus();await page.keyboard.press('Enter');await expect(name).toBeFocused();await expect(name).toHaveValue('Retain this draft');await button('Reset draft').click();
 checks.push('main canvas editor; retained dirty draft through zoom/page navigation/locate/same-field keyboard selection; mutation guards');
 await name.fill('Canvas selected name');await apply();await choose('I agree');await page.getByRole('checkbox',{name:'I agree',exact:true}).check();await apply();
 await choose('Country');await page.getByRole('combobox',{name:'Country',exact:true}).selectOption('1');await apply();
 await choose('Contact preference');await page.getByRole('combobox',{name:'Contact preference choice',exact:true}).selectOption('custom');await page.getByRole('textbox',{name:'Contact preference',exact:true}).fill('postal');await apply();
 await choose('Interests');await page.getByRole('listbox',{name:'Interests',exact:true}).selectOption(['1','2']);await apply();
 await choose('Comments');await page.getByRole('textbox',{name:'Comments',exact:true}).fill('First line\nSecond line');await apply();
 await choose('Delivery method — Mail');await page.getByRole('radio',{name:'Delivery method — Mail',exact:true}).check();await apply();
 checks.push('native text, checkbox, combo, editable combo, multiselect, multiline, radio Apply');
 await button('Next page').click();await choose('Full name',2);await expect(name).toHaveValue('Canvas selected name');
 await choose('Country',1);await expect(page.getByRole('combobox',{name:'Country',exact:true})).toBeFocused();await expect(page.getByRole('combobox',{name:'Country',exact:true})).toHaveValue('1');
 checks.push('cross-page non-first field selection focuses requested field without feedback');await button('Done').click();
 await button('Undo').click();await button('Page 1 of 2').click();await button('Fill forms').click();await choose('Delivery method — Mail');await expect(page.getByRole('radio',{name:'Delivery method — Mail'})).not.toBeChecked();await button('Done').click();await button('Redo').click();
 await button('Page 1 of 2').click();await expect(button('Go to page')).toHaveText('Page 1 of 2');await button('Rotate').click();await button('Fill forms').click();await capture('workspace-rotation');await choose('Country');
 await page.setViewportSize({width:800,height:600});await target('Country').scrollIntoViewIfNeeded();await expect(target('Country')).toHaveAttribute('aria-disabled','false');await target('Country').focus();await page.keyboard.press(' ');await expect(page.getByRole('combobox',{name:'Country',exact:true})).toBeFocused();
 const country=page.getByRole('combobox',{name:'Country',exact:true});
 for(const dirty of [false,true]){
  if(dirty)await country.selectOption('0');
  for(const method of ['pointer','Enter',' ']){
   await button('Properties').click();await expect(button('Properties')).toHaveAttribute('aria-expanded','false');
   await target('Country').scrollIntoViewIfNeeded();await expect(target('Country')).toHaveAttribute('aria-disabled','false');
   if(method==='pointer')await target('Country').click();else{await target('Country').focus();await page.keyboard.press(method);}
   await expect(button('Properties')).toHaveAttribute('aria-expanded','true');await expect(country).toBeFocused();await expect(country).toHaveValue(dirty?'0':'1');
  }
  if(dirty)await button('Reset draft').click();
 }
 checks.push('narrow collapsed Properties reopens on pointer/Enter/Space for clean and dirty same-field selection');
 await button('Done').scrollIntoViewIfNeeded();await expect(button('Done')).toBeInViewport();await page.screenshot({path:resolve(root,'narrow.png')});await button('Done').click();
 await expect(button('Fill forms')).toBeFocused();await page.setViewportSize({width:1280,height:900});
 await button('Save a copy').click();await picker('Save','Folio edited.pdf');await expect(page.getByLabel('Unsaved changes')).toHaveCount(0);
 await button('Open').click();await picker('Open','Folio edited.pdf');await button('Fill forms').click();await choose('Full name');await expect(name).toHaveValue('Canvas selected name');
 await choose('Comments');await expect(page.getByRole('textbox',{name:'Comments',exact:true})).toHaveValue('First line\nSecond line');await capture('reopened');await button('Done').click();
 checks.push('shared value on second page; undo/redo; intrinsic/workspace rotation; narrow keyboard/footer/focus; save/reopen');
 expect(errors).toEqual([]);const sourceHash=createHash('sha256').update(readFileSync(resolve(root,'Form.pdf'))).digest('hex');expect(sourceHash).toBe(read('input.json').sha256);
 writeFileSync(resolve(root,'results.json'),JSON.stringify({passed:true,checks,geometry,sourceHash,errors,exe:record},null,2));console.log('PASS canvas form filling Windows acceptance');
}catch(error){writeFileSync(resolve(root,'failure.json'),JSON.stringify({error:String(error.stack),checks,geometry,errors},null,2));await page.screenshot({path:resolve(root,'failure.png')}).catch(()=>{});throw error;}finally{await browser.close();}
