// Authored mixed-font Windows fit, history, publication and saved replay.
import {chromium,expect} from '@playwright/test';
import {execFile} from 'node:child_process';import {promisify} from 'node:util';
import {readFileSync,writeFileSync,existsSync,renameSync} from 'node:fs';import {resolve,basename} from 'node:path';import {createHash} from 'node:crypto';
const root=resolve(process.env.FOLIO_FIT_DESKTOP_ROOT||'artifacts/paragraph-fit-corrected-20261024/desktop-accepted');
const record=JSON.parse(readFileSync(resolve(root,'profile/process.json'),'utf8').replace(/^\uFEFF/,''));
const browser=await chromium.connectOverCDP(`http://127.0.0.1:${record.port}`),page=browser.contexts()[0].pages().find(p=>!p.url().startsWith('devtools:'));
page.setDefaultTimeout(30000);const checks=[],errors=[],outputs=[];page.on('pageerror',e=>errors.push(e.message));
const button=name=>page.getByRole('button',{name,exact:true}),editor=page.getByRole('textbox',{name:'Paragraph text',exact:true});
const sha=path=>createHash('sha256').update(readFileSync(path)).digest('hex');
async function picker(action,path,title){await promisify(execFile)('pwsh',['-NoProfile','-File',resolve('scripts/set-native-dialog.ps1'),'-AppProcessId',String(record.pid),'-FilePath',path,'-Action',action,...(title?['-DialogTitle',title]:[]),...(action==='Open'?['-NativeFileName']:[])],{windowsHide:true,timeout:60000});}
async function open(path){await page.getByRole('button',{name:/^(Open|Open a PDF)$/}).click();await picker('Open',path);await expect(page.getByRole('tab',{name:basename(path),exact:true}).last()).toHaveAttribute('aria-selected','true');await expect(page.locator('.document-page image').first()).toBeVisible();}
async function save(name){const native=resolve(root,'Folio edited.pdf'),output=resolve(root,name);if(existsSync(native)||existsSync(output))throw Error('Preserve prior output');await button('Save a copy').click();await picker('Save',native);await expect(page.locator('.statusbar')).toContainText('Saved a copy');renameSync(native,output);return output;}
async function paragraph(text){await button('Edit text').click();await page.getByRole('button',{name:new RegExp('^Edit text: '+text.split(' ')[0])}).first().click();await button('Edit paragraph…').click();await expect(editor).toBeVisible();}
async function preview(){await button('Preview paragraph').click();await expect(page.getByAltText('Paragraph PDF preview')).toBeVisible();}
async function bounds(){return Promise.all(['Left (pt)','Top (pt)','Area width (pt)','Area height (pt)'].map(label=>page.getByRole('spinbutton',{name:label,exact:true}).inputValue()));}
async function selectMark(){await editor.evaluate(el=>{el.focus();const mark=el.querySelector('[data-highlighted=true]');const r=document.createRange();r.selectNodeContents(mark);getSelection().removeAllRanges();getSelection().addRange(r);document.dispatchEvent(new Event('selectionchange'));});}
try{
 await page.setViewportSize({width:1280,height:900});
 for(const rotation of [0,90,180,270]){
  const source=resolve(root,`Mixed-${rotation}.pdf`),sourceHash=sha(source);await open(source);await paragraph('BB');
  await preview();const initial=await bounds();await button('Fit area to text').click();await expect(page.getByText('The current area already fits the text.',{exact:true})).toBeVisible();await expect(button('Apply paragraph')).toBeEnabled();expect(await bounds()).toEqual(initial);
  await selectMark();const imported=rotation===0;
  if(imported){await button('Choose substitute font…').click();await button('Import font…').click();await picker('Open',resolve(root,'DejaVuSerif.ttf'),'Import a font');await button('Apply font').click();await page.getByRole('spinbutton',{name:'Font size (pt)',exact:true}).fill('18');await page.getByLabel('Text color',{exact:true}).fill('#993366');await selectMark();}
  const replacement=imported?'Café marked words':'BB DD EE FF GG HH II JJ';await page.keyboard.insertText(replacement);await expect(editor).toHaveText(`AA ${replacement} CC`);await expect(editor.locator('[data-highlighted=true]')).toHaveText(replacement);
  await page.getByRole('spinbutton',{name:'Line spacing (× font size)',exact:true}).fill('1.5');
  await button('Preview paragraph').click();await expect(page.getByRole('alert')).toBeVisible();await expect(button('Apply paragraph')).toBeDisabled();
  const before=await bounds();await button('Fit area to text').click();await expect(page.getByText('Area adjusted. Preview the paragraph before applying.',{exact:true})).toBeVisible();const fitted=await bounds();expect(fitted).not.toEqual(before);await expect(button('Apply paragraph')).toBeDisabled();
  await page.getByRole('dialog').getByRole('button',{name:'Undo',exact:true}).click();expect(await bounds()).toEqual(before);await expect(editor).toHaveText(`AA ${replacement} CC`);await page.getByRole('dialog').getByRole('button',{name:'Redo',exact:true}).click();expect(await bounds()).toEqual(fitted);
  await preview();await button('Fit area to text').click();await expect(page.getByText('The current area already fits the text.',{exact:true})).toBeVisible();await expect(button('Apply paragraph')).toBeEnabled();
  await page.locator('.paragraph-body').evaluate(el=>el.scrollTop=0);await page.locator('.paragraph-page-scroll').evaluate(el=>el.scrollTop=0);await page.screenshot({path:resolve(root,`preview-${rotation}.png`)});await button('Apply paragraph').click();
  const output=await save(`Edited-${rotation}.pdf`);await open(output);await paragraph(replacement);await expect(editor).toHaveText(`AA ${replacement} CC`);await expect(editor.locator('[data-highlighted=true]')).toHaveText(replacement);await expect(page.getByRole('spinbutton',{name:'Line spacing (× font size)',exact:true})).toHaveValue('1.5');await preview();await button('Apply paragraph').click();const repeated=await save(`Reedited-${rotation}.pdf`);
  expect(sha(source)).toBe(sourceHash);outputs.push({rotation,source,sourceHash,output,repeated,replacement,imported,before,fitted});checks.push(`rotation ${rotation}: no-op preserves preview; longer draft refuses preview, fits, undo/redo preserves text, preview/apply/save/reopen/exact no-op; imported font ${imported}`);
 }
 expect(errors).toEqual([]);writeFileSync(resolve(root,'results.json'),JSON.stringify({passed:true,checks,errors,outputs,exe:record},null,2));console.log(JSON.stringify({passed:true,checks}));
}catch(error){writeFileSync(resolve(root,'failure.json'),JSON.stringify({error:String(error.stack),checks,errors,outputs},null,2));await page.screenshot({path:resolve(root,'failure.png')}).catch(()=>{});throw error;}finally{await browser.close();}
