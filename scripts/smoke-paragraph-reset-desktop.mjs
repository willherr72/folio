// Repository-owned acceptance of the numeric reset fix in packaged WebView2.
import {chromium,expect} from '@playwright/test';
import {execFile} from 'node:child_process';import {promisify} from 'node:util';
import {readFileSync,writeFileSync} from 'node:fs';import {resolve,basename} from 'node:path';import {createHash} from 'node:crypto';
const root=resolve('artifacts/paragraph-highlights-20261024/desktop');
const record=JSON.parse(readFileSync(resolve(root,'profile-3/process.json'),'utf8').replace(/^\uFEFF/,''));
const browser=await chromium.connectOverCDP(`http://127.0.0.1:${record.port}`),page=browser.contexts()[0].pages().find(p=>!p.url().startsWith('devtools:'));
page.setDefaultTimeout(30000);const checks=[],errors=[];page.on('pageerror',e=>errors.push(e.message));
const button=name=>page.getByRole('button',{name,exact:true}),field=name=>page.getByRole('spinbutton',{name,exact:true}),editor=page.getByRole('textbox',{name:'Paragraph text',exact:true});
const draftButton=name=>page.locator('.paragraph-dialog').getByRole('button',{name,exact:true});
const source=resolve('artifacts/styled-paragraph-editor-20261024/desktop/Mixed-0.pdf'),sha=path=>createHash('sha256').update(readFileSync(path)).digest('hex'),sourceHash=sha(source);
try{
 await page.setViewportSize({width:1280,height:900});await page.getByRole('button',{name:/^(Open|Open a PDF)$/}).click();
 await promisify(execFile)('pwsh',['-NoProfile','-File',resolve('scripts/set-native-dialog.ps1'),'-AppProcessId',String(record.pid),'-FilePath',source,'-Action','Open','-NativeFileName'],{windowsHide:true,timeout:60000});
 await expect(page.getByRole('tab',{name:basename(source),exact:true}).last()).toHaveAttribute('aria-selected','true');
 await button('Edit text').click();await button('Edit text: One ').first().click();await button('Edit paragraph…').click();await expect(editor).toBeVisible();
 await button('Adjust selection').click();await page.getByRole('checkbox',{name:'two',exact:true}).check();await button('Check selection').click();await expect(editor).toHaveText('One two');
 const left=await field('Left (pt)').inputValue(),top=await field('Top (pt)').inputValue(),width=await field('Area width (pt)').inputValue();
 await editor.focus();await page.keyboard.press('Control+End');await page.keyboard.type(' draft');
 await field('Left (pt)').fill('');await field('Top (pt)').fill('');await field('Area width (pt)').fill('');await button('Reset position').click();
 await expect(field('Left (pt)')).toHaveValue(left);await expect(field('Top (pt)')).toHaveValue(top);await expect(field('Area width (pt)')).toHaveValue('');await expect(editor).toHaveText('One two draft');await expect(button('Preview paragraph')).toBeDisabled();
 checks.push('native mixed selection: no-op position reset restores both invalid coordinates and preserves text plus invalid width');
 await field('Area width (pt)').fill(width);await draftButton('Undo').click();await expect(editor).toHaveText('One two');await expect(button('Preview paragraph')).toBeEnabled();
 checks.push('coordinate draft repair does not add a model undo step');
 const moved=String(Number(top)+20);await field('Top (pt)').fill(moved);await field('Left (pt)').fill('');await button('Reset position').click();await expect(field('Top (pt)')).toHaveValue(top);await draftButton('Undo').click();await expect(field('Top (pt)')).toHaveValue(moved);await draftButton('Redo').click();await expect(field('Top (pt)')).toHaveValue(top);
 checks.push('actual geometry reset remains undoable and redoable');
 await button('Preview paragraph').click();await expect(page.getByRole('alert')).toContainText('does not fit');await expect(editor).toHaveText('One two');
 await field('Area width (pt)').fill('250');await field('Area height (pt)').fill('100');
 await button('Preview paragraph').click();await expect(page.getByAltText('Paragraph PDF preview')).toBeVisible();await page.screenshot({path:resolve(root,'reset-preview.png')});await button('Apply paragraph').click();await expect(editor).toHaveCount(0);await button('Undo').click();
 checks.push('native PDF preview/apply and workspace undo work after numeric reset with explicitly enlarged area');
 expect(sha(source)).toBe(sourceHash);expect(errors).toEqual([]);
 writeFileSync(resolve(root,'results.json'),JSON.stringify({passed:true,checks,errors,source,sourceHash,exe:record,limits:['Suggested area refuses unchanged mixed-font sample; enlarging width/height succeeds. Separate sizing issue remains open.']},null,2));console.log(JSON.stringify({passed:true,checks}));
}catch(error){writeFileSync(resolve(root,'failure.json'),JSON.stringify({error:String(error.stack),checks,errors},null,2));await page.screenshot({path:resolve(root,'failure.png')}).catch(()=>{});throw error;}finally{await browser.close();}
