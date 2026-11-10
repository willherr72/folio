// Repository Windows/WebView acceptance against an isolated owned native app.
import {chromium,expect} from '@playwright/test';
import {execFile} from 'node:child_process';import {promisify} from 'node:util';
import {readFileSync,writeFileSync,existsSync,renameSync} from 'node:fs';
import {resolve,basename} from 'node:path';import {createHash} from 'node:crypto';
const root=resolve(process.env.FOLIO_PARAGRAPH_JUSTIFY_ROOT??'artifacts/paragraph-justify-acceptance');
const record=JSON.parse(readFileSync(resolve(root,'profile/process.json'),'utf8').replace(/^\uFEFF/,''));
const browser=await chromium.connectOverCDP(`http://127.0.0.1:${record.port}`);
const page=browser.contexts()[0].pages().find(p=>!p.url().startsWith('devtools:'));page.setDefaultTimeout(30000);
const errors=[],checks=[];page.on('pageerror',error=>errors.push(error.message));
const button=name=>page.getByRole('button',{name,exact:true}),field=name=>page.getByRole('spinbutton',{name,exact:true});
const text=()=>page.getByRole('textbox',{name:'Paragraph text',exact:true});
const hash=path=>createHash('sha256').update(readFileSync(path)).digest('hex');
async function picker(action,path,title){await promisify(execFile)('pwsh',['-NoProfile','-File',resolve('scripts/set-native-dialog.ps1'),'-AppProcessId',String(record.pid),'-FilePath',path,'-Action',action,...(title?['-DialogTitle',title]:[]),...(action==='Open'?['-NativeFileName']:[])],{windowsHide:true,timeout:60000});}
async function open(path){await page.getByRole('button',{name:/^(Open|Open a PDF)$/}).click();await picker('Open',path);await expect(page.getByRole('tab',{name:basename(path),exact:true}).last()).toHaveAttribute('aria-selected','true');await expect(page.locator('.document-page image').first()).toBeVisible();}
async function save(name){const destination=resolve(root,name),native=resolve(root,'Folio edited.pdf');if(existsSync(native)||existsSync(destination))throw Error('Output already exists');await button('Save a copy').click();await picker('Save',native);await expect(page.locator('.statusbar')).toContainText('Saved a copy');renameSync(native,destination);return destination;}
async function paragraph(value){await button('Edit text').click();await button(`Edit text: ${value}`).click();await button('Edit paragraph…').click();await expect(text()).toBeVisible();}
async function area(){return {x:Number(await field('Left (pt)').inputValue()),y:Number(await field('Top (pt)').inputValue()),width:Number(await field('Area width (pt)').inputValue()),height:Number(await field('Area height (pt)').inputValue())};}
async function preview(){await button('Preview paragraph').click();await expect(page.getByAltText('Paragraph PDF preview')).toBeVisible();}
async function contains(value){await expect.poll(()=>page.locator('.document-page .pdf-text-layer').first().textContent()).toContain(value);}
try {
 await page.setViewportSize({width:1280,height:900});const source=resolve(root,'Original paragraph.pdf'),sourceHash=hash(source),font=resolve(root,'Substitute.ttf'),fontHash=hash(font);
 await open(source);await save('Before justification.pdf');await button('Rotate').click();await paragraph('Original words.');const old=await area();
 for(const [name,value] of [['Left (pt)','60'],['Top (pt)','80'],['Area width (pt)','180'],['Area height (pt)','90'],['Line spacing (× font size)','1.5']])await field(name).fill(value);
 await page.getByRole('combobox',{name:'Paragraph alignment'}).selectOption('justify');
 await text().fill('Alpha beta\nGamma delta');await preview();await page.screenshot({path:resolve(root,'hard-break-preview.png')});
 const firstText='Alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu';await text().fill(firstText);await expect(button('Apply paragraph')).toBeDisabled();await preview();
 await button('Adjust selection').click();await button('Cancel adjustment').click();await expect(page.getByRole('combobox',{name:'Paragraph alignment'})).toHaveValue('justify');await expect(button('Apply paragraph')).toBeEnabled();
 await page.screenshot({path:resolve(root,'justify-preview.png')});await page.setViewportSize({width:640,height:720});await page.getByRole('combobox',{name:'Paragraph alignment'}).scrollIntoViewIfNeeded();await expect(page.getByRole('combobox',{name:'Paragraph alignment'})).toBeInViewport();await page.screenshot({path:resolve(root,'justify-narrow.png')});await button('Apply paragraph').scrollIntoViewIfNeeded();await expect(button('Apply paragraph')).toBeInViewport();await page.setViewportSize({width:1280,height:900});
 const changed=await area();await button('Apply paragraph').click();await contains('Alpha beta gamma');await button('Undo').click();await contains('Original words.');await button('Redo').click();await contains('Alpha beta gamma');const justified=await save('Justified paragraph.pdf');
 await open(justified);await paragraph('Alpha beta gamma delta');await expect(page.getByRole('combobox',{name:'Paragraph alignment'})).toHaveValue('justify');await expect(text()).toHaveValue(firstText);await expect(field('Line spacing (× font size)')).toHaveValue('1.5');
 const secondText='jj Café words carry a paragraph across several lines with enough room to justify the text.';await text().fill(secondText);await field('Font size (pt)').fill('14');
 for(const [name,value] of [['Left (pt)','330'],['Top (pt)','60'],['Area width (pt)','90'],['Area height (pt)','180']])await field(name).fill(value);
 await button('Choose substitute font…').click();await button('Import font…').click();await picker('Open',font,'Import a font');await expect(button('Apply font')).toBeEnabled();await button('Apply font').click();await expect(page.getByRole('combobox',{name:'Paragraph alignment'})).toHaveValue('justify');
 await button('Preview paragraph').click();await expect(page.getByRole('alert')).toContainText('does not fit');await expect(text()).toHaveValue(secondText);
 await field('Left (pt)').fill('300');await field('Area width (pt)').fill('120');const second=await area();await preview();await page.screenshot({path:resolve(root,'justify-substitute-rotated.png')});await button('Apply paragraph').click();const substituted=await save('Substituted justified paragraph.pdf');
 await open(substituted);await paragraph('jj Café words carry a');await expect(page.getByRole('combobox',{name:'Paragraph alignment'})).toHaveValue('justify');await expect(field('Font size (pt)')).toHaveValue('14');await expect(field('Line spacing (× font size)')).toHaveValue('1.5');await button('Cancel').click();await contains('Neighbor stays.');await page.screenshot({path:resolve(root,'justify-reopened.png')});
 expect(errors).toEqual([]);expect(hash(source)).toBe(sourceHash);expect(hash(font)).toBe(fontHash);
 checks.push('hard-break endings remain natural','justification fills soft wrapped lines','changing text invalidates candidate','canceled selection keeps justified candidate','640x720 controls/footer','undo redo','saved intrinsic90 rotation and justification reinference','14pt substitute font with justified Unicode and negative bearings','save reopen with native note and neighbor');
 writeFileSync(resolve(root,'results.json'),JSON.stringify({passed:true,checks,old,new:changed,second,firstText,secondText,sourceHash,fontHash,errors,exe:record},null,2));console.log('PASS Windows paragraph justification');
}catch(error){writeFileSync(resolve(root,'failure.json'),JSON.stringify({error:String(error.stack),errors,checks},null,2));await page.screenshot({path:resolve(root,'failure.png')}).catch(()=>{});throw error;}finally{await browser.close();}
