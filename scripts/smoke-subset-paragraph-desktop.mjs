// Native Windows/WebView workflow described in docs/corpus/README.md.
import {chromium,expect} from '@playwright/test';
import {execFile} from 'node:child_process';
import {promisify} from 'node:util';
import {readFileSync,writeFileSync,copyFileSync,mkdirSync,existsSync} from 'node:fs';
import {resolve,basename} from 'node:path';
import {createHash} from 'node:crypto';
const root=resolve(process.env.FOLIO_RELEASE_RUN),exec=promisify(execFile);
const state=JSON.parse(readFileSync(resolve(root,'process.json'),'utf8').replace(/^\uFEFF/,''));
const dir=resolve(root,'workflow');mkdirSync(dir);
const browser=await chromium.connectOverCDP(`http://127.0.0.1:${state.port}`);
const page=browser.contexts()[0].pages().find(p=>!p.url().startsWith('devtools:'));page.setDefaultTimeout(30000);
const errors=[],results=[];page.on('pageerror',error=>errors.push(String(error)));
const button=name=>page.getByRole('button',{name,exact:true});
const editor=page.getByRole('textbox',{name:'Paragraph text',exact:true});
const sha=file=>createHash('sha256').update(readFileSync(file)).digest('hex');
async function dialog(title,path,action){await exec('pwsh',['-NoProfile','-File',resolve('scripts/set-release-dialog.ps1'),'-AppProcessId',String(state.pid),'-FilePath',path,'-Title',title,'-Action',action],{windowsHide:true,timeout:25000});}
async function open(path,first=false){await button(first?'Open a PDF':'Open').click();await dialog('Open a PDF',path,'Open');await expect(page.getByRole('tab',{name:basename(path),selected:true,exact:true})).toBeVisible();await expect(page.locator('.page-canvas image')).toHaveCount(1);}
async function paragraph(text){await button('Edit text').click();const target=page.getByRole('button',{name:new RegExp('^Edit text: '+text)});await target.focus();await page.keyboard.press('Enter');await button('Edit paragraph…').click();await expect(editor).toHaveText(text);}
async function setText(text){await editor.focus();await page.keyboard.press('Control+A');await page.keyboard.insertText(text);await expect(editor).toHaveText(text);}
async function preview(){await button('Preview paragraph').click();await expect(page.getByAltText('Paragraph PDF preview')).toBeVisible();await expect(button('Apply paragraph')).toBeEnabled();}
async function save(name){const path=resolve(dir,name);if(existsSync(path))throw Error('Preserve prior output');await button('Save a copy').click();await dialog('Save a PDF copy',path,'Save');await expect.poll(()=>existsSync(path)).toBe(true);await expect(button('Save a copy')).toBeEnabled();return path;}
try{
 for(const rotation of [0,90,180,270]){
  const source=resolve(dir,`source-${rotation}.pdf`);copyFileSync(resolve('artifacts/subset-paragraph-20261024/corpus-final',`source-${rotation}-styled.pdf`),source);const original=sha(source);
  await open(source,rotation===0);await paragraph('Old');await setText('Edit Old');await button('Preview paragraph').click();await expect(page.getByRole('alert')).toContainText('U+0020');await expect(button('Apply paragraph')).toBeDisabled();
  await setText('Edit');await button('Fit area to text').click();await preview();await page.screenshot({path:resolve(root,`preview-${rotation}.png`)});await button('Apply paragraph').click();await expect(page.getByRole('dialog')).toHaveCount(0);const saved=await save(`saved-${rotation}.pdf`);
  await button('Undo').click();await paragraph('Old');await button('Cancel').click();
  await open(saved);await paragraph('Edit');await preview();await button('Apply paragraph').click();const repeated=await save(`repeated-${rotation}.pdf`);
  await open(repeated);await paragraph('Edit');await expect(editor).toHaveText('Edit');await button('Cancel').click();expect(sha(source)).toBe(original);
  results.push({rotation,source,original,saved,repeated,missingSpaceRefused:true,undo:true,reopened:true});
 }
 expect(errors).toEqual([]);writeFileSync(resolve(root,'results.json'),JSON.stringify({passed:true,results,errors,exe:state},null,2),{flag:'wx'});console.log('PASS subset paragraph fit, preview, Apply, Save, Undo and saved replay in four rotations');
}catch(error){writeFileSync(resolve(root,'failure.json'),JSON.stringify({error:String(error.stack),results,errors},null,2),{flag:'wx'});await page.screenshot({path:resolve(root,'failure.png')}).catch(()=>{});throw error;}finally{await browser.close();}
