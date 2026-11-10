// Windows/WebView native-dialog workflow documented in docs/corpus/README.md.
import {chromium,expect} from '@playwright/test';
import {execFile} from 'node:child_process';
import {promisify} from 'node:util';
import {readFileSync,writeFileSync,copyFileSync,mkdirSync,existsSync} from 'node:fs';
import {resolve} from 'node:path';
import {createHash} from 'node:crypto';
const run=resolve(process.env.FOLIO_RELEASE_RUN),exec=promisify(execFile);
const state=JSON.parse(readFileSync(resolve(run,'process.json'),'utf8').replace(/^\uFEFF/,''));
const dir=resolve(run,'workflow');mkdirSync(dir);
const fontPath=resolve(dir,'substitute.ttf');copyFileSync(resolve('tests/fixtures/corpus/fonts/DejaVuSerif.ttf'),fontPath);
const browser=await chromium.connectOverCDP(`http://127.0.0.1:${state.port}`);
const page=browser.contexts()[0].pages().find(p=>!p.url().startsWith('devtools:'));page.setDefaultTimeout(30000);
const errors=[],results=[];page.on('pageerror',e=>errors.push(String(e)));
const button=name=>page.getByRole('button',{name,exact:true});
const draft=()=>page.getByRole('textbox',{name:'Replacement text',exact:true});
const hash=path=>createHash('sha256').update(readFileSync(path)).digest('hex');
async function dialog(title,path,action){await exec('pwsh',['-NoProfile','-File',resolve('scripts/set-release-dialog.ps1'),'-AppProcessId',String(state.pid),'-FilePath',path,'-Title',title,'-Action',action],{windowsHide:true,timeout:25000});}
async function open(path,name,first=false){await button(first?'Open a PDF':'Open').click();await dialog('Open a PDF',path,'Open');await expect(page.getByRole('tab',{name,selected:true,exact:true})).toBeVisible();await expect(page.locator('.page-canvas image')).toHaveCount(1);}
async function select(){await button('Edit text').click();const target=page.locator('[data-text-object="0"]');await expect(target).toHaveCount(1);await expect(target).not.toHaveClass(/unsupported/);await target.focus();await page.keyboard.press('Enter');await expect(page.getByRole('dialog',{name:'Edit existing text'})).toBeVisible();}
async function save(name){const path=resolve(dir,name);await button('Save a copy').click();await dialog('Save a PDF copy',path,'Save');await expect.poll(()=>existsSync(path)).toBe(true);await expect(button('Save a copy')).toBeEnabled();return path;}
try{
 for(const rotation of [0,90,180,270]){
  const name=`source-${rotation}.pdf`,path=resolve(dir,name);
  copyFileSync(resolve('artifacts/subset-source-eligibility-20261024/corpus-corrected',name),path);const originalHash=hash(path);
  await open(path,name,rotation===0);await select();await expect(draft()).toHaveValue('Old');await expect(button('Apply changes')).toBeDisabled();await expect(button('Choose substitute font…')).toBeEnabled();
  await draft().fill('Edit ');await button('Apply changes').click();await expect(page.getByRole('alert')).toContainText('U+0020');await expect(draft()).toHaveValue('Edit ');
  await draft().fill('Edit');await button('Apply changes').click();await expect(page.getByRole('dialog')).toHaveCount(0);
  const saved=await save(`saved-${rotation}-complete.pdf`);
  await button('Undo').click();await select();await expect(draft()).toHaveValue('Old');await button('Cancel').click();
  await open(saved,`saved-${rotation}-complete.pdf`);await select();await expect(draft()).toHaveValue('Edit');await expect(button('Apply changes')).toBeDisabled();
  await page.screenshot({path:resolve(run,`reopened-${rotation}.png`)});
  await draft().fill('Old');await button('Apply changes').click();await expect(page.getByRole('dialog')).toHaveCount(0);await save(`restored-${rotation}-complete.pdf`);
  await open(path,name);await select();await draft().fill('Edit ');
  await button('Choose substitute font…').click();await button('Import font…').click();await dialog('Import a font',fontPath,'Open');await expect(button('Apply font')).toBeEnabled();await button('Apply font').click();
  await expect(page.getByLabel('Replacement font preview',{exact:true})).toContainText('Edit');
  await button('Apply changes').click();await expect(page.getByRole('dialog')).toHaveCount(0);const substituted=await save(`substituted-${rotation}-complete.pdf`);
  await open(substituted,`substituted-${rotation}-complete.pdf`);await select();await expect(draft()).toHaveValue('Edit ');await expect(button('Apply changes')).toBeDisabled();await button('Cancel').click();
  expect(hash(path)).toBe(originalHash);results.push({rotation,name,originalHash,missingRealSpaceRefused:true,originalFontEdited:true,undo:true,reopened:true,reedited:true,substituted:true,substituteFontHash:hash(fontPath)});
 }
 expect(errors).toEqual([]);writeFileSync(resolve(run,'results.json'),JSON.stringify({passed:true,results,errors,exe:state},null,2),{flag:'wx'});console.log('PASS subset source Windows editing, missing-space refusal, undo and saved re-edit');
}catch(e){await page.screenshot({path:resolve(run,'failure.png')}).catch(()=>{});throw e;}finally{await browser.close();}
