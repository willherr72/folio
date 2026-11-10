// Repository Windows/WebView workflow against an isolated owned native app.
import {chromium,expect} from '@playwright/test';
import {execFile} from 'node:child_process';
import {promisify} from 'node:util';
import {readFileSync,writeFileSync,existsSync,renameSync} from 'node:fs';
import {resolve,basename} from 'node:path';
import {createHash} from 'node:crypto';
const root=resolve(process.env.FOLIO_PARAGRAPH_REEDIT_ROOT??'artifacts/paragraph-reedit-20261023/desktop');
const colorChecks=process.env.FOLIO_PARAGRAPH_COLOR==='1';
const record=JSON.parse(readFileSync(resolve(root,'profile/process.json'),'utf8').replace(/^\uFEFF/,''));
const browser=await chromium.connectOverCDP(`http://127.0.0.1:${record.port}`);
const page=browser.contexts()[0].pages().find(p=>!p.url().startsWith('devtools:'));page.setDefaultTimeout(30000);
const errors=[],checks=[];page.on('pageerror',error=>errors.push(error.message));
const button=name=>page.getByRole('button',{name,exact:true});
const field=name=>page.getByRole('spinbutton',{name,exact:true});
const draft=()=>page.getByRole('textbox',{name:'Paragraph text',exact:true});
const alignment=()=>page.getByRole('combobox',{name:'Paragraph alignment'});
const hash=path=>createHash('sha256').update(readFileSync(path)).digest('hex');
async function picker(action,path,title){await promisify(execFile)('pwsh',['-NoProfile','-File',resolve('scripts/set-native-dialog.ps1'),'-AppProcessId',String(record.pid),'-FilePath',path,'-Action',action,...(title?['-DialogTitle',title]:[]),...(action==='Open'?['-NativeFileName']:[])],{windowsHide:true,timeout:60000});}
async function open(path){await page.getByRole('button',{name:/^(Open|Open a PDF)$/}).click();await picker('Open',path);await expect(page.getByRole('tab',{name:basename(path),exact:true}).last()).toHaveAttribute('aria-selected','true');await expect(page.locator('.document-page image').first()).toBeVisible();}
async function save(name){const destination=resolve(root,name),native=resolve(root,'Folio edited.pdf');if(existsSync(native)||existsSync(destination))throw Error('Output already exists');await button('Save a copy').click();await picker('Save',native);await expect(page.locator('.statusbar')).toContainText('Saved a copy');renameSync(native,destination);return destination;}
async function paragraph(anchor,fontLabel){await button('Edit text').click();await page.getByRole('button',{name:anchor}).first().click();if(fontLabel){await expect(page.locator('.existing-text-font')).toContainText(fontLabel);await page.screenshot({path:resolve(root,'reopened-font-label.png')});}await button('Edit paragraph…').click();await expect(draft()).toBeVisible();if(fontLabel)await expect(page.locator('.existing-text-font')).toContainText(fontLabel);}
async function preview(){await button('Preview paragraph').click();await expect(page.getByAltText('Paragraph PDF preview')).toBeVisible();await expect(button('Apply paragraph')).toBeEnabled();}
async function contains(value){await expect.poll(()=>page.locator('.document-page .pdf-text-layer').first().textContent()).toContain(value);}
async function checkArea(values){for(const [name,value] of Object.entries(values))await expect(field(name)).toHaveValue(String(value));}
const firstText='  Alpha beta gamma delta epsilon\n  \n  Gamma delta  \n';
const secondText='  jj Café words carry a paragraph\n  \n  More words  \n';
const originalArea={'Left (pt)':60,'Top (pt)':80,'Area width (pt)':180,'Area height (pt)':140};
const rotatedArea={'Left (pt)':280,'Top (pt)':60,'Area width (pt)':140,'Area height (pt)':180};
try{
 await page.setViewportSize({width:1280,height:900});
 const source=resolve(root,'Original paragraph.pdf'),font=resolve(root,'Substitute.ttf'),sourceHash=hash(source),fontHash=hash(font);
 await open(source);await save('Before edit.pdf');await paragraph(/^Edit text: Original words\.$/);
 for(const [name,value] of Object.entries({...originalArea,'Line spacing (× font size)':1.5}))await field(name).fill(String(value));
 await alignment().selectOption('justify');await draft().fill(firstText);await preview();
 if(colorChecks){const originalColor=await page.getByLabel('Text color',{exact:true}).inputValue();await page.getByLabel('Text color',{exact:true}).fill('#1a80ef');await expect(button('Apply paragraph')).toBeDisabled();await preview();await button('Use original color').click();await expect(page.getByLabel('Text color',{exact:true})).toHaveValue(originalColor);await expect(button('Apply paragraph')).toBeDisabled();await page.getByLabel('Text color',{exact:true}).fill('#1a80ef');await preview();}
 await page.screenshot({path:resolve(root,'first-preview.png')});await button('Apply paragraph').click();
 await contains('Alpha beta');await button('Undo').click();await contains('Original words.');if(colorChecks)await save('Undo edit.pdf');await button('Redo').click();await contains('Alpha beta');
 const first=await save('First edit.pdf');await open(first);
 // Anchor below the blank line: validated memory must select the whole paragraph.
 await paragraph(/^Edit text: .*Gamma delta/);await expect(draft()).toHaveValue(firstText);await checkArea(originalArea);
 if(colorChecks){await expect(page.getByLabel('Text color',{exact:true})).toHaveValue('#1a80ef');expect((await page.getByLabel('Text color',{exact:true}).boundingBox()).height).toBeGreaterThanOrEqual(38);}
 await expect(alignment()).toHaveValue('justify');await expect(field('Line spacing (× font size)')).toHaveValue('1.5');
 await preview();await page.screenshot({path:resolve(root,'restored-preview.png')});
 await page.setViewportSize({width:640,height:720});await draft().scrollIntoViewIfNeeded();await expect(draft()).toHaveValue(firstText);
 await page.screenshot({path:resolve(root,'restored-narrow.png')});await expect(button('Apply paragraph')).toBeInViewport();await page.setViewportSize({width:1280,height:900});
 await button('Apply paragraph').click();const unchanged=await save('Unchanged re-edit.pdf');
 await open(unchanged);await paragraph(/^Edit text: .*Alpha beta/);await expect(draft()).toHaveValue(firstText);
 await draft().fill(secondText);await field('Font size (pt)').fill('14');
 await button('Choose substitute font…').click();await button('Import font…').click();await picker('Open',font,'Import a font');
 await expect(button('Apply font')).toBeEnabled();await button('Apply font').click();if(colorChecks)await page.getByLabel('Text color',{exact:true}).fill('#a02bc3');await preview();await button('Apply paragraph').click();
 await button('Rotate').click();const substituted=await save('Rotated substitute.pdf');await open(substituted);
 await paragraph(/^Edit text: .*More words/,'DejaVu Serif');await expect(draft()).toHaveValue(secondText);await checkArea(rotatedArea);
 if(colorChecks)await expect(page.getByLabel('Text color',{exact:true})).toHaveValue('#a02bc3');
 await expect(field('Font size (pt)')).toHaveValue('14');await expect(field('Line spacing (× font size)')).toHaveValue('1.5');await expect(alignment()).toHaveValue('justify');
 await preview();
 const editBox=await draft().boundingBox(),previewBox=await page.locator('.paragraph-page-preview').boundingBox();
 expect(editBox.x+editBox.width).toBeLessThanOrEqual(previewBox.x-10);
 await page.screenshot({path:resolve(root,'rotated-restored-preview.png')});await button('Apply paragraph').click();
 const final=await save('Rotated unchanged re-edit.pdf');await open(final);await contains('Neighbor stays.');await page.screenshot({path:resolve(root,'final-reopened.png')});
 expect(errors).toEqual([]);expect(hash(source)).toBe(sourceHash);expect(hash(font)).toBe(fontHash);
 checks.push('hard and soft breaks, blank lines, edge spaces','whole paragraph from anchor below blank line','exact area/alignment/spacing restore','undo redo','640x720 controls','save reopen and no-op re-edit','substitute font with Unicode and negative bearings','readable embedded font label in reopened run and paragraph dialogs','workspace rotation and rotated no-op re-edit');
 if(colorChecks)checks.push('color change/reset invalidates candidate','color with original and substitute font survives reopen','undo restores original text and color');
 writeFileSync(resolve(root,'results.json'),JSON.stringify({passed:true,checks,colorChecks,firstText,secondText,originalArea,rotatedArea,sourceHash,fontHash,errors,exe:record},null,2));
 console.log('PASS Windows faithful paragraph re-edit');
}catch(error){writeFileSync(resolve(root,'failure.json'),JSON.stringify({error:String(error.stack),checks,errors},null,2));await page.screenshot({path:resolve(root,'failure.png')}).catch(()=>{});throw error;}
finally{await browser.close();}
