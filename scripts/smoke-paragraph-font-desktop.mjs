// Repository Windows/WebView acceptance against an isolated owned native app.
import {chromium,expect} from '@playwright/test';
import {execFile} from 'node:child_process';import {promisify} from 'node:util';
import {readFileSync,writeFileSync,existsSync,renameSync} from 'node:fs';
import {resolve,basename} from 'node:path';import {createHash} from 'node:crypto';
const root=resolve(process.env.FOLIO_PARAGRAPH_FONT_ROOT??'artifacts/paragraph-font-acceptance');
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
 await page.setViewportSize({width:1280,height:900});const source=resolve(root,'Annotated subset.pdf'),font=resolve(root,'Substitute.ttf'),sourceHash=hash(source),fontHash=hash(font);
 await open(source);await save('Before font.pdf');await button('Rotate').click();await paragraph('Café old');const old=await area();
 await text().fill('Cafè new\njj');for(const [name,value] of [['Font size (pt)','16'],['Left (pt)','30'],['Top (pt)','84'],['Area width (pt)','130'],['Area height (pt)','55'],['Line spacing (× font size)','1.5']])await field(name).fill(value);
 await page.getByRole('combobox',{name:'Paragraph alignment'}).selectOption('right');await button('Preview paragraph').click();await expect(page.getByRole('alert')).toContainText('U+00E8');
 await button('Choose substitute font…').click();await button('Cancel').click();await expect(text()).toHaveValue('Cafè new\njj');await expect(field('Font size (pt)')).toHaveValue('16');
 await button('Choose substitute font…').click();await button('Import font…').click();await picker('Open',font,'Import a font');await expect(button('Apply font')).toBeEnabled();await button('Apply font').click();await expect(page.getByText('DejaVu Serif',{exact:true})).toBeVisible();await preview();
 await button('Choose substitute font…').click();await button('Cancel').click();await expect(button('Apply paragraph')).toBeEnabled();await expect(field('Line spacing (× font size)')).toHaveValue('1.5');
 await page.screenshot({path:resolve(root,'font-preview.png')});await page.setViewportSize({width:640,height:720});await button('Choose substitute font…').scrollIntoViewIfNeeded();await expect(button('Choose substitute font…')).toBeInViewport();await page.screenshot({path:resolve(root,'font-narrow.png')});await button('Apply paragraph').scrollIntoViewIfNeeded();await expect(button('Apply paragraph')).toBeInViewport();await page.setViewportSize({width:1280,height:900});
 const changed=await area();await button('Apply paragraph').click();await contains('Cafè new');await button('Undo').click();await contains('Café old');await button('Redo').click();await contains('Cafè new');const saved=await save('Font paragraph.pdf');
 // Only move the owned acceptance copy, proving the PDF no longer needs it.
 const unavailable=resolve(root,'Unavailable substitute.ttf');if(existsSync(unavailable))throw Error('Preserve previous font');renameSync(font,unavailable);
 await open(saved);await paragraph('Cafè new');await expect(text()).toHaveValue('Cafè new jj');await expect(field('Font size (pt)')).toHaveValue('16');await expect(field('Line spacing (× font size)')).toHaveValue('1.5');await expect(page.getByRole('combobox',{name:'Paragraph alignment'})).toHaveValue('right');
 await text().fill('Cafè again\njj');for(const [name,value] of [['Left (pt)','261'],['Top (pt)','30'],['Area width (pt)','55'],['Area height (pt)','130']])await field(name).fill(value);
 const second=await area();await preview();await page.screenshot({path:resolve(root,'font-reedit-rotated.png')});await button('Apply paragraph').click();const edited=await save('Reedited font paragraph.pdf');await open(edited);await contains('Cafè again');await contains('Neighbor');
 await button('Find').click();await page.getByRole('searchbox').fill('Cafè again');await expect(page.locator('.search-count')).toContainText('1 of 1');await button('Close search').click();await page.screenshot({path:resolve(root,'font-reopened.png')});
 expect(errors).toEqual([]);expect(hash(source)).toBe(sourceHash);expect(hash(unavailable)).toBe(fontHash);
 checks.push('missing original subset character rejected','font picker cancellation keeps draft','native TrueType import supplies absent character','size/right alignment/1.5 spacing retained','canceling picker keeps candidate','640x720 controls/footer','apply undo redo','saved intrinsic90 rotation','re-edit without original font file','native note and neighbor retained','searchable replacement');
 writeFileSync(resolve(root,'results.json'),JSON.stringify({passed:true,checks,old,new:changed,second,sourceHash,fontHash,errors,exe:record},null,2));console.log('PASS Windows paragraph font substitution');
}catch(error){writeFileSync(resolve(root,'failure.json'),JSON.stringify({error:String(error.stack),errors,checks},null,2));await page.screenshot({path:resolve(root,'failure.png')}).catch(()=>{});throw error;}finally{await browser.close();}
