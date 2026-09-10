import { chromium, expect } from '@playwright/test';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';

const artifacts = resolve('artifacts/upgrades-smoke');
mkdirSync(artifacts, { recursive: true });
const browser = await chromium.launch({ headless: true });
const page = await browser.newPage({ viewport: { width: 1440, height: 1050 }, deviceScaleFactor: 1 });
const errors = [];
const nativePrompts = [];
page.on('pageerror', error => errors.push(error.message));
page.on('dialog', async dialog => {
  if (dialog.type() !== 'beforeunload') nativePrompts.push(dialog.type());
  await dialog.accept();
});
const canvas = index => page.locator('[data-page-id]').nth(index).locator('.page-canvas');
async function stroke(target, points) {
  const box = await target.boundingBox();
  if (!box) throw Error('Drawing page unavailable');
  await page.mouse.move(box.x + points[0][0], box.y + points[0][1]);
  await page.mouse.down();
  for (const [x,y] of points.slice(1)) await page.mouse.move(box.x+x, box.y+y, { steps: 6 });
  await page.mouse.up();
}
try {
  await page.goto('http://127.0.0.1:1420/?demo=1');
  await expect(page.getByText('Folio welcome.pdf', {exact:true})).toBeVisible();
  await expect(page.locator('[data-page-id]')).toHaveCount(3);
  await expect(canvas(0).locator('image')).toHaveCount(1);
  await expect(page.locator('.engine-pill')).toHaveCount(0);

  await page.getByRole('button',{name:'Draw',exact:true}).click();
  await stroke(canvas(0), [[100,250],[130,210],[155,275],[190,230],[220,255]]);
  await expect(canvas(0).locator('[data-overlay]')).toHaveCount(1);
  await page.getByRole('button',{name:'Undo',exact:true}).click();
  await expect(canvas(0).locator('[data-overlay]')).toHaveCount(0);
  await page.getByRole('button',{name:'Redo',exact:true}).click();
  await expect(canvas(0).locator('[data-overlay] polyline')).toHaveCount(1);

  const viewport = page.getByRole('main',{name:'Document',exact:true});
  await viewport.hover();
  await page.mouse.wheel(0,660);
  await expect(page.getByLabel('Page 2 of 3',{exact:true})).toHaveClass(/selected/);
  await expect(canvas(1)).toBeVisible();
  await page.getByRole('button',{name:'Text',exact:true}).click();
  await canvas(1).click({position:{x:100,y:170}});
  await page.getByRole('textbox',{name:'Content',exact:true}).fill('Second page note');
  await expect(canvas(1).locator('[data-overlay] text')).toHaveCount(1);
  await expect(canvas(0).locator('[data-overlay] text')).toHaveCount(0);

  // A modal opened during a drag must finish that drag's undo transaction.
  const text = canvas(1).locator('[data-overlay] tspan');
  const originalX = Number(await text.getAttribute('x'));
  const textBox = await text.boundingBox();
  await page.mouse.move(textBox.x+5,textBox.y+textBox.height/2);
  await page.mouse.down();
  await page.mouse.move(textBox.x+65,textBox.y+textBox.height/2,{steps:6});
  await page.keyboard.press('Control+o');
  const confirm = page.getByRole('dialog',{name:'Discard unsaved changes?'});
  await expect(confirm).toBeVisible();
  await page.mouse.up();
  await expect(page.getByRole('button',{name:'Keep editing'})).toBeFocused();
  const modalBox = await confirm.boundingBox();
  expect(Math.abs(modalBox.x + modalBox.width/2 - 720)).toBeLessThan(3);
  await page.screenshot({path:resolve(artifacts,'01-modal.png')});
  await page.keyboard.press('Control+z');
  await page.getByRole('button',{name:'Keep editing'}).click();
  await page.getByRole('button',{name:'Undo',exact:true}).click();
  await expect(canvas(1).locator('[data-overlay] text')).toHaveCount(1);
  expect(Number(await text.getAttribute('x'))).toBeCloseTo(originalX,2);

  const before = await canvas(1).boundingBox();
  const host = await viewport.boundingBox();
  const point = {x:before.x+before.width/2,y:Math.max(before.y,host.y)+140};
  const local = {x:(point.x-before.x)/before.width,y:(point.y-before.y)/before.height};
  await page.mouse.move(point.x,point.y);
  await page.keyboard.down('Control');
  await page.mouse.wheel(0,-80);
  await page.keyboard.up('Control');
  await expect(page.locator('.zoom-value')).toHaveText('100%');
  const after = await canvas(1).boundingBox();
  expect(Math.abs(after.x+local.x*after.width-point.x)).toBeLessThan(3);
  expect(Math.abs(after.y+local.y*after.height-point.y)).toBeLessThan(3);

  // Real pointer-driven HTML drag, from the last thumbnail to before the first.
  const firstThumb = page.getByLabel('Page 1 of 3',{exact:true});
  const lastThumb = page.getByLabel('Page 3 of 3',{exact:true});
  const firstRect = await firstThumb.boundingBox();
  const lastRect = await lastThumb.boundingBox();
  await page.mouse.move(lastRect.x+10,lastRect.y+lastRect.height/2);
  await page.mouse.down();
  await page.mouse.move(lastRect.x+20,lastRect.y+lastRect.height/2,{steps:3});
  await page.mouse.move(firstRect.x+40,firstRect.y+10,{steps:12});
  await expect(firstThumb).toHaveClass(/drop-before/);
  await page.mouse.up();
  await expect(firstThumb.locator('image')).toHaveAttribute('href',/Export%20with%20confidence/);

  await page.getByRole('button',{name:'Settings',exact:true}).click();
  await page.getByLabel('Theme',{exact:true}).selectOption('dark');
  await expect(page.locator('html')).toHaveAttribute('data-theme','dark');
  await page.getByLabel('Page view',{exact:true}).selectOption('single');
  await page.getByRole('button',{name:'Done',exact:true}).click();
  await expect(page.locator('[data-page-id]')).toHaveCount(1);
  await page.screenshot({path:resolve(artifacts,'02-dark.png')});

  await page.getByRole('button',{name:'Settings',exact:true}).click();
  await page.getByLabel('Page view',{exact:true}).selectOption('continuous');
  await page.getByRole('button',{name:'Done',exact:true}).click();
  await expect(page.locator('[data-page-id]')).toHaveCount(3);
  const downloadPromise=page.waitForEvent('download');
  await page.getByRole('button',{name:'Export demo plan',exact:true}).click();
  const download=await downloadPromise;
  const output=resolve(artifacts,'edited-plan.json');
  await download.saveAs(output);
  const plan=JSON.parse(readFileSync(output,'utf8'));
  expect(plan.pages.map(p=>p.pageIndex)).toEqual([2,0,1]);
  expect(plan.pages[1].overlays[0].type).toBe('ink');
  expect(plan.pages[1].overlays[0].paths[0].length).toBeGreaterThan(10);
  expect(plan.pages[2].overlays[0].text).toBe('Second page note');
  await page.reload();
  await expect(page.locator('html')).toHaveAttribute('data-theme','dark');
  await expect(page.locator('[data-page-id]')).toHaveCount(3);
  expect(errors).toEqual([]);
  expect(nativePrompts).toEqual([]);
  const result={passed:true,checks:['continuous scroll selects page','drawing undo/redo','page-scoped text','centered modal and focus','interrupted drag undo','Ctrl+wheel cursor anchor','real thumbnail drag and insertion marker','dark theme','single/continuous settings','preference persistence','vector ink in exported plan'],pageErrors:errors,browserConfirmPrompts:nativePrompts};
  writeFileSync(resolve(artifacts,'results.json'),JSON.stringify(result,null,2));
  console.log(JSON.stringify(result,null,2));
} catch(error) {
  await page.screenshot({path:resolve(artifacts,'failure.png')});
  throw error;
} finally { await browser.close(); }
