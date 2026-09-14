// Developer evidence viewer and native-vector/PDFium comparison. No app UI changes.
import { chromium, expect } from '@playwright/test';
import { readFile, writeFile, mkdir } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { inkTolerance } from './preview-ink-comparison.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const input = path.resolve(process.env.FOLIO_TEXT_AREA_INPUT ?? path.join(root, 'artifacts/text-area-native'));
const output = path.resolve(process.env.FOLIO_TEXT_AREA_OUTPUT ?? path.join(root, 'artifacts/text-area-browser'));
const manifest = JSON.parse(await readFile(path.join(input, 'results.json'), 'utf8'));
const cases = manifest.cases.filter(item => item.layout.canExport);
expect(cases.length).toBeGreaterThan(0);
expect(new Set(manifest.cases.map(item => item.name)).size).toBe(manifest.cases.length);
const escape = value => String(value).replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('>', '&gt;').replaceAll('"', '&quot;');
function svg(record, scale) {
  const content = record.overlays.map(({overlay, result}, index) => {
    const {preview, origin} = result;
    const prefix = `line-${index}-`;
    const paths = preview.outlines.map(outline => `<path id="${prefix}${outline.glyphId}" d="${escape(outline.path)}"/>`).join('');
    const uses = preview.glyphs.map(glyph => `<use href="#${prefix}${glyph.glyphId}" transform="matrix(${glyph.transform.join(' ')})"/>`).join('');
    return `<g transform="translate(${overlay.x} ${overlay.y})"><g transform="translate(${-origin[0]} ${overlay.fontSize + origin[1]}) scale(1 -1)" fill="rgb(${preview.color.map(c => `${c * 100}%`).join(' ')})"><defs>${paths}</defs>${uses}</g></g>`;
  }).join('');
  return `<svg xmlns="http://www.w3.org/2000/svg" role="img" aria-label="Native text-area preview" width="${manifest.pageWidth * scale}" height="${manifest.pageHeight * scale}" viewBox="0 0 ${manifest.pageWidth} ${manifest.pageHeight}">${content}</svg>`;
}
await mkdir(output, {recursive:true});
const comparator = (await readFile(path.join(root, 'scripts/preview-ink-comparison.mjs'))).toString('base64');
const browser = await chromium.launch({headless:true});
const results = [], errors = [];
try {
  const page = await browser.newPage({viewport:{width:1800,height:1700},deviceScaleFactor:1});
  page.on('pageerror', error => errors.push(error.message));
  for (const record of cases) for (const raster of record.rasters) {
    const scale = raster.zoom / 100;
    await page.setContent(`<style>html,body{margin:0;background:white}svg{display:block;background:white}</style>${svg(record, scale)}`);
    const proof = page.locator('svg');
    await expect(proof.locator('text,tspan,foreignObject')).toHaveCount(0);
    await expect(proof.locator('use')).toHaveCount(record.overlays.reduce((n, item) => n + item.result.preview.glyphs.length, 0));
    const actual = await proof.screenshot({path:path.join(output, `${record.name}-${raster.zoom}.browser.png`)});
    const expected = await readFile(path.join(input, raster.file));
    const result = await page.evaluate(async ({actual, expected, comparator}) => {
      const {compareInk} = await import(`data:text/javascript;base64,${comparator}`);
      const images = await Promise.all([actual,expected].map(async bytes => {const image = new Image();image.src = `data:image/png;base64,${bytes}`;await image.decode();return image;}));
      const [a,b] = images, width = b.naturalWidth, height = b.naturalHeight;
      if (a.naturalWidth !== width || a.naturalHeight !== height) return {pass:false,reason:'Raster dimensions differ'};
      const canvas = document.createElement('canvas');canvas.width = width;canvas.height = height;
      const ctx = canvas.getContext('2d',{willReadFrequently:true});
      const pixels = image => {ctx.fillStyle='white';ctx.fillRect(0,0,width,height);ctx.drawImage(image,0,0);return ctx.getImageData(0,0,width,height).data;};
      return {width,height,...compareInk(width,height,pixels(a),pixels(b))};
    },{actual:actual.toString('base64'),expected:expected.toString('base64'),comparator});
    results.push({name:record.name,zoom:raster.zoom,...result});
    console.log(`${result.pass?'PASS':'FAIL'} ${record.name} ${raster.zoom}% ${JSON.stringify(result)}`);
  }
  const views = manifest.cases.map(record => ({name:record.name,logical:record.request.text,
    overflow:record.layout.overflow,canExport:record.layout.canExport,request:record.request,
    lines:record.layout.lines.map(line => ({text:line.shaped?.text ?? '',break:line.breakKind,source:line.source,delimiter:line.delimiter})),
    svg:svg(record,1)}));
  const data = JSON.stringify(views).replaceAll('<','\\u003c');
  const html = `<!doctype html><meta charset="utf-8"><title>Folio — text-area layout checkpoint</title><style>body{font:16px system-ui;background:#f4f1eb;color:#292824;max-width:1100px;margin:32px auto;padding:0 20px}h1{font-size:28px}select,textarea{font:inherit;padding:10px;border:1px solid #b9b1a5;border-radius:6px}textarea{width:95%;height:130px}main{display:grid;grid-template-columns:480px 1fr;gap:24px;margin-top:24px}#preview{background:white;border:1px solid #d7cfc4}#status{font-weight:600;color:#9a3f24}pre{white-space:pre-wrap;font-size:13px}svg{display:block}</style><h1>Text areas: native layout checkpoint</h1><p>Development preview, not an enabled desktop tool. Native font outlines determine every visible line. The original text stays separate from soft wrapping; external-reader copy is still an integration gate.</p><label>Specimen <select id="cases"></select></label><p id="status"></p><main><div id="preview"></div><section><label>Original logical text<textarea id="logical" readonly></textarea></label><h2>Layout settings</h2><pre id="settings"></pre><details><summary>Source ranges and line endings</summary><pre id="lines"></pre></details></section></main><script type="application/json" id="data">${data}</script><script>const cases=JSON.parse(document.getElementById('data').textContent);const picker=document.getElementById('cases');cases.forEach((c,i)=>picker.add(new Option(c.name,i)));function show(){const c=cases[Number(picker.value)];document.getElementById('preview').innerHTML=c.svg;document.getElementById('logical').value=c.logical;document.getElementById('status').textContent=c.canExport?'Fits the area. Preview/PDF evidence is available.':'Overflow: '+Object.entries(c.overflow).filter(x=>x[1]).map(x=>x[0]).join(' and ')+'. PDF generation refused; original text retained.';const {text,...settings}=c.request;document.getElementById('settings').textContent=JSON.stringify(settings,null,2);document.getElementById('lines').textContent=JSON.stringify(c.lines,null,2)}picker.onchange=show;show();</script>`;
  await writeFile(path.join(output,'index.html'),html);
  await writeFile(path.join(output,'results.json'),JSON.stringify({description:'Native vector paths versus PDFium; rasterizer tolerances unchanged.',inkTolerance,errors,cases:results},null,2));
  expect(errors).toEqual([]);
  expect(results.length).toBe(cases.length*2);
  expect(results.filter(item=>!item.pass)).toEqual([]);
} finally {await browser.close();}
