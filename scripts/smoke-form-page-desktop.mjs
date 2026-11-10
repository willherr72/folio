import {chromium,expect} from '@playwright/test';
import {readFileSync,writeFileSync} from 'node:fs';import {resolve} from 'node:path';
import {execFile} from 'node:child_process';import {promisify} from 'node:util';import {createHash} from 'node:crypto';
const root=resolve(process.env.FOLIO_FORM_PAGE_ROOT??'artifacts/form-page-acceptance'),record=JSON.parse(readFileSync(resolve(root,'profile/process.json'),'utf8').replace(/^\uFEFF/,''));
const browser=await chromium.connectOverCDP(`http://127.0.0.1:${record.port}`),page=browser.contexts()[0].pages().find(p=>!p.url().startsWith('devtools:'));page.setDefaultTimeout(30000);
const errors=[],checks=[],geometry=[];page.on('pageerror',error=>errors.push(error.message));const button=name=>page.getByRole('button',{name,exact:true}),target=name=>button(`Select form field: ${name}`);
const testZoom=process.env.FOLIO_FORM_PREVIEW_ZOOM==='1';
async function locateSelected(label){
 await button('Show selected field').click();
 const positioned=await target(label).evaluate(el=>{
  const r=el.getBoundingClientRect(),viewport=el.closest('.form-fill-preview'),v=viewport.getBoundingClientRect();
  return {intersects:r.right>v.left&&r.left<v.right&&r.bottom>v.top&&r.top<v.bottom,scrollLeft:viewport.scrollLeft,scrollTop:viewport.scrollTop};
 });expect(positioned.intersects).toBe(true);return positioned;
}
async function picker(action,name){await promisify(execFile)('pwsh',['-NoProfile','-File',resolve('scripts/set-native-dialog.ps1'),'-AppProcessId',String(record.pid),'-Action',action,'-FilePath',resolve(root,name),...(action==='Open'?['-NativeFileName']:[])],{windowsHide:true,timeout:60000});}
async function choose(name){await target(name).click();}
async function apply(){await button('Apply field').click();await expect(page.getByRole('combobox',{name:'Field',exact:true})).toBeEnabled();await expect(button('Apply field')).toBeDisabled();}
async function capture(name){
 await expect(page.getByAltText('Form page preview')).toBeVisible();await expect(target('Read only marker')).toBeVisible();
 const entry=await target('Read only marker').evaluate(el=>({bounds:Object.fromEntries(['x','y','width','height'].map(k=>[k,Number(el.getAttribute(k))])),viewBox:el.ownerSVGElement.getAttribute('viewBox')}));
 const bytes=await page.getByAltText('Form page preview').evaluate(async img=>Array.from(new Uint8Array(await(await fetch(img.src)).arrayBuffer())));writeFileSync(resolve(root,`${name}-native.png`),Buffer.from(bytes));geometry.push({name,...entry});
 await page.screenshot({path:resolve(root,`${name}.png`)});
}
try{
 await page.setViewportSize({width:1280,height:900});await button('Open a PDF').click();await picker('Open','Form.pdf');await button('Fill forms').click();
 await capture('intrinsic-rotation');await expect(target('Hidden field')).toHaveCount(0);await choose('Read only marker');await expect(page.getByRole('textbox',{name:'Read only marker'})).toBeDisabled();await expect(page.getByText('This field is read-only.',{exact:true})).toBeVisible();
 await choose('Full name');const name=page.getByRole('textbox',{name:'Full name',exact:true});await expect(name).toBeFocused();await name.fill('Discarded draft');await expect(target('Country')).toHaveAttribute('aria-disabled','true');await choose('Full name');await expect(name).toHaveValue('Discarded draft');
 if(testZoom){
  const measure=()=>target('Full name').evaluate(el=>{
   const surface=el.closest('.form-page-surface'),viewport=surface.parentElement,style=getComputedStyle(viewport),box=el.getBoundingClientRect();
   return {targetWidth:box.width,surfaceWidth:surface.getBoundingClientRect().width,availableWidth:viewport.clientWidth-parseFloat(style.paddingLeft)-parseFloat(style.paddingRight),logicalWidth:Number(el.getAttribute('width')),pageWidth:el.ownerSVGElement.viewBox.baseVal.width};
  });const original=await measure();
  await page.getByRole('combobox',{name:'Form preview zoom'}).selectOption('3');
  await expect.poll(()=>page.getByAltText('Form page preview').evaluate(img=>img.naturalWidth)).toBe(2400);
  const enlarged=await measure();writeFileSync(resolve(root,'zoom-measurements.json'),JSON.stringify({original,enlarged},null,2));
  expect(enlarged.surfaceWidth/enlarged.availableWidth).toBeCloseTo(3,1);
  expect(enlarged.targetWidth/enlarged.surfaceWidth).toBeCloseTo(enlarged.logicalWidth/enlarged.pageWidth,3);
  await locateSelected('Full name');await expect(name).toHaveValue('Discarded draft');await expect(target('Country')).toHaveAttribute('aria-disabled','true');
  await capture('zoom-three');await page.getByRole('combobox',{name:'Form preview zoom'}).selectOption('1');await locateSelected('Full name');
  checks.push('3x zoom sharpens to2400px and preserves dirty draft; locate scrolls to selected target');
 }
 await button('Reset draft').click();await expect(name).toHaveValue('Original name');
 await name.fill('Page selected name');await apply();await choose('I agree');const consent=page.getByRole('checkbox',{name:'I agree'});await expect(consent).toBeFocused();await consent.check();await apply();
 await choose('Country');await page.getByRole('combobox',{name:'Country',exact:true}).selectOption('1');await apply();
 await choose('Delivery method — Mail');await page.getByRole('radio',{name:'Delivery method — Mail'}).check();await apply();checks.push('page selection focuses text checkbox choice and radio','dirty same-field draft retained and other targets blocked','reset and native apply','hidden target omitted and readonly field explained');
 await button('Done').click();await button('Undo').click();await button('Fill forms').click();await choose('Delivery method — Mail');await expect(page.getByRole('radio',{name:'Delivery method — Mail'})).not.toBeChecked();await button('Done').click();await button('Redo').click();
 await button('Rotate').click();await button('Fill forms').click();await capture('workspace-rotation');await choose('Full name');await expect(name).toHaveValue('Page selected name');
 await page.setViewportSize({width:800,height:600});await target('Country').scrollIntoViewIfNeeded();await target('Country').focus();await page.keyboard.press('Enter');await expect(page.getByRole('combobox',{name:'Country',exact:true})).toBeFocused();
 if(testZoom){
  await page.getByRole('combobox',{name:'Form preview zoom'}).selectOption('2');await locateSelected('Country');
  await expect.poll(()=>page.getByAltText('Form page preview').evaluate(img=>img.naturalWidth)).toBe(1800);await capture('zoom-rotated-small');
  await page.setViewportSize({width:640,height:720});await locateSelected('Country');await page.screenshot({path:resolve(root,'zoom-narrow.png')});
  await expect(page.getByRole('combobox',{name:'Country',exact:true})).toHaveValue('1');
  checks.push('2x rotated preview at800x600 and640x720; selected field stays reachable after resize');
 }
 await button('Done').scrollIntoViewIfNeeded();await expect(button('Done')).toBeInViewport();await page.screenshot({path:resolve(root,'small-window.png')});await button('Done').click();await page.setViewportSize({width:1280,height:900});
 await button('Save a copy').click();await picker('Save','Folio edited.pdf');await expect(page.getByLabel('Unsaved changes')).toHaveCount(0);await button('Open').click();await picker('Open','Folio edited.pdf');await button('Fill forms').click();await choose('Full name');await expect(name).toHaveValue('Page selected name');await choose('Country');await expect(page.getByRole('combobox',{name:'Country',exact:true})).toHaveValue('1');await choose('Delivery method — Mail');await expect(page.getByRole('radio',{name:'Delivery method — Mail'})).toBeChecked();await capture('reopened');await button('Done').click();
 checks.push('undo redo','intrinsic plus workspace rotation','keyboard selection and footer access at800x600','save reopen retains native field values');expect(errors).toEqual([]);const sourceHash=createHash('sha256').update(readFileSync(resolve(root,'Form.pdf'))).digest('hex');expect(sourceHash).toBe(JSON.parse(readFileSync(resolve(root,'input.json'),'utf8')).sha256);
 writeFileSync(resolve(root,'results.json'),JSON.stringify({passed:true,checks,geometry,sourceHash,errors,exe:record},null,2));console.log('PASS form page selection Windows acceptance');
}catch(error){writeFileSync(resolve(root,'failure.json'),JSON.stringify({error:String(error.stack),checks,geometry,errors},null,2));await page.screenshot({path:resolve(root,'failure.png')}).catch(()=>{});throw error;}finally{await browser.close();}
