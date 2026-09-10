import { mkdirSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const out = fileURLToPath(new URL('../examples/Welcome to Folio.pdf', import.meta.url));
const objects = [];
const add = value => (objects.push(value), objects.length);
const escape = value => value.replaceAll('\\', '\\\\').replaceAll('(', '\\(').replaceAll(')', '\\)');
const text = (x, y, size, value, rgb = '0.16 0.17 0.18') =>
  `BT /F1 ${size} Tf ${rgb} rg 1 0 0 1 ${x} ${y} Tm (${escape(value)}) Tj ET\n`;
const line = (x1, y1, x2, y2) => `0.78 0.76 0.72 RG 0.7 w ${x1} ${y1} m ${x2} ${y2} l S\n`;
add('<< /Type /Catalog /Pages 2 0 R >>');
add('PAGES');
add('<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>');
const pageIds = [];
function page(content, rotate = 0) {
  const stream = add(`<< /Length ${Buffer.byteLength(content, 'latin1')} >>\nstream\n${content}endstream`);
  pageIds.push(add(`<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Rotate ${rotate} /Resources << /Font << /F1 3 0 R >> >> /Contents ${stream} 0 R >>`));
}
let content = '0.98 0.97 0.95 rg 0 0 612 792 re f\n';
content += text(48, 735, 12, 'F O L I O   /   YOUR EVERYDAY PDF WORKSPACE', '0.65 0.27 0.18');
content += line(48, 713, 564, 713);
content += text(48, 655, 40, 'Make it yours.');
content += text(48, 620, 14, 'A small document for trying your new PDF editor.');
content += text(48, 566, 18, '01   Add a note');
content += text(48, 542, 11, 'Choose Text, then click in the space below. Edit and move your note.');
content += '1 1 1 rg 48 408 516 111 re f\n';
content += text(48, 368, 18, '02   Leave your signature');
content += text(48, 344, 11, 'Draw your signature, then place it above the line.');
content += line(48, 230, 380, 230);
content += text(48, 211, 10, 'YOUR SIGNATURE');
content += text(48, 155, 18, '03   Make a copy');
content += text(48, 131, 11, 'Save a new PDF, then reopen it to see your changes.');
content += line(48, 76, 564, 76);
content += text(48, 52, 10, 'LOCAL FILES. YOUR TOOLS. YOUR WORK.');
content += text(540, 52, 10, '1 / 3');
page(content);
content = text(48, 734, 12, 'F O L I O   /   PAGE TOOLS', '0.65 0.27 0.18');
content += text(48, 667, 36, 'A little room to think.');
content += text(48, 627, 12, 'Try moving this page, duplicating it, or adding another PDF.');
for (let y = 550; y >= 160; y -= 39) content += line(48, y, 564, y);
content += text(48, 85, 11, 'Notes and ideas');
content += text(540, 52, 10, '2 / 3');
page(content);
// This page has intrinsic rotation. Rotate its content back so it reads naturally.
content = '0 1 -1 0 612 0 cm\n';
content += text(48, 552, 12, 'F O L I O   /   A DIFFERENT PERSPECTIVE', '0.65 0.27 0.18');
content += text(48, 477, 38, 'Turn the page.');
content += text(48, 436, 13, 'This is a landscape page. Try adding text or a signature here, too.');
content += line(48, 405, 744, 405);
content += '0.97 0.95 0.91 rg 48 130 696 235 re f\n';
content += text(48, 78, 11, 'Use the rotation control to change orientation.');
content += text(720, 40, 10, '3 / 3');
page(content, 90);
objects[1] = `<< /Type /Pages /Count ${pageIds.length} /Kids [${pageIds.map(id => `${id} 0 R`).join(' ')}] >>`;
let pdf = '%PDF-1.7\n%\xE2\xE3\xCF\xD3\n';
const offsets = [0];
objects.forEach((object, index) => {
  offsets.push(Buffer.byteLength(pdf, 'latin1'));
  pdf += `${index + 1} 0 obj\n${object}\nendobj\n`;
});
const xref = Buffer.byteLength(pdf, 'latin1');
pdf += `xref\n0 ${objects.length + 1}\n0000000000 65535 f \n`;
for (const offset of offsets.slice(1)) pdf += `${String(offset).padStart(10, '0')} 00000 n \n`;
pdf += `trailer\n<< /Size ${objects.length + 1} /Root 1 0 R >>\nstartxref\n${xref}\n%%EOF\n`;
mkdirSync(fileURLToPath(new URL('../examples/', import.meta.url)), { recursive: true });
writeFileSync(out, Buffer.from(pdf, 'latin1'));
console.log(out);
