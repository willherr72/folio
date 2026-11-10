// Follows the repository's Windows WebView2 acceptance workflow on an owned app.
import {chromium,expect} from '@playwright/test';
import {execFile} from 'node:child_process';import {promisify} from 'node:util';
import {readFileSync,writeFileSync,existsSync,renameSync} from 'node:fs';
import {resolve,basename} from 'node:path';import {createHash} from 'node:crypto';
const root=resolve(process.env.FOLIO_CROP_DESKTOP_ROOT||'artifacts/page-cropping-20261024/desktop-accepted');
const record=JSON.parse(readFileSync(resolve(root,'profile/process.json'),'utf8').replace(/^\uFEFF/,''));
const browser=await chromium.connectOverCDP(`http://127.0.0.1:${record.port}`),page=browser.contexts()[0].pages().find(p=>!p.url().startsWith('devtools:'));
page.setDefaultTimeout(30000);const errors=[],checks=[],outputs=[];page.on('pageerror',e=>errors.push(e.message));
const button=name=>page.getByRole('button',{name,exact:true});const sha=path=>createHash('sha256').update(readFileSync(path)).digest('hex');
async function picker(action,path){await promisify(execFile)('pwsh',['-NoProfile','-File',resolve('artifacts/set-multifont-dialog.ps1'),'-AppProcessId',String(record.pid),'-FilePath',path,'-Action',action,...(action==='Open'?['-NativeFileName']:[])],{windowsHide:true,timeout:60000});}
async function open(path){await page.getByRole('button',{name:/^(Open|Open a PDF)$/}).click();await picker('Open',path);await expect(page.getByRole('tab',{name:basename(path),exact:true}).last()).toHaveAttribute('aria-selected','true');await expect(page.locator('.document-page image').first()).toBeVisible();}
async function save(name){const native=resolve(root,'Folio edited.pdf'),output=resolve(root,name);if(existsSync(native)||existsSync(output))throw Error('Preserve prior output');await button('Save a copy').click();await picker('Save',native);await expect(page.locator('.statusbar')).toContainText('Saved a copy');renameSync(native,output);return output;}
try{
  await page.setViewportSize({width:1280,height:900});
  for(const [unit,intrinsic] of [[0.5,0],[1,90],[1.7,180],[2,270]]){
    const source=resolve(root,`Crop-${unit}-${intrinsic}.pdf`),sourceHash=sha(source);await open(source);await button('Select').click();
    await button('Crop page…').click();await expect(page.getByAltText('Original page with current edits')).toBeVisible();
    const handle=page.locator('[data-crop-handle="se"]'),box=await handle.boundingBox();
    await page.mouse.move(box.x+box.width/2,box.y+box.height/2);await page.mouse.down();await page.mouse.move(box.x+box.width/2-20,box.y+box.height/2-25,{steps:8});await page.mouse.up();
    expect(Number(await page.getByLabel('Right margin (pt)',{exact:true}).inputValue())).toBeGreaterThan(0);
    await button('Reset crop').click();await expect(page.getByLabel('Right margin (pt)',{exact:true})).toHaveValue('0');
    for(const [side,value] of [['Left',15],['Top',20],['Right',25],['Bottom',30]])await page.getByLabel(`${side} margin (pt)`,{exact:true}).fill(String(value));
    await button('Preview crop').click();await expect(page.getByAltText('Cropped page preview')).toBeVisible();
    expect((await page.getByRole('dialog').boundingBox()).width).toBeGreaterThan(800);
    await page.getByLabel('Left margin (pt)',{exact:true}).focus();await page.keyboard.press('Tab');await expect(page.getByLabel('Top margin (pt)',{exact:true})).toBeFocused();
    if(intrinsic===0){
      await page.setViewportSize({width:520,height:740});
      expect(await page.getByRole('dialog').evaluate(el=>el.scrollWidth<=el.clientWidth+1)).toBeTruthy();
      await button('Apply crop').scrollIntoViewIfNeeded();await expect(button('Apply crop')).toBeVisible();
      await page.screenshot({path:resolve(root,'crop-narrow.png')});await page.setViewportSize({width:1280,height:900});
    }
    await page.screenshot({path:resolve(root,`crop-preview-${intrinsic}.png`)});await button('Apply crop').click();await expect(page.getByRole('dialog')).toHaveCount(0);
    const cropView=await page.locator('.page-canvas').first().getAttribute('viewBox');expect(cropView.startsWith('15 20 ')).toBeTruthy();
    await button('Undo').click();expect((await page.locator('.page-canvas').first().getAttribute('viewBox')).startsWith('0 0 ')).toBeTruthy();
    await button('Redo').click();await expect(page.locator('.page-canvas').first()).toHaveAttribute('viewBox',cropView);
    await button('Rotate').click();await button('Edit text').click();await page.getByRole('button',{name:'Edit text: Retained content',exact:true}).click();
    await page.getByLabel('Replacement text',{exact:true}).fill('Retained');await button('Apply changes').click();await expect(page.getByRole('dialog')).toHaveCount(0);
    await page.screenshot({path:resolve(root,`cropped-edited-${intrinsic}.png`)});const output=await save(`Cropped-${unit}-${intrinsic}.pdf`);
    await open(output);await button('Edit text').click();await page.getByRole('button',{name:'Edit text: Retained',exact:true}).click();await expect(page.getByLabel('Replacement text',{exact:true})).toHaveValue('Retained');await button('Cancel').click();
    const repeated=await save(`Reopened-${unit}-${intrinsic}.pdf`);expect(sha(source)).toBe(sourceHash);
    outputs.push({unit,intrinsic,workspaceRotation:90,margins:{left:15,top:20,right:25,bottom:30},source,sourceHash,output,repeated});
    checks.push(`unit ${unit}, intrinsic ${intrinsic}: drag, reset, numeric margins, native preview, apply, undo/redo, rotate, edit existing text, save/reopen and re-save`);
  }
  expect(errors).toEqual([]);writeFileSync(resolve(root,'results.json'),JSON.stringify({passed:true,checks,errors,outputs,exe:record},null,2));console.log(JSON.stringify({passed:true,checks}));
}catch(error){writeFileSync(resolve(root,'failure.json'),JSON.stringify({error:String(error.stack),checks,errors,outputs},null,2));await page.screenshot({path:resolve(root,'failure.png')}).catch(()=>{});throw error;}finally{await browser.close();}
