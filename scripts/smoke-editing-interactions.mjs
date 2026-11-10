// Isolated synthetic component fixture; no PDF files or native application are changed.
import {chromium,expect} from '@playwright/test';
import {mkdirSync,writeFileSync} from 'node:fs';
import {resolve} from 'node:path';
const directory=resolve('artifacts/editing-interactions');mkdirSync(directory,{recursive:true});
const browser=await chromium.launch({headless:true});
const page=await browser.newPage({viewport:{width:900,height:600}}),errors=[];
page.on('pageerror',error=>errors.push(error.message));
try{
 await page.goto('http://127.0.0.1:1475/tests/fixtures/editing-interactions.html');
 await page.getByRole('button',{name:'Paragraph fixture',exact:true}).click();
 await expect(page.getByRole('checkbox')).toHaveCount(65);
 for(let index=1;index<64;index++)await page.getByRole('checkbox',{name:`Piece ${index}`,exact:true}).check();
 const target=index=>page.getByRole('button',{name:`Select text piece: Piece ${index}`,exact:true});
 await expect(page.getByRole('status')).toContainText('64 pieces selected');await expect(target(64)).toHaveAttribute('aria-disabled','true');
 await expect(target(64)).toHaveAttribute('tabindex','-1');await target(64).click({force:true});
 await expect(page.getByRole('checkbox',{name:'Piece 64',exact:true})).not.toBeChecked();
 await target(63).focus();await page.keyboard.press('Tab');await expect(page.getByRole('button',{name:'Cancel',exact:true})).toBeFocused();
 await target(63).focus();await page.keyboard.press('Space');await expect(target(64)).toHaveAttribute('aria-disabled','false');
 await target(64).focus();await page.keyboard.press('Enter');await expect(page.getByRole('checkbox',{name:'Piece 64',exact:true})).toBeChecked();
 await expect(page.getByRole('status')).toContainText('64 pieces selected');
 await page.screenshot({path:resolve(directory,'paragraph-limit.png')});
 await page.getByRole('button',{name:'Cancel',exact:true}).click();await page.getByRole('button',{name:'Image fixture',exact:true}).click();
 await expect(page.getByText(/32,768 pixels wide or tall/)).toBeVisible();await page.getByRole('button',{name:'Add images…',exact:true}).click();
 await expect(page.getByRole('button',{name:'Create PDF',exact:true})).toBeEnabled();
 const footer=await page.getByRole('button',{name:'Create PDF',exact:true}).boundingBox();expect(footer.y+footer.height).toBeLessThanOrEqual(600);
 await page.screenshot({path:resolve(directory,'image-guidance.png')});expect(errors).toEqual([]);
 writeFileSync(resolve(directory,'results.json'),JSON.stringify({passed:true,viewport:{width:900,height:600},checks:['64-piece state and pointer refusal','Tab skips disabled page target','Space deselects and Enter reselects','image dimension guidance and visible footer'],errors},null,2));
 console.log('PASS editing interaction browser acceptance');
}catch(error){await page.screenshot({path:resolve(directory,'failure.png')}).catch(()=>{});throw error;}finally{await browser.close();}
