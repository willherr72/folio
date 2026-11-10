// Actual supplied quiz, isolated package/profile, no modification of the attachment.
import {chromium,expect} from '@playwright/test';
import {readFileSync,writeFileSync} from 'node:fs';import {resolve} from 'node:path';
import {execFile} from 'node:child_process';import {promisify} from 'node:util';import {createHash} from 'node:crypto';
const root=resolve(process.env.FOLIO_QUIZ_ACCEPTANCE_ROOT),record=JSON.parse(readFileSync(resolve(root,'profile/process.json'),'utf8').replace(/^\uFEFF/,''));
const browser=await chromium.connectOverCDP(`http://127.0.0.1:${record.port}`),page=browser.contexts()[0].pages().find(p=>!p.url().startsWith('devtools:'));page.setDefaultTimeout(30000);
const errors=[];page.on('pageerror',e=>errors.push(e.message));const button=name=>page.getByRole('button',{name,exact:true}),hash=path=>createHash('sha256').update(readFileSync(path)).digest('hex');
async function picker(action,name){await promisify(execFile)('pwsh',['-NoProfile','-File',resolve('scripts/set-native-dialog.ps1'),'-AppProcessId',String(record.pid),'-Action',action,'-FilePath',resolve(root,name),...(action==='Open'?['-NativeFileName']:[])],{windowsHide:true,timeout:60000});}
const source=resolve(root,'Quiz.pdf'),sourceHash=hash(source),original='How is vaping defined in the MeSH database? What year was the term introduced in the MeSH database?',replacement=original.replace('How is','How this is');
const suggestions=process.env.FOLIO_EXPECT_PARAGRAPH_SUGGESTION==='1';
const editorFlow=process.env.FOLIO_EXPECT_PARAGRAPH_FLOW==='1',viewports=[];
async function checkEditorViewports(){
 const originalSize=await page.evaluate(()=>({width:innerWidth,height:innerHeight}));
 for(const size of [{width:1280,height:720},{width:1024,height:700},{width:800,height:600},{width:640,height:720}]){
  await page.setViewportSize(size);await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
  const text=await page.getByRole('textbox',{name:'Paragraph text',exact:true}).boundingBox(),apply=await button('Apply paragraph').boundingBox(),body=await page.locator('.paragraph-body').boundingBox();
  expect(text.y).toBeGreaterThanOrEqual(body.y);expect(text.y+text.height).toBeLessThanOrEqual(body.y+body.height+1);
  expect(apply.y+apply.height).toBeLessThanOrEqual(size.height);expect(text.x).toBeGreaterThanOrEqual(0);expect(text.x+text.width).toBeLessThanOrEqual(size.width);
  expect(await page.locator('.paragraph-body').evaluate(el=>el.scrollWidth<=el.clientWidth+1)).toBe(true);
  viewports.push({...size,text,apply});await page.screenshot({path:resolve(root,`layout-${size.width}x${size.height}.png`)});
 }
 await page.setViewportSize(originalSize);await page.locator('.paragraph-body').evaluate(el=>el.scrollTop=0);
}
try{
 await button('Open a PDF').click();await picker('Open','Quiz.pdf');await button('Edit text').click();
 if(suggestions){
  await page.getByRole('button',{name:/^(Edit text|Inspect unsupported text): IMPORTANT/}).first().click();await button('Edit paragraph…').click();
  await expect(page.getByText('Select the paragraph pieces manually, then choose Check selection.')).toBeVisible();await expect(page.getByRole('textbox',{name:'Paragraph text',exact:true})).toHaveCount(0);await button('Cancel').click();
 }
 await button('Edit text: How is vaping defined in the MeSH data').click();await button('Edit paragraph…').click();
 if(suggestions){
  await expect(page.getByRole('textbox',{name:'Paragraph text',exact:true})).toHaveValue(original);await expect(page.getByText(/Suggested paragraph\./)).toBeVisible();
  if(editorFlow){await expect(page.getByRole('checkbox')).toHaveCount(0);await expect(button('Check selection')).toHaveCount(0);await checkEditorViewports();}
  else expect(await page.getByRole('checkbox',{checked:true}).count()).toBe(3);
  await button('Adjust selection').click();expect(await page.getByRole('checkbox',{checked:true}).count()).toBe(3);await expect(page.getByRole('textbox',{name:'Paragraph text',exact:true})).toHaveCount(0);
  await page.getByRole('checkbox',{name:/^in the MeSH database\?/}).uncheck();await button('Check selection').click();await expect(page.getByRole('textbox',{name:'Paragraph text',exact:true})).not.toHaveValue(original);await button('Cancel').click();
  await button('Edit text: How is vaping defined in the MeSH data').click();await button('Edit paragraph…').click();
 }else{
  await page.getByRole('checkbox',{name:/^base\? What year was the term introduced/}).check();await page.getByRole('checkbox',{name:/^in the MeSH database\?/}).check();await button('Check selection').click();
 }
 const box=page.getByRole('textbox',{name:'Paragraph text',exact:true});await expect(box).toHaveValue(original);await expect(page.locator('.existing-text-font')).toContainText('Calibri · 12 pt');
 if(process.env.FOLIO_EXPECT_ROUNDED_SPACING==='1')await expect(page.getByRole('spinbutton',{name:'Line spacing (× font size)',exact:true})).toHaveValue('1.22');
 const area=Object.fromEntries(await Promise.all([['x','Left (pt)'],['y','Top (pt)'],['width','Area width (pt)'],['height','Area height (pt)']].map(async([key,name])=>[key,Number(await page.getByRole('spinbutton',{name,exact:true}).inputValue())])));
 await box.fill(replacement);await button('Preview paragraph').click();await expect(page.getByAltText('Paragraph PDF preview')).toBeVisible();await expect(button('Apply paragraph')).toBeEnabled();
 if(editorFlow){
  const candidate=await page.getByAltText('Paragraph PDF preview').getAttribute('src');
  await button('Adjust selection').click();await page.getByRole('checkbox',{name:/^in the MeSH database\?/}).uncheck();await expect(button('Apply paragraph')).toBeDisabled();await expect(button('Preview paragraph')).toBeDisabled();await expect(page.getByAltText('Original PDF page')).toBeVisible();
  await button('Cancel adjustment').click();await expect(box).toHaveValue(replacement);await expect(box).toBeFocused();await expect(button('Apply paragraph')).toBeEnabled();await expect(page.getByAltText('Paragraph PDF preview')).toHaveAttribute('src',candidate);
  await button('Adjust selection').click();await button('Check selection').click();await expect(box).toHaveValue(replacement);await expect(page.getByAltText('Paragraph PDF preview')).toHaveAttribute('src',candidate);await expect(button('Apply paragraph')).toBeEnabled();
  await button('Adjust selection').click();await page.getByRole('checkbox',{name:/^ELET 4350\/6350/}).check();await button('Check selection').click();await expect(page.getByRole('alert')).toBeVisible();await button('Cancel adjustment').click();await expect(box).toHaveValue(replacement);await expect(page.getByAltText('Paragraph PDF preview')).toHaveAttribute('src',candidate);await expect(button('Apply paragraph')).toBeEnabled();
 }
 await page.screenshot({path:resolve(root,'preview.png')});await button('Apply paragraph').click();await expect(page.getByRole('dialog')).toHaveCount(0);
 const text=()=>page.locator('.pdf-text-layer').first().textContent();await expect.poll(text).toContain('How this is vaping');await button('Undo').click();await expect.poll(text).not.toContain('How this is vaping');await button('Redo').click();await expect.poll(text).toContain('How this is vaping');
 await button('Save a copy').click();await picker('Save','Folio edited.pdf');await expect(page.getByLabel('Unsaved changes')).toHaveCount(0);await button('Open').click();await picker('Open','Folio edited.pdf');await expect(page.getByRole('tab',{name:'Folio edited.pdf',exact:true})).toHaveAttribute('aria-selected','true');await expect.poll(text).toContain('How this is vaping');
 await button('Find').click();await page.getByRole('searchbox').fill('How this');await expect(page.locator('.search-count')).toContainText('1 of 1');await button('Close search').click();await page.screenshot({path:resolve(root,'reopened.png')});expect(errors).toEqual([]);expect(hash(source)).toBe(sourceHash);
 writeFileSync(resolve(root,'results.json'),JSON.stringify({passed:true,sourceHash,original,replacement,area,errors,viewports,executableSha256:record.sha256,checks:[...(editorFlow?['text editor and actions visible at four practical viewport sizes','draft and exact candidate retained after canceled, unchanged and rejected selection adjustments','keyboard focus returns to draft']:[]),...(suggestions?['automatic three-piece paragraph selection','manual selection adjustment and cancellation','mixed-format instructions fall back to manual selection']:[]),'supplied quiz word insertion through paragraph UI','Calibri 12pt and unchanged area','native preview and Apply','undo/redo','save/reopen and Find','original unchanged']},null,2));console.log('PASS supplied quiz paragraph insertion');
}catch(error){writeFileSync(resolve(root,'failure.json'),JSON.stringify({error:String(error.stack),errors},null,2));await page.screenshot({path:resolve(root,'failure.png')}).catch(()=>{});throw error;}finally{await browser.close();}
