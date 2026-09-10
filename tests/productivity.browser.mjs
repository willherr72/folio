import {chromium,expect} from '@playwright/test';
const browser=await chromium.launch({headless:true});
try {
  const page=await browser.newPage({viewport:{width:1440,height:1050}});
  await page.goto(process.env.FOLIO_UI_URL || 'http://127.0.0.1:1422/?demo=1');
  await page.getByRole('button',{name:'Text',exact:true}).click();
  await page.locator('.page-canvas').first().click({position:{x:100,y:285}});
  const content=page.getByRole('textbox',{name:'Content',exact:true});
  await expect(content).toBeFocused();
  await page.keyboard.type('Immediate keyboard editing');
  await expect(content).toHaveValue('Immediate keyboard editing');
  await expect(page.locator('[data-overlay] tspan')).toHaveText('Immediate keyboard editing');
  console.log('A real page click focuses Content and immediate typing replaces the placeholder.');
} finally {await browser.close();}
