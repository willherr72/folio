// Actual Windows dialog/native PDF acceptance, using authored fixtures only.
import {chromium,expect} from '@playwright/test';
import {execFile} from 'node:child_process';import {promisify} from 'node:util';
import {readFileSync,writeFileSync,existsSync,copyFileSync,renameSync} from 'node:fs';import {resolve,basename} from 'node:path';import {createHash} from 'node:crypto';
const root=resolve('artifacts/paragraph-initial-area-20261024/desktop');
const record=JSON.parse(readFileSync(resolve(root,'profile/process.json'),'utf8').replace(/^\uFEFF/,''));
const browser=await chromium.connectOverCDP(`http://127.0.0.1:${record.port}`),page=browser.contexts()[0].pages().find(p=>!p.url().startsWith('devtools:'));
page.setDefaultTimeout(30000);const checks=[],errors=[],outputs=[];page.on('pageerror',e=>errors.push(e.message));
const button=name=>page.getByRole('button',{name,exact:true}),field=name=>page.getByRole('spinbutton',{name,exact:true}),editor=page.getByRole('textbox',{name:'Paragraph text',exact:true});
const sha=path=>createHash('sha256').update(readFileSync(path)).digest('hex');
async function picker(action,path){await promisify(execFile)('pwsh',['-NoProfile','-File',resolve('scripts/set-native-dialog.ps1'),'-AppProcessId',String(record.pid),'-FilePath',path,'-Action',action,...(action==='Open'?['-NativeFileName']:[])],{windowsHide:true,timeout:60000});}
async function open(path){await page.getByRole('button',{name:/^(Open|Open a PDF)$/}).click();await picker('Open',path);await expect(page.getByRole('tab',{name:basename(path),exact:true}).last()).toHaveAttribute('aria-selected','true');await expect(page.locator('.document-page image').first()).toBeVisible();}
async function save(name){const native=resolve(root,'Folio edited.pdf'),output=resolve(root,name);if(existsSync(native)||existsSync(output))throw Error('Preserve prior output');await button('Save a copy').click();await picker('Save',native);await expect(page.locator('.statusbar')).toContainText('Saved a copy');renameSync(native,output);return output;}
async function paragraph(text){await button('Edit text').click();await button(`Edit text: ${text}`).first().click();await button('Edit paragraph…').click();await expect(editor).toBeVisible();}
async function bounds(){return Promise.all(['Left (pt)','Top (pt)','Area width (pt)','Area height (pt)'].map(label=>field(label).inputValue()));}
async function preview(){await button('Preview paragraph').click();await expect(page.getByAltText('Paragraph PDF preview')).toBeVisible();}
try{
 await page.setViewportSize({width:1280,height:900});
 for(const rotation of [0,90,180,270]){
  const original=resolve(`artifacts/styled-paragraph-editor-20261024/desktop/Mixed-${rotation}.pdf`),source=resolve(root,`Mixed-${rotation}.pdf`);if(existsSync(source))throw Error('Preserve fixture copy');copyFileSync(original,source);const sourceHash=sha(source);
  await open(source);await paragraph('One ');await button('Adjust selection').click();await page.getByRole('checkbox',{name:'two',exact:true}).check();await button('Check selection').click();await expect(editor).toHaveText('One two');
  const suggested=await bounds(),width=Number(suggested[rotation%180?3:2]);expect(width).toBeGreaterThan(56.9);expect(width).toBeLessThan(57.1);
  await preview();await expect(editor).toHaveText('One two');await expect.poll(bounds).toEqual(suggested);await page.screenshot({path:resolve(root,`preview-${rotation}.png`)});await button('Apply paragraph').click();
  const output=await save(`Initial-${rotation}.pdf`);await open(output);await paragraph('One ');await expect(editor).toHaveText('One two');await expect.poll(bounds).toEqual(suggested);await preview();await button('Apply paragraph').click();const repeated=await save(`Reedited-${rotation}.pdf`);
  expect(sha(source)).toBe(sourceHash);expect(sha(original)).toBe(sourceHash);outputs.push({rotation,source,sourceHash,output,repeated,suggested});checks.push(`rotation ${rotation}: unchanged mixed paragraph previews with suggested area; saved bounds and no-op re-edit retained`);
 }
 expect(errors).toEqual([]);writeFileSync(resolve(root,'results.json'),JSON.stringify({passed:true,checks,errors,outputs,exe:record},null,2));console.log(JSON.stringify({passed:true,checks}));
}catch(error){writeFileSync(resolve(root,'failure.json'),JSON.stringify({error:String(error.stack),checks,errors,outputs},null,2));await page.screenshot({path:resolve(root,'failure.png')}).catch(()=>{});throw error;}finally{await browser.close();}
