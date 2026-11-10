// Real Windows UI: repeated highlighted glyphs in distinct color styles.
import {chromium,expect} from '@playwright/test';
import {execFile} from 'node:child_process';import {promisify} from 'node:util';
import {readFileSync,writeFileSync,existsSync,renameSync} from 'node:fs';
import {resolve,basename} from 'node:path';import {createHash} from 'node:crypto';
const root=resolve(process.env.FOLIO_MULTIFONT_DESKTOP_ROOT||'artifacts/multifont-highlights-20261024/desktop-accepted');
const record=JSON.parse(readFileSync(resolve(root,'profile/process.json'),'utf8').replace(/^\uFEFF/,''));
const browser=await chromium.connectOverCDP(`http://127.0.0.1:${record.port}`),page=browser.contexts()[0].pages().find(p=>!p.url().startsWith('devtools:'));
page.setDefaultTimeout(30000);const checks=[],errors=[],outputs=[];page.on('pageerror',e=>errors.push(e.message));
const button=name=>page.getByRole('button',{name,exact:true}),editor=page.getByRole('textbox',{name:'Paragraph text',exact:true});
const sha=path=>createHash('sha256').update(readFileSync(path)).digest('hex');
async function picker(action,path,title){await promisify(execFile)('pwsh',['-NoProfile','-File',resolve('artifacts/set-multifont-dialog.ps1'),'-AppProcessId',String(record.pid),'-FilePath',path,'-Action',action,...(title?['-DialogTitle',title]:[]),...(action==='Open'?['-NativeFileName']:[])],{windowsHide:true,timeout:60000});}
async function open(path){await page.getByRole('button',{name:/^(Open|Open a PDF)$/}).click();await picker('Open',path);await expect(page.getByRole('tab',{name:basename(path),exact:true}).last()).toHaveAttribute('aria-selected','true');await expect(page.locator('.document-page image').first()).toBeVisible();}
async function save(name){const native=resolve(root,'Folio edited.pdf'),output=resolve(root,name);if(existsSync(native)||existsSync(output))throw Error('Preserve prior output');await button('Save a copy').click();await picker('Save',native);await expect(page.locator('.statusbar')).toContainText('Saved a copy');renameSync(native,output);return output;}
async function paragraph(prefix){await button('Edit text').click();await page.getByRole('button',{name:new RegExp('^Edit text: '+prefix)}).first().click();await button('Edit paragraph…').click();await expect(editor).toBeVisible();}
async function preview(){await button('Preview paragraph').click();await expect(page.getByAltText('Paragraph PDF preview')).toBeVisible();}
async function select(start,end){await editor.evaluate((el,[start,end])=>{el.focus();const walker=document.createTreeWalker(el,NodeFilter.SHOW_TEXT);let node,offset=0,a,b;while(node=walker.nextNode()){const next=offset+node.textContent.length;if(!a&&start<=next)a=[node,start-offset];if(end<=next){b=[node,end-offset];break;}offset=next;}if(!a||!b)throw Error('Selection outside editor');const r=document.createRange();r.setStart(...a);r.setEnd(...b);getSelection().removeAllRanges();getSelection().addRange(r);document.dispatchEvent(new Event('selectionchange'));},[start,end]);}
try{
 await page.setViewportSize({width:1280,height:900});
 for(const rotation of [0,90,180,270]){
  const source=resolve(root,`Same-${rotation}.pdf`),sourceHash=sha(source);await open(source);await paragraph('AA');await expect(editor).toHaveText('AA BB');
  await select(0,5);await button('Choose substitute font…').click();
  await button('Import font…').click();await picker('Open',resolve(root,'DejaVuSerif.ttf'),'Import a font');
  await button('Apply font').click();await page.getByRole('spinbutton',{name:'Font size (pt)',exact:true}).fill('18');await page.getByLabel('Text color',{exact:true}).fill('#993366');
  await select(0,5);await page.keyboard.insertText('Café Café');await expect(editor).toHaveText('Café Café');
  await select(5,9);await page.getByLabel('Text color',{exact:true}).fill('#336699');
  await page.getByRole('spinbutton',{name:'Line spacing (× font size)',exact:true}).fill('1.5');
  await button('Fit area to text').click();await expect(page.getByText('Area adjusted. Preview the paragraph before applying.',{exact:true})).toBeVisible();await preview();
  await page.screenshot({path:resolve(root,`preview-${rotation}.png`)});await button('Apply paragraph').click();const output=await save(`Edited-${rotation}.pdf`);
  await open(output);await paragraph('Café');await expect(editor).toHaveText('Café Café');await expect(page.getByRole('spinbutton',{name:'Line spacing (× font size)',exact:true})).toHaveValue('1.5');
  await select(0,4);await expect(page.getByLabel('Text color',{exact:true})).toHaveValue('#993366');await select(5,9);await expect(page.getByLabel('Text color',{exact:true})).toHaveValue('#336699');
  await preview();await button('Apply paragraph').click();const repeated=await save(`Reedited-${rotation}.pdf`);expect(sha(source)).toBe(sourceHash);
  outputs.push({rotation,source,sourceHash,output,repeated});checks.push(`rotation ${rotation}: import font, replace text, color second identical word, fit, preview, save, reopen, restore colors, unchanged replay`);
 }
 expect(errors).toEqual([]);writeFileSync(resolve(root,'results.json'),JSON.stringify({passed:true,checks,errors,outputs,exe:record},null,2));console.log(JSON.stringify({passed:true,checks}));
}catch(error){writeFileSync(resolve(root,'failure.json'),JSON.stringify({error:String(error.stack),checks,errors,outputs},null,2));await page.screenshot({path:resolve(root,'failure.png')}).catch(()=>{});throw error;}finally{await browser.close();}
