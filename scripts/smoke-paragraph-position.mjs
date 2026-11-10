// Real component/gesture checks with synthetic adapters; native fidelity is verified separately.
import {chromium,expect} from '@playwright/test';
import {mkdirSync,writeFileSync} from 'node:fs';
import {resolve} from 'node:path';
const directory=resolve('artifacts/paragraph-position-browser');mkdirSync(directory,{recursive:true});
const browser=await chromium.launch({headless:true}),page=await browser.newPage({viewport:{width:900,height:600}}),errors=[];
page.on('pageerror',error=>errors.push(error.message));const button=name=>page.getByRole('button',{name,exact:true});
const field=name=>page.getByRole('spinbutton',{name,exact:true});
async function at(x,y){return page.locator('.paragraph-page-targets').evaluate((svg,p)=>{const point=svg.createSVGPoint();point.x=p.x;point.y=p.y;const result=point.matrixTransform(svg.getScreenCTM());return{x:result.x,y:result.y};},{x,y});}
async function start(x,y){await page.locator('.paragraph-body').evaluate(node=>node.scrollTop=0);await page.locator('.paragraph-page-scroll').evaluate(node=>{node.scrollLeft=0;node.scrollTop=0;});const point=await at(x,y);await page.mouse.move(point.x,point.y);await page.mouse.down();}
async function move(x,y){const point=await at(x,y);await page.mouse.move(point.x,point.y,{steps:4});}
async function preview(){await button('Preview paragraph').click();await expect(page.getByAltText('Paragraph PDF preview')).toBeVisible();await button('Original').click();}
try{
 await page.goto('http://127.0.0.1:1476/tests/fixtures/editing-interactions.html');await button('Paragraph fixture').click();await button('Check selection').click();
 await expect(field('Left (pt)')).toHaveValue('20');await preview();
 await start(30,35);await expect(button('Apply paragraph')).toBeDisabled();await expect(button('Preview paragraph')).toBeDisabled();await expect(button('Original')).toBeDisabled();await expect(button('Edited')).toBeDisabled();await expect(field('Left (pt)')).toBeDisabled();await expect(page.getByRole('combobox',{name:'Preview zoom',exact:true})).toBeDisabled();
 await move(40,45);await page.keyboard.press('Escape');await page.mouse.up();
 await expect(page.getByRole('dialog')).toBeVisible();await expect(field('Left (pt)')).toHaveValue('20');await expect(field('Top (pt)')).toHaveValue('25');await expect(button('Apply paragraph')).toBeEnabled();
 expect(await page.evaluate(()=>window.interactionProof.closed)).toEqual([]);
 await start(30,35);await move(50,45);await page.mouse.up();await expect(field('Left (pt)')).toHaveValue('40');await expect(field('Top (pt)')).toHaveValue('35');
 await expect(button('Apply paragraph')).toBeDisabled();expect(await page.evaluate(()=>window.interactionProof.closed)).toEqual(['candidate-1']);await preview();
 await page.getByRole('combobox',{name:'Preview zoom',exact:true}).selectOption('200');
 await start(50,45);await page.mouse.up();await expect(button('Apply paragraph')).toBeEnabled();expect(await page.evaluate(()=>window.interactionProof.closed)).toEqual(['candidate-1']);
 await start(50,45);await move(40,40);await page.mouse.up();await expect(field('Left (pt)')).toHaveValue('30');await expect(field('Top (pt)')).toHaveValue('30');
 expect(await page.evaluate(()=>window.interactionProof.closed)).toEqual(['candidate-1','candidate-2']);
 await page.getByRole('combobox',{name:'Preview zoom',exact:true}).selectOption('100');await page.screenshot({path:resolve(directory,'position-controls.png')});
 await preview();await button('Cancel').click();expect(await page.evaluate(()=>window.interactionProof.closed)).toEqual(['candidate-1','candidate-2','candidate-3']);
 expect(errors).toEqual([]);writeFileSync(resolve(directory,'results.json'),JSON.stringify({passed:true,viewport:{width:900,height:600},checks:['drag blocks fields, comparison and Apply','Escape preserves candidate and position','commit releases old candidate','200% zoom uses source coordinates','no-op preserves candidate','Cancel releases remaining candidate'],errors},null,2));console.log('PASS paragraph positioning browser acceptance');
}catch(error){await page.screenshot({path:resolve(directory,'failure.png')}).catch(()=>{});throw error;}finally{await browser.close();}
