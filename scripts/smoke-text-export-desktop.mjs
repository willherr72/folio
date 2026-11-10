import {chromium,expect} from '@playwright/test';
import {readFileSync,writeFileSync,existsSync} from 'node:fs';
import {resolve} from 'node:path';import {execFile} from 'node:child_process';import {promisify} from 'node:util';import {createHash} from 'node:crypto';
const root=resolve(process.env.FOLIO_TEXT_EXPORT_ROOT??'artifacts/text-export-20261024/desktop');
const record=JSON.parse(readFileSync(resolve(root,'process.json'),'utf8').replace(/^\uFEFF/,''));
const browser=await chromium.connectOverCDP(`http://127.0.0.1:${record.port}`),page=browser.contexts()[0].pages().find(p=>!p.url().startsWith('devtools:'));page.setDefaultTimeout(30000);
const errors=[],checks=[],outputs=[];page.on('pageerror',e=>errors.push(e.message));const button=name=>page.getByRole('button',{name,exact:true});
const sha=path=>createHash('sha256').update(readFileSync(path)).digest('hex');
async function picker(action,name,cancel=false){await promisify(execFile)('pwsh',['-NoProfile','-File',resolve('artifacts/text-export-20261024/set-dialog.ps1'),'-AppProcessId',String(record.pid),'-Action',action,'-Title',action==='Open'?'Open a PDF':'Export text','-FilePath',resolve(root,name),...(cancel?['-Cancel']:[])],{windowsHide:true,timeout:60000});}
async function dialog(){await button('Save options').click();await page.getByRole('menuitem',{name:'Export text…',exact:true}).click();await expect(page.getByRole('dialog',{name:'Export text',exact:true})).toBeVisible();}
async function output(name,mode,range='',fields=true){
 await dialog();await page.getByRole('combobox',{name:'Pages',exact:true}).selectOption(mode);if(mode==='range')await page.getByRole('textbox',{name:'Page range',exact:true}).fill(range);
 await page.getByRole('checkbox',{name:'Include form values'}).setChecked(fields);await button('Choose file and export…').click();await picker('Save',name);
 await expect(page.getByText(/Exported \d+ pages? to/)).toBeVisible();await expect(button('Done')).toBeEnabled();
 if(fields)await expect(page.getByText('1 password or unsupported field was omitted.',{exact:true})).toBeVisible();
 outputs.push({name,text:readFileSync(resolve(root,name),'utf8'),sha256:sha(resolve(root,name))});await page.screenshot({path:resolve(root,name+'.png')});await button('Done').click();await expect(page.getByLabel('Unsaved changes')).toBeVisible();
}
try{
 await page.setViewportSize({width:1280,height:900});const source=resolve(root,'Text sample.pdf'),original=sha(source);
 await button('Open a PDF').click();await picker('Open','Text sample.pdf');await expect(page.getByRole('tab',{name:'Text sample.pdf',exact:true})).toBeVisible();
 await button('Crop page…').click();await page.getByLabel('Bottom margin (pt)',{exact:true}).fill('100');await button('Preview crop').click();await expect(button('Apply crop')).toBeEnabled();await button('Apply crop').click();
 await button('Edit text').click();await page.getByRole('button',{name:'Edit text: First page',exact:true}).click();await page.getByLabel('Replacement text',{exact:true}).fill('Edited page');await button('Apply changes').click();await expect(page.getByRole('dialog')).toHaveCount(0);
 await button('Text').click();await page.getByLabel('Page 1 container',{exact:true}).locator('.page-canvas').click({position:{x:80,y:125}});await page.getByRole('textbox',{name:'Content',exact:true}).fill('Unsaved overlay');await page.getByRole('textbox',{name:'Content',exact:true}).press('Tab');await button('Select').click();await button('Rotate').click();
 await output('all.txt','all');expect(outputs[0].text).toContain('Edited page');expect(outputs[0].text).toContain('Unsaved overlay');expect(outputs[0].text).toContain('Second page');expect(outputs[0].text).toContain('Customer: Ada');expect(outputs[0].text).not.toContain('password-value');expect(outputs[0].text).not.toContain('Cropped away');expect(outputs[0].text.split('\f')).toHaveLength(2);
 checks.push('all edited pages; crop, rotation, added text, native text edit, labeled form values and password omission; dirty state retained');
 await output('range.txt','range','2',false);expect(outputs[1].text).toContain('Second page');expect(outputs[1].text).not.toContain('Edited page');expect(outputs[1].text).not.toContain('Unsaved overlay');checks.push('selected range excludes unselected content');
 await dialog();await page.setViewportSize({width:620,height:720});await button('Choose file and export…').scrollIntoViewIfNeeded();await expect(button('Choose file and export…')).toBeInViewport();expect(await page.getByRole('dialog').evaluate(el=>el.scrollWidth<=el.clientWidth+1)).toBe(true);await page.screenshot({path:resolve(root,'narrow.png')});
 await button('Choose file and export…').click();await picker('Save','cancelled.txt',true);await expect(page.getByText('Export cancelled. The PDF is unchanged.',{exact:true})).toBeVisible();expect(existsSync(resolve(root,'cancelled.txt'))).toBe(false);await button('Cancel').click();checks.push('native picker cancel; narrow footer reachable');
 await page.setViewportSize({width:1280,height:900});await expect(button('Undo')).toBeEnabled();await button('Undo').click();await expect(page.getByLabel('Unsaved changes')).toBeVisible();
 expect(sha(source)).toBe(original);expect(errors).toEqual([]);writeFileSync(resolve(root,'results.json'),JSON.stringify({passed:true,checks,outputs,source,sourceHash:original,errors,exe:record},null,2));console.log('PASS Windows text export');
}catch(error){writeFileSync(resolve(root,'failure.json'),JSON.stringify({error:String(error.stack),checks,outputs,errors},null,2));await page.screenshot({path:resolve(root,'failure.png')}).catch(()=>{});throw error;}finally{await browser.close();}
