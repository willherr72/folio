import {chromium,expect} from '@playwright/test';
import {writeFileSync} from 'node:fs';
const browser=await chromium.launch({headless:true});const checks=[],errors=[];
try{
 for(const theme of ['light','dark'])for(const viewport of [{width:1280,height:820},{width:900,height:600}]){
  const page=await browser.newPage({viewport});page.on('pageerror',e=>errors.push(e.message));
  await page.goto(`http://127.0.0.1:1432/tests/fixtures/embedded-text-dialog.html?theme=${theme}`);
  await page.getByRole('button',{name:'Edit embedded text',exact:true}).click();
  await page.getByRole('textbox',{name:'Replacement text'}).fill('Edited café');
  await page.getByRole('button',{name:'Apply changes',exact:true}).click();await expect(page.getByRole('alert')).toContainText('subset lacks');
  await page.getByRole('button',{name:'Choose substitute font…'}).click();
  await page.getByRole('button',{name:'DejaVu Serif',exact:true}).click();
  await expect(page.getByRole('button',{name:'Apply font',exact:true})).toBeEnabled();
  expect(await page.getByLabel('Applied replacement').textContent()).toBe('null');
  await page.getByRole('button',{name:'Apply font',exact:true}).click();
  await expect(page.getByRole('textbox',{name:'Replacement text'})).toHaveValue('Edited café');
  await expect(page.getByLabel('Replacement font preview')).toHaveCSS('font-family','FolioFont_107244956e9962b9e96faccdc551825e0ae0898ae13737133e1b921a2fd35ffa');
  expect(await page.getByRole('dialog').evaluate(node=>node.closest('[inert]')===null)).toBe(true);
  const footer=await page.getByRole('button',{name:'Apply changes',exact:true}).boundingBox();
  expect(footer.y+footer.height).toBeLessThanOrEqual(viewport.height-24);
  await page.screenshot({path:`artifacts/embedded-dialog-${theme}-${viewport.width}.png`});
  await page.getByRole('button',{name:'Apply changes',exact:true}).click();
  const result=JSON.parse(await page.getByLabel('Applied replacement').textContent());expect(result.replacement).toBe('Edited café');expect(result.fontId).toHaveLength(64);
  await expect(page.getByRole('button',{name:'Edit embedded text',exact:true})).toBeFocused();
  checks.push({theme,viewport});await page.close();
 }
 expect(errors).toEqual([]);writeFileSync('artifacts/embedded-dialog-browser.json',JSON.stringify({passed:true,checks,errors},null,2));console.log('PASS embedded-font dialog light/dark and two viewport sizes');
}finally{await browser.close();}
